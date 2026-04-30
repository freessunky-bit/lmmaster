//! Bench IPC 모듈 — Phase 2'.c.2.
//!
//! - registry: in-flight CancellationToken 관리.
//! - cache_store: app_data_dir 기반 BenchReport 디스크 캐시.
//! - commands: start_bench / cancel_bench / get_last_bench_report Tauri commands.

pub mod cache_store;
pub mod commands;
pub mod registry;

pub use commands::{cancel_bench, get_last_bench_report, list_recent_bench_reports, start_bench};
pub use registry::BenchRegistry;
