//! Runtimes IPC 모듈 — Phase 4.c.
//!
//! - commands: list_runtime_statuses / list_runtime_models Tauri commands.
//! - 어댑터(Ollama / LM Studio)의 detect + health + list_models 결과를 합쳐서 화면에 노출.
//! - start/stop/restart는 어댑터에서 no-op (외부 데몬). UI v1은 list-only.

pub mod commands;

pub use commands::{list_runtime_models, list_runtime_statuses};
