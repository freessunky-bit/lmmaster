//! Bench runner — warmup 1회 + measure 2회 + 분리된 timeout + cancel.
//!
//! 정책 (phase-2pc-bench-decision.md §4, §5 + 2026-04-30 사용자 첫 실행 보강):
//! - **warmup**은 cold load (모델을 메모리에 처음 올림) 포함이라 측정 budget과 분리.
//!   - 1.2B 모델도 CPU에선 첫 응답까지 15-30초 걸릴 수 있고, 7B+면 30-60초.
//!   - warmup 자체 timeout 90s, 본 측정과 별개.
//! - **measure budget**: warmup 끝난 후부터 60초. 이때 모델은 warm 상태라 호출당 1-3초.
//!   - 6 호출 (2 패스 × 3 prompt) × 평균 5초 = 30초 표준, CPU only면 60초까지 여유.
//! - cancel 시 stream drop → server abort.
//! - 0 패스 partial = error: ColdLoadTimeout (warmup 단계) 또는 Timeout (measure 단계).
//! - 각 호출 elapsed_ms를 tracing::info로 로그 — 사용자 진단 + 후속 튜닝.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use shared_types::{HostFingerprint, RuntimeKind};

use crate::adapter::BenchAdapter;
use crate::error::BenchError;
use crate::types::{
    fingerprint_short, BenchErrorReport, BenchKey, BenchMetricsSource, BenchReport, BenchSample,
    PromptSeed,
};

/// 본 측정 (warmup 후) budget. 6 호출에 충분.
pub const BENCH_BUDGET_SECS: u64 = 60;
/// Warmup (cold model load + 첫 응답) timeout. measure budget 시작 전에 별도 소진.
pub const WARMUP_TIMEOUT_SECS: u64 = 90;
pub const WARMUP_KEEP_ALIVE: &str = "5m";
pub const MEASURE_KEEP_ALIVE: &str = "5m";

/// 통합 입력 — UI/IPC가 만들어 넘겨주는 1회 측정 명세.
#[derive(Debug, Clone)]
pub struct BenchPlan {
    pub runtime_kind: RuntimeKind,
    pub model_id: String,
    pub quant_label: Option<String>,
    pub digest_at_bench: Option<String>,
    pub prompts: Vec<PromptSeed>,
    pub host: HostFingerprint,
}

impl BenchPlan {
    pub fn key(&self) -> BenchKey {
        BenchKey {
            runtime_kind: self.runtime_kind,
            model_id: self.model_id.clone(),
            quant_label: self.quant_label.clone(),
            host_fingerprint_short: fingerprint_short(&self.host),
        }
    }
}

