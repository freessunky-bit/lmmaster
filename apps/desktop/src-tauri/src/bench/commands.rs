//! Bench Tauri commands — start / cancel / get_last_bench_report.
//!
//! 정책 (Phase 2'.c.2 결정 노트 §0):
//! - start_bench: model_id + runtime_kind + 옵션(quant) → BenchReport (30s budget). 동일 model 진행 중이면 AlreadyRunning.
//! - cancel_bench: idempotent. 진행 없으면 no-op.
//! - get_last_bench_report: 디스크 캐시에서 최근 리포트 조회. None = 측정 없음.
//! - 카탈로그가 동일 process 안에 있어 host fingerprint는 runtime_detector::probe로 즉시 산출.

use std::sync::Arc;
use std::time::SystemTime;

use bench_harness::{
    baseline_korean_seeds, fingerprint_short, run_bench, BenchErrorReport, BenchKey,
    BenchMetricsSource, BenchPlan, BenchReport,
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

    // ── Preflight (Ollama only) ─────────────────────────────────────────
    //
    // 정책 (phase-install-bench-bugfix-decision §2.4):
    // - /api/version 실패 → RuntimeUnreachable.
    // - /api/tags에 모델 없음 → ModelNotLoaded.
    // - 30초 budget 낭비 + 6회 즉시 fail 노이즈 차단.
    //
    // LM Studio는 v1에서 preflight skip — 어댑터 자체가 model 미존재를 그대로 매핑함.
    if matches!(runtime_kind, RuntimeKind::Ollama) {
        if let Some(report) = ollama_preflight(
            &app,
            runtime_kind,
            &model_id,
            &quant_label,
            &digest_at_bench,
            &host,
        )
        .await
        {
            // 캐시 저장 안 함 — 잠깐 끄거나 한 번만 모델 안 받았을 수 있어, 다음 시도엔 재검증 필요.
            let _ = app.emit("bench:finished", &report);
            return Ok(report);
        }
    }

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

/// Ollama preflight — 런타임 + 모델 존재 빠른 확인. 실패 시 즉시 보고서 반환.
///
/// Returns:
/// - `None` — preflight 통과 → 본 측정 진행.
/// - `Some(report)` — preflight 실패 → 본 측정 skip + 한국어 안내 노출.
async fn ollama_preflight(
    app: &AppHandle,
    runtime_kind: RuntimeKind,
    model_id: &str,
    quant_label: &Option<String>,
    digest_at_bench: &Option<String>,
    host: &shared_types::HostFingerprint,
) -> Option<BenchReport> {
    use adapter_ollama::OllamaAdapter;
    let adapter = OllamaAdapter::new();

    // 1) /api/tags로 모델 + 런타임 동시 검증.
    //    has_model 자체가 /api/tags 호출 → reqwest 연결 실패면 RuntimeUnreachable.
    match adapter.has_model(model_id).await {
        Ok(true) => None,
        Ok(false) => Some(preflight_failed_report(
            runtime_kind,
            model_id,
            quant_label,
            digest_at_bench,
            host,
            BenchErrorReport::ModelNotLoaded {
                model_id: model_id.to_string(),
            },
        )),
        Err(e) => {
            tracing::debug!(error = %e, "ollama preflight failed — runtime unreachable");
            // emit hint 이벤트 — 사용자 향 카피는 BenchChip이 i18n 키로 처리.
            let _ = app.emit(
                "bench:preflight",
                &serde_json::json!({
                    "model_id": model_id,
                    "kind": "runtime-unreachable",
                }),
            );
            Some(preflight_failed_report(
                runtime_kind,
                model_id,
                quant_label,
                digest_at_bench,
                host,
                BenchErrorReport::RuntimeUnreachable {
                    message: e.to_string(),
                },
            ))
        }
    }
}

fn preflight_failed_report(
    runtime_kind: RuntimeKind,
    model_id: &str,
    quant_label: &Option<String>,
    digest_at_bench: &Option<String>,
    host: &shared_types::HostFingerprint,
    error: BenchErrorReport,
) -> BenchReport {
    let host_short = fingerprint_short(host);
    BenchReport {
        runtime_kind,
        model_id: model_id.to_string(),
        quant_label: quant_label.clone(),
        host_fingerprint_short: host_short,
        bench_at: SystemTime::now(),
        digest_at_bench: digest_at_bench.clone(),
        tg_tps: 0.0,
        ttft_ms: 0,
        pp_tps: None,
        e2e_ms: 0,
        cold_load_ms: None,
        peak_vram_mb: None,
        peak_ram_delta_mb: None,
        metrics_source: BenchMetricsSource::Native,
        sample_count: 0,
        prompts_used: Vec::new(),
        timeout_hit: false,
        sample_text_excerpt: None,
        took_ms: 0,
        error: Some(error),
    }
}

/// 진행 중인 측정 취소 — idempotent.
#[tauri::command]
pub fn cancel_bench(bench_registry: tauri::State<'_, Arc<BenchRegistry>>, model_id: String) {
    bench_registry.cancel(&model_id);
}

/// 최근 측정한 모델 N개 일괄 조회 — Diagnostics 페이지 카드 (Phase 13'.b).
///
/// 정책: bench cache 디렉터리 mtime 정렬, 상위 N개 deserialize. limit 누락 시 5.
#[tauri::command]
pub async fn list_recent_bench_reports(
    app: AppHandle,
    limit: Option<u32>,
) -> Result<Vec<BenchReport>, BenchApiError> {
    let n = limit.unwrap_or(5).min(50) as usize;
    cache_store::list_recent(&app, n).map_err(|e| BenchApiError::Internal {
        message: e.to_string(),
    })
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
