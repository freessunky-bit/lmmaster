//! Bench runner — warmup 1회 + measure 2회 + 30초 timeout + cancel.
//!
//! 정책 (phase-2pc-bench-decision.md §4, §5):
//! - tokio::select! { harness, sleep(30s), cancel } — partial report 정책.
//! - warmup pass: keep_alive "5m", load_duration만 보존.
//! - measure pass × 2: 3 prompts × 2 = 6 호출, 산술평균.
//! - cancel 시 stream drop → server abort.
//! - 0 패스 partial = error: Timeout.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use shared_types::{HostFingerprint, RuntimeKind};

use crate::adapter::BenchAdapter;
use crate::error::BenchError;
use crate::types::{
    fingerprint_short, BenchErrorReport, BenchKey, BenchMetricsSource, BenchReport, BenchSample,
    PromptSeed,
};

pub const BENCH_BUDGET_SECS: u64 = 30;
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
pub async fn run_bench(
    adapter: Arc<dyn BenchAdapter>,
    plan: BenchPlan,
    cancel: CancellationToken,
) -> BenchReport {
    let started = Instant::now();
    let bench_at = SystemTime::now();
    let key = plan.key();

    let bench_fut = harness_loop(adapter.clone(), &plan, &cancel);
    let timeout_fut = sleep(Duration::from_secs(BENCH_BUDGET_SECS));
    let cancel_fut = cancel.cancelled();

    let (samples, cold_load_ms, partial_reason) = tokio::select! {
        result = bench_fut => result,
        () = timeout_fut => {
            warn!("bench budget hit ({}s)", BENCH_BUDGET_SECS);
            (Vec::new(), None, Some(PartialReason::Timeout))
        }
        () = cancel_fut => {
            (Vec::new(), None, Some(PartialReason::Cancelled))
        }
    };

    aggregate(
        &plan,
        &key,
        samples,
        cold_load_ms,
        partial_reason,
        bench_at,
        started,
    )
}

#[derive(Debug, Clone, Copy)]
enum PartialReason {
    Timeout,
    Cancelled,
}

/// warmup 1회 + 3 prompts × 2 패스 = 6회 측정 — 30초 budget는 호출 측에서 보장.
async fn harness_loop(
    adapter: Arc<dyn BenchAdapter>,
    plan: &BenchPlan,
    cancel: &CancellationToken,
) -> (Vec<BenchSample>, Option<u32>, Option<PartialReason>) {
    if plan.prompts.is_empty() {
        return (Vec::new(), None, Some(PartialReason::Timeout));
    }

    let mut cold_load_ms: Option<u32> = None;

    // ── warmup ────────────────────────────────────────────────────
    let warmup_seed = &plan.prompts[0];
    match adapter
        .run_prompt(
            &plan.model_id,
            &warmup_seed.id,
            &warmup_seed.text,
            WARMUP_KEEP_ALIVE,
            cancel,
        )
        .await
    {
        Ok(s) => {
            cold_load_ms = s.load_ms;
        }
        Err(BenchError::Cancelled) => return (Vec::new(), None, Some(PartialReason::Cancelled)),
        Err(e) => {
            warn!(error = %e, "warmup failed; aborting");
            return (Vec::new(), cold_load_ms, None);
        }
    }

    // ── measure passes — 2회 × prompts ──────────────────────────
    let mut samples: Vec<BenchSample> = Vec::with_capacity(plan.prompts.len() * 2);
    for pass_index in 0..2u8 {
        for seed in &plan.prompts {
            if cancel.is_cancelled() {
                return (samples, cold_load_ms, Some(PartialReason::Cancelled));
            }
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
                Ok(s) => samples.push(s),
                Err(BenchError::Cancelled) => {
                    return (samples, cold_load_ms, Some(PartialReason::Cancelled));
                }
                Err(e) => {
                    warn!(pass = pass_index, prompt = %seed.id, error = %e, "measure failed");
                    // 한 prompt 실패해도 다음 prompt 계속 시도.
                }
            }
        }
    }

    (samples, cold_load_ms, None)
}

fn aggregate(
    plan: &BenchPlan,
    key: &BenchKey,
    samples: Vec<BenchSample>,
    cold_load_ms: Option<u32>,
    partial_reason: Option<PartialReason>,
    bench_at: SystemTime,
    started: Instant,
) -> BenchReport {
    let took_ms = started.elapsed().as_millis() as u64;
    let timeout_hit = matches!(partial_reason, Some(PartialReason::Timeout));
    let cancelled = matches!(partial_reason, Some(PartialReason::Cancelled));

    let prompts_used: Vec<String> = plan.prompts.iter().map(|p| p.id.clone()).collect();

    if samples.is_empty() {
        let error = if cancelled {
            Some(BenchErrorReport::Cancelled)
        } else if timeout_hit {
            Some(BenchErrorReport::Timeout)
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
        assert!(report.error.is_some());
        assert!(matches!(report.error, Some(BenchErrorReport::Other { .. })));
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
        let r = aggregate(&p, &k, s, Some(50), None, SystemTime::now(), Instant::now());
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
            SystemTime::now(),
            Instant::now(),
        );
        assert!(matches!(r.metrics_source, BenchMetricsSource::WallclockEst));
        // pp_tps는 valid sample 1개만 → 평균 = 80.
        assert_eq!(r.pp_tps, Some(80.0));
    }
}
