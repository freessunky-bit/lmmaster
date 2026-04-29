//! SQLite-backed ApiKey 저장소 (ADR-0022 §7, ADR-0035).
//!
//! 정책:
//! - rusqlite `bundled-sqlcipher-vendored-openssl` — SQLCipher 벤더 통합 (Phase 8'.0.a, ADR-0035).
//!   `sqlcipher` feature 켜야 활성. dev 빌드에서 끄면 `PRAGMA key`는 stock SQLite에서 unknown
//!   pragma로 무해하게 무시되고, sqlcipher_export 호출은 마이그레이션 시 에러로 표면화.
//! - 스키마: api_keys (id PRIMARY KEY, alias, key_prefix INDEX, key_hash, scope_json, created_at, last_used_at NULL, revoked_at NULL).
//! - prefix lookup (인덱스) → argon2 verify로 narrow.
//! - WAL 모드 + busy_timeout=5000 + synchronous=NORMAL — Phase 8'.0.b 안정성.
//! - 평문 → 암호화 마이그레이션은 `migrate_unencrypted_to_encrypted` 헬퍼 (Phase 8'.0.a).

use std::path::Path;

use rusqlite::{params, Connection};
use thiserror::Error;
use time::OffsetDateTime;

use crate::scope::Scope;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("time format: {0}")]
    Time(#[from] time::error::Format),
    #[error("키를 찾을 수 없어요 (id={0})")]
    NotFound(String),
    #[error("키 저장소 패스프레이즈가 비어 있어요")]
    EmptyPassphrase,
    #[error("키 저장소 마이그레이션에 실패했어요: {0}")]
    MigrationFailed(String),
}

/// ApiKey row — 평문 키는 포함하지 않음 (1회 reveal 후 폐기).
#[derive(Debug, Clone)]
pub struct ApiKeyRow {
    pub id: String,
    pub alias: String,
    pub key_prefix: String,
    pub key_hash: String,
    pub scope: Scope,
    pub created_at: OffsetDateTime,
    pub last_used_at: Option<OffsetDateTime>,
    pub revoked_at: Option<OffsetDateTime>,
}

pub struct KeyStore {
    conn: Connection,
}

impl KeyStore {
    /// SQLCipher 키로 암호화된 DB를 연다 (또는 새로 만든다).
    ///
    /// `passphrase`는 호출자가 OS 키체인에서 가져온 32바이트 hex 문자열을 권장.
    /// 빈 문자열이면 `EmptyPassphrase`. 잘못된 passphrase로 기존 DB 열면 `Sqlite` 에러.
    pub fn open(path: &Path, passphrase: &str) -> Result<Self, StoreError> {
        if passphrase.is_empty() {
            return Err(StoreError::EmptyPassphrase);
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(path)?;
        // 정확한 순서: PRAGMA key 먼저 (모든 read/write 전), 그 다음 stability PRAGMA.
        apply_passphrase(&conn, passphrase)?;
        apply_stability_pragmas(&conn)?;
        Self::init_schema(&conn)?;
        Ok(Self { conn })
    }

    /// 암호화 없이 평문 DB를 연다 (테스트 / Linux headless 폴백 용).
    ///
    /// CLAUDE.md §6 안전 가드: 정상 데스크톱 경로는 항상 `open(...)` 사용.
    pub fn open_unencrypted(path: &Path) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(path)?;
        apply_stability_pragmas(&conn)?;
        Self::init_schema(&conn)?;
        Ok(Self { conn })
    }

