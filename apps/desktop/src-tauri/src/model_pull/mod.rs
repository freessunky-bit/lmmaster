//! 모델 풀 IPC — Ollama `/api/pull` 스트리밍 wrapper.
//!
//! 정책 (phase-install-bench-bugfix-decision §2.2):
//! - `tauri::ipc::Channel<ModelPullEvent>` per-call 스트림 (emit broadcast 회피).
//! - `ModelPullRegistry`로 동일 model_id 중복 풀 차단 + cancel token 보관.
//! - LM Studio: 풀 미지원 — Tauri shell으로 lmstudio.ai 안내 페이지 open (silent install 금지).
//! - 어댑터: 기존 `adapter_ollama::OllamaAdapter::pull_model_stream` 재사용.

pub mod registry;

use std::sync::Arc;

use adapter_ollama::{ModelPullEvent, OllamaAdapter, PullOutcome};
use serde::Serialize;
use shared_types::RuntimeKind;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use thiserror::Error;

use registry::ModelPullRegistry;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ModelPullApiError {
    #[error("이 모델을 이미 받고 있어요 (id={model_id})")]
    AlreadyPulling { model_id: String },

    #[error("아직 지원하지 않는 런타임이에요: {runtime}")]
    UnsupportedRuntime { runtime: String },

    #[error("Ollama 연결 실패: {message}")]
    Unreachable { message: String },

    #[error("모델 풀 중 내부 오류: {message}")]
    Internal { message: String },
}

/// id 등록 해제를 보장하는 Drop guard. 어떤 path로 빠져나가도 finish 호출.
struct PullGuard {
    registry: Arc<ModelPullRegistry>,
    model_id: String,
}

impl Drop for PullGuard {
    fn drop(&mut self) {
        self.registry.finish(&self.model_id);
    }
}

/// `start_model_pull(model_id, runtime_kind, channel)` Tauri command.
///
/// - 등록 → cancel token 발급.
/// - 어댑터별 풀 실행 (Ollama 우선).
/// - 이벤트는 caller-only Channel<ModelPullEvent>로 전달 — broadcast 미사용.
/// - `RAII` guard로 finish 보장 — panic / early return 모두 안전.
#[tauri::command]
pub async fn start_model_pull(
    app: AppHandle,
    registry: State<'_, Arc<ModelPullRegistry>>,
    model_id: String,
    runtime_kind: RuntimeKind,
    channel: Channel<ModelPullEvent>,
) -> Result<PullOutcomeIpc, ModelPullApiError> {
    let registry: Arc<ModelPullRegistry> = (*registry).clone();

    let cancel = registry
        .try_start(&model_id)
        .map_err(|_| ModelPullApiError::AlreadyPulling {
            model_id: model_id.clone(),
        })?;

    let _guard = PullGuard {
        registry,
        model_id: model_id.clone(),
    };

    match runtime_kind {
        RuntimeKind::Ollama => {
            let _ = app; // AppHandle은 v1에선 미사용 — 향후 cache_dir / window emit 확장.
            let adapter = OllamaAdapter::new();
            let channel_tx = channel.clone();
            // Phase R-E.6 (ADR-0058) — Channel close → cancel cascade.
            // 큰 모델 다운로드 도중 사용자 페이지 이탈 시 네트워크 + 디스크 자원 즉시 회수.
            let cancel_for_emit = cancel.clone();
            let outcome = adapter
                .pull_model_stream(
                    &model_id,
                    move |event| {
                        if channel_tx.send(event).is_err() {
                            tracing::debug!(
                                "model_pull channel closed — cancelling backend stream"
                            );
                            cancel_for_emit.cancel();
                        }
                    },
                    &cancel,
                )
                .await;
            Ok(outcome.into())
        }
        RuntimeKind::LmStudio => {
            // LM Studio는 silent install 금지 — 사용자가 LM Studio 앱에서 직접 받도록 가이드.
            let msg = "LM Studio는 자체 앱에서 모델을 받아주세요. 카탈로그의 \"공식 사이트로 이동\"을 눌러 LM Studio 검색을 사용해 주세요.".to_string();
            // Failed 이벤트 emit으로 frontend가 한국어 안내 + 외부 링크 표시 가능.
            let _ = channel.send(ModelPullEvent::Failed {
                message: msg.clone(),
            });
            Err(ModelPullApiError::UnsupportedRuntime {
                runtime: "lm-studio".into(),
            })
        }
        other => Err(ModelPullApiError::UnsupportedRuntime {
            runtime: format!("{other:?}").to_lowercase(),
        }),
    }
}

/// `cancel_model_pull(model_id)` — idempotent. 미존재면 no-op.
#[tauri::command]
pub fn cancel_model_pull(registry: State<'_, Arc<ModelPullRegistry>>, model_id: String) {
    registry.cancel(&model_id);
}

/// IPC 노출용 outcome — backend `PullOutcome`과 1:1 미러.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PullOutcomeIpc {
    Completed,
    Cancelled,
    Failed { message: String },
}

impl From<PullOutcome> for PullOutcomeIpc {
    fn from(o: PullOutcome) -> Self {
        match o {
            PullOutcome::Completed => Self::Completed,
            PullOutcome::Cancelled => Self::Cancelled,
            PullOutcome::Failed(m) => Self::Failed { message: m },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pull_outcome_ipc_serializes_kebab() {
        let v = serde_json::to_value(PullOutcomeIpc::Completed).unwrap();
        assert_eq!(v["kind"], "completed");

        let v2 = serde_json::to_value(PullOutcomeIpc::Failed {
            message: "찾지 못했어요".into(),
        })
        .unwrap();
        assert_eq!(v2["kind"], "failed");
        assert_eq!(v2["message"], "찾지 못했어요");
    }

    #[test]
    fn api_error_serializes_with_kind_tag() {
        let e = ModelPullApiError::AlreadyPulling {
            model_id: "polyglot-ko".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "already-pulling");
        assert_eq!(v["model_id"], "polyglot-ko");
    }

    #[test]
    fn pull_guard_releases_on_drop() {
        let registry = Arc::new(ModelPullRegistry::new());
        let _ = registry.try_start("polyglot-ko").unwrap();
        assert_eq!(registry.in_flight_count(), 1);
        {
            let _g = PullGuard {
                registry: registry.clone(),
                model_id: "polyglot-ko".into(),
            };
        }
        assert_eq!(registry.in_flight_count(), 0);
    }
}
