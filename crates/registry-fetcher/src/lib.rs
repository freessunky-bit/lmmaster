//! crate: registry-fetcher — Manifest 4-tier fallback fetcher with ETag + SQLite cache.
//!
//! Phase 1' 결정 (`docs/research/phase-1p-registry-fetcher-decision.md`):
//! - **4-tier**: Vendor → GitHub Releases → jsDelivr → Bundled. Sequential first-success.
//! - **ETag/If-Modified-Since** + **Stale-while-error** (TTL 1h, grace 24h).
//! - **rusqlite + tokio::spawn_blocking** (sqlx 미사용 — read-mostly 단일 테이블).
//! - **JSON parse error는 폴백 안 함** (cache poisoning 방지).
//! - Korean error/tracing 메시지.

pub mod cache;
pub mod error;
pub mod fetcher;
pub mod source;

use std::path::PathBuf;
use std::time::Duration;

pub use cache::{Cache, CachePutInput, CacheRow};
pub use error::FetcherError;
pub use fetcher::{FetchedManifest, FetcherCore};
pub use source::{default_sources, SourceConfig, SourceTier};

const DEFAULT_TTL_SEC: u64 = 3600;
const DEFAULT_STALE_GRACE_SEC: u64 = 86400;

/// 외부 진입점 옵션.
#[derive(Debug, Clone)]
pub struct FetcherOptions {
    /// SQLite 캐시 DB 파일 경로. 부모 디렉터리는 caller가 보장 (`create_dir_all`).
    pub cache_db: PathBuf,
    /// 기본은 `default_sources(...)` 결과 사용.
    pub sources: Vec<SourceConfig>,
    /// Tauri의 `BaseDirectory::Resource`로 해결한 manifests/apps 디렉터리.
    /// `None`이면 Bundled tier가 항상 BundledMissing 반환.
    pub bundled_dir: Option<PathBuf>,
    /// hot-path TTL — 이 안에선 네트워크 안 탐. 기본 1h.
    pub ttl: Duration,
    /// 모든 네트워크 실패 시 stale 캐시를 사용할 수 있는 최대 age. 기본 24h.
    pub stale_grace: Duration,
    /// 외부에서 reqwest::Client 주입 — None이면 기본 client 생성.
    pub http: Option<reqwest::Client>,
}

impl FetcherOptions {
    pub fn new(cache_db: PathBuf, sources: Vec<SourceConfig>) -> Self {
        Self {
            cache_db,
            sources,
            bundled_dir: None,
            ttl: Duration::from_secs(DEFAULT_TTL_SEC),
            stale_grace: Duration::from_secs(DEFAULT_STALE_GRACE_SEC),
            http: None,
        }
    }

    pub fn with_bundled_dir(mut self, dir: PathBuf) -> Self {
        self.bundled_dir = Some(dir);
        self
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    pub fn with_stale_grace(mut self, grace: Duration) -> Self {
        self.stale_grace = grace;
        self
    }

    pub fn with_http(mut self, http: reqwest::Client) -> Self {
        self.http = Some(http);
        self
    }
}

/// public 진입점. 내부적으로 `FetcherCore`를 보유.
pub struct RegistryFetcher {
    core: FetcherCore,
}

impl RegistryFetcher {
    /// 비동기 생성 — DB open + schema 초기화.
    pub async fn new(opts: FetcherOptions) -> Result<Self, FetcherError> {
        if let Some(parent) = opts.cache_db.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }
        let cache = Cache::open(&opts.cache_db).await?;
        let http = opts.http.unwrap_or_else(|| {
            reqwest::Client::builder()
                .user_agent(format!("LMmaster/{}", env!("CARGO_PKG_VERSION")))
                .pool_idle_timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new())
        });
        let core = FetcherCore {
            http,
            cache,
            sources: opts.sources,
            bundled_dir: opts.bundled_dir,
            ttl: opts.ttl,
            stale_grace: opts.stale_grace,
        };
        Ok(Self { core })
    }

    /// 단일 manifest fetch. 4-tier fallback + 캐시 활용.
    pub async fn fetch(&self, manifest_id: &str) -> Result<FetchedManifest, FetcherError> {
        self.core.fetch_one(manifest_id).await
    }

    /// 다중 manifest 병렬 fetch (id 단위로 4-concurrency).
    pub async fn fetch_all(
        &self,
        ids: &[&str],
    ) -> Vec<(String, Result<FetchedManifest, FetcherError>)> {
        use futures::stream::StreamExt;
        let owned_ids: Vec<String> = ids.iter().map(|s| (*s).to_string()).collect();
        let stream = futures::stream::iter(owned_ids.into_iter().map(|id| {
            let id_for_pair = id.clone();
            async move {
                let r = self.core.fetch_one(&id).await;
                (id_for_pair, r)
            }
        }))
        .buffer_unordered(4);
        stream.collect().await
    }

    /// 캐시 무효화. None = 전체 삭제, Some(id) = 해당 id의 모든 source.
    pub async fn invalidate(&self, manifest_id: Option<&str>) -> Result<(), FetcherError> {
        self.core.cache.invalidate(manifest_id).await
    }

    /// FetchedManifest body를 임의 타입으로 파싱.
    pub fn parse<T: serde::de::DeserializeOwned>(
        &self,
        fm: &FetchedManifest,
    ) -> Result<T, FetcherError> {
        Ok(serde_json::from_slice(&fm.body)?)
    }
}
