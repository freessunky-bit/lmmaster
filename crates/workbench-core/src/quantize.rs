//! 양자화 CLI subprocess wrapper trait + v1 mock.
//!
//! 정책 (phase-5p-workbench-decision.md §1.4, ADR-0023 §Decision 2):
//! - 실 CLI는 `llama-quantize` (llama.cpp). v1.b에서 `LlamaQuantizer` subprocess + stdout 파싱.
//! - `MockQuantizer`는 0/25/50/75/100% 5-step emit으로 IPC 구조 검증.
//! - cancel 시 즉시 `WorkbenchError::Cancelled`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::error::WorkbenchError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuantizeJob {
    pub input_gguf: String,
    pub output_gguf: String,
    /// 예: "Q4_K_M" / "Q5_K_M" / "Q8_0".
    pub quant_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuantizeProgress {
    pub percent: u8,
    /// "loading" / "quantizing" / "writing" 등 단계 라벨.
    pub stage: String,
    pub message: Option<String>,
}

#[async_trait]
pub trait Quantizer: Send + Sync {
    async fn run(
        &self,
        job: QuantizeJob,
        cancel: &CancellationToken,
    ) -> Result<Vec<QuantizeProgress>, WorkbenchError>;
}

/// v1 mock — 실 subprocess 없이 5-step progress emit. cancel 협력.
pub struct MockQuantizer;

#[async_trait]
impl Quantizer for MockQuantizer {
    async fn run(
        &self,
        job: QuantizeJob,
        cancel: &CancellationToken,
    ) -> Result<Vec<QuantizeProgress>, WorkbenchError> {
        let stages: Vec<(u8, &str, String)> = vec![
            (0, "loading", "모델을 로드하고 있어요".to_string()),
            (
                25,
                "loading",
                format!("{}을(를) 메모리에 올리고 있어요", job.input_gguf),
            ),
            (
                50,
                "quantizing",
                format!("{} 양자화 중이에요", job.quant_type),
            ),
            (75, "quantizing", "거의 끝났어요".to_string()),
            (
                100,
                "writing",
                format!("{}에 저장 중이에요", job.output_gguf),
            ),
        ];

        let mut out = Vec::with_capacity(stages.len());
        for (pct, stage, msg) in stages {
            if cancel.is_cancelled() {
                return Err(WorkbenchError::Cancelled);
            }
            // 1ms sleep — schedule yield + cancel timing 검증.
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            out.push(QuantizeProgress {
                percent: pct,
                stage: stage.to_string(),
                message: Some(msg),
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job() -> QuantizeJob {
        QuantizeJob {
            input_gguf: "./models/in.gguf".into(),
            output_gguf: "./models/out.gguf".into(),
            quant_type: "Q4_K_M".into(),
        }
    }

    #[tokio::test]
    async fn mock_emits_5_stages() {
        let q = MockQuantizer;
        let cancel = CancellationToken::new();
        let progress = q.run(job(), &cancel).await.unwrap();
        assert_eq!(progress.len(), 5);
    }

    #[tokio::test]
    async fn mock_progress_increases() {
        let q = MockQuantizer;
        let cancel = CancellationToken::new();
        let progress = q.run(job(), &cancel).await.unwrap();
        let mut last_pct = 0u8;
        for (i, p) in progress.iter().enumerate() {
            if i > 0 {
                assert!(p.percent > last_pct, "progress must strictly increase");
            }
            last_pct = p.percent;
        }
        assert_eq!(progress.last().unwrap().percent, 100);
    }

    #[tokio::test]
    async fn cancel_returns_cancelled_error() {
        let q = MockQuantizer;
        let cancel = CancellationToken::new();
        cancel.cancel(); // 시작 전 cancel.
        let err = q.run(job(), &cancel).await.unwrap_err();
        assert!(matches!(err, WorkbenchError::Cancelled));
    }

    #[tokio::test]
    async fn first_progress_is_loading_at_0_percent() {
        let q = MockQuantizer;
        let cancel = CancellationToken::new();
        let progress = q.run(job(), &cancel).await.unwrap();
        assert_eq!(progress[0].percent, 0);
        assert_eq!(progress[0].stage, "loading");
    }

    #[tokio::test]
    async fn last_progress_is_writing_at_100_percent() {
        let q = MockQuantizer;
        let cancel = CancellationToken::new();
        let progress = q.run(job(), &cancel).await.unwrap();
        let last = progress.last().unwrap();
        assert_eq!(last.percent, 100);
        assert_eq!(last.stage, "writing");
    }

    #[tokio::test]
    async fn second_progress_includes_input_path() {
        let q = MockQuantizer;
        let cancel = CancellationToken::new();
        let progress = q.run(job(), &cancel).await.unwrap();
        assert!(progress[1]
            .message
            .as_ref()
            .unwrap()
            .contains("./models/in.gguf"));
    }

    #[tokio::test]
    async fn quantizing_stage_includes_quant_type() {
        let q = MockQuantizer;
        let cancel = CancellationToken::new();
        let progress = q.run(job(), &cancel).await.unwrap();
        let quant_msg = progress
            .iter()
            .find(|p| p.stage == "quantizing")
            .and_then(|p| p.message.clone())
            .unwrap();
        assert!(quant_msg.contains("Q4_K_M"));
    }
}
