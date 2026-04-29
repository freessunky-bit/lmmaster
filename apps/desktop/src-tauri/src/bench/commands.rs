//! Bench Tauri commands — start / cancel / get_last_bench_report.
//!
//! 정책 (Phase 2'.c.2 결정 노트 §0):
//! - start_bench: model_id + runtime_kind + 옵션(quant) → BenchReport (30s budget). 동일 model 진행 중이면 AlreadyRunning.
//! - cancel_bench: idempotent. 진행 없으면 no-op.
//! - get_last_bench_report: 디스크 캐시에서 최근 리포트 조회. None = 측정 없음.
//! - 카탈로그가 동일 process 안에 있어 host fingerprint는 runtime_detector::probe로 즉시 산출.

use std::sync::Arc;

use bench_harness::{
    baseline_korean_seeds, fingerprint_short, run_bench, BenchKey, BenchPlan, BenchReport,
};
use scopeguard::defer;
use serde::Serialize;
use shared_types::RuntimeKind;
use tauri::{AppHandle, Emitter};
use thiserror::Error;

use crate::bench::cache_store;
use crate::bench::registry::{BenchRegistry, BenchRegistryError};

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum BenchApiError {
    #[error("이 모델은 이미 측정 중이에요")]
    AlreadyRunning,

    #[error("호스트 점검 결과를 찾을 수 없어요")]
    HostNotProbed,

    #[error("아직 지원하지 않는 런타임이에요: {runtime}")]
    UnsupportedRuntime { runtime: String },

    #[error("측정 중 내부 오류: {message}")]
    Internal { message: String },
}

impl From<BenchRegistryError> for BenchApiError {
    fn from(e: BenchRegistryError) -> Self {
        match e {
            BenchRegistryError::AlreadyRunning(_) => Self::AlreadyRunning,
        }
    }
}

/// 즉시 30초 벤치마크 실행. host probe + 어댑터 선택 + run_bench.
#[tauri::command]
pub async fn start_bench(
    app: AppHandle,
    bench_registry: tauri::State<'_, Arc<BenchRegistry>>,
    model_id: String,
    runtime_kind: RuntimeKind,
    quant_label: Option<String>,
    digest_at_bench: Option<String>,
) -> Result<BenchReport, BenchApiError> {
    let registry = bench_registry.inner().clone();
    let cancel = registry.try_start(&model_id)?;

    // RAII finish.
    let finish_id = model_id.clone();
    let finish_registry = registry.clone();
    defer! {
        finish_registry.finish(&finish_id);
    }

    // 호스트 fingerprint — runtime_detector probe.
    let env = runtime_detector::probe_environment().await;
    let host = host_fingerprint_from_report(&env).ok_or(BenchApiError::HostNotProbed)?;

    // 어댑터 선택.
    let adapter: Arc<dyn bench_harness::BenchAdapter> = match runtime_kind {
        RuntimeKind::Ollama => Arc::new(adapter_ollama::OllamaAdapter::new()),
        RuntimeKind::LmStudio => Arc::new(adapter_lmstudio::LmStudioAdapter::new()),
        other => {
            return Err(BenchApiError::UnsupportedRuntime {
                runtime: format!("{other:?}"),
            });
        }
    };

    let plan = BenchPlan {
        runtime_kind,
        model_id: model_id.clone(),
        quant_label,
        digest_at_bench,
        prompts: baseline_korean_seeds(),
        host,
    };

    // 시작 이벤트 emit — UI 진행 표시용.
    let _ = app.emit(
        "bench:started",
        &serde_json::json!({ "model_id": model_id }),
    );

    let report = run_bench(adapter, plan, cancel).await;

    // 캐시 저장 — 실패해도 보고서 자체는 반환.
    let key = BenchKey {
        runtime_kind,
        model_id: model_id.clone(),
        quant_label: report.quant_label.clone(),
        host_fingerprint_short: report.host_fingerprint_short.clone(),
    };
    if let Err(e) = cache_store::save(&app, &report, &key) {
        tracing::warn!(error = %e, "bench cache save failed");
    }

    let _ = app.emit("bench:finished", &report);
    Ok(report)
}

/// 진행 중인 측정 취소 — idempotent.
#[tauri::command]
pub fn cancel_bench(bench_registry: tauri::State<'_, Arc<BenchRegistry>>, model_id: String) {
    bench_registry.cancel(&model_id);
}

/// 최근 측정 결과 — 캐시에서. 없으면 None.
#[tauri::command]
pub async fn get_last_bench_report(
    app: AppHandle,
    model_id: String,
    runtime_kind: RuntimeKind,
    quant_label: Option<String>,
    digest_at_bench: Option<String>,
) -> Result<Option<BenchReport>, BenchApiError> {
    let env = runtime_detector::probe_environment().await;
    let host = match host_fingerprint_from_report(&env) {
        Some(h) => h,
        None => return Ok(None),
    };
    let key = BenchKey {
        runtime_kind,
        model_id,
        quant_label,
        host_fingerprint_short: fingerprint_short(&host),
    };
    cache_store::load_if_fresh(&app, &key, digest_at_bench.as_deref()).map_err(|e| {
        BenchApiError::Internal {
            message: e.to_string(),
        }
    })
}

fn host_fingerprint_from_report(
    report: &runtime_detector::EnvironmentReport,
) -> Option<shared_types::HostFingerprint> {
    let h = &report.hardware;
    let primary_gpu = h.gpus.first();
    Some(shared_types::HostFingerprint {
        os: format!("{:?}", h.os.family).to_lowercase(),
        arch: h.os.arch.clone(),
        cpu: h.cpu.brand.clone(),
        ram_mb: h.mem.total_bytes / (1024 * 1024),
        gpu_vendor: primary_gpu.map(|g| format!("{:?}", g.vendor).to_lowercase()),
        gpu_model: primary_gpu.map(|g| g.model.clone()),
        vram_mb: primary_gpu.and_then(|g| g.vram_bytes.map(|b| b / (1024 * 1024))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn already_running_serializes_kebab() {
        let v = serde_json::to_value(BenchApiError::AlreadyRunning).unwrap();
        assert_eq!(v["kind"], "already-running");
    }

    #[test]
    fn unsupported_runtime_serializes_kebab() {
        let v = serde_json::to_value(BenchApiError::UnsupportedRuntime {
            runtime: "Vllm".into(),
        })
        .unwrap();
        assert_eq!(v["kind"], "unsupported-runtime");
        assert_eq!(v["runtime"], "Vllm");
    }
}
