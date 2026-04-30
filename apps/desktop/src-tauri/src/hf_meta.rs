//! HuggingFace Hub metadata fetch — Phase 13'.e.2.
//!
//! 정책:
//! - 외부 통신 0 정책 예외 (ADR-0026 §1과 같은 갈래 — 이미 jsDelivr/GitHub 허용).
//! - HF Hub API: `GET https://huggingface.co/api/models/{repo}` → downloads/likes/lastModified.
//! - rate limit unauth 1000 req/h — 50-200 모델 규모에서 충분.
//! - 결과는 `HfMetaCache` (메모리)에 캐시 — TTL 6h. 디스크 영속 불필요 (앱 재시작 시 다시 fetch).
//! - `get_catalog`이 entries 반환 시 cache의 hf_meta를 머지.
//! - 동시 호출 5개 제한 — bulk refresh 시 HF 부하 + 사용자 PC 네트워크 부하 적정선.

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use futures::stream::{self, StreamExt};
use model_registry::{HfMeta, ModelEntry, ModelSource};
use serde::Deserialize;

/// HF Hub API 응답의 일부 — 우리가 쓰는 필드만.
#[derive(Debug, Deserialize)]
struct HfApiModel {
    #[serde(default)]
    downloads: Option<u64>,
    #[serde(default)]
    likes: Option<u64>,
    #[serde(default, rename = "lastModified")]
    last_modified: Option<String>,
}

/// 메모리 캐시 — model_id → (meta, fetched_at).
pub struct HfMetaCache {
    inner: RwLock<HashMap<String, (HfMeta, Instant)>>,
}

impl HfMetaCache {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    pub fn get(&self, model_id: &str) -> Option<HfMeta> {
        self.inner
            .read()
            .ok()?
            .get(model_id)
            .map(|(m, _)| m.clone())
    }

    pub fn set(&self, model_id: &str, meta: HfMeta) {
        if let Ok(mut g) = self.inner.write() {
            g.insert(model_id.to_string(), (meta, Instant::now()));
        }
    }

    pub fn entry_count(&self) -> usize {
        self.inner.read().map(|g| g.len()).unwrap_or(0)
    }

    /// 단순 expiry — TTL 안에 들어오면 fresh, 아니면 stale (재 fetch 권장).
    pub fn is_fresh(&self, model_id: &str, ttl: Duration) -> bool {
        self.inner
            .read()
            .ok()
            .and_then(|g| g.get(model_id).map(|(_, t)| t.elapsed() < ttl))
            .unwrap_or(false)
    }
}

impl Default for HfMetaCache {
    fn default() -> Self {
        Self::new()
    }
}

/// 단일 모델 HF metadata fetch.
pub async fn fetch_one(
    http: &reqwest::Client,
    repo: &str,
) -> Result<HfMeta, anyhow::Error> {
    let url = format!("https://huggingface.co/api/models/{repo}");
    let resp = http
        .get(&url)
        .timeout(Duration::from_secs(8))
        .send()
        .await?;
    if !resp.status().is_success() {
        anyhow::bail!("HF API {} HTTP {}", repo, resp.status());
    }
    let body: HfApiModel = resp.json().await?;
    Ok(HfMeta {
        downloads: body.downloads.unwrap_or(0),
        likes: body.likes.unwrap_or(0),
        last_modified: body.last_modified.unwrap_or_default(),
    })
}

/// 모든 entries 중 HuggingFace source인 것만 골라 bulk fetch.
///
/// 동시성 제한 5 — `buffer_unordered`로 자연스러운 흐름 제어.
/// HF API 부하 + 사용자 PC 네트워크 부담 회피.
/// 실패는 warn 로깅 + 다음 모델 계속 — 일부 실패해도 나머지는 캐시에 채움.
/// 반환: (성공 N, 실패 N).
pub async fn refresh_all(
    http: &reqwest::Client,
    cache: &HfMetaCache,
    entries: &[ModelEntry],
) -> (usize, usize) {
    let candidates: Vec<(String, String)> = entries
        .iter()
        .filter_map(|e| match &e.source {
            ModelSource::HuggingFace { repo, .. } => Some((e.id.clone(), repo.clone())),
            ModelSource::DirectUrl { .. } => None,
        })
        .collect();

    const CONCURRENCY: usize = 5;
    let results: Vec<(String, String, Result<HfMeta, anyhow::Error>)> = stream::iter(candidates)
        .map(|(id, repo)| async move {
            let result = fetch_one(http, &repo).await;
            (id, repo, result)
        })
        .buffer_unordered(CONCURRENCY)
        .collect()
        .await;

    let mut ok_n = 0usize;
    let mut fail_n = 0usize;
    for (id, repo, result) in results {
        match result {
            Ok(meta) => {
                cache.set(&id, meta);
                ok_n += 1;
            }
            Err(e) => {
                tracing::debug!(
                    repo = %repo,
                    error = %e,
                    "HF metadata fetch 실패 — 다음 모델 계속"
                );
                fail_n += 1;
            }
        }
    }
    tracing::info!(ok = ok_n, fail = fail_n, "HF metadata bulk refresh 완료");
    (ok_n, fail_n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_set_get_round_trip() {
        let c = HfMetaCache::new();
        let m = HfMeta {
            downloads: 100,
            likes: 5,
            last_modified: "2026-04-30T00:00:00Z".into(),
        };
        c.set("model-1", m.clone());
        let got = c.get("model-1").expect("cached");
        assert_eq!(got.downloads, 100);
        assert_eq!(got.likes, 5);
        assert_eq!(c.entry_count(), 1);
    }

    #[test]
    fn cache_miss_returns_none() {
        let c = HfMetaCache::new();
        assert!(c.get("missing").is_none());
        assert!(!c.is_fresh("missing", Duration::from_secs(1)));
    }

    #[test]
    fn cache_freshness_window() {
        let c = HfMetaCache::new();
        let m = HfMeta {
            downloads: 0,
            likes: 0,
            last_modified: "x".into(),
        };
        c.set("model-1", m);
        assert!(c.is_fresh("model-1", Duration::from_secs(60)));
    }
}