    /// 메모리 DB — 테스트용. SQLCipher 미적용 (in-memory에 의미 없음).
    pub fn open_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        // in-memory에 WAL은 의미 없지만 busy_timeout만 일관되게 설정.
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        Self::init_schema(&conn)?;
        Ok(Self { conn })
    }

    /// 평문 DB → 암호화 DB 마이그레이션.
    ///
    /// 절차 (rusqlite + sqlcipher 표준):
    /// 1. 평문 DB(`plain_path`)를 attach.
    /// 2. 새 암호화 DB(`encrypted_path`)에 PRAGMA key 적용.
    /// 3. `sqlcipher_export('encrypted')` 호출 — 모든 테이블 + 인덱스 + 데이터 복제.
    /// 4. detach.
    ///
    /// 호출 후 plain_path는 caller가 결정에 따라 삭제. 본 함수는 원본 보존.
    pub fn migrate_unencrypted_to_encrypted(
        plain_path: &Path,
        encrypted_path: &Path,
        passphrase: &str,
    ) -> Result<(), StoreError> {
        if passphrase.is_empty() {
            return Err(StoreError::EmptyPassphrase);
        }
        if !plain_path.exists() {
            return Err(StoreError::MigrationFailed(format!(
                "원본 평문 DB가 없어요: {}",
                plain_path.display()
            )));
        }
        if let Some(parent) = encrypted_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                StoreError::MigrationFailed(format!("대상 디렉터리 생성 실패: {e}"))
            })?;
        }
        // 새 암호화 DB로 시작 (없으면 생성).
        let conn = Connection::open(encrypted_path)?;
        apply_passphrase(&conn, passphrase)?;
        apply_stability_pragmas(&conn)?;

        // ATTACH plaintext.
        conn.execute(
            &format!(
                "ATTACH DATABASE '{}' AS plaintext KEY ''",
                plain_path.display()
            ),
            [],
        )
        .map_err(|e| StoreError::MigrationFailed(format!("평문 DB attach 실패: {e}")))?;

        // sqlcipher_export — `from` 인자가 평문 DB alias("plaintext").
        conn.query_row("SELECT sqlcipher_export('main', 'plaintext')", [], |_| {
            Ok(())
        })
        .map_err(|e| StoreError::MigrationFailed(format!("암호화 export 실패: {e}")))?;

        conn.execute("DETACH DATABASE plaintext", [])
            .map_err(|e| StoreError::MigrationFailed(format!("detach 실패: {e}")))?;
        Ok(())
    }

    fn init_schema(conn: &Connection) -> Result<(), StoreError> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS api_keys (
                id            TEXT PRIMARY KEY,
                alias         TEXT NOT NULL,
                key_prefix    TEXT NOT NULL,
                key_hash      TEXT NOT NULL,
                scope_json    TEXT NOT NULL,
                created_at    TEXT NOT NULL,
                last_used_at  TEXT,
                revoked_at    TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_api_keys_prefix ON api_keys(key_prefix);
            CREATE INDEX IF NOT EXISTS idx_api_keys_revoked ON api_keys(revoked_at);
            "#,
        )?;
        Ok(())
    }

    pub fn insert(&self, row: &ApiKeyRow) -> Result<(), StoreError> {
        let scope_json = serde_json::to_string(&row.scope)?;
        let created_at = format_dt(&row.created_at)?;
        let last_used_at = row.last_used_at.as_ref().map(format_dt).transpose()?;
        let revoked_at = row.revoked_at.as_ref().map(format_dt).transpose()?;
        self.conn.execute(
            r#"INSERT INTO api_keys (id, alias, key_prefix, key_hash, scope_json,
                created_at, last_used_at, revoked_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
            params![
                row.id,
                row.alias,
                row.key_prefix,
                row.key_hash,
                scope_json,
                created_at,
                last_used_at,
                revoked_at,
            ],
        )?;
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<ApiKeyRow>, StoreError> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, alias, key_prefix, key_hash, scope_json,
               created_at, last_used_at, revoked_at FROM api_keys
               ORDER BY created_at DESC"#,
        )?;
        let rows = stmt
            .query_map([], row_to_api_key)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn find_by_prefix(&self, prefix: &str) -> Result<Vec<ApiKeyRow>, StoreError> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, alias, key_prefix, key_hash, scope_json,
               created_at, last_used_at, revoked_at FROM api_keys
               WHERE key_prefix = ?1 AND revoked_at IS NULL"#,
        )?;
        let rows = stmt
            .query_map([prefix], row_to_api_key)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn find_by_id(&self, id: &str) -> Result<Option<ApiKeyRow>, StoreError> {
        let mut stmt = self.conn.prepare(
            r#"SELECT id, alias, key_prefix, key_hash, scope_json,
               created_at, last_used_at, revoked_at FROM api_keys WHERE id = ?1"#,
        )?;
        let mut rows = stmt.query_map([id], row_to_api_key)?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    pub fn touch_last_used(&self, id: &str, at: OffsetDateTime) -> Result<(), StoreError> {
        let s = format_dt(&at)?;
        self.conn.execute(
            "UPDATE api_keys SET last_used_at = ?1 WHERE id = ?2",
            params![s, id],
        )?;
        Ok(())
    }

    /// 회수 — idempotent. 이미 revoked면 no-op (revoked_at 유지).
    pub fn revoke(&self, id: &str, at: OffsetDateTime) -> Result<(), StoreError> {
        let s = format_dt(&at)?;
        let n = self.conn.execute(
            "UPDATE api_keys SET revoked_at = ?1 WHERE id = ?2 AND revoked_at IS NULL",
            params![s, id],
        )?;
        if n == 0 {
            // 행 자체가 없을 수도, 이미 revoked일 수도 — 후자는 no-op.
            if self.find_by_id(id)?.is_none() {
                return Err(StoreError::NotFound(id.to_string()));
            }
        }
        Ok(())
    }

    /// 현재 connection의 `journal_mode` PRAGMA 값을 반환 — 검증용.
    /// in-memory DB는 `memory`, file DB는 WAL 활성 시 `wal`.
    pub fn journal_mode(&self) -> Result<String, StoreError> {
        let mode: String = self
            .conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))?;
        Ok(mode)
    }
}