/// 30초 budget 안에서 warmup + measure 패스 수행 → BenchReport.
///
/// 외부에서 `cancel.cancelled()` 호출 시 즉시 부분 보고서 반환.
/// timeout 발생 시 `timeout_hit: true`로 부분 보고서 반환.
///
/// 모든 호출이 실패해 sample이 0개일 때, **마지막 어댑터 에러를 그대로 보존**해
/// `BenchErrorReport`로 매핑한다. 사용자가 "Ollama가 안 켜졌어요"인지 "이 모델 안 받았어요"인지
/// 구분할 수 있게 하기 위함 — generic "측정 호출이 모두 실패했어요"는 이제 `BenchError::Internal`이
/// 마지막일 때만 노출된다.
pub async fn run_bench(
    adapter: Arc<dyn BenchAdapter>,
    plan: BenchPlan,
    cancel: CancellationToken,
) -> BenchReport {
    let started = Instant::now();
    let bench_at = SystemTime::now();
    let key = plan.key();

    info!(
        runtime = ?plan.runtime_kind,
        model = %plan.model_id,
        warmup_timeout_s = WARMUP_TIMEOUT_SECS,
        measure_budget_s = BENCH_BUDGET_SECS,
        "bench: starting"
    );

    // ── Phase 1: Warmup (cold load 포함, 90s 한도) — measure budget과 완전 분리 ──
    if plan.prompts.is_empty() {
        return aggregate(
            &plan,
            &key,
            Vec::new(),
            None,
            Some(PartialReason::Timeout),
            None,
            bench_at,
            started,
        );
    }
    let warmup_started = Instant::now();
    let warmup_seed = &plan.prompts[0];
    let warmup_fut = adapter.run_prompt(
        &plan.model_id,
        &warmup_seed.id,
        &warmup_seed.text,
        WARMUP_KEEP_ALIVE,
        &cancel,
    );
    let cold_load_ms: Option<u32> = tokio::select! {
        result = warmup_fut => match result {
            Ok(s) => {
                let elapsed = warmup_started.elapsed().as_millis() as u32;
                info!(
                    elapsed_ms = elapsed,
                    load_ms = ?s.load_ms,
                    ttft_ms = s.ttft_ms,
                    "bench: warmup done (cold load 포함)"
                );
                s.load_ms
            }
            Err(BenchError::Cancelled) => {
                return aggregate(&plan, &key, Vec::new(), None, Some(PartialReason::Cancelled), None, bench_at, started);
            }
            Err(e) => {
                warn!(error = %e, elapsed_ms = warmup_started.elapsed().as_millis() as u64, "bench: warmup failed");
                return aggregate(&plan, &key, Vec::new(), None, None, err_to_report(&e), bench_at, started);
            }
        },
        () = sleep(Duration::from_secs(WARMUP_TIMEOUT_SECS)) => {
            let elapsed = warmup_started.elapsed().as_secs();
            warn!(elapsed_s = elapsed, timeout_s = WARMUP_TIMEOUT_SECS, "bench: warmup timeout");
            return aggregate(
                &plan,
                &key,
                Vec::new(),
                None,
                Some(PartialReason::WarmupTimeout),
                None,
                bench_at,
                started,
            );
        }
        () = cancel.cancelled() => {
            return aggregate(&plan, &key, Vec::new(), None, Some(PartialReason::Cancelled), None, bench_at, started);
        }
    };

    // ── Phase 2: Measurement (60s budget, warm 상태 모델로 6 호출) ──
    let measure_started = Instant::now();
    let measure_fut = measure_loop(adapter.clone(), &plan, &cancel);
    let (samples, partial_reason, last_error) = tokio::select! {
        result = measure_fut => {
            let (s, r, e) = result;
            info!(
                count = s.len(),
                elapsed_ms = measure_started.elapsed().as_millis() as u64,
                partial = ?r,
                "bench: measure done"
            );
            (s, r, e)
        }
        () = sleep(Duration::from_secs(BENCH_BUDGET_SECS)) => {
            warn!(
                budget_s = BENCH_BUDGET_SECS,
                "bench: measure budget hit — partial report"
            );
            (Vec::new(), Some(PartialReason::Timeout), None)
        }
        () = cancel.cancelled() => {
            (Vec::new(), Some(PartialReason::Cancelled), None)
        }
    };

    aggregate(
        &plan,
        &key,
        samples,
        cold_load_ms,
        partial_reason,
        last_error,
        bench_at,
        started,
    )
}

#[derive(Debug, Clone, Copy)]
enum PartialReason {
    /// measure budget (warmup 후) 타임아웃.
    Timeout,
    /// warmup 단계 자체가 90s 안에 못 끝남 — 모델이 처음 켜지는 중일 가능성.
    WarmupTimeout,
    Cancelled,
}

/// 어댑터 에러를 사용자 향 `BenchErrorReport`로 매핑. `Cancelled`는 호출 측이 별도 처리.
fn err_to_report(e: &BenchError) -> Option<BenchErrorReport> {
    match e {
        BenchError::RuntimeUnreachable(m) => {
            Some(BenchErrorReport::RuntimeUnreachable { message: m.clone() })
        }
        BenchError::ModelNotLoaded(m) => Some(BenchErrorReport::ModelNotLoaded {
            model_id: m.clone(),
        }),
        BenchError::InsufficientVram { need_mb, have_mb } => {
            Some(BenchErrorReport::InsufficientVram {
                need_mb: *need_mb,
                have_mb: *have_mb,
            })
        }
        BenchError::Timeout => Some(BenchErrorReport::Timeout),
        BenchError::Cancelled => Some(BenchErrorReport::Cancelled),
        BenchError::Internal(m) => Some(BenchErrorReport::Other { message: m.clone() }),
    }
}

