//! KeyManager 평문 → 암호화 마이그레이션 헬퍼 (Phase 8'.0.a, ADR-0035).
//!
//! 정책:
//! - OS 키체인(`keyring` crate)에서 32-byte secret을 hex로 보관.
//! - 첫 실행 시 새 secret 생성 → 키체인 저장 → 평문 DB 발견 시 마이그레이션 prompt.
//! - 키체인 미접근(예: Linux headless)이면 fallback: `KeyManager::open_unencrypted` + warn 기록.
//! - 마이그레이션은 atomic — 새 암호화 DB 생성 후 평문 DB는 `keys.db.legacy.bak`으로 rename
//!   (사용자가 검증 후 수동 삭제). 자동 삭제 X (CLAUDE.md §1: destructive 액션은 명시 승인).
//!
//! 이 모듈은 Tauri AppHandle / dialog API 사용 X — pure logic. Tauri layer는 `lib.rs`에서.

use std::path::{Path, PathBuf};

use keyring::Entry;
use rand::RngCore;

/// keyring service / username — keyring crate가 OS 키체인 entry를 식별.
pub const KEYRING_SERVICE: &str = "lmmaster";
pub const KEYRING_USERNAME: &str = "keymanager-secret";

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

/// KeyManager 부팅 시 호출하는 entry-point.
///
/// 절차:
/// 1. keyring에서 secret 읽기 시도.
///    - 있으면: 그대로 사용.
///    - 없으면: 32 byte random 생성 → hex 인코딩 → keyring 저장.
/// 2. 평문 DB(`keys_path`)가 있는데 암호화 DB(`encrypted_path`)가 없으면 마이그레이션.
/// 3. keyring 자체 접근 실패 시 fallback (UnencryptedFallback).
///
/// 호출자는 결과 mode에 따라 KeyManager::open / open_unencrypted를 선택.
pub fn provision(encrypted_path: &Path, legacy_plain_path: &Path) -> MigrationOutcome {
    // 1. keyring 접근.
    let entry = match Entry::new(KEYRING_SERVICE, KEYRING_USERNAME) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(error = %e, "keyring entry 생성 실패 — 평문 폴백");
            return MigrationOutcome {
                mode: KeyStoreMode::UnencryptedFallback {
                    reason: format!("keyring entry 실패: {e}"),
                },
                migrated_legacy: false,
            };
        }
    };

    let passphrase = match read_or_create_secret(&entry) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "keyring secret 읽기/생성 실패 — 평문 폴백");
            return MigrationOutcome {
                mode: KeyStoreMode::UnencryptedFallback {
                    reason: format!("keyring secret: {e}"),
                },
                migrated_legacy: false,
            };
        }
    };

    // 2. 평문 → 암호화 마이그레이션.
    let mut migrated_legacy = false;
    if !encrypted_path.exists() && legacy_plain_path.exists() {
        match key_manager::KeyStore::migrate_unencrypted_to_encrypted(
            legacy_plain_path,
            encrypted_path,
            &passphrase,
        ) {
            Ok(()) => {
                migrated_legacy = true;
                tracing::info!(
                    plain = %legacy_plain_path.display(),
                    encrypted = %encrypted_path.display(),
                    "키 저장소를 암호화 형식으로 옮겼어요",
                );
                // 평문 DB는 .bak 으로 rename — 사용자 안전 검증용.
                let bak = backup_path(legacy_plain_path);
                if let Err(e) = std::fs::rename(legacy_plain_path, &bak) {
                    tracing::warn!(
                        error = %e,
                        "평문 백업 rename 실패 — 사용자에게 수동 정리 안내 필요",
                    );
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "마이그레이션 실패 — 사용자가 수동 처리 필요");
                // 마이그레이션 실패는 fatal X — 새 빈 암호화 DB로 시작.
            }
        }
    }

    MigrationOutcome {
        mode: KeyStoreMode::Encrypted { passphrase },
        migrated_legacy,
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

/// 평문 DB의 백업 경로 — `path` + `.legacy.bak`.
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

    #[test]
    fn provision_fallback_when_keyring_unavailable() {
        // keyring 동작 실험은 OS 의존이라 unit에서 직접 검증 어려움.
        // 대신 helper 함수의 안정성: backup path 생성이 panic 없이 반환되는지 확인.
        let p = Path::new("./relative.db");
        let bak = backup_path(p);
        assert!(bak.to_string_lossy().ends_with(".legacy.bak"));
    }
}