/// SQLCipher PRAGMA key 적용 — 모든 read/write보다 먼저.
///
/// `sqlcipher` feature OFF 빌드에서는 stock SQLite가 `PRAGMA key`를 unknown pragma로 무시.
/// 즉 빈 DB / 평문 DB는 정상 작동, 잘못된 키 검증은 OFF 모드에서 스킵 (production은 항상 ON).
fn apply_passphrase(conn: &Connection, passphrase: &str) -> Result<(), StoreError> {
    // SQL injection 방지 — `'` 이스케이프. passphrase는 caller가 hex/random이라 보통 안전하지만
    // 엄격히 처리.
    let escaped = passphrase.replace('\'', "''");
    conn.execute_batch(&format!("PRAGMA key = '{escaped}'"))?;
    // 키가 맞는지 즉시 검증 — sqlite_master 조회는 잘못된 키면 NotADatabase로 실패.
    // 새 DB에서는 빈 master가 정상 반환되니 항상 OK.
    // Stock SQLite (feature off)에서는 PRAGMA key 자체가 no-op — 평문 master 그대로 반환.
    let _: i64 = conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0))?;
    Ok(())
}

/// WAL + busy_timeout + synchronous=NORMAL — 안정성 PRAGMA.
fn apply_stability_pragmas(conn: &Connection) -> Result<(), StoreError> {
    // WAL은 단일 statement로 적용해야 PRAGMA reply 처리 가능.
    // execute_batch는 result row를 무시하므로 query_row로 명시.
    let _: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
    conn.busy_timeout(std::time::Duration::from_millis(5000))?;
    conn.execute_batch("PRAGMA synchronous = NORMAL;")?;
    Ok(())
}

fn format_dt(at: &OffsetDateTime) -> Result<String, time::error::Format> {
    at.format(&time::format_description::well_known::Rfc3339)
}

fn parse_dt(s: &str) -> Result<OffsetDateTime, rusqlite::Error> {
    OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}

