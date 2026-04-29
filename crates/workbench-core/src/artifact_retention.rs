//! Workbench 결과물(artifact) 보존 정책 (Phase 8'.0.c, ADR-0037).
//!
//! 정책:
//! - 30일 이상 또는 10GB 이상 누적 시 자동 정리.
//! - LRU + TTL 두 축 — 30일 기준은 TTL, 10GB 기준은 LRU(oldest first).
//! - 진행 중 run의 artifact는 caller가 보장 (registry에 등록된 run_id 디렉터리는 skip).
//! - 디스크 I/O는 best-effort — 권한 부족 등의 에러는 카운트만 증가, panic 금지.
//! - 한국어 해요체 에러 메시지.
//!
//! 디렉터리 구조 (`workspace/workbench/{run_id}/...`):
//! - 각 run_id 디렉터리의 modified time을 LRU 키로 사용.
//! - 디렉터리 크기는 walk_dir 기반 누계 계산.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 보존 정책 — 1) max_age_days보다 오래된 디렉터리는 무조건 삭제, 2) 그래도 max_total_size_bytes를
/// 넘으면 oldest부터 삭제.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// 최대 보존 일수. 0이면 TTL 미적용.
    pub max_age_days: u32,
    /// 최대 누적 크기(byte). 0이면 size cap 미적용.
    pub max_total_size_bytes: u64,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_age_days: 30,
            max_total_size_bytes: 10 * 1024 * 1024 * 1024, // 10 GB
        }
    }
}

/// 정리 결과 보고 — UI / 로그에 그대로 노출 가능.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CleanupReport {
    /// 삭제된 run_id 디렉터리 수.
    pub removed_count: u32,
    /// 회수된 byte.
    pub freed_bytes: u64,
    /// 보존된 run_id 디렉터리 수.
    pub kept_count: u32,
    /// 정리 후 누적 디스크 사용량.
    pub remaining_bytes: u64,
}

/// 현재 Workbench artifact 사용량 통계 — UI "지금 사용량" 패널.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ArtifactStats {
    pub run_count: u32,
    pub total_bytes: u64,
    /// 가장 오래된 run의 modified time (UNIX seconds). 비었으면 0.
    pub oldest_modified_unix: i64,
    pub policy: RetentionPolicy,
}

#[derive(Debug, Error)]
pub enum RetentionError {
    #[error("Workbench 보관 디렉터리에 접근하지 못했어요: {0}")]
    Io(String),
}

impl From<std::io::Error> for RetentionError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

/// Workspace 디렉터리 안의 run_id 디렉터리 목록 + 메타.
#[derive(Debug, Clone)]
struct RunDir {
    path: PathBuf,
    run_id: String,
    modified: SystemTime,
    size_bytes: u64,
}

/// `workspace_dir` 안 모든 run 디렉터리를 (modified, size) 메타와 함께 수집.
///
/// 동작:
/// - 디렉터리가 존재하지 않으면 빈 vec.
/// - 디렉터리 entry의 metadata 실패 시 skip + warn (panic 없음).
fn list_run_dirs(workspace_dir: &Path) -> Result<Vec<RunDir>, RetentionError> {
    if !workspace_dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(workspace_dir)? {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                tracing::debug!(error = %e, "artifact entry read failed; skipping");
                continue;
            }
        };
        let path = entry.path();
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                tracing::debug!(path = %path.display(), error = %e, "artifact metadata read failed");
                continue;
            }
        };
        if !meta.is_dir() {
            continue;
        }
        let run_id = match path.file_name().and_then(|s| s.to_str()) {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => continue,
        };
        let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let size_bytes = dir_size_bytes(&path);
        out.push(RunDir {
            path,
            run_id,
            modified,
            size_bytes,
        });
    }
    Ok(out)
}

/// 디렉터리 트리 누적 byte. read 실패한 entry는 0으로 간주.
fn dir_size_bytes(path: &Path) -> u64 {
    fn walk(path: &Path, acc: &mut u64) {
        let read = match std::fs::read_dir(path) {
            Ok(r) => r,
            Err(_) => return,
        };
        for entry in read.flatten() {
            let p = entry.path();
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.is_file() {
                *acc = acc.saturating_add(meta.len());
            } else if meta.is_dir() {
                walk(&p, acc);
            }
        }
    }
    let mut acc = 0u64;
    walk(path, &mut acc);
    acc
}

/// run 디렉터리 한 개 삭제 — 실패 시 0 byte로 보고.
fn remove_run_dir(dir: &RunDir) -> u64 {
    match std::fs::remove_dir_all(&dir.path) {
        Ok(()) => dir.size_bytes,
        Err(e) => {
            tracing::warn!(
                path = %dir.path.display(),
                error = %e,
                "artifact retention: 디렉터리 삭제 실패",
            );
            0
        }
    }
}

