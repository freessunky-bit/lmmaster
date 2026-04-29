//! 결과 캐시 — `cache/bench/{runtime}-{slug}-{host_short}.json` 단일 파일.
//!
//! 정책 (phase-2pc-bench-decision.md §1.4):
//! - 30일 TTL, host fingerprint 변경 시 자동 invalidate.
//! - JSON 직렬화 — 단순 fs::write/read.
//! - 사용자 명시 "다시 측정"은 cache 파일 삭제로 처리.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::types::{BenchKey, BenchReport};

const TTL: Duration = Duration::from_secs(60 * 60 * 24 * 30); // 30일.

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

/// 결과 디렉터리 + 키 → 파일 경로.
pub fn cache_path(dir: &Path, key: &BenchKey) -> PathBuf {
    dir.join(key.cache_file_name())
}

/// 캐시에서 읽음 — TTL/digest 검사 후 stale이면 None.
pub fn load_if_fresh(
    dir: &Path,
    key: &BenchKey,
    expected_digest: Option<&str>,
) -> Result<Option<BenchReport>, CacheError> {
    let path = cache_path(dir, key);
    if !path.exists() {
        return Ok(None);
    }
    let body = std::fs::read_to_string(&path)?;
    let report: BenchReport = serde_json::from_str(&body)?;

    if !is_fresh(&report, key, expected_digest) {
        return Ok(None);
    }
    Ok(Some(report))
}

fn is_fresh(report: &BenchReport, key: &BenchKey, expected_digest: Option<&str>) -> bool {
    // host fingerprint 다르면 stale.
    if report.host_fingerprint_short != key.host_fingerprint_short {
        return false;
    }
    // runtime/model/quant 다르면 stale.
    if report.runtime_kind != key.runtime_kind || report.model_id != key.model_id {
        return false;
    }
    if report.quant_label != key.quant_label {
        return false;
    }
    // digest mismatch (Ollama 모델 교체 등).
    if let (Some(expected), Some(at_bench)) = (expected_digest, report.digest_at_bench.as_deref()) {
        if expected != at_bench {
            return false;
        }
    }
    // TTL.
    match report.bench_at.elapsed() {
        Ok(d) if d <= TTL => true,
        Ok(_) => false,
        Err(_) => false, // 시계가 미래로 — 안전하게 stale.
    }
}

/// 디스크에 저장 — 부모 디렉터리 자동 생성.
pub fn save(dir: &Path, report: &BenchReport, key: &BenchKey) -> Result<(), CacheError> {
    if !dir.exists() {
        std::fs::create_dir_all(dir)?;
    }
    let path = cache_path(dir, key);
    let body = serde_json::to_string_pretty(report)?;
    std::fs::write(path, body)?;
    Ok(())
}

/// 사용자 명시 "다시 측정" — 단일 파일 삭제.
pub fn invalidate(dir: &Path, key: &BenchKey) -> Result<(), CacheError> {
    let path = cache_path(dir, key);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::BenchMetricsSource;
    use shared_types::RuntimeKind;
    use std::time::SystemTime;
    use tempfile::tempdir;

    fn make_key() -> BenchKey {
        BenchKey {
            runtime_kind: RuntimeKind::Ollama,
            model_id: "exaone-1.2b".into(),
            quant_label: Some("Q4_K_M".into()),
            host_fingerprint_short: "abcdef0123456789".into(),
        }
    }

    fn make_report(at: SystemTime) -> BenchReport {
        BenchReport {
            runtime_kind: RuntimeKind::Ollama,
            model_id: "exaone-1.2b".into(),
            quant_label: Some("Q4_K_M".into()),
            host_fingerprint_short: "abcdef0123456789".into(),
            bench_at: at,
            digest_at_bench: Some("sha256:abc".into()),
            tg_tps: 12.5,
            ttft_ms: 800,
            pp_tps: Some(80.0),
            e2e_ms: 4000,
            cold_load_ms: Some(50),
            peak_vram_mb: Some(2000),
            peak_ram_delta_mb: Some(500),
            metrics_source: BenchMetricsSource::Native,
            sample_count: 6,
            prompts_used: vec!["chat".into(), "summary".into(), "reasoning".into()],
            timeout_hit: false,
            sample_text_excerpt: Some("응답".into()),
            took_ms: 18_000,
            error: None,
        }
    }

    #[test]
    fn save_then_load_round_trip() {
        let tmp = tempdir().unwrap();
        let key = make_key();
        let report = make_report(SystemTime::now());
        save(tmp.path(), &report, &key).unwrap();
        let loaded = load_if_fresh(tmp.path(), &key, Some("sha256:abc"))
            .unwrap()
            .unwrap();
        assert_eq!(loaded.tg_tps, 12.5);
    }

    #[test]
    fn missing_file_returns_none() {
        let tmp = tempdir().unwrap();
        let key = make_key();
        assert!(load_if_fresh(tmp.path(), &key, None).unwrap().is_none());
    }

    #[test]
    fn host_fingerprint_change_invalidates() {
        let tmp = tempdir().unwrap();
        let key = make_key();
        let report = make_report(SystemTime::now());
        save(tmp.path(), &report, &key).unwrap();

        let mut other_host = key.clone();
        other_host.host_fingerprint_short = "ffffffffffffffff".into();
        // 캐시 파일은 다른 host에 대해서는 없어야 함.
        assert!(load_if_fresh(tmp.path(), &other_host, None)
            .unwrap()
            .is_none());
    }

    #[test]
    fn digest_mismatch_invalidates() {
        let tmp = tempdir().unwrap();
        let key = make_key();
        let report = make_report(SystemTime::now());
        save(tmp.path(), &report, &key).unwrap();
        // 다른 digest 요구하면 stale.
        let r = load_if_fresh(tmp.path(), &key, Some("sha256:OTHER")).unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn expired_ttl_invalidates() {
        let tmp = tempdir().unwrap();
        let key = make_key();
        // 60일 전 — TTL 30일 초과.
        let stale_at = SystemTime::now() - Duration::from_secs(60 * 60 * 24 * 60);
        let report = make_report(stale_at);
        save(tmp.path(), &report, &key).unwrap();
        assert!(load_if_fresh(tmp.path(), &key, None).unwrap().is_none());
    }

    #[test]
    fn invalidate_removes_file() {
        let tmp = tempdir().unwrap();
        let key = make_key();
        let report = make_report(SystemTime::now());
        save(tmp.path(), &report, &key).unwrap();
        invalidate(tmp.path(), &key).unwrap();
        assert!(!cache_path(tmp.path(), &key).exists());
        // 두 번 invalidate해도 에러 안 남.
        invalidate(tmp.path(), &key).unwrap();
    }

    #[test]
    fn save_creates_parent_dir() {
        let tmp = tempdir().unwrap();
        let nested = tmp.path().join("nested/cache/bench");
        let key = make_key();
        let report = make_report(SystemTime::now());
        save(&nested, &report, &key).unwrap();
        assert!(cache_path(&nested, &key).exists());
    }
}
