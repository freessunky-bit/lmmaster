//! 4-tier fallback 로직 + 조건부 헤더 + stale-while-error.
//!
//! 정책 (Phase 1' 결정 §1, §4):
//! - sequential first-success.
//! - JSON 파싱 에러는 폴백 안 함 — cache poisoning 방지.
//! - 304 Not Modified → 캐시된 body 그대로 반환 + fetched_at 갱신.
//! - 모든 네트워크 tier 실패 → cache 검사: TTL 내 fresh / 24h 내 stale / 24h 초과 bundled.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::cache::{Cache, CachePutInput, CacheRow};
use crate::error::FetcherError;
use crate::signature::SignatureVerifier;
use crate::source::{SourceConfig, SourceTier};

/// fetch 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchedManifest {
    pub source: SourceTier,
    pub manifest_id: String,
    /// 캐시에서 바로 반환됐는지 (304 또는 stale 포함).
    pub from_cache: bool,
    /// stale-while-error 모드로 반환됐는지.
    pub stale: bool,
    pub fetched_at: SystemTime,
    pub etag: Option<String>,
    pub body: Vec<u8>,
    /// Phase R-B (ADR-0054) — 본 row의 minisign 서명이 검증된 상태인지.
    /// caller는 verifier 활성 정책에서 false 시 invalidate + 재페치 또는 사용 거부.
    /// `fetch_one`은 verify를 수행하지 않으므로 *cache 적재 시점의 marker* 그대로 노출.
    /// `fetch_one_with_signature`는 verify 통과 시 본 필드를 true로 채워 반환.
    #[serde(default)]
    pub signature_verified: bool,
}

impl FetchedManifest {
    fn from_cache_row(row: &CacheRow, stale: bool) -> Self {
        Self {
            source: row.source,
            manifest_id: row.manifest_id.clone(),
            from_cache: true,
            stale,
            fetched_at: row.fetched_at,
            etag: row.etag.clone(),
            body: row.body.clone(),
            signature_verified: row.signature_verified,
        }
    }
}

pub struct FetcherCore {
    pub http: reqwest::Client,
    pub cache: Cache,
    pub sources: Vec<SourceConfig>,
    pub bundled_dir: Option<PathBuf>,
    pub ttl: Duration,
    pub stale_grace: Duration,
}

impl FetcherCore {
    /// id에 대해 4-tier fallback 시도 + minisign 서명 검증. (Phase 13'.g.2.c, ADR-0047 + R-B / ADR-0054)
    ///
    /// 정책:
    /// - 네트워크 fresh fetch면 `.minisig` 추가 fetch + `verifier.verify` 강제. 통과 시 cache row를
    ///   `signature_verified=true`로 마킹.
    /// - cache 적중(`from_cache=true`)이면 row의 `signature_verified` 검사 — false면 invalidate +
    ///   네트워크 재페치 (Phase R-B 기존 v1 캐시 또는 verifier 비활성 상태에서 적재된 row 거부).
    /// - Bundled tier(`is_network()=false`)는 verify skip — 빌드 시점 신뢰 (signature_verified=true).
    /// - verify 실패 → `FetcherError::SignatureFailed` (caller가 bundled fallback로 강등).
    /// - `.minisig` 파일 미존재(404) → `FetcherError::SignatureMissing` (CI 서명 파이프라인 미작동).
    pub async fn fetch_one_with_signature(
        &self,
        id: &str,
        verifier: &SignatureVerifier,
    ) -> Result<FetchedManifest, FetcherError> {
        let manifest = self.fetch_one(id).await?;

        // Bundled — verify skip (신뢰 경로).
        if !manifest.source.is_network() {
            return Ok(manifest);
        }

        // cache 적중 시 signature_verified 검사 (Phase R-B / ADR-0054).
        if manifest.from_cache {
            let row = self.cache.get(manifest.source, id).await?;
            if matches!(row, Some(ref r) if r.signature_verified) {
                return Ok(manifest);
            }
            // 미검증 cache row — invalidate + 네트워크 재페치 강제.
            tracing::warn!(
                tier = manifest.source.as_db_str(),
                manifest_id = %id,
                "캐시 row가 signature_verified=false — invalidate 후 재페치"
            );
            let _ = self.cache.invalidate(Some(id)).await;
            // 재귀 1회 — 재귀 후에는 from_cache=false 또는 Bundled로 빠짐.
            return Box::pin(self.fetch_one_with_signature(id, verifier)).await;
        }

        // 네트워크 fresh — 같은 tier에서 .minisig fetch + verify.
        let source_config = self
            .sources
            .iter()
            .find(|s| s.tier == manifest.source)
            .ok_or_else(|| {
                FetcherError::SignatureFailed(format!(
                    "source tier {:?} not configured",
                    manifest.source
                ))
            })?;

        let sig_url = source_config.resolve_signature_url(id)?;
        let sig_resp = self
            .http
            .get(&sig_url)
            .timeout(source_config.timeout)
            .send()
            .await
            .map_err(|e| FetcherError::SignatureMissing(format!("{id}: {e}")))?;

        if !sig_resp.status().is_success() {
            return Err(FetcherError::SignatureMissing(format!(
                "{id}: HTTP {}",
                sig_resp.status()
            )));
        }

        let sig_text = sig_resp
            .text()
            .await
            .map_err(|e| FetcherError::SignatureMissing(format!("{id}: {e}")))?;

        verifier
            .verify(&manifest.body, &sig_text)
            .map_err(|e| FetcherError::SignatureFailed(e.to_string()))?;

        // Phase R-B (ADR-0054) — verify 통과 → cache row를 verified로 마킹 + 반환 시그널 true.
        let _ = self.cache.mark_verified(manifest.source, id).await;
        let mut verified = manifest;
        verified.signature_verified = true;
        Ok(verified)
    }

