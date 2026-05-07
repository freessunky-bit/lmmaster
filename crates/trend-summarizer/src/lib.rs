//! `trend-summarizer` — 로컬 LLM 한국어 트렌드 요약 — Phase 22'.e.1 (ADR-0060 §6).
//!
//! 정책:
//! - 4B+ 모델(`Gemma 3 4B` / `Nemotron 3 Nano 4B` / `EXAONE 3.5 7.8B` / `HCX-SEED 8B`)이
//!   trends-bundle 항목들을 카테고리별로 묶어 *한국어 해요체 1~2문장*으로 요약.
//! - 외부 통신 0 — 본 crate는 *순수 로직 + Summarizer trait*. 실 LLM 호출은 호출자(Tauri
//!   command)가 ollama/lm-studio adapter로 inject (.e.3 후속).
//! - SQLite 캐시는 `.e.2`에서 — 본 crate는 *cache key 계산*까지만 (sha256 prompt + items).
//! - LLM judge 0 — system prompt + 결정적 캐시 키만.

pub mod error;
pub mod prompt;
pub mod summarizer;
pub mod types;

pub use error::{SummarizerError, SummarizerResult};
pub use prompt::{build_system_prompt, build_user_prompt, cache_key};
pub use summarizer::{summarize_bundle, MockSummarizer, Summarizer};
pub use types::{SummaryInput, SummaryKind, TrendsSummary};
