//! SQLite 기반 manifest 캐시 — `tokio::task::spawn_blocking`으로 wrap.
//!
//! 정책 (Phase 1' 결정 §3):
//! - rusqlite + WAL + WITHOUT ROWID 컴포지트 PK.
//! - body_sha256 무결성 검증 — 손상 row는 read 시 자동 drop + Err(CacheCorrupt).
//! - schema_meta('version','1') 미래 마이그레이션용.

use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

use crate::error::FetcherError;
use crate::source::SourceTier;

/// Phase R-B (ADR-0054) — 스키마 v2: `signature_verified` 컬럼 추가.
const SCHEMA_VERSION_MAX: u32 = 2;

/// 캐시 row — `get`이 반환.
#[derive(Debug, Clone)]
pub struct CacheRow {
    pub source: SourceTier,
    pub manifest_id: String,
    pub url: String,
    pub body: Vec<u8>,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub fetched_at: SystemTime,
    /// Phase R-B (ADR-0054) — 본 row를 적재할 때 minisign 서명이 *검증됐는지*.
    /// `false`면: 검증되지 않은 채 캐시됐음 (verifier 미설정 빌드 또는 stock SQLite).
    /// caller(`fetch_one_with_signature`)는 verifier 활성 시 `false` row를 invalidate + 재페치.
    pub signature_verified: bool,
}

/// `put`이 받는 파라미터.
#[derive(Debug, Clone)]
pub struct CachePutInput {
    pub source: SourceTier,
    pub manifest_id: String,
    pub url: String,
    pub body: Vec<u8>,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub fetched_at: SystemTime,
    /// Phase R-B (ADR-0054) — 본 적재가 서명 검증을 통과했는지.
    /// 네트워크 fetch + verify 통과 → `true`.
    /// 네트워크 fetch + verify 미수행(verifier 미설정) → `false`.
    /// Bundled tier(빌드 시점 신뢰)는 caller가 `true`로 표시.
    pub signature_verified: bool,
}

/// SQLite 캐시 핸들. 단일 connection + tokio::sync::Mutex.
pub struct Cache {
    conn: Arc<Mutex<Connection>>,
}

