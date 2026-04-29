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

const SCHEMA_VERSION_MAX: u32 = 1;

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
                "SELECT url, body, body_sha256, content_type, etag, last_modified, fetched_at
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
                    Ok((
                        url,
                        body,
                        body_sha256,
                        content_type,
                        etag,
                        last_modified,
                        fetched_at_unix,
                    ))
                })
                .ok();

            let Some((url, body, body_sha256, content_type, etag, last_modified, fetched_at_unix)) =
                row
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
                 (source, manifest_id, url, body, body_sha256, content_type, etag, last_modified, fetched_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
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
                ],
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
        INSERT OR IGNORE INTO schema_meta(key, value) VALUES ('version', '1');

        CREATE TABLE IF NOT EXISTS manifest_cache (
            source        TEXT NOT NULL CHECK(source IN ('vendor','github','jsdelivr','bundled')),
            manifest_id   TEXT NOT NULL,
            url           TEXT NOT NULL,
            body          BLOB NOT NULL,
            body_sha256   BLOB NOT NULL,
            content_type  TEXT,
            etag          TEXT,
            last_modified TEXT,
            fetched_at    INTEGER NOT NULL,
            PRIMARY KEY (source, manifest_id)
        ) WITHOUT ROWID;

        CREATE INDEX IF NOT EXISTS manifest_cache_fetched ON manifest_cache(fetched_at);
        "#,
    )?;

    // 스키마 버전 검사.
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
        };
        cache.put(put).await.unwrap();
        cache.invalidate(None).await.unwrap();
        assert!(cache
            .get(SourceTier::Github, "ollama")
            .await
            .unwrap()
            .is_none());
    }
}
