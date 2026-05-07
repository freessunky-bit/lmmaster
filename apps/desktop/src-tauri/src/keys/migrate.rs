//! KeyManager 평문 → 암호화 마이그레이션 헬퍼 (Phase 8'.0.a, ADR-0035).
//!
//! Phase R-F+R-G hotfix (ADR-0064 §5) 재작성:
//! - 단일 경로 모델 (`keys.db`) — caller 인자 swap 회귀 차단.
//! - magic bytes(`SQLite format 3\0`)로 plaintext/encrypted 사전 분기.
//! - `.migrating` temp + atomic 2-phase rename (백업 → promote) + crash recovery.
//! - 자동 .bak 보존 (UTC timestamp suffix) — 사용자 데이터 손실 방지.
//!
//! 정책:
//! - OS 키체인(`keyring` crate)에서 32-byte secret을 hex로 보관.
//! - 첫 실행 시 새 secret 생성 → 키체인 저장 → 평문 DB 발견 시 in-place 마이그레이션.
//! - 키체인 미접근(예: Linux headless)이면 fallback: `KeyManager::open_unencrypted` + warn 기록.
//! - 마이그레이션은 atomic — `keys.db.migrating` (encrypted) 생성 후 `keys.db` →
//!   `keys.db.legacy.bak.{utc_ts}`로 백업, `migrating` → `keys.db`로 promote.
//!
//! 이 모듈은 Tauri AppHandle / dialog API 사용 X — pure logic. Tauri layer는 `lib.rs`에서.

use std::io::Read;
use std::path::{Path, PathBuf};

use keyring::Entry;
use rand::RngCore;
use time::OffsetDateTime;

/// keyring service / username — keyring crate가 OS 키체인 entry를 식별.
pub const KEYRING_SERVICE: &str = "lmmaster";
pub const KEYRING_USERNAME: &str = "keymanager-secret";

/// SQLite plaintext file의 magic bytes — 첫 16 byte. SQLCipher는 헤더가 random이라 미매치.
const SQLITE_MAGIC: &[u8; 16] = b"SQLite format 3\0";

/// 마이그레이션 결과 — 호출자가 분기 처리.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyStoreMode {
    /// 새 / 기존 암호화 DB로 정상 open. passphrase 포함.
    Encrypted { passphrase: String },
    /// 키체인 미접근 — 평문 폴백.
    UnencryptedFallback { reason: String },
}

#[derive(Debug)]
pub struct MigrationOutcome {
    pub mode: KeyStoreMode,
    /// 평문 DB가 있어 마이그레이션이 수행된 경우 true. 새 설치(평문 DB 없음)면 false.
    pub migrated_legacy: bool,
}

/// keys.db의 현재 상태 — magic bytes로 분류 (Phase R-F+R-G hotfix).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbState {
    /// 파일 자체가 없음 (또는 0 byte).
    NotExist,
    /// SQLite plaintext (`SQLite format 3\0` magic 매치).
    PlaintextSqlite,
    /// SQLCipher encrypted (header가 random — magic 미매치).
    EncryptedSqlcipher,
    /// 1~15 byte / partial write — 손상 의심.
    Corrupt,
}

/// 첫 16 byte를 read해 magic bytes로 plaintext/encrypted 분류.
///
/// # 함정
/// - 0 byte 파일은 `NotExist` 반환 (caller가 신규 생성으로 진입).
/// - 16 byte 미만은 `Corrupt` — partial write / antivirus 격리 흔적.
/// - read 실패는 caller가 `Err`로 받음 — 권한 문제 등.
pub fn detect_db_state(path: &Path) -> std::io::Result<DbState> {
    if !path.exists() {
        return Ok(DbState::NotExist);
    }
    let mut f = std::fs::File::open(path)?;
    let mut buf = [0u8; 16];
    let n = f.read(&mut buf)?;
    Ok(match n {
        0 => DbState::NotExist,
        1..=15 => DbState::Corrupt,
        _ if &buf == SQLITE_MAGIC => DbState::PlaintextSqlite,
        _ => DbState::EncryptedSqlcipher,
    })
}

