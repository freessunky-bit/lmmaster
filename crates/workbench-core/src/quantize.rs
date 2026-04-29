//! 양자화 CLI subprocess wrapper trait + v1 mock + v1.b 실 binary.
//!
//! 정책 (phase-5p-workbench-decision.md §1.4, ADR-0023 §Decision 2, ADR-0043):
//! - 실 CLI는 `llama-quantize` (llama.cpp). `LlamaQuantizer` subprocess + stdout 파싱.
//! - `MockQuantizer`는 0/25/50/75/100% 5-step emit으로 IPC 구조 검증.
//! - cancel 시 즉시 `WorkbenchError::Cancelled`.
//! - 진행 streaming은 `run_streaming` (mpsc::Sender) 채널로 1라인당 1 emit.
//! - 기존 `run`은 `run_streaming`을 buffered collector로 wrapping (호환성).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
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
    /// 모든 progress를 `Vec`으로 모아 한 번에 반환 — 짧은 mock 흐름 / unit test 용.
    async fn run(
        &self,
        job: QuantizeJob,
        cancel: &CancellationToken,
    ) -> Result<Vec<QuantizeProgress>, WorkbenchError>;

    /// progress를 `mpsc::Sender`로 streaming. 장시간 실행되는 실 binary는 이 메서드를 override.
    /// 기본 구현은 `run`을 호출 후 결과를 forward — 짧은 작업/테스트 용.
    ///
    /// 실패 시 sender는 caller가 drop. cancel cooperative.
    async fn run_streaming(
        &self,
        job: QuantizeJob,
        progress: mpsc::Sender<QuantizeProgress>,
        cancel: &CancellationToken,
    ) -> Result<(), WorkbenchError> {
        let items = self.run(job, cancel).await?;
        for p in items {
            // send 실패 = 수신자 drop → 무시하고 계속 진행 (워커가 결과를 더 이상 보지 않음).
            if progress.send(p).await.is_err() {
                break;
            }
        }
        Ok(())
    }
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