/// 본 측정 루프 — 3 prompts × 2 패스 = 6회. warmup은 호출 측이 별도 처리.
///
/// 정책:
/// - 각 호출 elapsed_ms를 tracing::info로 로그 (사용자 진단 + 후속 튜닝).
/// - 모든 호출이 실패하면 마지막 어댑터 에러를 `last_error`로 반환 (sample_count==0 케이스 진단).
/// - 한 호출 실패해도 다음 호출 계속 — 일시 hiccup 흡수.
#[allow(clippy::type_complexity)]
async fn measure_loop(
    adapter: Arc<dyn BenchAdapter>,
    plan: &BenchPlan,
    cancel: &CancellationToken,
) -> (
    Vec<BenchSample>,
    Option<PartialReason>,
    Option<BenchErrorReport>,
) {
    let mut last_error: Option<BenchErrorReport> = None;
    let mut samples: Vec<BenchSample> = Vec::with_capacity(plan.prompts.len() * 2);
    for pass_index in 0..2u8 {
        for seed in &plan.prompts {
            if cancel.is_cancelled() {
                return (samples, Some(PartialReason::Cancelled), last_error);
            }
            let call_started = Instant::now();
            match adapter
                .run_prompt(
                    &plan.model_id,
                    &seed.id,
                    &seed.text,
                    MEASURE_KEEP_ALIVE,
                    cancel,
                )
                .await
            {
                Ok(s) => {
                    let elapsed = call_started.elapsed().as_millis() as u64;
                    info!(
                        pass = pass_index,
                        prompt = %seed.id,
                        elapsed_ms = elapsed,
                        tg_tps = s.tg_tps,
                        ttft_ms = s.ttft_ms,
                        "bench: measure call ok"
                    );
                    samples.push(s);
                }
                Err(BenchError::Cancelled) => {
                    return (samples, Some(PartialReason::Cancelled), last_error);
                }
                Err(e) => {
                    let elapsed = call_started.elapsed().as_millis() as u64;
                    warn!(
                        pass = pass_index,
                        prompt = %seed.id,
                        elapsed_ms = elapsed,
                        error = %e,
                        "bench: measure call failed"
                    );
                    last_error = err_to_report(&e);
                    // 한 prompt 실패해도 다음 prompt 계속 시도 — 일시 hiccup도 있을 수 있음.
                }
            }
        }
    }

    (samples, None, last_error)
}

