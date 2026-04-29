//! Phase 4.c — runtimes IPC.
//!
//! 정책 (phase-4-screens-decision.md §1.1 runtimes, phase-4c-runtimes-decision.md):
//! - 어댑터(`OllamaAdapter`, `LmStudioAdapter`) 직접 호출 — `detect()` + `health()` + `list_models()`.
//! - `last_ping_at`은 호출 시점 RFC3339.
//! - LM Studio는 `list_models`가 size를 0으로 리턴 — 그대로 표시 (sha256은 `digest`로 미러).
//! - start/stop/restart은 v1 노출 안 함 (외부 데몬이라 안전 위험).

use runtime_manager::{LocalModel, RuntimeAdapter};
use serde::Serialize;
use shared_types::{RuntimeKind, RuntimeState};
use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuntimesApiError {
    #[error("어댑터에 도달할 수 없어요: {message}")]
    Unreachable { message: String },
    #[error("내부 오류: {message}")]
    Internal { message: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeStatus {
    pub kind: RuntimeKind,
    pub installed: bool,
    pub version: Option<String>,
    pub running: bool,
    pub latency_ms: Option<u32>,
    pub model_count: usize,
    /// RFC3339 — 호출 시점.
    pub last_ping_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeModelView {
    pub runtime_kind: RuntimeKind,
    pub id: String,
    pub size_bytes: u64,
    pub digest: String,
}

/// 모든 어댑터(Ollama / LM Studio)의 상태를 한 번의 invoke로 모은다.
#[tauri::command]
pub async fn list_runtime_statuses() -> Result<Vec<RuntimeStatus>, RuntimesApiError> {
    let ollama: Box<dyn RuntimeAdapter> = Box::new(adapter_ollama::OllamaAdapter::new());
    let lmstudio: Box<dyn RuntimeAdapter> = Box::new(adapter_lmstudio::LmStudioAdapter::new());

    let mut out = Vec::with_capacity(2);
    for adapter in [ollama, lmstudio] {
        out.push(probe_status(adapter.as_ref()).await);
    }
    Ok(out)
}

/// 특정 어댑터에 로드된 모델 목록.
///
/// 어댑터가 실행 중이 아니면 `Unreachable` 반환 — 화면에서 빈 상태 + 안내로 전환.
#[tauri::command]
pub async fn list_runtime_models(
    runtime_kind: RuntimeKind,
) -> Result<Vec<RuntimeModelView>, RuntimesApiError> {
    let adapter: Box<dyn RuntimeAdapter> = adapter_for(runtime_kind)?;
    let models = adapter
        .list_models()
        .await
        .map_err(|e| RuntimesApiError::Unreachable {
            message: e.to_string(),
        })?;
    Ok(models
        .into_iter()
        .map(|m| local_model_to_view(runtime_kind, m))
        .collect())
}

fn adapter_for(kind: RuntimeKind) -> Result<Box<dyn RuntimeAdapter>, RuntimesApiError> {
    match kind {
        RuntimeKind::Ollama => Ok(Box::new(adapter_ollama::OllamaAdapter::new())),
        RuntimeKind::LmStudio => Ok(Box::new(adapter_lmstudio::LmStudioAdapter::new())),
        other => Err(RuntimesApiError::Internal {
            message: format!("아직 지원하지 않는 런타임이에요: {other:?}"),
        }),
    }
}

fn local_model_to_view(kind: RuntimeKind, m: LocalModel) -> RuntimeModelView {
    RuntimeModelView {
        runtime_kind: kind,
        id: m.file_rel_path,
        size_bytes: m.size_bytes,
        digest: m.sha256,
    }
}

/// 단일 어댑터의 detect + health + list_models를 모아 RuntimeStatus를 만든다.
///
/// 정책:
/// - detect()로 installed + version 추출. detect가 실패해도 status 자체는 반환 (running=false).
/// - health()는 임시 핸들로 호출 (외부 데몬 attach라 instance_id는 설명용).
/// - list_models() 실패는 model_count=0으로 처리 (running 자체는 health 결과 우선).
async fn probe_status<A: RuntimeAdapter + ?Sized>(adapter: &A) -> RuntimeStatus {
    let kind = adapter.kind();
    let (installed, version) = match adapter.detect().await {
        Ok(d) => (d.installed, d.version),
        Err(e) => {
            tracing::warn!(?kind, error = %e, "runtime detect 실패");
            (false, None)
        }
    };

    // health는 임시 핸들로 호출. instance_id는 설명용이며 외부 데몬 라이프사이클은 어댑터 책임.
    let temp_handle = runtime_manager::RuntimeHandle {
        kind,
        instance_id: format!("external-{}", runtime_kind_slug(kind)),
        internal_port: default_port(kind),
    };

    let health = adapter.health(&temp_handle).await;
    let running = matches!(health.state, Some(RuntimeState::Active));
    let latency_ms = health.latency_ms;

    let model_count = if running {
        adapter.list_models().await.map(|v| v.len()).unwrap_or(0)
    } else {
        0
    };

    let last_ping_at = OffsetDateTime::now_utc().format(&Rfc3339).ok();

    RuntimeStatus {
        kind,
        installed,
        version,
        running,
        latency_ms,
        model_count,
        last_ping_at,
    }
}

fn runtime_kind_slug(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Ollama => "ollama",
        RuntimeKind::LmStudio => "lm-studio",
        RuntimeKind::LlamaCpp => "llama-cpp",
        RuntimeKind::KoboldCpp => "kobold-cpp",
        RuntimeKind::Vllm => "vllm",
    }
}

fn default_port(kind: RuntimeKind) -> u16 {
    match kind {
        RuntimeKind::Ollama => 11434,
        RuntimeKind::LmStudio => 1234,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unreachable_serializes_kebab_tag() {
        let v = serde_json::to_value(RuntimesApiError::Unreachable {
            message: "io".into(),
        })
        .unwrap();
        assert_eq!(v["kind"], "unreachable");
        assert_eq!(v["message"], "io");
    }

    #[test]
    fn internal_serializes_kebab_tag() {
        let v = serde_json::to_value(RuntimesApiError::Internal {
            message: "boom".into(),
        })
        .unwrap();
        assert_eq!(v["kind"], "internal");
        assert_eq!(v["message"], "boom");
    }

    #[test]
    fn runtime_status_serializes_with_snake_case_fields() {
        let s = RuntimeStatus {
            kind: RuntimeKind::Ollama,
            installed: true,
            version: Some("0.4.0".into()),
            running: true,
            latency_ms: Some(12),
            model_count: 3,
            last_ping_at: Some("2026-04-27T00:00:00Z".into()),
        };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["kind"], "ollama");
        assert_eq!(v["installed"], true);
        assert_eq!(v["model_count"], 3);
        assert_eq!(v["last_ping_at"], "2026-04-27T00:00:00Z");
    }

    #[test]
    fn local_model_to_view_preserves_fields() {
        let m = LocalModel {
            r#ref: None,
            file_rel_path: "exaone:1.2b".into(),
            size_bytes: 800_000_000,
            sha256: "abc".into(),
        };
        let view = local_model_to_view(RuntimeKind::Ollama, m);
        assert_eq!(view.runtime_kind, RuntimeKind::Ollama);
        assert_eq!(view.id, "exaone:1.2b");
        assert_eq!(view.size_bytes, 800_000_000);
        assert_eq!(view.digest, "abc");
    }
}
