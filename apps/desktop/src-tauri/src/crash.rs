//! Phase 13'.c — Crash report 뷰어 IPC.
//!
//! 정책:
//! - panic_hook이 적재한 `crash_dir()`을 source of truth로 사용. 미설정이면 빈 list.
//! - `list_crash_reports(limit)` — `crash-*.txt` 파일을 mtime DESC로 정렬.
//! - `read_crash_log(filename)` — 정확히 `crash-` prefix + `.txt` suffix만 허용 (path traversal 방어).
//! - 파일 크기 limit 1 MB — 비정상적으로 큰 backtrace 방어.
//! - timestamp는 파일명에서 추출 (RFC3339에서 `:` `.` 가 `-`로 치환된 형태 → 역치환 시도).
//! - 모든 경로 비교는 OS 표준 path 형태 (Windows `\` / Unix `/` 모두).

use std::io::Read as _;
use std::path::PathBuf;
use std::time::SystemTime;

use serde::Serialize;
use thiserror::Error;

use crate::panic_hook;

const CRASH_PREFIX: &str = "crash-";
const CRASH_SUFFIX: &str = ".txt";
/// 1 MB — 비정상 backtrace 보호.
const MAX_CRASH_FILE_BYTES: u64 = 1024 * 1024;
const DEFAULT_LIMIT: usize = 50;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CrashIpcError {
    #[error("크래시 디렉터리가 아직 설정되지 않았어요")]
    NotInitialized,
    #[error("파일 이름이 올바르지 않아요")]
    InvalidFilename,
    #[error("크래시 파일을 찾지 못했어요")]
    NotFound,
    #[error("파일이 너무 커서 읽지 못했어요 ({bytes} 바이트)")]
    TooLarge { bytes: u64 },
    #[error("입출력 오류: {message}")]
    Io { message: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct CrashSummary {
    /// 파일 이름 (예: `crash-2026-04-30T08-12-34-123456789Z.txt`).
    pub filename: String,
    /// 파일 크기 (byte).
    pub size_bytes: u64,
    /// 파일명에서 추출한 RFC3339 timestamp 시도. 실패 시 None.
    pub ts_rfc3339: Option<String>,
    /// 파일 mtime UNIX epoch ms — 정렬 기준.
    pub mtime_ms: i64,
}

/// crash 디렉터리에서 `crash-*.txt`를 mtime DESC로 정렬해서 최대 limit개 반환.
#[tauri::command]
pub fn list_crash_reports(limit: Option<u32>) -> Result<Vec<CrashSummary>, CrashIpcError> {
    let dir = panic_hook::crash_dir().ok_or(CrashIpcError::NotInitialized)?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let cap = limit.unwrap_or(DEFAULT_LIMIT as u32) as usize;
    let mut entries: Vec<CrashSummary> = std::fs::read_dir(&dir)
        .map_err(|e| CrashIpcError::Io {
            message: e.to_string(),
        })?
        .filter_map(|res| res.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with(CRASH_PREFIX) || !name.ends_with(CRASH_SUFFIX) {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() {
                return None;
            }
            let mtime_ms = metadata
                .modified()
                .ok()
                .and_then(|t| {
                    t.duration_since(SystemTime::UNIX_EPOCH)
                        .ok()
                        .map(|d| d.as_millis() as i64)
                })
                .unwrap_or(0);
            Some(CrashSummary {
                ts_rfc3339: filename_to_rfc3339(&name),
                filename: name,
                size_bytes: metadata.len(),
                mtime_ms,
            })
        })
        .collect();
    entries.sort_by_key(|e| std::cmp::Reverse(e.mtime_ms));
    entries.truncate(cap);
    Ok(entries)
}

/// 단일 crash 파일을 통째로 읽어서 반환.
///
/// 보안: filename은 `crash-` prefix + `.txt` suffix + path separator 미포함만 허용.
#[tauri::command]
pub fn read_crash_log(filename: String) -> Result<String, CrashIpcError> {
    if !is_safe_crash_filename(&filename) {
        return Err(CrashIpcError::InvalidFilename);
    }
    let dir = panic_hook::crash_dir().ok_or(CrashIpcError::NotInitialized)?;
    let path: PathBuf = dir.join(&filename);
    if !path.is_file() {
        return Err(CrashIpcError::NotFound);
    }
    let metadata = std::fs::metadata(&path).map_err(|e| CrashIpcError::Io {
        message: e.to_string(),
    })?;
    if metadata.len() > MAX_CRASH_FILE_BYTES {
        return Err(CrashIpcError::TooLarge {
            bytes: metadata.len(),
        });
    }
    let mut file = std::fs::File::open(&path).map_err(|e| CrashIpcError::Io {
        message: e.to_string(),
    })?;
    let mut buf = String::with_capacity(metadata.len() as usize);
    file.read_to_string(&mut buf)
        .map_err(|e| CrashIpcError::Io {
            message: e.to_string(),
        })?;
    Ok(buf)
}

/// 파일명 안전성 검증 — path traversal / 외부 경로 차단.
fn is_safe_crash_filename(name: &str) -> bool {
    if !name.starts_with(CRASH_PREFIX) || !name.ends_with(CRASH_SUFFIX) {
        return false;
    }
    // separator / 부모 참조 / 절대경로 표시 차단.
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return false;
    }
    // null byte 차단.
    if name.as_bytes().contains(&0) {
        return false;
    }
    true
}

/// 파일명에서 timestamp 부분 추출 + RFC3339 역치환 시도.
/// `crash-2026-04-30T08-12-34-123456789Z.txt` → `2026-04-30T08:12:34.123456789Z`.
/// 단순 휴리스틱 — 실패 시 None (UI는 mtime fallback).
fn filename_to_rfc3339(name: &str) -> Option<String> {
    let inner = name
        .strip_prefix(CRASH_PREFIX)?
        .strip_suffix(CRASH_SUFFIX)?;
    // T 위치 찾기 — date / time 분리.
    let t_pos = inner.find('T')?;
    let (date, time_with_extras) = inner.split_at(t_pos);
    let time = &time_with_extras[1..];
    // 시간 부분: `08-12-34-123456789Z` → `08:12:34.123456789Z`.
    // 첫 두 `-`는 `:`로, 그 다음 `-`는 `.`로.
    let parts: Vec<&str> = time.splitn(4, '-').collect();
    if parts.len() < 3 {
        return None;
    }
    let h = parts[0];
    let m = parts[1];
    let s_with_z = parts[2];
    let frac_with_z = parts.get(3).copied().unwrap_or("");
    if frac_with_z.is_empty() {
        Some(format!("{date}T{h}:{m}:{s_with_z}"))
    } else {
        // `s_with_z`는 초만 있고 (`34`) 마지막은 `frac_with_z` (`123456789Z`).
        Some(format!("{date}T{h}:{m}:{s_with_z}.{frac_with_z}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_filename_accepts_normal_crash() {
        assert!(is_safe_crash_filename("crash-2026-04-30T08-12-34Z.txt"));
    }

    #[test]
    fn safe_filename_rejects_path_traversal() {
        assert!(!is_safe_crash_filename("crash-../etc/passwd.txt"));
        assert!(!is_safe_crash_filename("../crash-evil.txt"));
        assert!(!is_safe_crash_filename("crash-..\\foo.txt"));
    }

    #[test]
    fn safe_filename_rejects_separators() {
        assert!(!is_safe_crash_filename("crash-x/y.txt"));
        assert!(!is_safe_crash_filename("crash-x\\y.txt"));
    }

    #[test]
    fn safe_filename_rejects_wrong_prefix_or_suffix() {
        assert!(!is_safe_crash_filename("evil.txt"));
        assert!(!is_safe_crash_filename("crash-evil.exe"));
        assert!(!is_safe_crash_filename("crash-evil"));
    }

    #[test]
    fn safe_filename_rejects_null_byte() {
        assert!(!is_safe_crash_filename("crash-evil\0.txt"));
    }

    #[test]
    fn timestamp_extraction_round_trip() {
        let name = "crash-2026-04-30T08-12-34Z.txt";
        let rfc = filename_to_rfc3339(name).unwrap();
        assert_eq!(rfc, "2026-04-30T08:12:34Z");
    }

    #[test]
    fn timestamp_extraction_with_fractional() {
        let name = "crash-2026-04-30T08-12-34-123456789Z.txt";
        let rfc = filename_to_rfc3339(name).unwrap();
        assert_eq!(rfc, "2026-04-30T08:12:34.123456789Z");
    }

    #[test]
    fn timestamp_extraction_returns_none_on_garbage() {
        assert!(filename_to_rfc3339("crash-garbage.txt").is_none());
    }

    #[test]
    fn error_serialization_uses_kebab_kind() {
        let v = serde_json::to_value(CrashIpcError::NotInitialized).unwrap();
        assert_eq!(v["kind"], "not-initialized");
        let v = serde_json::to_value(CrashIpcError::InvalidFilename).unwrap();
        assert_eq!(v["kind"], "invalid-filename");
        let v = serde_json::to_value(CrashIpcError::NotFound).unwrap();
        assert_eq!(v["kind"], "not-found");
        let v = serde_json::to_value(CrashIpcError::TooLarge { bytes: 9999 }).unwrap();
        assert_eq!(v["kind"], "too-large");
        assert_eq!(v["bytes"], 9999);
    }
}
