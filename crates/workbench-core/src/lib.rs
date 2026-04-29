//! crate: workbench-core — Phase 5' Workbench v1 (Thesis #5).
//!
//! 5단계 플로우: Data → Quantize → LoRA → Validate → Register.
//! 각 단계 trait + v1 mock impl. 실 CLI subprocess는 Phase 5'.b/.c.
//!
//! Korean-first: alpaca-ko 시드 prompt template, 한국어 QA evals 10건.
//!
//! 정책 (ADR-0023, phase-5p-workbench-decision.md):
//! - JSONL 4 포맷 자동 변환 (Alpaca/ShareGPT/OpenAI/한국어 Q&A) → OpenAI messages.
//! - GGUF→Ollama Modelfile generator (escape + multi-stop sequences).
//! - llama-quantize CLI subprocess wrapper (v1.b).
//! - LLaMA-Factory CLI subprocess wrapper (v1.c).
//! - Korean QA evals: deterministic substring matching (LLM-as-judge 거부).

pub mod artifact_retention;
pub mod error;
pub mod eval;
pub mod flow;
pub mod jsonl;
pub mod lora;
pub mod modelfile;
pub mod quantize;

pub use artifact_retention::{
    cleanup as cleanup_artifacts, stats as artifact_stats, ArtifactStats, CleanupReport,
    RetentionError, RetentionPolicy,
};
pub use error::WorkbenchError;
pub use eval::{
    aggregate, aggregate_with_cases, baseline_korean_eval_cases, evaluate_response, run_eval_suite,
    EvalCase, EvalReport, EvalResult, MockResponder, Responder,
};
pub use flow::{RunStatus, WorkbenchConfig, WorkbenchRun, WorkbenchStep};
pub use jsonl::{parse_jsonl, parse_line, to_jsonl_line, write_jsonl, ChatExample, ChatMessage};
pub use lora::{LoRAJob, LoRATrainer, MockLoRATrainer};
pub use modelfile::{escape_system_prompt, render, ModelfileSpec};
pub use quantize::{MockQuantizer, QuantizeJob, QuantizeProgress, Quantizer};