fn row_to_api_key(row: &rusqlite::Row<'_>) -> rusqlite::Result<ApiKeyRow> {
    let scope_json: String = row.get(4)?;
    let scope: Scope = serde_json::from_str(&scope_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let created_at: String = row.get(5)?;
    let last_used_at: Option<String> = row.get(6)?;
    let revoked_at: Option<String> = row.get(7)?;
    Ok(ApiKeyRow {
        id: row.get(0)?,
        alias: row.get(1)?,
        key_prefix: row.get(2)?,
        key_hash: row.get(3)?,
        scope,
        created_at: parse_dt(&created_at)?,
        last_used_at: last_used_at.as_deref().map(parse_dt).transpose()?,
        revoked_at: revoked_at.as_deref().map(parse_dt).transpose()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::Scope;

    fn make_row(id: &str, prefix: &str, alias: &str) -> ApiKeyRow {
        ApiKeyRow {
            id: id.into(),
            alias: alias.into(),
            key_prefix: prefix.into(),
            key_hash: "$argon2id$dummy".into(),
            scope: Scope {
                models: vec!["*".into()],
                endpoints: vec!["/v1/*".into()],
                allowed_origins: vec!["http://localhost:5173".into()],
                ..Default::default()
            },
            created_at: OffsetDateTime::now_utc(),
            last_used_at: None,
            revoked_at: None,
        }
    }

    #[test]
    fn insert_and_list() {
        let s = KeyStore::open_memory().unwrap();
        s.insert(&make_row("a", "lm-aaaa1111", "first")).unwrap();
        s.insert(&make_row("b", "lm-bbbb2222", "second")).unwrap();
        let rows = s.list().unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn find_by_prefix_excludes_revoked() {
        let s = KeyStore::open_memory().unwrap();
        s.insert(&make_row("a", "lm-aaaa1111", "first")).unwrap();
        s.insert(&make_row("b", "lm-aaaa1111", "duplicate-prefix")) // 충돌 가능 — argon2가 narrow.
            .unwrap();
        let found = s.find_by_prefix("lm-aaaa1111").unwrap();
        assert_eq!(found.len(), 2);

        s.revoke("a", OffsetDateTime::now_utc()).unwrap();
        let found = s.find_by_prefix("lm-aaaa1111").unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "b");
    }

    #[test]
    fn find_by_id_returns_revoked_too() {
        let s = KeyStore::open_memory().unwrap();
        s.insert(&make_row("a", "lm-aaaa1111", "first")).unwrap();
        s.revoke("a", OffsetDateTime::now_utc()).unwrap();
        let row = s.find_by_id("a").unwrap().unwrap();
        assert!(row.revoked_at.is_some());
    }

    #[test]
    fn touch_last_used_updates() {
        let s = KeyStore::open_memory().unwrap();
        s.insert(&make_row("a", "lm-x", "x")).unwrap();
        let now = OffsetDateTime::now_utc();
        s.touch_last_used("a", now).unwrap();
        let r = s.find_by_id("a").unwrap().unwrap();
        assert!(r.last_used_at.is_some());
    }

    #[test]
    fn revoke_unknown_returns_not_found() {
        let s = KeyStore::open_memory().unwrap();
        let r = s.revoke("missing", OffsetDateTime::now_utc());
        assert!(matches!(r, Err(StoreError::NotFound(_))));
    }

    #[test]
    fn revoke_idempotent_for_already_revoked() {
        let s = KeyStore::open_memory().unwrap();
        s.insert(&make_row("a", "lm-x", "x")).unwrap();
        let now = OffsetDateTime::now_utc();
        s.revoke("a", now).unwrap();
        // 두 번째 revoke도 OK (no-op).
        s.revoke("a", now).unwrap();
    }

    #[test]
    fn scope_round_trips_through_json() {
        let s = KeyStore::open_memory().unwrap();
        let mut row = make_row("a", "lm-x", "x");
        row.scope.models = vec!["exaone-*".into(), "qwen-*".into()];
        row.scope.allowed_origins = vec!["https://my-app.com".into()];
        s.insert(&row).unwrap();
        let r = s.find_by_id("a").unwrap().unwrap();
        assert_eq!(r.scope.models, vec!["exaone-*", "qwen-*"]);
        assert_eq!(r.scope.allowed_origins, vec!["https://my-app.com"]);
    }

    // ── Phase 8'.0.a / 8'.0.b 테스트 ────────────────────────────────

    #[test]
    fn open_with_passphrase_creates_encrypted_db() {
        // 임시 파일에 암호화 DB 생성 후 row 1 insert → list로 검증.
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("keys.db");
        let s = KeyStore::open(&path, "test-pass-32-bytes-hex-aaaaaaaa").unwrap();
        s.insert(&make_row("a", "lm-x", "first")).unwrap();
        assert_eq!(s.list().unwrap().len(), 1);
        // 파일이 존재하고 0 byte 이상.
        let meta = std::fs::metadata(&path).unwrap();
        assert!(meta.len() > 0);
    }

    #[test]
    fn open_with_empty_passphrase_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("keys.db");
        let r = KeyStore::open(&path, "");
        assert!(matches!(r, Err(StoreError::EmptyPassphrase)));
    }

    /// SQLCipher 활성 빌드에서만 — stock SQLite는 `PRAGMA key`를 무시해 잘못된 passphrase여도 통과.
    #[cfg(feature = "sqlcipher")]
    #[test]
    fn open_wrong_passphrase_fails_on_existing_db() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("keys.db");
        // 1) 첫 open — passphrase A.
        {
            let s = KeyStore::open(&path, "passphrase-aaaaaaaaaaaaaaaaa").unwrap();
            s.insert(&make_row("a", "lm-x", "x")).unwrap();
        }
        // 2) 다른 passphrase로 다시 열기 — sqlite 에러.
        let r = KeyStore::open(&path, "passphrase-different-bbbbbbbbb");
        assert!(
            matches!(r, Err(StoreError::Sqlite(_))),
            "잘못된 passphrase면 Sqlite 에러여야 해요"
        );
    }

    #[test]
    fn open_correct_passphrase_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("keys.db");
        let pass = "correct-horse-battery-staple-1234";
        {
            let s = KeyStore::open(&path, pass).unwrap();
            s.insert(&make_row("a", "lm-x", "alias")).unwrap();
        }
        // 동일 passphrase → row 그대로.
        let s = KeyStore::open(&path, pass).unwrap();
        let rows = s.list().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].alias, "alias");
    }

    #[test]
    fn open_unencrypted_works_for_legacy_dbs() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("plain.db");
        {
            let s = KeyStore::open_unencrypted(&path).unwrap();
            s.insert(&make_row("a", "lm-x", "alias")).unwrap();
        }
        let s = KeyStore::open_unencrypted(&path).unwrap();
        assert_eq!(s.list().unwrap().len(), 1);
    }

    /// `sqlcipher_export()`는 SQLCipher feature 빌드에서만 작동.
    #[cfg(feature = "sqlcipher")]
    #[test]
    fn migrate_unencrypted_to_encrypted_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let plain_path = tmp.path().join("plain.db");
        let enc_path = tmp.path().join("enc.db");

        // 1) 평문 DB에 row 2 작성.
        {
            let s = KeyStore::open_unencrypted(&plain_path).unwrap();
            s.insert(&make_row("a", "lm-aa", "first")).unwrap();
            s.insert(&make_row("b", "lm-bb", "second")).unwrap();
        }
        // 2) 마이그레이션.
        let pass = "migration-test-passphrase-aaaaaa";
        KeyStore::migrate_unencrypted_to_encrypted(&plain_path, &enc_path, pass).unwrap();
        // 3) 암호화 DB로 열어서 검증.
        let s = KeyStore::open(&enc_path, pass).unwrap();
        let rows = s.list().unwrap();
        assert_eq!(rows.len(), 2);
        let aliases: Vec<_> = rows.iter().map(|r| r.alias.clone()).collect();
        assert!(aliases.contains(&"first".to_string()));
        assert!(aliases.contains(&"second".to_string()));
    }

    #[test]
    fn migrate_with_missing_source_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let plain_path = tmp.path().join("missing.db");
        let enc_path = tmp.path().join("enc.db");
        let r = KeyStore::migrate_unencrypted_to_encrypted(
            &plain_path,
            &enc_path,
            "any-passphrase-here-aaaaaaaaaa",
        );
        assert!(matches!(r, Err(StoreError::MigrationFailed(_))));
    }

    #[test]
    fn migrate_empty_passphrase_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let plain_path = tmp.path().join("plain.db");
        let enc_path = tmp.path().join("enc.db");
        let _ = KeyStore::open_unencrypted(&plain_path).unwrap();
        let r = KeyStore::migrate_unencrypted_to_encrypted(&plain_path, &enc_path, "");
        assert!(matches!(r, Err(StoreError::EmptyPassphrase)));
    }

    #[test]
    fn open_file_uses_wal_journal() {
        // file-backed DB는 WAL 모드 활성.
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("wal.db");
        let s = KeyStore::open(&path, "wal-test-passphrase-aaaaaaaaaaaa").unwrap();
        let mode = s.journal_mode().unwrap();
        assert_eq!(
            mode.to_lowercase(),
            "wal",
            "file-backed DB는 WAL 모드여야 해요"
        );
    }

    #[test]
    fn open_unencrypted_uses_wal_journal() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("wal-plain.db");
        let s = KeyStore::open_unencrypted(&path).unwrap();
        let mode = s.journal_mode().unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
    }
}