    /// id에 대해 4-tier fallback 시도.
    pub async fn fetch_one(&self, id: &str) -> Result<FetchedManifest, FetcherError> {
        if id.is_empty() {
            return Err(FetcherError::EmptyManifestId);
        }
        // 입력 검증 — '..' 등 차단은 source.resolve_url가 처리.
        let mut tried: Vec<SourceTier> = Vec::new();
        let mut last_network_err: Option<FetcherError> = None;

        for src in &self.sources {
            tried.push(src.tier);
            match self.try_source(src, id).await {
                Ok(fm) => return Ok(fm),
                Err(FetcherError::JsonParse(e)) => {
                    // JSON 파싱 에러는 폴백 안 함 (cache poisoning 방지).
                    return Err(FetcherError::JsonParse(e));
                }
                Err(e) => {
                    tracing::warn!(
                        tier = src.tier.as_db_str(),
                        manifest_id = %id,
                        error = %e,
                        "tier 실패, 다음 미러로 넘어가요"
                    );
                    last_network_err = Some(e);
                }
            }
        }

        // 모든 tier 실패 — stale-while-error 검사.
        if let Ok(Some(stale_fm)) = self.try_stale_cache(id).await {
            tracing::warn!(
                manifest_id = %id,
                "오프라인 — 캐시 본문을 사용해요"
            );
            return Ok(stale_fm);
        }

        Err(last_network_err.unwrap_or(FetcherError::AllSourcesFailed {
            id: id.into(),
            tried,
        }))
    }