#[allow(clippy::too_many_arguments)]
fn aggregate(
    plan: &BenchPlan,
    key: &BenchKey,
    samples: Vec<BenchSample>,
    cold_load_ms: Option<u32>,
    partial_reason: Option<PartialReason>,
    last_error: Option<BenchErrorReport>,
    bench_at: SystemTime,
    started: Instant,
) -> BenchReport {
    let took_ms = started.elapsed().as_millis() as u64;
    let timeout_hit = matches!(
        partial_reason,
        Some(PartialReason::Timeout) | Some(PartialReason::WarmupTimeout)
    );
    let cancelled = matches!(partial_reason, Some(PartialReason::Cancelled));
    let warmup_timeout = matches!(partial_reason, Some(PartialReason::WarmupTimeout));

    let prompts_used: Vec<String> = plan.prompts.iter().map(|p| p.id.clone()).collect();

    if samples.is_empty() {
        // 우선순위: cancelled > warmup-timeout > timeout > 실제 어댑터 에러 > generic.
        // warmup_timeout은 사용자에게 "처음 켜지는 중이에요. 다시 하면 빨라져요"로 안내해야
        // 하므로 분리. 일반 timeout보다 진단성이 높음.
        let error = if cancelled {
            Some(BenchErrorReport::Cancelled)
        } else if warmup_timeout {
            Some(BenchErrorReport::Other {
                message: format!(
                    "모델을 처음 켜는 중이에요 ({}초). 한 번 더 시도하면 빨라져요.",
                    WARMUP_TIMEOUT_SECS
                ),
            })
        } else if timeout_hit {
            Some(BenchErrorReport::Timeout)
        } else if let Some(reported) = last_error {
            Some(reported)
        } else {
            Some(BenchErrorReport::Other {
                message: "측정 호출이 모두 실패했어요".into(),
            })
        };
        return BenchReport {
            runtime_kind: plan.runtime_kind,
            model_id: plan.model_id.clone(),
            quant_label: plan.quant_label.clone(),
            host_fingerprint_short: key.host_fingerprint_short.clone(),
            bench_at,
            digest_at_bench: plan.digest_at_bench.clone(),
            tg_tps: 0.0,
            ttft_ms: 0,
            pp_tps: None,
            e2e_ms: 0,
            cold_load_ms,
            peak_vram_mb: None,
            peak_ram_delta_mb: None,
            metrics_source: BenchMetricsSource::Native,
            sample_count: 0,
            prompts_used,
            timeout_hit,
            sample_text_excerpt: None,
            took_ms,
            error,
        };
    }

    let n = samples.len() as f64;
    let tg_tps = samples.iter().map(|s| s.tg_tps).sum::<f64>() / n;
    let ttft_ms =
        (samples.iter().map(|s| s.ttft_ms as u64).sum::<u64>() / samples.len() as u64) as u32;
    let e2e_ms =
        (samples.iter().map(|s| s.e2e_ms as u64).sum::<u64>() / samples.len() as u64) as u32;

    let pp_samples: Vec<f64> = samples.iter().filter_map(|s| s.pp_tps).collect();
    let pp_tps = if pp_samples.is_empty() {
        None
    } else {
        Some(pp_samples.iter().sum::<f64>() / pp_samples.len() as f64)
    };

    // metrics_source — 모두 Native면 Native, 하나라도 WallclockEst이면 WallclockEst.
    let metrics_source = if samples
        .iter()
        .all(|s| s.metrics_source == BenchMetricsSource::Native)
    {
        BenchMetricsSource::Native
    } else {
        BenchMetricsSource::WallclockEst
    };

    let sample_text_excerpt = samples.iter().find_map(|s| s.sample_text_excerpt.clone());

    BenchReport {
        runtime_kind: plan.runtime_kind,
        model_id: plan.model_id.clone(),
        quant_label: plan.quant_label.clone(),
        host_fingerprint_short: key.host_fingerprint_short.clone(),
        bench_at,
        digest_at_bench: plan.digest_at_bench.clone(),
        tg_tps,
        ttft_ms,
        pp_tps,
        e2e_ms,
        cold_load_ms,
        peak_vram_mb: None,
        peak_ram_delta_mb: None,
        metrics_source,
        sample_count: samples.len() as u8,
        prompts_used,
        timeout_hit,
        sample_text_excerpt,
        took_ms,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BenchSample, PromptTask};
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn host() -> HostFingerprint {
        HostFingerprint {
            os: "windows".into(),
            arch: "x86_64".into(),
            cpu: "test".into(),
            ram_mb: 16384,
            gpu_vendor: None,
            gpu_model: None,
            vram_mb: None,
        }
    }

    fn seed(id: &str, task: PromptTask) -> PromptSeed {
        PromptSeed {
            id: id.into(),
            task,
            text: "테스트 프롬프트".into(),
            target_tokens: 30,
        }
    }

    fn sample(prompt_id: &str, tg_tps: f64) -> BenchSample {
        BenchSample {
            tg_tps,
            pp_tps: Some(80.0),
            ttft_ms: 800,
            e2e_ms: 4000,
            load_ms: Some(50),
            sample_text_excerpt: Some("응답".into()),
            prompt_id: prompt_id.into(),
            metrics_source: BenchMetricsSource::Native,
        }
    }

    /// 항상 같은 sample을 반환하는 fake adapter.
    struct OkAdapter {
        calls: AtomicUsize,
        tg_tps: f64,
    }

    #[async_trait]
    impl BenchAdapter for OkAdapter {
        fn runtime_label(&self) -> &'static str {
            "test"
        }
        async fn run_prompt(
            &self,
            _model_id: &str,
            prompt_id: &str,
            _text: &str,
            _keep_alive: &str,
            _cancel: &CancellationToken,
        ) -> Result<BenchSample, BenchError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(sample(prompt_id, self.tg_tps))
        }
    }

    /// 항상 실패하는 adapter.
    struct FailAdapter;

    #[async_trait]
    impl BenchAdapter for FailAdapter {
        fn runtime_label(&self) -> &'static str {
            "fail"
        }
        async fn run_prompt(
            &self,
            _model_id: &str,
            _prompt_id: &str,
            _text: &str,
            _keep_alive: &str,
            _cancel: &CancellationToken,
        ) -> Result<BenchSample, BenchError> {
            Err(BenchError::RuntimeUnreachable("offline".into()))
        }
    }

    /// 첫 호출(워밍업)만 성공, 나머지는 cancel을 sleep 후 발동시킬 수 있게 token 보유.
    struct CancelAdapter {
        token: CancellationToken,
        calls: AtomicUsize,
    }

    #[async_trait]
    impl BenchAdapter for CancelAdapter {
        fn runtime_label(&self) -> &'static str {
            "cancel"
        }
        async fn run_prompt(
            &self,
            _model_id: &str,
            prompt_id: &str,
            _text: &str,
            _keep_alive: &str,
            _cancel: &CancellationToken,
        ) -> Result<BenchSample, BenchError> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                return Ok(sample(prompt_id, 12.0));
            }
            // 두 번째 호출이 시작되면 cancel 발동.
            self.token.cancel();
            Err(BenchError::Cancelled)
        }
    }

    fn plan() -> BenchPlan {
        BenchPlan {
            runtime_kind: RuntimeKind::Ollama,
            model_id: "test-model".into(),
            quant_label: Some("Q4_K_M".into()),
            digest_at_bench: Some("sha256:abc".into()),
            prompts: vec![
                seed("chat", PromptTask::Chat),
                seed("summary", PromptTask::Summary),
                seed("reasoning", PromptTask::Reasoning),
            ],
            host: host(),
        }
    }

    #[tokio::test]
    async fn ok_adapter_produces_average_report() {
        let adapter = Arc::new(OkAdapter {
            calls: AtomicUsize::new(0),
            tg_tps: 12.5,
        });
        let cancel = CancellationToken::new();
        let report = run_bench(adapter.clone(), plan(), cancel).await;

        // 1 warmup + 3 prompts × 2 = 7 호출.
        assert_eq!(adapter.calls.load(Ordering::SeqCst), 7);
        // 6 측정 sample.
        assert_eq!(report.sample_count, 6);
        assert!(report.error.is_none());
        assert!((report.tg_tps - 12.5).abs() < 0.01);
        assert_eq!(report.ttft_ms, 800);
        assert!(report.cold_load_ms.is_some());
        assert!(matches!(report.metrics_source, BenchMetricsSource::Native));
        assert_eq!(report.prompts_used.len(), 3);
        assert!(!report.timeout_hit);
    }

    #[tokio::test]
    async fn fail_adapter_returns_partial_with_error() {
        let adapter = Arc::new(FailAdapter);
        let cancel = CancellationToken::new();
        let report = run_bench(adapter, plan(), cancel).await;
        assert_eq!(report.sample_count, 0);
        // FailAdapter가 RuntimeUnreachable을 던지므로 그대로 매핑 — 더 이상 generic Other 아님.
        // 사용자가 "Ollama가 안 켜졌어요" 같은 구체적 안내를 받는 것이 새 정책.
        assert!(matches!(
            report.error,
            Some(BenchErrorReport::RuntimeUnreachable { .. })
        ));
    }

    #[tokio::test]
    async fn cancel_returns_partial_cancelled() {
        let cancel = CancellationToken::new();
        let adapter = Arc::new(CancelAdapter {
            token: cancel.clone(),
            calls: AtomicUsize::new(0),
        });
        let report = run_bench(adapter, plan(), cancel).await;
        // warmup 1회 성공 후 measure 첫 prompt에서 cancel.
        assert!(report.sample_count <= 1);
        // partial이지만 cancelled로 마크 — error_report가 Cancelled여야 함 (sample_count=0일 때만).
        if report.sample_count == 0 {
            assert!(matches!(report.error, Some(BenchErrorReport::Cancelled)));
        }
    }

    #[tokio::test]
    async fn empty_prompts_returns_timeout_error() {
        let adapter = Arc::new(OkAdapter {
            calls: AtomicUsize::new(0),
            tg_tps: 10.0,
        });
        let mut p = plan();
        p.prompts.clear();
        let cancel = CancellationToken::new();
        let report = run_bench(adapter, p, cancel).await;
        assert_eq!(report.sample_count, 0);
        assert!(report.error.is_some());
    }

    #[test]
    fn aggregate_all_native_keeps_native() {
        let s = vec![sample("a", 10.0), sample("b", 14.0)];
        let p = plan();
        let k = p.key();
        let r = aggregate(
            &p,
            &k,
            s,
            Some(50),
            None,
            None,
            SystemTime::now(),
            Instant::now(),
        );
        assert_eq!(r.tg_tps, 12.0);
        assert!(matches!(r.metrics_source, BenchMetricsSource::Native));
        assert_eq!(r.sample_count, 2);
    }

    #[test]
    fn aggregate_mixed_sources_yields_wallclock() {
        let mut s1 = sample("a", 10.0);
        s1.metrics_source = BenchMetricsSource::WallclockEst;
        s1.pp_tps = None;
        let s2 = sample("b", 14.0);
        let p = plan();
        let k = p.key();
        let r = aggregate(
            &p,
            &k,
            vec![s1, s2],
            None,
            None,
            None,
            SystemTime::now(),
            Instant::now(),
        );
        assert!(matches!(r.metrics_source, BenchMetricsSource::WallclockEst));
        // pp_tps는 valid sample 1개만 → 평균 = 80.
        assert_eq!(r.pp_tps, Some(80.0));
    }

    /// invariant: 어댑터가 RuntimeUnreachable을 던지면 BenchReport에 그대로 전달돼야 함.
    /// 사용자에게 "Ollama 안 켜졌어요" 같은 구체적 안내를 주기 위함 — generic 메시지는 폐기.
    #[tokio::test]
    async fn runtime_unreachable_propagates_specific_error() {
        let adapter = Arc::new(FailAdapter);
        let cancel = CancellationToken::new();
        let report = run_bench(adapter, plan(), cancel).await;
        assert_eq!(report.sample_count, 0);
        assert!(matches!(
            report.error,
            Some(BenchErrorReport::RuntimeUnreachable { .. })
        ));
    }

    /// 모델이 등록되지 않은 어댑터 — ModelNotLoaded가 그대로 전달돼야 함.
    struct ModelMissingAdapter;
    #[async_trait]
    impl BenchAdapter for ModelMissingAdapter {
        fn runtime_label(&self) -> &'static str {
            "missing"
        }
        async fn run_prompt(
            &self,
            model_id: &str,
            _prompt_id: &str,
            _text: &str,
            _keep_alive: &str,
            _cancel: &CancellationToken,
        ) -> Result<BenchSample, BenchError> {
            Err(BenchError::ModelNotLoaded(model_id.into()))
        }
    }

    #[tokio::test]
    async fn model_missing_propagates_specific_error() {
        let adapter = Arc::new(ModelMissingAdapter);
        let cancel = CancellationToken::new();
        let report = run_bench(adapter, plan(), cancel).await;
        assert_eq!(report.sample_count, 0);
        match report.error {
            Some(BenchErrorReport::ModelNotLoaded { model_id }) => {
                assert_eq!(model_id, "test-model");
            }
            other => panic!("기대: ModelNotLoaded, 실제: {other:?}"),
        }
    }
}