impl Cache {
    /// 파일 경로에 DB를 열고 schema 보장. 부모 디렉터리는 caller가 만들어야 함.
    pub async fn open(path: &Path) -> Result<Self, FetcherError> {
        let path = path.to_owned();
        let conn = tokio::task::spawn_blocking(move || -> Result<Connection, FetcherError> {
            let conn = Connection::open(&path)?;
            init_schema(&conn)?;
            Ok(conn)
        })
        .await??;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// 메모리 DB — 통합 테스트용.
    pub async fn open_in_memory() -> Result<Self, FetcherError> {
        let conn = tokio::task::spawn_blocking(|| -> Result<Connection, FetcherError> {
            let conn = Connection::open_in_memory()?;
            init_schema(&conn)?;
            Ok(conn)
        })
        .await??;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// (source, manifest_id) row 조회. body_sha256 미스매치면 row 삭제 + Err(CacheCorrupt).
    pub async fn get(
        &self,
        source: SourceTier,
        manifest_id: &str,
    ) -> Result<Option<CacheRow>, FetcherError> {
        let conn = self.conn.clone();
        let id = manifest_id.to_owned();
        tokio::task::spawn_blocking(move || -> Result<Option<CacheRow>, FetcherError> {
            let conn = conn.blocking_lock();
            let mut stmt = conn.prepare(
                "SELECT url, body, body_sha256, content_type, etag, last_modified, fetched_at, signature_verified
                 FROM manifest_cache WHERE source = ?1 AND manifest_id = ?2",
            )?;
            let row = stmt
                .query_row(params![source.as_db_str(), &id], |r| {
                    let url: String = r.get(0)?;
                    let body: Vec<u8> = r.get(1)?;
                    let body_sha256: Vec<u8> = r.get(2)?;
                    let content_type: Option<String> = r.get(3)?;
                    let etag: Option<String> = r.get(4)?;
                    let last_modified: Option<String> = r.get(5)?;
                    let fetched_at_unix: i64 = r.get(6)?;
                    let signature_verified_int: i64 = r.get(7)?;
                    Ok((
                        url,
                        body,
                        body_sha256,
                        content_type,
                        etag,
                        last_modified,
                        fetched_at_unix,
                        signature_verified_int != 0,
                    ))
                })
                .ok();

            let Some((
                url,
                body,
                body_sha256,
                content_type,
                etag,
                last_modified,
                fetched_at_unix,
                signature_verified,
            )) = row
            else {
                return Ok(None);
            };

            // 무결성 검증.
            let mut hasher = Sha256::new();
            hasher.update(&body);
            let actual = hasher.finalize();
            if actual.as_slice() != body_sha256.as_slice() {
                tracing::warn!(
                    source = source.as_db_str(),
                    manifest_id = %id,
                    "cache row sha256 mismatch — dropping"
                );
                conn.execute(
                    "DELETE FROM manifest_cache WHERE source = ?1 AND manifest_id = ?2",
                    params![source.as_db_str(), &id],
                )?;
                return Err(FetcherError::CacheCorrupt);
            }

            let fetched_at = if fetched_at_unix < 0 {
                SystemTime::UNIX_EPOCH
            } else {
                SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(fetched_at_unix as u64)
            };

            Ok(Some(CacheRow {
                source,
                manifest_id: id,
                url,
                body,
                content_type,
                etag,
                last_modified,
                fetched_at,
                signature_verified,
            }))
        })
        .await?
    }

    /// upsert. body_sha256 자동 계산.
    pub async fn put(&self, input: CachePutInput) -> Result<(), FetcherError> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || -> Result<(), FetcherError> {
            let mut hasher = Sha256::new();
            hasher.update(&input.body);
            let body_sha256 = hasher.finalize().to_vec();

            let fetched_at_unix = input
                .fetched_at
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            let conn = conn.blocking_lock();
            conn.execute(
                "INSERT OR REPLACE INTO manifest_cache
                 (source, manifest_id, url, body, body_sha256, content_type, etag, last_modified, fetched_at, signature_verified)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    input.source.as_db_str(),
                    input.manifest_id,
                    input.url,
                    input.body,
                    body_sha256,
                    input.content_type,
                    input.etag,
                    input.last_modified,
                    fetched_at_unix,
                    if input.signature_verified { 1_i64 } else { 0_i64 },
                ],
            )?;
            Ok(())
        })
        .await?
    }

    /// Phase R-B (ADR-0054) — 서명 검증 통과 시 row를 verified로 마킹.
    /// `fetch_one_with_signature`가 verify 성공 직후 호출.
    pub async fn mark_verified(
        &self,
        source: SourceTier,
        manifest_id: &str,
    ) -> Result<(), FetcherError> {
        let conn = self.conn.clone();
        let id = manifest_id.to_owned();
        tokio::task::spawn_blocking(move || -> Result<(), FetcherError> {
            let conn = conn.blocking_lock();
            conn.execute(
                "UPDATE manifest_cache SET signature_verified = 1 \
                 WHERE source = ?1 AND manifest_id = ?2",
                params![source.as_db_str(), &id],
            )?;
            Ok(())
        })
        .await?
    }

    /// 304 Not Modified 시 호출 — body 그대로 두고 fetched_at만 갱신.
    pub async fn bump_fetched_at(
        &self,
        source: SourceTier,
        manifest_id: &str,
        when: SystemTime,
    ) -> Result<(), FetcherError> {
        let conn = self.conn.clone();
        let id = manifest_id.to_owned();
        let when_unix = when
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        tokio::task::spawn_blocking(move || -> Result<(), FetcherError> {
            let conn = conn.blocking_lock();
            conn.execute(
                "UPDATE manifest_cache SET fetched_at = ?3 WHERE source = ?1 AND manifest_id = ?2",
                params![source.as_db_str(), &id, when_unix],
            )?;
            Ok(())
        })
        .await?
    }

    /// invalidate — None이면 전체 삭제, Some(id)면 해당 id의 모든 source 삭제.
    pub async fn invalidate(&self, manifest_id: Option<&str>) -> Result<(), FetcherError> {
        let conn = self.conn.clone();
        let id = manifest_id.map(|s| s.to_owned());
        tokio::task::spawn_blocking(move || -> Result<(), FetcherError> {
            let conn = conn.blocking_lock();
            match id {
                Some(id) => {
                    conn.execute(
                        "DELETE FROM manifest_cache WHERE manifest_id = ?1",
                        params![id],
                    )?;
                }
                None => {
                    conn.execute("DELETE FROM manifest_cache", params![])?;
                }
            }
            Ok(())
        })
        .await?
    }
}

