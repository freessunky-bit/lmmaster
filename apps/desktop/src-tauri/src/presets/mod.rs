//! Phase 4.h — Korean preset IPC 모듈.
//!
//! 정책 (phase-4-screens-decision.md §1.2, phase-4h-presets-decision.md):
//! - 99+ presets across 7 categories (coding/translation/legal/marketing/medical/education/research).
//! - 의료 / 법률은 disclaimer 의무 — preset-registry crate가 build-time 검증.
//! - resource_dir(prod) → workspace-root(dev) 폴백으로 manifests/presets/ 해결.
//! - PresetCache로 첫 호출 시 한 번만 로드 + 이후 invoke마다 cached clone 반환.
//! - get_presets(category?) — 전체 또는 카테고리 필터.
//! - get_preset(id) — 단일 조회.

pub mod commands;

pub use commands::{get_preset, get_presets, PresetApiError, PresetCache};