    /// 단일 source 시도. 캐시된 ETag/Last-Modified를 conditional 헤더로 사용.
    async fn try_source(
        &self,
        src: &SourceConfig,
        id: &str,
    ) -> Result<FetchedManifest, FetcherError> {
        if src.tier == SourceTier::Bundled {
            return self.try_bundled(id).await;
        }

        let url = src.resolve_url(id)?;
        let cached = self.cache.get(src.tier, id).await.ok().flatten();

        // TTL 내 fresh 캐시면 네트워크 안 타고 즉시 반환.
        if let Some(row) = &cached {
            if self.is_fresh(row.fetched_at) {
                tracing::debug!(
                    tier = src.tier.as_db_str(),
                    manifest_id = %id,
                    "fresh 캐시 — 네트워크 skip"
                );
                return Ok(FetchedManifest::from_cache_row(row, false));
            }
        }

        // 조건부 GET.
        let mut req = self.http.get(&url).timeout(src.timeout);
        if let Some(row) = &cached {
            if let Some(etag) = &row.etag {
                req = req.header(reqwest::header::IF_NONE_MATCH, etag);
            }
            if let Some(lm) = &row.last_modified {
                req = req.header(reqwest::header::IF_MODIFIED_SINCE, lm);
            }
        }

        let resp = req.send().await?;
        let status = resp.status();

        if status == reqwest::StatusCode::NOT_MODIFIED {
            if let Some(row) = &cached {
                let now = SystemTime::now();
                let _ = self.cache.bump_fetched_at(src.tier, id, now).await;
                return Ok(FetchedManifest {
                    source: row.source,
                    manifest_id: row.manifest_id.clone(),
                    from_cache: true,
                    stale: false,
                    fetched_at: now,
                    etag: row.etag.clone(),
                    body: row.body.clone(),
                    signature_verified: row.signature_verified,
                });
            }
            // 304인데 캐시 없음 — 비정상. 다음 tier로.
            return Err(FetcherError::HttpStatus {
                status: status.as_u16(),
                tier: src.tier,
            });
        }

        if !status.is_success() {
            return Err(FetcherError::HttpStatus {
                status: status.as_u16(),
                tier: src.tier,
            });
        }

        // ETag/Last-Modified 추출.
        let etag = resp
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let last_modified = resp
            .headers()
            .get(reqwest::header::LAST_MODIFIED)
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let body = resp.bytes().await?.to_vec();

        // JSON 유효성 한 번 검증 — 잘못된 응답이 캐시되는 걸 막음.
        if let Err(e) = serde_json::from_slice::<serde::de::IgnoredAny>(&body) {
            return Err(FetcherError::JsonParse(e));
        }

        let fetched_at = SystemTime::now();
        // Phase R-B (ADR-0054) — 본 적재는 *서명 검증 전*. fetch_one_with_signature가
        // verify 통과 시 mark_verified로 별도 표시. fetch_one 단독 caller는 unverified.
        let _ = self
            .cache
            .put(CachePutInput {
                source: src.tier,
                manifest_id: id.to_owned(),
                url: url.clone(),
                body: body.clone(),
                content_type,
                etag: etag.clone(),
                last_modified,
                fetched_at,
                signature_verified: false,
            })
            .await;

        Ok(FetchedManifest {
            source: src.tier,
            manifest_id: id.to_owned(),
            from_cache: false,
            stale: false,
            fetched_at,
            etag,
            body,
            // 네트워크 fresh — caller(`fetch_one_with_signature`)가 verify 후 mark_verified로 갱신.
            signature_verified: false,
        })
    }

    /// Bundled 디렉터리에서 `<id>.json` 파일 read.
    async fn try_bundled(&self, id: &str) -> Result<FetchedManifest, FetcherError> {
        let dir = self
            .bundled_dir
            .as_ref()
            .ok_or_else(|| FetcherError::BundledMissing(format!("bundled_dir 미설정 (id={id})")))?;
        let path = dir.join(format!("{id}.json"));
        if !path.exists() {
            return Err(FetcherError::BundledMissing(path.display().to_string()));
        }
        let body = tokio::fs::read(&path).await?;

        // JSON 유효성 검사.
        if let Err(e) = serde_json::from_slice::<serde::de::IgnoredAny>(&body) {
            return Err(FetcherError::JsonParse(e));
        }

        let fetched_at = SystemTime::now();
        // Phase R-B (ADR-0054) — Bundled tier는 빌드 시점에 큐레이터가 선별 + 코드 서명한 경로라
        // 신뢰 가능. signature_verified=true로 표시.
        let put = CachePutInput {
            source: SourceTier::Bundled,
            manifest_id: id.to_owned(),
            url: format!("file://{}", path.display()),
            body: body.clone(),
            content_type: Some("application/json".into()),
            etag: None,
            last_modified: None,
            fetched_at,
            signature_verified: true,
        };
        let _ = self.cache.put(put).await;

        Ok(FetchedManifest {
            source: SourceTier::Bundled,
            manifest_id: id.to_owned(),
            from_cache: false,
            stale: false,
            fetched_at,
            etag: None,
            body,
            // Bundled tier — 빌드 시점 큐레이터 검증 + 코드 서명. 신뢰 가능.
            signature_verified: true,
        })
    }