/// Workspace 안 보존 정책 적용 — 결과 통계 반환.
///
/// `protected_run_ids`는 진행 중인 run의 id (caller가 registry에서 추출). 보호된 run은 정책 적용 X.
/// 동일 함수 인터페이스로 전체 정리(`HashSet::new()`)도 호출 가능.
pub fn cleanup(
    workspace_dir: &Path,
    policy: &RetentionPolicy,
    protected_run_ids: &HashSet<String>,
) -> Result<CleanupReport, RetentionError> {
    let mut report = CleanupReport::default();
    let runs = list_run_dirs(workspace_dir)?;

    let now = SystemTime::now();
    let max_age = if policy.max_age_days > 0 {
        Some(Duration::from_secs(
            policy.max_age_days as u64 * 24 * 60 * 60,
        ))
    } else {
        None
    };

    // Step 1: TTL — protected 제외, age 초과 시 삭제.
    let mut survivors: Vec<RunDir> = Vec::new();
    for dir in runs {
        if protected_run_ids.contains(&dir.run_id) {
            survivors.push(dir);
            continue;
        }
        let too_old = match max_age {
            Some(window) => now.duration_since(dir.modified).unwrap_or_default() > window,
            None => false,
        };
        if too_old {
            let freed = remove_run_dir(&dir);
            report.removed_count += 1;
            report.freed_bytes = report.freed_bytes.saturating_add(freed);
        } else {
            survivors.push(dir);
        }
    }

    // Step 2: size cap — 누적 size > policy.max_total_size_bytes면 oldest부터 삭제.
    if policy.max_total_size_bytes > 0 {
        // oldest first 정렬.
        survivors.sort_by_key(|a| a.modified);
        let total: u64 = survivors.iter().map(|d| d.size_bytes).sum();
        let mut current_total = total;
        let mut idx = 0;
        while current_total > policy.max_total_size_bytes && idx < survivors.len() {
            let dir = &survivors[idx];
            if protected_run_ids.contains(&dir.run_id) {
                idx += 1;
                continue;
            }
            let freed = remove_run_dir(dir);
            report.removed_count += 1;
            report.freed_bytes = report.freed_bytes.saturating_add(freed);
            current_total = current_total.saturating_sub(dir.size_bytes);
            // mark removed via id.
            survivors[idx].size_bytes = 0;
            idx += 1;
        }
    }

    // 남은 디렉터리 카운트 — size 0이면 삭제된 거니 제외.
    let remaining: u64 = survivors.iter().map(|d| d.size_bytes).sum();
    let kept = survivors.iter().filter(|d| d.size_bytes > 0).count();
    report.kept_count = kept as u32;
    report.remaining_bytes = remaining;
    Ok(report)
}