/// KeyManager 부팅 시 호출하는 entry-point — 단일 경로 모델 (Phase R-F+R-G hotfix).
///
/// 절차:
/// 1. `.migrating` 잔재 정리 (또는 orphan promote 회복).
/// 2. keyring에서 secret 읽기 시도 (없으면 새로 생성).
/// 3. `detect_db_state`로 plaintext/encrypted 분기:
///    - `NotExist` → 첫 KeyManager::open이 새 암호화 DB 생성 (caller 책임).
///    - `EncryptedSqlcipher` → 그대로 사용.
///    - `PlaintextSqlite` + `sqlcipher` feature ON → atomic in-place migration.
///    - `PlaintextSqlite` + feature OFF → dev/test 빌드라 그대로 사용.
///    - `Corrupt` → fatal — UnencryptedFallback로 강등.
/// 4. keyring 자체 접근 실패 시 fallback (UnencryptedFallback).
///
/// 호출자는 결과 mode에 따라 `KeyManager::open` / `open_unencrypted`를 선택.
pub fn provision_v2(keys_path: &Path) -> MigrationOutcome {
    let migrating = with_suffix(keys_path, "migrating");

    // Step 0: .migrating 잔재 정리 + orphan 회복.
    if migrating.exists() {
        if !keys_path.exists() {
            // Phase C 1단계 (백업 rename) 완료 후 죽은 시뮬레이션 — promote 회복.
            match std::fs::rename(&migrating, keys_path) {
                Ok(()) => tracing::info!(
                    migrating = %migrating.display(),
                    "크래시 복구: .migrating → keys.db promote 완료"
                ),
                Err(e) => tracing::error!(
                    error = %e,
                    "promote 회복 실패 — 사용자 수동 개입 필요"
                ),
            }
        } else if let Err(e) = std::fs::remove_file(&migrating) {
            tracing::warn!(error = %e, "stale .migrating 제거 실패");
        }
    }

    // Step 1: keyring 접근.
    let entry = match Entry::new(KEYRING_SERVICE, KEYRING_USERNAME) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(error = %e, "keyring entry 생성 실패 — 평문 폴백");
            return fallback(format!("keyring entry 실패: {e}"));
        }
    };
    let passphrase = match read_or_create_secret(&entry) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "keyring secret 읽기/생성 실패 — 평문 폴백");
            return fallback(format!("keyring secret: {e}"));
        }
    };

    // Step 2: state 감지 후 분기.
    let state = match detect_db_state(keys_path) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "detect_db_state 실패 — 평문 폴백");
            return fallback(format!("detect_db_state: {e}"));
        }
    };

    // sqlcipher feature OFF 빌드에서는 migrated_legacy가 변경되지 않으므로 unused_mut allow.
    #[allow(unused_mut)]
    let mut migrated_legacy = false;
    match state {
        DbState::NotExist | DbState::EncryptedSqlcipher => {
            // 신규 또는 이미 암호화 — KeyManager::open이 그대로 사용.
        }
        DbState::PlaintextSqlite => {
            // sqlcipher feature ON일 때만 마이그레이션. OFF (dev/test) 빌드는 평문 그대로 사용.
            #[cfg(feature = "sqlcipher")]
            {
                match migrate_inplace(keys_path, &migrating, &passphrase) {
                    Ok(()) => {
                        migrated_legacy = true;
                        tracing::info!("키 저장소를 암호화 형식으로 옮겼어요");
                    }
                    Err(e) => {
                        tracing::error!(
                            error = %e,
                            "마이그레이션 실패 — keys.db 원본 보존, 사용자 안내 필요"
                        );
                        // 원본 keys.db 보존 — 절대 덮어쓰지 않음. caller가 open 시도 시 fail해
                        // 메모리 폴백으로 자연 분기.
                    }
                }
            }
            #[cfg(not(feature = "sqlcipher"))]
            {
                tracing::info!("dev 빌드 (sqlcipher feature OFF): 평문 keys.db 그대로 사용");
            }
        }
        DbState::Corrupt => {
            tracing::error!("keys.db가 손상됐어요 (1~15 byte) — 평문 폴백");
            return fallback("keys.db 손상".into());
        }
    }

    MigrationOutcome {
        mode: KeyStoreMode::Encrypted { passphrase },
        migrated_legacy,
    }
}