    /// 모든 network tier 실패 후 호출 — 캐시 grace 검사.
    /// stale-grace 내면 가장 신선한 캐시 row를 stale=true로 반환.
    async fn try_stale_cache(&self, id: &str) -> Result<Option<FetchedManifest>, FetcherError> {
        let now = SystemTime::now();
        // 모든 source에서 row 조회 후 가장 fresh 한 row 선택.
        let mut best: Option<CacheRow> = None;
        for src in &self.sources {
            if !src.tier.is_network() {
                continue;
            }
            if let Ok(Some(row)) = self.cache.get(src.tier, id).await {
                let row_fresh = best.as_ref().is_none_or(|b| row.fetched_at > b.fetched_at);
                if row_fresh {
                    best = Some(row);
                }
            }
        }
        let Some(row) = best else { return Ok(None) };

        let age = now.duration_since(row.fetched_at).unwrap_or(Duration::ZERO);
        if age > self.stale_grace {
            return Ok(None);
        }

        Ok(Some(FetchedManifest::from_cache_row(&row, true)))
    }

    fn is_fresh(&self, fetched_at: SystemTime) -> bool {
        SystemTime::now()
            .duration_since(fetched_at)
            .map(|age| age <= self.ttl)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_core(sources: Vec<SourceConfig>, bundled_dir: Option<PathBuf>) -> FetcherCore {
        let cache = futures::executor::block_on(Cache::open_in_memory()).unwrap();
        FetcherCore {
            http: reqwest::Client::new(),
            cache,
            sources,
            bundled_dir,
            ttl: Duration::from_secs(3600),
            stale_grace: Duration::from_secs(86400),
        }
    }

    #[tokio::test]
    async fn empty_id_rejected() {
        let core = make_core(Vec::new(), None);
        let r = core.fetch_one("").await;
        assert!(matches!(r, Err(FetcherError::EmptyManifestId)));
    }

    #[tokio::test]
    async fn no_sources_no_cache_no_bundled_returns_all_failed() {
        let core = make_core(Vec::new(), None);
        let r = core.fetch_one("ollama").await;
        match r {
            Err(FetcherError::AllSourcesFailed { id, .. }) => assert_eq!(id, "ollama"),
            other => panic!("expected AllSourcesFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn bundled_only_reads_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let body = br#"{"schema_version":1,"id":"x"}"#;
        std::fs::write(dir.path().join("x.json"), body).unwrap();

        let core = make_core(
            vec![SourceConfig {
                tier: SourceTier::Bundled,
                url_template: String::new(),
                timeout: Duration::from_secs(1),
            }],
            Some(dir.path().to_path_buf()),
        );
        let fm = core.fetch_one("x").await.unwrap();
        assert_eq!(fm.source, SourceTier::Bundled);
        assert_eq!(fm.body, body);
    }

    #[tokio::test]
    async fn bundled_missing_file_errors() {
        let dir = tempfile::TempDir::new().unwrap();
        let core = make_core(
            vec![SourceConfig {
                tier: SourceTier::Bundled,
                url_template: String::new(),
                timeout: Duration::from_secs(1),
            }],
            Some(dir.path().to_path_buf()),
        );
        let r = core.fetch_one("missing").await;
        assert!(matches!(r, Err(FetcherError::BundledMissing(_))));
    }

    #[tokio::test]
    async fn bundled_invalid_json_does_not_fall_through() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("bad.json"), b"not json").unwrap();
        let core = make_core(
            vec![SourceConfig {
                tier: SourceTier::Bundled,
                url_template: String::new(),
                timeout: Duration::from_secs(1),
            }],
            Some(dir.path().to_path_buf()),
        );
        let r = core.fetch_one("bad").await;
        assert!(matches!(r, Err(FetcherError::JsonParse(_))));
    }
}