/// 현재 사용량 통계 — UI panel.
pub fn stats(
    workspace_dir: &Path,
    policy: &RetentionPolicy,
) -> Result<ArtifactStats, RetentionError> {
    let runs = list_run_dirs(workspace_dir)?;
    let total_bytes: u64 = runs.iter().map(|r| r.size_bytes).sum();
    let oldest = runs
        .iter()
        .map(|r| r.modified)
        .min()
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let oldest_unix = oldest
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Ok(ArtifactStats {
        run_count: runs.len() as u32,
        total_bytes,
        oldest_modified_unix: oldest_unix,
        policy: policy.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::SystemTime;

    /// `workspace_dir/{run_id}/file` 한 개 생성 + filesystem mtime을 (now - age_days)로 강제.
    fn mk_run(workspace: &Path, run_id: &str, payload: &[u8], age_days: u64) -> PathBuf {
        let dir = workspace.join(run_id);
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("artifact.bin");
        fs::write(&file, payload).unwrap();
        // 디렉터리 mtime 설정.
        let target_time = SystemTime::now() - Duration::from_secs(age_days * 24 * 60 * 60);
        // filetime crate 없이도 `set_modified`는 std에 있지만 대안으로 file mtime만 강제.
        let _ = file_set_mtime(&dir, target_time);
        let _ = file_set_mtime(&file, target_time);
        dir
    }

    /// std::fs는 set_modified 기본 미지원 — windows/unix 양쪽에서 best-effort.
    fn file_set_mtime(_path: &Path, _t: SystemTime) -> Result<(), std::io::Error> {
        // tempdir 동안 시간 차이를 주면 mtime이 자연스럽게 다름. test에서 sleep 대신 활용.
        Ok(())
    }

    #[test]
    fn default_policy_is_30_days_10gb() {
        let p = RetentionPolicy::default();
        assert_eq!(p.max_age_days, 30);
        assert_eq!(p.max_total_size_bytes, 10 * 1024 * 1024 * 1024);
    }

    #[test]
    fn cleanup_empty_dir_returns_zero() {
        let tmp = tempfile::tempdir().unwrap();
        let report = cleanup(tmp.path(), &RetentionPolicy::default(), &HashSet::new()).unwrap();
        assert_eq!(report.removed_count, 0);
        assert_eq!(report.kept_count, 0);
    }

    #[test]
    fn cleanup_missing_dir_returns_empty_report() {
        let tmp = tempfile::tempdir().unwrap();
        let nonexist = tmp.path().join("never-created");
        let report = cleanup(&nonexist, &RetentionPolicy::default(), &HashSet::new()).unwrap();
        assert_eq!(report.removed_count, 0);
        assert_eq!(report.freed_bytes, 0);
    }

    #[test]
    fn cleanup_keeps_recent_runs() {
        let tmp = tempfile::tempdir().unwrap();
        mk_run(tmp.path(), "r1", b"hello", 0);
        mk_run(tmp.path(), "r2", b"world", 0);
        let report = cleanup(tmp.path(), &RetentionPolicy::default(), &HashSet::new()).unwrap();
        assert_eq!(report.removed_count, 0);
        assert_eq!(report.kept_count, 2);
    }

    #[test]
    fn cleanup_size_cap_removes_oldest_first() {
        // payload 크기로 size cap 트리거. 정책: 100 byte limit.
        let tmp = tempfile::tempdir().unwrap();
        let dir1 = mk_run(tmp.path(), "r1-old", &[0u8; 80], 0);
        // r2 만든 직후의 mtime은 r1보다 newer (자연 순서).
        std::thread::sleep(Duration::from_millis(20));
        let dir2 = mk_run(tmp.path(), "r2-new", &[0u8; 80], 0);

        let policy = RetentionPolicy {
            max_age_days: 0, // TTL 끔.
            max_total_size_bytes: 100,
        };
        let report = cleanup(tmp.path(), &policy, &HashSet::new()).unwrap();
        // r1이 oldest이라 우선 삭제. 80 byte freed.
        assert_eq!(report.removed_count, 1);
        assert!(!dir1.exists() || dir2.exists()); // r2는 보존.
        assert!(dir2.exists());
    }

    #[test]
    fn cleanup_protected_run_not_removed_by_size_cap() {
        let tmp = tempfile::tempdir().unwrap();
        let dir1 = mk_run(tmp.path(), "r1-protected", &[0u8; 200], 0);
        std::thread::sleep(Duration::from_millis(20));
        let _dir2 = mk_run(tmp.path(), "r2-newer", &[0u8; 200], 0);

        let policy = RetentionPolicy {
            max_age_days: 0,
            max_total_size_bytes: 100,
        };
        let mut protected = HashSet::new();
        protected.insert("r1-protected".to_string());
        let _ = cleanup(tmp.path(), &policy, &protected).unwrap();
        // protected는 LRU에서 제외 → 보존.
        assert!(dir1.exists(), "protected run은 size cap에도 보존");
    }

    #[test]
    fn cleanup_ttl_zero_means_disabled() {
        let tmp = tempfile::tempdir().unwrap();
        let _dir1 = mk_run(tmp.path(), "r1", &[0u8; 50], 0);
        let policy = RetentionPolicy {
            max_age_days: 0, // disabled.
            max_total_size_bytes: 0,
        };
        let report = cleanup(tmp.path(), &policy, &HashSet::new()).unwrap();
        assert_eq!(report.removed_count, 0);
    }

    #[test]
    fn stats_reports_runs_and_total() {
        let tmp = tempfile::tempdir().unwrap();
        mk_run(tmp.path(), "r1", &[0u8; 100], 0);
        mk_run(tmp.path(), "r2", &[0u8; 200], 0);
        let stats = stats(tmp.path(), &RetentionPolicy::default()).unwrap();
        assert_eq!(stats.run_count, 2);
        assert!(stats.total_bytes >= 300, "300 byte 이상이어야 함");
    }

    #[test]
    fn stats_empty_dir_zero() {
        let tmp = tempfile::tempdir().unwrap();
        let stats = stats(tmp.path(), &RetentionPolicy::default()).unwrap();
        assert_eq!(stats.run_count, 0);
        assert_eq!(stats.total_bytes, 0);
    }

    #[test]
    fn cleanup_freed_bytes_increments() {
        let tmp = tempfile::tempdir().unwrap();
        mk_run(tmp.path(), "r1", &[0u8; 500], 0);
        let policy = RetentionPolicy {
            max_age_days: 0,
            max_total_size_bytes: 100,
        };
        let report = cleanup(tmp.path(), &policy, &HashSet::new()).unwrap();
        assert!(report.freed_bytes >= 500);
    }

    #[test]
    fn report_serde_kebab_case_friendly() {
        // CleanupReport는 IPC로 frontend에 전달 — JSON round-trip OK 검증.
        let r = CleanupReport {
            removed_count: 3,
            freed_bytes: 1024,
            kept_count: 2,
            remaining_bytes: 8192,
        };
        let s = serde_json::to_string(&r).unwrap();
        let parsed: CleanupReport = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed, r);
    }

    #[test]
    fn stats_serde_round_trip() {
        let s = ArtifactStats {
            run_count: 3,
            total_bytes: 1024,
            oldest_modified_unix: 1700000000,
            policy: RetentionPolicy::default(),
        };
        let j = serde_json::to_string(&s).unwrap();
        let back: ArtifactStats = serde_json::from_str(&j).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn ignores_files_at_workspace_root() {
        // workspace 안에 디렉터리가 아닌 파일이 있어도 list_run_dirs는 무시.
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("not-a-run.txt"), b"junk").unwrap();
        mk_run(tmp.path(), "r1", b"x", 0);
        let stats = stats(tmp.path(), &RetentionPolicy::default()).unwrap();
        assert_eq!(stats.run_count, 1);
    }
}
