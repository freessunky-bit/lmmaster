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

/// Phase 13'.b — Diagnostics 페이지의 "최근 측정한 모델 N개" 표시.
///
/// 정책 (research §6 옵션 A 채택):
/// - bench cache 디렉터리 walk + 파일 mtime 기준 내림차순 정렬 + 상위 N개 deserialize.
/// - 메모리 인덱스 (옵션 B)는 process 재시작 시 비어 있어 첫 진입이 빈 상태 — 사용자 신뢰도 직격이라 거부.
/// - SQLite history (옵션 C)는 KB 단위 JSON 비대화 + 어차피 file scan 필요 — 중복.
pub fn list_recent(
    app: &tauri::AppHandle,
    limit: usize,
) -> Result<Vec<BenchReport>, bench_harness::CacheError> {
    let dir = cache_dir(app).map_err(|e| {
        bench_harness::CacheError::Io(std::io::Error::other(format!("path resolve: {e}")))
    })?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    // (path, mtime) 페어 수집 — sort_by_key로 내림차순.
    let mut entries: Vec<(PathBuf, std::time::SystemTime)> = std::fs::read_dir(&dir)
        .map_err(bench_harness::CacheError::Io)?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) != Some("json") {
                return None;
            }
            let mtime = e.metadata().ok()?.modified().ok()?;
            Some((p, mtime))
        })
        .collect();
    entries.sort_by_key(|e| std::cmp::Reverse(e.1));
    entries.truncate(limit);

    // 각 파일을 BenchReport로 deserialize. 파싱 실패 entry는 skip (호환성).
    let mut out = Vec::new();
    for (path, _) in entries {
        let body = match std::fs::read_to_string(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        if let Ok(report) = serde_json::from_str::<BenchReport>(&body) {
            out.push(report);
        }
    }
    Ok(out)
}
