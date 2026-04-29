//! `BenchAdapter` trait — runtime 어댑터들이 구현해야 할 측정 인터페이스.
//!
//! 정책 (phase-2pc-bench-decision.md §0):
//! - 어댑터(Ollama/LMStudio)에 streaming 측정 메서드만 추가, 새 HTTP layer 만들지 않음.
//! - 반환 BenchSample은 1회 호출 결과 — 평균/peak는 runner.rs가 합성.
//! - Native counter 받을 수 있으면 (Ollama) `metrics_source: Native`, 그 외 WallclockEst.

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::error::BenchError;
use crate::types::BenchSample;

/// 1회 측정 호출.
#[async_trait]
pub trait BenchAdapter: Send + Sync {
    /// 어댑터 식별 — runtime kind. 캐시 키 + report 라벨.
    fn runtime_label(&self) -> &'static str;

    /// 단일 한국어 prompt를 stream으로 보내고 sample 수집.
    ///
    /// `keep_alive`는 모델을 메모리에 유지할 시간 — warmup 후 측정 시 "5m" 권장.
    /// `cancel`이 발동하면 즉시 중단 — connection drop이 server abort 신호.
    async fn run_prompt(
        &self,
        model_id: &str,
        prompt_id: &str,
        prompt_text: &str,
        keep_alive: &str,
        cancel: &CancellationToken,
    ) -> Result<BenchSample, BenchError>;
}
