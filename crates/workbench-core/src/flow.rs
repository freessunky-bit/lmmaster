//! 5단계 state machine — Data → Quantize → LoRA → Validate → Register.
//!
//! 정책 (phase-5p-workbench-decision.md §1.1, ADR-0023 §Decision 7):
//! - kebab-case serde, tag-less.
//! - `advance_to(step)` — 현재 step을 completed_steps에 push 후 step으로 전이 (중복 push 방지).
//! - `next_step()` — 현재 step 다음 단계 계산 (Register 다음 None).
//! - `mark_completed` / `mark_failed` / `mark_cancelled` — terminal status.
//! - 재실행 cache는 v1.b portable-workspace 통합 시 `workspace/workbench/{run_id}/{step}/`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum WorkbenchStep {
    Data,
    Quantize,
    Lora,
    Validate,
    Register,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RunStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct WorkbenchConfig {
    pub base_model_id: String,
    pub data_jsonl_path: String,
    pub quant_type: String,
    pub lora_epochs: u32,
    pub korean_preset: bool,
    pub register_to_ollama: bool,
    /// Validate stage가 dispatch할 런타임. None이면 Mock(deterministic stub).
    /// Phase 5'.e — 실 HTTP 호출 분기 ("ollama" / "lm-studio" / "mock").
    #[serde(default)]
    pub responder_runtime: Option<String>,
    /// 런타임 base URL — runtime이 Ollama/LmStudio일 때 명시. None이면 기본값 사용.
    #[serde(default)]
    pub responder_base_url: Option<String>,
    /// 런타임 모델 식별자 — Ollama tag / LM Studio loaded model id.
    /// None이면 base_model_id를 그대로 씀.
    #[serde(default)]
    pub responder_model_id: Option<String>,
    /// Phase 9'.b — 실 `llama-quantize` binary 사용. false면 MockQuantizer (기본).
    #[serde(default)]
    pub use_real_quantizer: bool,
    /// Phase 9'.b — 실 LLaMA-Factory CLI 사용. false면 MockLoRATrainer (기본).
    /// 미부트스트랩이면 caller가 사전 동의 후 `LlamaFactoryTrainer::bootstrap_or_open` 호출 필수.
    #[serde(default)]
    pub use_real_trainer: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkbenchRun {
    pub id: String,
    pub created_at: String,
    pub current_step: WorkbenchStep,
    pub completed_steps: Vec<WorkbenchStep>,
    pub config: WorkbenchConfig,
    pub status: RunStatus,
}

impl WorkbenchRun {
    /// 새 run — Pending + Data step.
    pub fn new(config: WorkbenchConfig) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            created_at: time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
            current_step: WorkbenchStep::Data,
            completed_steps: Vec::new(),
            config,
            status: RunStatus::Pending,
        }
    }

    /// 현재 step의 다음 step. Register 다음은 None.
    pub fn next_step(&self) -> Option<WorkbenchStep> {
        match self.current_step {
            WorkbenchStep::Data => Some(WorkbenchStep::Quantize),
            WorkbenchStep::Quantize => Some(WorkbenchStep::Lora),
            WorkbenchStep::Lora => Some(WorkbenchStep::Validate),
            WorkbenchStep::Validate => Some(WorkbenchStep::Register),
            WorkbenchStep::Register => None,
        }
    }

    /// 다음 step으로 전이. 현재 step을 completed에 push (중복 방지) + status Running.
    pub fn advance_to(&mut self, step: WorkbenchStep) {
        if !self.completed_steps.contains(&self.current_step) {
            self.completed_steps.push(self.current_step);
        }
        self.current_step = step;
        self.status = RunStatus::Running;
    }

    /// 마지막 step까지 마쳤음을 표시. 현재 step도 completed에 push.
    pub fn mark_completed(&mut self) {
        if !self.completed_steps.contains(&self.current_step) {
            self.completed_steps.push(self.current_step);
        }
        self.status = RunStatus::Completed;
    }

    /// 실패 상태 — terminal. completed_steps는 그대로 보존 (어디서 멈췄는지 디버그 용).
    pub fn mark_failed(&mut self) {
        self.status = RunStatus::Failed;
    }

    /// 취소 상태 — terminal.
    pub fn mark_cancelled(&mut self) {
        self.status = RunStatus::Cancelled;
    }

    /// 재실행 cache 경로 helper — `workspace/workbench/{run_id}/{step}/`.
    /// 실 I/O는 v1.b portable-workspace 통합 시 처리.
    pub fn cache_path_for(&self, step: WorkbenchStep) -> String {
        let step_label = match step {
            WorkbenchStep::Data => "data",
            WorkbenchStep::Quantize => "quantize",
            WorkbenchStep::Lora => "lora",
            WorkbenchStep::Validate => "validate",
            WorkbenchStep::Register => "register",
        };
        format!("workspace/workbench/{}/{}", self.id, step_label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> WorkbenchConfig {
        WorkbenchConfig {
            base_model_id: "Qwen2.5-3B".into(),
            data_jsonl_path: "./data/train.jsonl".into(),
            quant_type: "Q4_K_M".into(),
            lora_epochs: 3,
            korean_preset: true,
            register_to_ollama: true,
            ..Default::default()
        }
    }

    #[test]
    fn new_run_pending_at_data() {
        let run = WorkbenchRun::new(config());
        assert_eq!(run.current_step, WorkbenchStep::Data);
        assert_eq!(run.status, RunStatus::Pending);
        assert!(run.completed_steps.is_empty());
        assert!(!run.id.is_empty());
        assert!(!run.created_at.is_empty());
    }

    #[test]
    fn next_step_sequence_data_to_register_then_none() {
        let mut run = WorkbenchRun::new(config());
        assert_eq!(run.next_step(), Some(WorkbenchStep::Quantize));
        run.current_step = WorkbenchStep::Quantize;
        assert_eq!(run.next_step(), Some(WorkbenchStep::Lora));
        run.current_step = WorkbenchStep::Lora;
        assert_eq!(run.next_step(), Some(WorkbenchStep::Validate));
        run.current_step = WorkbenchStep::Validate;
        assert_eq!(run.next_step(), Some(WorkbenchStep::Register));
        run.current_step = WorkbenchStep::Register;
        assert_eq!(run.next_step(), None);
    }

    #[test]
    fn advance_to_quantize_pushes_data_to_completed() {
        let mut run = WorkbenchRun::new(config());
        run.advance_to(WorkbenchStep::Quantize);
        assert_eq!(run.current_step, WorkbenchStep::Quantize);
        assert_eq!(run.completed_steps, vec![WorkbenchStep::Data]);
    }

    #[test]
    fn status_running_after_advance() {
        let mut run = WorkbenchRun::new(config());
        run.advance_to(WorkbenchStep::Quantize);
        assert_eq!(run.status, RunStatus::Running);
    }

    #[test]
    fn double_advance_no_duplicate_in_completed() {
        let mut run = WorkbenchRun::new(config());
        run.advance_to(WorkbenchStep::Quantize);
        // 첫 advance: Data가 completed에 push.
        assert_eq!(run.completed_steps, vec![WorkbenchStep::Data]);
        // 동일 step 재진입 — Quantize가 completed에 push 되지만, 두 번째 호출은 Quantize가 이미
        // completed_steps에 있으므로 중복 push 안 됨.
        run.advance_to(WorkbenchStep::Quantize);
        run.advance_to(WorkbenchStep::Quantize);
        // 두 번째 호출에서 Quantize 한 번만 push. 세 번째 호출은 이미 있어서 skip.
        let q_count = run
            .completed_steps
            .iter()
            .filter(|&&s| s == WorkbenchStep::Quantize)
            .count();
        assert_eq!(q_count, 1, "중복 push 방지");
        let d_count = run
            .completed_steps
            .iter()
            .filter(|&&s| s == WorkbenchStep::Data)
            .count();
        assert_eq!(d_count, 1, "Data도 한 번만");
    }

    #[test]
    fn mark_completed_pushes_current_to_completed() {
        let mut run = WorkbenchRun::new(config());
        run.advance_to(WorkbenchStep::Quantize);
        run.advance_to(WorkbenchStep::Lora);
        run.advance_to(WorkbenchStep::Validate);
        run.advance_to(WorkbenchStep::Register);
        run.mark_completed();
        assert_eq!(run.status, RunStatus::Completed);
        assert_eq!(run.completed_steps.len(), 5);
        assert!(run.completed_steps.contains(&WorkbenchStep::Data));
        assert!(run.completed_steps.contains(&WorkbenchStep::Quantize));
        assert!(run.completed_steps.contains(&WorkbenchStep::Lora));
        assert!(run.completed_steps.contains(&WorkbenchStep::Validate));
        assert!(run.completed_steps.contains(&WorkbenchStep::Register));
    }

    #[test]
    fn mark_cancelled_status() {
        let mut run = WorkbenchRun::new(config());
        run.advance_to(WorkbenchStep::Quantize);
        run.mark_cancelled();
        assert_eq!(run.status, RunStatus::Cancelled);
        assert_eq!(run.completed_steps, vec![WorkbenchStep::Data]);
    }

    #[test]
    fn mark_failed_status() {
        let mut run = WorkbenchRun::new(config());
        run.mark_failed();
        assert_eq!(run.status, RunStatus::Failed);
    }

    #[test]
    fn cache_path_uses_run_id_and_step_label() {
        let run = WorkbenchRun::new(config());
        let path = run.cache_path_for(WorkbenchStep::Quantize);
        assert!(path.starts_with("workspace/workbench/"));
        assert!(path.ends_with("/quantize"));
        assert!(path.contains(&run.id));
    }

    #[test]
    fn serde_round_trip() {
        let mut run = WorkbenchRun::new(config());
        run.advance_to(WorkbenchStep::Quantize);
        let s = serde_json::to_string(&run).unwrap();
        let parsed: WorkbenchRun = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed, run);
    }

    #[test]
    fn workbench_step_kebab_case_serde() {
        let s = serde_json::to_string(&WorkbenchStep::Lora).unwrap();
        assert_eq!(s, r#""lora""#);
        let s2 = serde_json::to_string(&WorkbenchStep::Quantize).unwrap();
        assert_eq!(s2, r#""quantize""#);
    }

    #[test]
    fn run_status_kebab_case_serde() {
        let s = serde_json::to_string(&RunStatus::Cancelled).unwrap();
        assert_eq!(s, r#""cancelled""#);
    }

    #[test]
    fn new_uuid_unique_per_run() {
        let r1 = WorkbenchRun::new(config());
        let r2 = WorkbenchRun::new(config());
        assert_ne!(r1.id, r2.id);
    }
}