fn init_schema(conn: &Connection) -> Result<(), FetcherError> {
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;

        CREATE TABLE IF NOT EXISTS schema_meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        -- Phase R-B (ADR-0054) — v2 스키마: signature_verified 포함.
        -- 신규 설치는 IF NOT EXISTS로 v2 직접 생성. 기존 v1 사용자는 아래 ALTER로 마이그레이션.
        CREATE TABLE IF NOT EXISTS manifest_cache (
            source              TEXT    NOT NULL CHECK(source IN ('vendor','github','jsdelivr','bundled')),
            manifest_id         TEXT    NOT NULL,
            url                 TEXT    NOT NULL,
            body                BLOB    NOT NULL,
            body_sha256         BLOB    NOT NULL,
            content_type        TEXT,
            etag                TEXT,
            last_modified       TEXT,
            fetched_at          INTEGER NOT NULL,
            signature_verified  INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (source, manifest_id)
        ) WITHOUT ROWID;

        CREATE INDEX IF NOT EXISTS manifest_cache_fetched ON manifest_cache(fetched_at);
        "#,
    )?;

    // schema_meta 초기 row 결정 — 신규 설치면 v2, 기존이면 그대로 둠.
    // (CREATE TABLE IF NOT EXISTS가 신규 생성됐는지 여부는 row count로 추정.)
    let cache_row_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM manifest_cache", [], |r| r.get(0))
        .unwrap_or(0);
    let initial_version = if cache_row_count == 0 { "2" } else { "1" };
    conn.execute(
        "INSERT OR IGNORE INTO schema_meta(key, value) VALUES ('version', ?1)",
        params![initial_version],
    )?;

    // 스키마 버전 + 마이그레이션.
    let version: String = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE key = 'version'",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "1".to_string());
    let version_n: u32 = version.parse().unwrap_or(0);
    if version_n > SCHEMA_VERSION_MAX {
        return Err(FetcherError::SchemaMismatch {
            found: version_n,
            max: SCHEMA_VERSION_MAX,
        });
    }

    // Phase R-B (ADR-0054) — v1 → v2: signature_verified 컬럼 추가.
    // 기존 행은 default 0(unverified) — verifier 활성 모드면 caller가 invalidate + 재페치.
    if version_n < 2 {
        // CREATE TABLE은 v2 schema라 신규 설치는 컬럼이 이미 있음. 그래서 ALTER는 *기존 v1 사용자 한정*.
        // 컬럼 존재 시 "duplicate column" 에러 → 무시 (idempotent).
        let _ = conn.execute_batch(
            "ALTER TABLE manifest_cache \
             ADD COLUMN signature_verified INTEGER NOT NULL DEFAULT 0;",
        );
        conn.execute(
            "UPDATE schema_meta SET value = '2' WHERE key = 'version'",
            [],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn open_in_memory_and_basic_round_trip() {
        let cache = Cache::open_in_memory().await.unwrap();
        let put = CachePutInput {
            source: SourceTier::Github,
            manifest_id: "ollama".into(),
            url: "https://example.com/ollama.json".into(),
            body: br#"{"id":"ollama"}"#.to_vec(),
            content_type: Some("application/json".into()),
            etag: Some("\"abc\"".into()),
            last_modified: None,
            fetched_at: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000),
            signature_verified: false,
        };
        cache.put(put).await.unwrap();

        let row = cache
            .get(SourceTier::Github, "ollama")
            .await
            .unwrap()
            .expect("row exists");
        assert_eq!(row.source, SourceTier::Github);
        assert_eq!(row.manifest_id, "ollama");
        assert_eq!(row.body, br#"{"id":"ollama"}"#);
        assert_eq!(row.etag.as_deref(), Some("\"abc\""));
    }

    #[tokio::test]
    async fn missing_returns_none() {
        let cache = Cache::open_in_memory().await.unwrap();
        let got = cache.get(SourceTier::Github, "missing").await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn bump_fetched_at_updates_only_timestamp() {
        let cache = Cache::open_in_memory().await.unwrap();
        let put = CachePutInput {
            source: SourceTier::Github,
            manifest_id: "ollama".into(),
            url: "u".into(),
            body: b"x".to_vec(),
            content_type: None,
            etag: None,
            last_modified: None,
            fetched_at: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1000),
            signature_verified: false,
        };
        cache.put(put).await.unwrap();

        let new_time = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(2000);
        cache
            .bump_fetched_at(SourceTier::Github, "ollama", new_time)
            .await
            .unwrap();

        let row = cache
            .get(SourceTier::Github, "ollama")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.body, b"x");
        assert_eq!(
            row.fetched_at
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            2000
        );
    }

    #[tokio::test]
    async fn invalidate_specific_id() {
        let cache = Cache::open_in_memory().await.unwrap();
        for id in ["ollama", "lm-studio"] {
            let put = CachePutInput {
                source: SourceTier::Github,
                manifest_id: id.into(),
                url: "u".into(),
                body: b"x".to_vec(),
                content_type: None,
                etag: None,
                last_modified: None,
                fetched_at: SystemTime::UNIX_EPOCH,
                signature_verified: false,
            };
            cache.put(put).await.unwrap();
        }
        cache.invalidate(Some("ollama")).await.unwrap();
        assert!(cache
            .get(SourceTier::Github, "ollama")
            .await
            .unwrap()
            .is_none());
        assert!(cache
            .get(SourceTier::Github, "lm-studio")
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn invalidate_all() {
        let cache = Cache::open_in_memory().await.unwrap();
        let put = CachePutInput {
            source: SourceTier::Github,
            manifest_id: "ollama".into(),
            url: "u".into(),
            body: b"x".to_vec(),
            content_type: None,
            etag: None,
            last_modified: None,
            fetched_at: SystemTime::UNIX_EPOCH,
            signature_verified: false,
        };
        cache.put(put).await.unwrap();
        cache.invalidate(None).await.unwrap();
        assert!(cache
            .get(SourceTier::Github, "ollama")
            .await
            .unwrap()
            .is_none());
    }

    // ── Phase R-B (ADR-0054) — signature_verified marker ────────────────

    #[tokio::test]
    async fn signature_verified_round_trip_true() {
        let cache = Cache::open_in_memory().await.unwrap();
        let put = CachePutInput {
            source: SourceTier::Github,
            manifest_id: "catalog".into(),
            url: "u".into(),
            body: b"verified-body".to_vec(),
            content_type: None,
            etag: None,
            last_modified: None,
            fetched_at: SystemTime::UNIX_EPOCH,
            signature_verified: true,
        };
        cache.put(put).await.unwrap();
        let row = cache
            .get(SourceTier::Github, "catalog")
            .await
            .unwrap()
            .unwrap();
        assert!(row.signature_verified, "verified=true 라운드트립");
    }

    #[tokio::test]
    async fn signature_verified_round_trip_false() {
        let cache = Cache::open_in_memory().await.unwrap();
        let put = CachePutInput {
            source: SourceTier::Github,
            manifest_id: "catalog".into(),
            url: "u".into(),
            body: b"unverified-body".to_vec(),
            content_type: None,
            etag: None,
            last_modified: None,
            fetched_at: SystemTime::UNIX_EPOCH,
            signature_verified: false,
        };
        cache.put(put).await.unwrap();
        let row = cache
            .get(SourceTier::Github, "catalog")
            .await
            .unwrap()
            .unwrap();
        assert!(!row.signature_verified, "verified=false 라운드트립");
    }

    #[tokio::test]
    async fn mark_verified_flips_false_to_true() {
        let cache = Cache::open_in_memory().await.unwrap();
        let put = CachePutInput {
            source: SourceTier::Github,
            manifest_id: "catalog".into(),
            url: "u".into(),
            body: b"x".to_vec(),
            content_type: None,
            etag: None,
            last_modified: None,
            fetched_at: SystemTime::UNIX_EPOCH,
            signature_verified: false,
        };
        cache.put(put).await.unwrap();
        cache
            .mark_verified(SourceTier::Github, "catalog")
            .await
            .unwrap();
        let row = cache
            .get(SourceTier::Github, "catalog")
            .await
            .unwrap()
            .unwrap();
        assert!(row.signature_verified);
    }

    /// 신규 설치는 schema_meta version='2'로 직접 진입.
    #[tokio::test]
    async fn fresh_install_starts_at_schema_v2() {
        let cache = Cache::open_in_memory().await.unwrap();
        let conn = cache.conn.lock().await;
        let v: String = conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = 'version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(v, "2");
    }
}
