//! Bench 결과 캐시 디스크 저장소 헬퍼.
//!
//! 정책 (Phase 2'.c.2):
//! - tauri::path::resolve("cache/bench") 기반.
//! - bench-harness::cache::{save, load_if_fresh, invalidate} wrap.
//! - 디렉터리 자동 생성 — 첫 실행 시 빈 디렉터리.

use std::path::PathBuf;
use std::sync::OnceLock;

use bench_harness::{BenchKey, BenchReport};
use tauri::Manager;

/// 캐시 디렉터리 — 한 번 계산하고 process 동안 재사용.
static CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// app data dir 안에 cache/bench 경로를 lazy하게 계산.
pub fn cache_dir(app: &tauri::AppHandle) -> Result<PathBuf, tauri::Error> {
    if let Some(p) = CACHE_DIR.get() {
        return Ok(p.clone());
    }
    let base = app.path().app_data_dir()?;
    let dir = base.join("cache").join("bench");
    let _ = CACHE_DIR.set(dir.clone());
    Ok(dir)
}

pub fn save(
    app: &tauri::AppHandle,
    report: &BenchReport,
    key: &BenchKey,
) -> Result<(), bench_harness::CacheError> {
    let dir = cache_dir(app).map_err(|e| {
        bench_harness::CacheError::Io(std::io::Error::other(format!("path resolve: {e}")))
    })?;
    bench_harness::save(&dir, report, key)
}

pub fn load_if_fresh(
    app: &tauri::AppHandle,
    key: &BenchKey,
    expected_digest: Option<&str>,
) -> Result<Option<BenchReport>, bench_harness::CacheError> {
    let dir = cache_dir(app).map_err(|e| {
        bench_harness::CacheError::Io(std::io::Error::other(format!("path resolve: {e}")))
    })?;
    bench_harness::load_if_fresh(&dir, key, expected_digest)
}

pub fn invalidate(app: &tauri::AppHandle, key: &BenchKey) -> Result<(), bench_harness::CacheError> {
    let dir = cache_dir(app).map_err(|e| {
        bench_harness::CacheError::Io(std::io::Error::other(format!("path resolve: {e}")))
    })?;
    bench_harness::invalidate(&dir, key)
}
