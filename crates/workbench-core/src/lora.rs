//! LoRA CLI subprocess wrapper trait + v1 mock.
//!
//! 정책 (phase-5p-workbench-decision.md §1.5, ADR-0023 §Decision 3):
//! - 실 CLI는 LLaMA-Factory `llamafactory-cli train`. v1.c에서 subprocess wrapper.
//! - `MockLoRATrainer`는 mock progress emit. korean_preset = true 시 alpaca-ko 명시.
//! - cancel 시 즉시 `WorkbenchError::Cancelled`.
//! - 진행률 shape는 `QuantizeProgress` 재활용 (UI도 동일 progress pill 재사용).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::error::WorkbenchError;
use crate::quantize::QuantizeProgress;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoRAJob {
    pub base_model: String,
    pub dataset_jsonl: String,
    pub output_adapter: String,
    pub epochs: u32,
    pub lr: f32,
    /// true면 한국어 instruction-tuning preset (alpaca-ko / KoAlpaca template).
    pub korean_preset: bool,
}

#[async_trait]
pub trait LoRATrainer: Send + Sync {
    async fn run(
        &self,
        job: LoRAJob,
        cancel: &CancellationToken,
    ) -> Result<Vec<QuantizeProgress>, WorkbenchError>;
}

/// v1 mock — 실 subprocess 없이 5-step progress emit. cancel 협력.
pub struct MockLoRATrainer;

#[async_trait]
impl LoRATrainer for MockLoRATrainer {
    async fn run(
        &self,
        job: LoRAJob,
        cancel: &CancellationToken,
    ) -> Result<Vec<QuantizeProgress>, WorkbenchError> {
        let template_msg = if job.korean_preset {
            "한국어 alpaca-ko 템플릿으로 학습 중이에요".to_string()
        } else {
            "alpaca 템플릿으로 학습 중이에요".to_string()
        };

        // epochs / 2가 0이 되지 않게 max(1).
        let mid_epoch = std::cmp::max(1, job.epochs / 2);

        let stages: Vec<(u8, &str, String)> = vec![
            (
                0,
                "preparing",
                format!("데이터 {}을(를) 로드하고 있어요", job.dataset_jsonl),
            ),
            (20, "training", template_msg),
            (
                50,
                "training",
                format!("epoch {}/{} 진행 중이에요", mid_epoch, job.epochs),
            ),
            (
                80,
                "training",
                format!("learning rate {}, 마무리 중이에요", job.lr),
            ),
            (
                100,
                "saving",
                format!("어댑터를 {}에 저장했어요", job.output_adapter),
            ),
        ];

        let mut out = Vec::with_capacity(stages.len());
        for (pct, stage, msg) in stages {
            if cancel.is_cancelled() {
                return Err(WorkbenchError::Cancelled);
            }
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

    fn job(korean: bool) -> LoRAJob {
        LoRAJob {
            base_model: "Qwen2.5-3B".into(),
            dataset_jsonl: "./data/train.jsonl".into(),
            output_adapter: "./out/adapter".into(),
            epochs: 4,
            lr: 0.0002,
            korean_preset: korean,
        }
    }

    #[tokio::test]
    async fn mock_emits_5_stages() {
        let t = MockLoRATrainer;
        let cancel = CancellationToken::new();
        let progress = t.run(job(true), &cancel).await.unwrap();
        assert_eq!(progress.len(), 5);
    }

    #[tokio::test]
    async fn korean_preset_true_message_contains_alpaca_ko() {
        let t = MockLoRATrainer;
        let cancel = CancellationToken::new();
        let progress = t.run(job(true), &cancel).await.unwrap();
        // 두 번째 stage에 alpaca-ko 키워드가 들어있어야 함.
        let msg = progress[1].message.as_ref().unwrap();
        assert!(msg.contains("alpaca-ko"));
        assert!(msg.contains("한국어"));
    }

    #[tokio::test]
    async fn korean_preset_false_message_no_alpaca_ko() {
        let t = MockLoRATrainer;
        let cancel = CancellationToken::new();
        let progress = t.run(job(false), &cancel).await.unwrap();
        let msg = progress[1].message.as_ref().unwrap();
        assert!(!msg.contains("alpaca-ko"));
        assert!(msg.contains("alpaca"));
    }

    #[tokio::test]
    async fn cancel_returns_cancelled() {
        let t = MockLoRATrainer;
        let cancel = CancellationToken::new();
        cancel.cancel();
        let err = t.run(job(true), &cancel).await.unwrap_err();
        assert!(matches!(err, WorkbenchError::Cancelled));
    }

    #[tokio::test]
    async fn first_stage_is_preparing_with_dataset_path() {
        let t = MockLoRATrainer;
        let cancel = CancellationToken::new();
        let progress = t.run(job(true), &cancel).await.unwrap();
        assert_eq!(progress[0].stage, "preparing");
        assert!(progress[0]
            .message
            .as_ref()
            .unwrap()
            .contains("./data/train.jsonl"));
    }

    #[tokio::test]
    async fn last_stage_is_saving_with_adapter_path() {
        let t = MockLoRATrainer;
        let cancel = CancellationToken::new();
        let progress = t.run(job(true), &cancel).await.unwrap();
        let last = progress.last().unwrap();
        assert_eq!(last.stage, "saving");
        assert!(last.message.as_ref().unwrap().contains("./out/adapter"));
        assert_eq!(last.percent, 100);
    }
}
