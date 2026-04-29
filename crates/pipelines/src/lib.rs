//! crate: pipelines — Phase 6'.b gateway-side filter framework.
//!
//! 정책 (ADR-0025, phase-6p-updater-pipelines-decision.md §4·§5):
//! - `Pipeline` trait + `PipelineChain` ordered execution.
//! - `apply_request`은 forward 순서, `apply_response`은 reverse 순서 (LIFO middleware standard).
//! - 사용자-노출 메시지 1차 한국어 해요체 (PipelineError variants).
//! - 외부 통신 0 — Pipeline 내부 로직만, 외부 LLM call 없음.
//! - SSE / streaming response는 byte-perfect pass-through (ADR-0022 §2 invariant) — 본 crate는
//!   *full response* 변환만 담당. 게이트웨이 통합 레이어가 SSE 감지 시 우회.
//! - audit log는 매 Pipeline 실행마다 1줄 (`tracing::info!` + `PipelineContext::audit_log`).
//!
//! v1 시드 Pipeline 4종 (Phase 8'.c.1에 PromptSanitize 합류):
//! - `PiiRedactPipeline` — 한국어 PII (주민/휴대폰/카드/이메일) 정규식 redact.
//! - `TokenQuotaPipeline` — `scope.token_budget` 추적 + 초과 시 `BudgetExceeded`.
//! - `ObservabilityPipeline` — request_id / model / pipeline_id tracing 이벤트.
//! - `PromptSanitizePipeline` — NFC 정규화 + zero-width / RTL override 제어 문자 제거.

pub mod chain;
pub mod error;
pub mod observability;
pub mod pii_redact;
pub mod pipeline;
pub mod prompt_sanitize;
pub mod token_quota;

pub use chain::PipelineChain;
pub use error::PipelineError;
pub use observability::ObservabilityPipeline;
pub use pii_redact::PiiRedactPipeline;
pub use pipeline::{AuditEntry, Pipeline, PipelineContext, PipelineStage};
pub use prompt_sanitize::PromptSanitizePipeline;
pub use token_quota::TokenQuotaPipeline;