/// 평문 → 암호화 atomic in-place migration (sqlcipher feature 활성 시).
///
/// Phase A: `keys.db` (plaintext) → `keys.db.migrating` (encrypted) export.
/// Phase B: 부모 디렉터리 fsync (UNIX) — Windows는 MoveFileEx가 metadata flush.
/// Phase C: 두-단계 rename — `keys.db` → `.legacy.bak.{utc_ts}` (백업), `.migrating` → `keys.db` (promote).
#[cfg(feature = "sqlcipher")]
fn migrate_inplace(
    keys_path: &Path,
    migrating: &Path,
    passphrase: &str,
) -> Result<(), key_manager::StoreError> {
    // Phase A: encrypted export.
    key_manager::KeyStore::migrate_unencrypted_to_encrypted(keys_path, migrating, passphrase)?;
    // Phase B: 부모 fsync.
    if let Some(parent) = keys_path.parent() {
        sync_dir(parent);
    }
    // Phase C: 백업 rename → promote rename.
    let bak = backup_path_ts(keys_path);
    std::fs::rename(keys_path, &bak)
        .map_err(|e| key_manager::StoreError::MigrationFailed(format!("백업 rename 실패: {e}")))?;
    std::fs::rename(migrating, keys_path).map_err(|e| {
        key_manager::StoreError::MigrationFailed(format!("promote rename 실패: {e}"))
    })?;
    if let Some(parent) = keys_path.parent() {
        sync_dir(parent);
    }
    Ok(())
}

/// fallback helper — keyring 접근 실패 / DB 손상 시 평문 폴백 결과 생성.
fn fallback(reason: String) -> MigrationOutcome {
    MigrationOutcome {
        mode: KeyStoreMode::UnencryptedFallback { reason },
        migrated_legacy: false,
    }
}

/// 부모 디렉터리 fsync — UNIX 한정. Windows는 MoveFileEx가 atomic + metadata flush.
/// sqlcipher feature 활성 빌드에서만 호출돼요 (migrate_inplace 종속).
#[cfg(feature = "sqlcipher")]
fn sync_dir(_dir: &Path) {
    #[cfg(unix)]
    if let Ok(f) = std::fs::File::open(_dir) {
        let _ = f.sync_all();
    }
}

/// keyring entry에서 secret을 읽어오거나 새로 생성한다.
fn read_or_create_secret(entry: &Entry) -> Result<String, keyring::Error> {
    match entry.get_password() {
        Ok(p) if !p.is_empty() => Ok(p),
        Ok(_) | Err(keyring::Error::NoEntry) => {
            // 새 secret 생성 — 32 byte random → hex.
            let mut buf = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut buf);
            let hex_secret = hex::encode(buf);
            entry.set_password(&hex_secret)?;
            Ok(hex_secret)
        }
        Err(e) => Err(e),
    }
}

/// `path`에 suffix를 추가한 PathBuf (예: `keys.db` + `migrating` → `keys.db.migrating`).
pub fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".");
    s.push(suffix);
    PathBuf::from(s)
}

/// UTC 타임스탬프 suffix가 붙은 백업 경로 (`keys.db.legacy.bak.20260508T091230Z`).
pub fn backup_path_ts(plain_path: &Path) -> PathBuf {
    let ts = OffsetDateTime::now_utc()
        .format(
            &time::format_description::parse("[year][month][day]T[hour][minute][second]Z")
                .expect("static format description"),
        )
        .unwrap_or_else(|_| "unknown".into());
    let mut s = plain_path.as_os_str().to_owned();
    s.push(format!(".legacy.bak.{ts}"));
    PathBuf::from(s)
}

/// (Deprecated) 평문 DB의 백업 경로 — `path` + `.legacy.bak`.
/// Phase R-F+R-G hotfix에서 `backup_path_ts`로 대체. backward-compat 보존만.
#[allow(dead_code)]
pub fn backup_path(plain_path: &Path) -> PathBuf {
    let mut s = plain_path.as_os_str().to_owned();
    s.push(".legacy.bak");
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn backup_path_appends_legacy_suffix() {
        let p = Path::new("/tmp/keys.db");
        let bak = backup_path(p);
        assert_eq!(bak.to_string_lossy(), "/tmp/keys.db.legacy.bak");
    }

    #[test]
    fn keystore_mode_variants_distinguishable() {
        let a = KeyStoreMode::Encrypted {
            passphrase: "x".into(),
        };
        let b = KeyStoreMode::UnencryptedFallback { reason: "y".into() };
        assert_ne!(a, b);
    }

    // ── Phase R-F+R-G hotfix (ADR-0064 §5) — magic bytes detection + helpers ───

    #[test]
    fn with_suffix_appends_dot_suffix() {
        let p = Path::new("/tmp/keys.db");
        let migrating = with_suffix(p, "migrating");
        assert_eq!(migrating.to_string_lossy(), "/tmp/keys.db.migrating");
    }

    #[test]
    fn backup_path_ts_includes_timestamp_or_unknown() {
        let p = Path::new("/tmp/keys.db");
        let bak = backup_path_ts(p);
        let s = bak.to_string_lossy();
        assert!(s.contains(".legacy.bak."));
        // YYYYMMDDTHHMMSSZ 또는 unknown.
        assert!(s.ends_with('Z') || s.ends_with("unknown"));
    }

    #[test]
    fn detects_not_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("missing.db");
        assert_eq!(detect_db_state(&path).unwrap(), DbState::NotExist);
    }

    #[test]
    fn detects_empty_as_not_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("empty.db");
        std::fs::write(&path, b"").unwrap();
        assert_eq!(detect_db_state(&path).unwrap(), DbState::NotExist);
    }

    #[test]
    fn detects_corrupt_partial() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("partial.db");
        std::fs::write(&path, b"SQLite f").unwrap(); // 8 byte
        assert_eq!(detect_db_state(&path).unwrap(), DbState::Corrupt);
    }

    #[test]
    fn detects_plaintext_via_magic_bytes() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("plain.db");
        // SQLite plaintext file 시뮬레이션 — magic bytes만 있어도 분류 충분.
        let mut data = Vec::with_capacity(100);
        data.extend_from_slice(b"SQLite format 3\0");
        data.extend_from_slice(&[0u8; 84]);
        std::fs::write(&path, &data).unwrap();
        assert_eq!(detect_db_state(&path).unwrap(), DbState::PlaintextSqlite);
    }

    #[test]
    fn detects_encrypted_when_no_magic() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("enc.db");
        // SQLCipher header는 random — magic 미매치.
        let data = vec![0xa5u8; 100];
        std::fs::write(&path, &data).unwrap();
        assert_eq!(detect_db_state(&path).unwrap(), DbState::EncryptedSqlcipher);
    }

    /// 실 SQLite plaintext DB를 `KeyStore::open_unencrypted`로 만들어 detect 통과 검증.
    #[test]
    fn detects_real_sqlite_plaintext_db() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("real.db");
        let _ = key_manager::KeyStore::open_unencrypted(&path).unwrap();
        assert_eq!(detect_db_state(&path).unwrap(), DbState::PlaintextSqlite);
    }
}
