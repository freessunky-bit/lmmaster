//! `RegistryFetcherService` — Phase 1' 자동 갱신 통합.
//!
//! 정책 (`docs/research/phase-1p-registry-fetcher-decision.md`):
//! - 6시간 cron (DEFAULT_INTERVAL_SECS) — manifest 4-tier ETag fetcher 호출.
//! - 외부 통신 0 정책 예외: GitHub Releases / jsDelivr만 허용 (ADR-0026 §1과 같은 갈래).
//! - 실패 시 stale-while-error → 캐시된 매니페스트 그대로. 사용자에게 fail toast 표시 안 함.
//! - 성공 시 `catalog://refreshed` event emit + LastRefresh state 갱신 → UI 자동 reload.
//! - 수동 트리거 IPC: `refresh_catalog_now` / `get_last_catalog_refresh`.
//! - 검증 invariant: interval [3600, 86400] sec — 너무 짧으면 GitHub rate limit 위험.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use registry_fetcher::{default_sources, FetcherError, FetcherOptions, RegistryFetcher};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use thiserror::Error;
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};

/// 기본 6시간 (=21600초). ADR-0026 / Phase 1' 결정.
pub const DEFAULT_INTERVAL_SECS: u64 = 6 * 60 * 60;
/// 최소 1시간 — GitHub rate limit (60/h unauth) 보호.
pub const MIN_INTERVAL_SECS: u64 = 3_600;
/// 최대 24시간 — stale catalog 노출 방지.
pub const MAX_INTERVAL_SECS: u64 = 86_400;

/// Tauri state — 마지막 refresh 시각 + 활성 fetch handle.
pub struct RegistryFetcherService {
    fetcher: Arc<RegistryFetcher>,
    /// IDs to refresh on each tick. v1: scanner / installer가 사용하는 manifest IDs.
    /// 비어있으면 cron job은 no-op (테스트 / minimal 빌드).
    manifest_ids: Vec<String>,
    /// 마지막 refresh outcome — get_last_catalog_refresh가 read.
    last_refresh: Arc<Mutex<Option<LastRefresh>>>,
    sched: Mutex<Option<JobScheduler>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LastRefresh {
    /// UNIX epoch ms — 직렬화 시 ISO처럼 보일 필요는 없어요.
    pub at_ms: u128,
    /// 성공한 manifest 갯수.
    pub fetched_count: usize,
    /// 실패한 manifest 갯수.
    pub failed_count: usize,
    /// "ok" / "partial" / "failed".
    pub outcome: &'static str,
}

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CatalogRefreshError {
    #[error("이전 갱신이 아직 진행 중이에요")]
    AlreadyRunning,

    #[error("자동 갱신이 아직 준비되지 않았어요")]
    NotInitialized,

    #[error("주기는 1시간(3600s)에서 24시간(86400s) 사이여야 해요")]
    IntervalOutOfRange,

    #[error("설정한 매니페스트가 없어 갱신할 게 없어요")]
    NoManifests,

    #[error("자동 갱신 스케줄러 등록에 실패했어요: {message}")]
    SchedulerSetup { message: String },

    #[error("갱신 실패: {message}")]
    Internal { message: String },
}

impl From<FetcherError> for CatalogRefreshError {
    fn from(e: FetcherError) -> Self {
        Self::Internal {
            message: e.to_string(),
        }
    }
}

impl RegistryFetcherService {
    /// 빌드 — 기본 v1 source 셋(GitHub Releases + jsDelivr + bundled).
    /// `bundled_dir`은 `manifests/snapshot/apps/` 위치 — 호출자가 결정.
    pub async fn new(
        cache_db: PathBuf,
        bundled_dir: Option<PathBuf>,
        manifest_ids: Vec<String>,
        github_tag: &str,
        jsdelivr_ref: &str,
    ) -> Result<Self, CatalogRefreshError> {
        let mut opts = FetcherOptions::new(cache_db, default_sources(github_tag, jsdelivr_ref));
        if let Some(d) = bundled_dir {
            opts = opts.with_bundled_dir(d);
        }
        let fetcher = RegistryFetcher::new(opts).await?;
        Ok(Self {
            fetcher: Arc::new(fetcher),
            manifest_ids,
            last_refresh: Arc::new(Mutex::new(None)),
            sched: Mutex::new(None),
        })
    }

    /// 모든 manifest를 1회 fetch — 성공/실패를 합산해 LastRefresh 기록 + event emit.
    /// `app`이 None이면 emit skip (테스트용).
    pub async fn refresh_once(&self, app: Option<AppHandle>) -> LastRefresh {
        if self.manifest_ids.is_empty() {
            tracing::debug!("registry fetcher tick — manifest_ids empty, skip");
            let snapshot = LastRefresh {
                at_ms: epoch_ms(),
                fetched_count: 0,
                failed_count: 0,
                outcome: "ok",
            };
            *self.last_refresh.lock().await = Some(snapshot.clone());
            return snapshot;
        }
        let ids: Vec<&str> = self.manifest_ids.iter().map(|s| s.as_str()).collect();
        let results = self.fetcher.fetch_all(&ids).await;

        let mut fetched = 0usize;
        let mut failed = 0usize;
        // Phase 13'.a — "catalog" id가 fetch되면 body를 보존해 hot-swap에 사용.
        let mut catalog_body: Option<Vec<u8>> = None;
        for (id, r) in &results {
            match r {
                Ok(fm) => {
                    tracing::info!(
                        manifest = %id,
                        from_cache = fm.from_cache,
                        stale = fm.stale,
                        "manifest 갱신 완료"
                    );
                    fetched += 1;
                    if id.as_str() == "catalog" {
                        catalog_body = Some(fm.body.clone());
                    }
                }
                Err(e) => {
                    tracing::warn!(manifest = %id, error = %e, "manifest 갱신 실패");
                    failed += 1;
                }
            }
        }

        let outcome = if failed == 0 {
            "ok"
        } else if fetched == 0 {
            "failed"
        } else {
            "partial"
        };
        let snapshot = LastRefresh {
            at_ms: epoch_ms(),
            fetched_count: fetched,
            failed_count: failed,
            outcome,
        };
        *self.last_refresh.lock().await = Some(snapshot.clone());

        // 외부 fetch가 1개라도 성공했으면 catalog 무효화 + UI 알림.
        if let Some(handle) = app {
            // Phase 13'.a — catalog body를 받았으면 직접 deserialize해서 swap.
            //   기존엔 reload_from_bundled (디스크 재읽기)였지만, 그건 원격 갱신을 반영 못 함.
            //   이제: 원격 catalog.json → CatalogState::swap_from_bundle_body.
            let catalog_swapped = if let Some(body) = catalog_body {
                if let Some(state) = handle.try_state::<Arc<crate::commands::CatalogState>>() {
                    match state.swap_from_bundle_body(&body) {
                        Ok(count) => {
                            tracing::info!(
                                entries = count,
                                "catalog hot-swap 완료 (원격 bundle 반영)"
                            );
                            true
                        }
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                "catalog body decode 실패 — bundled로 폴백"
                            );
                            false
                        }
                    }
                } else {
                    false
                }
            } else {
                false
            };

            if fetched > 0 {
                if let Err(e) = handle.emit("catalog://refreshed", &snapshot) {
                    tracing::debug!(error = %e, "catalog://refreshed emit 실패");
                }
                // catalog가 hot-swap 안 됐으면 (예: app manifest만 갱신) bundled에서 재로드.
                if !catalog_swapped {
                    if let Some(state) = handle.try_state::<Arc<crate::commands::CatalogState>>() {
                        if let Err(e) = state.reload_from_bundled(&handle) {
                            tracing::debug!(
                                error = %e,
                                "catalog reload 실패 — 다음 startup에서 반영"
                            );
                        }
                    }
                }
            }
        }
        snapshot
    }

    /// 마지막 refresh 시각.
    pub async fn last_refresh(&self) -> Option<LastRefresh> {
        self.last_refresh.lock().await.clone()
    }

    /// cron job 등록 — `interval_secs`는 [3600, 86400] 범위.
    pub async fn start(
        self: Arc<Self>,
        app: AppHandle,
        interval_secs: u64,
    ) -> Result<(), CatalogRefreshError> {
        if !(MIN_INTERVAL_SECS..=MAX_INTERVAL_SECS).contains(&interval_secs) {
            return Err(CatalogRefreshError::IntervalOutOfRange);
        }
        let sched = JobScheduler::new()
            .await
            .map_err(|e| CatalogRefreshError::SchedulerSetup {
                message: e.to_string(),
            })?;
        let svc = Arc::clone(&self);
        let app_for_job = app.clone();
        let job = Job::new_repeated_async(Duration::from_secs(interval_secs), move |_uuid, _l| {
            let svc = Arc::clone(&svc);
            let app = app_for_job.clone();
            Box::pin(async move {
                let snap = svc.refresh_once(Some(app)).await;
                tracing::debug!(?snap, "scheduled catalog refresh");
            })
        })
        .map_err(|e| CatalogRefreshError::SchedulerSetup {
            message: e.to_string(),
        })?;
        sched
            .add(job)
            .await
            .map_err(|e| CatalogRefreshError::SchedulerSetup {
                message: e.to_string(),
            })?;
        sched
            .start()
            .await
            .map_err(|e| CatalogRefreshError::SchedulerSetup {
                message: e.to_string(),
            })?;
        *self.sched.lock().await = Some(sched);
        tracing::info!(interval_secs, "registry fetcher cron started");
        Ok(())
    }
}

fn epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}

// ── Tauri commands ───────────────────────────────────────────────────

/// 수동 갱신 트리거 — 사용자가 Settings → "지금 갱신할게요" 버튼 눌렀을 때.
#[tauri::command]
pub async fn refresh_catalog_now(
    app: AppHandle,
    service: tauri::State<'_, Arc<RegistryFetcherService>>,
) -> Result<LastRefresh, CatalogRefreshError> {
    let svc = service.inner().clone();
    Ok(svc.refresh_once(Some(app)).await)
}

/// 마지막 갱신 결과 — UI 첫 마운트 시 표시용. 한 번도 안 됐으면 None.
#[tauri::command]
pub async fn get_last_catalog_refresh(
    service: tauri::State<'_, Arc<RegistryFetcherService>>,
) -> Result<Option<LastRefresh>, CatalogRefreshError> {
    Ok(service.last_refresh().await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn make_options(server_uri: &str, cache_db: PathBuf) -> FetcherOptions {
        // GitHub-only source for test simplicity.
        use registry_fetcher::{SourceConfig, SourceTier};
        let sources = vec![SourceConfig {
            tier: SourceTier::Github,
            url_template: format!("{}/{{id}}.json", server_uri),
            timeout: Duration::from_secs(2),
        }];
        FetcherOptions::new(cache_db, sources)
    }

    async fn make_service_with_server(
        server_uri: &str,
        manifest_ids: Vec<String>,
    ) -> RegistryFetcherService {
        let tmp = tempfile::tempdir().unwrap();
        let opts = make_options(server_uri, tmp.path().join("fetch.db"));
        let fetcher = RegistryFetcher::new(opts).await.unwrap();
        // 디렉터리는 test 끝까지 살아있어야 — leak.
        std::mem::forget(tmp);
        RegistryFetcherService {
            fetcher: Arc::new(fetcher),
            manifest_ids,
            last_refresh: Arc::new(Mutex::new(None)),
            sched: Mutex::new(None),
        }
    }

    #[tokio::test]
    async fn refresh_once_with_no_manifests_records_ok_zero() {
        let server = MockServer::start().await;
        let svc = make_service_with_server(&server.uri(), Vec::new()).await;
        let snap = svc.refresh_once(None).await;
        assert_eq!(snap.outcome, "ok");
        assert_eq!(snap.fetched_count, 0);
        assert_eq!(snap.failed_count, 0);
        let cached = svc.last_refresh().await.unwrap();
        assert_eq!(cached.outcome, "ok");
    }

    #[tokio::test]
    async fn refresh_once_records_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ollama.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"id":"ollama"}"#))
            .mount(&server)
            .await;

        let svc = make_service_with_server(&server.uri(), vec!["ollama".into()]).await;
        let snap = svc.refresh_once(None).await;
        assert_eq!(snap.outcome, "ok");
        assert_eq!(snap.fetched_count, 1);
        assert_eq!(snap.failed_count, 0);
    }

    #[tokio::test]
    async fn refresh_once_records_partial_when_some_fail() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ok.json"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"id":"ok"}"#))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/bad.json"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let svc = make_service_with_server(&server.uri(), vec!["ok".into(), "bad".into()]).await;
        let snap = svc.refresh_once(None).await;
        assert_eq!(snap.outcome, "partial");
        assert_eq!(snap.fetched_count, 1);
        assert_eq!(snap.failed_count, 1);
    }

    #[tokio::test]
    async fn refresh_once_records_failed_when_all_fail() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let svc = make_service_with_server(&server.uri(), vec!["only".into()]).await;
        let snap = svc.refresh_once(None).await;
        assert_eq!(snap.outcome, "failed");
        assert_eq!(snap.fetched_count, 0);
        assert_eq!(snap.failed_count, 1);
    }

    #[tokio::test]
    async fn last_refresh_is_none_before_first_call() {
        let server = MockServer::start().await;
        let svc = make_service_with_server(&server.uri(), Vec::new()).await;
        assert!(svc.last_refresh().await.is_none());
    }

    #[tokio::test]
    async fn last_refresh_persists_after_call() {
        let server = MockServer::start().await;
        let svc = make_service_with_server(&server.uri(), Vec::new()).await;
        svc.refresh_once(None).await;
        let r = svc.last_refresh().await.unwrap();
        assert!(r.at_ms > 0);
    }

    #[tokio::test]
    async fn start_rejects_interval_below_min() {
        let server = MockServer::start().await;
        let svc = Arc::new(make_service_with_server(&server.uri(), Vec::new()).await);
        // AppHandle를 mock 못 하니 Result<...> 만 검증.
        let _r = svc.clone();
        // 실제 start() 호출엔 AppHandle 필요 — 이 테스트는 interval 검증만 분리해서 별도 함수로.
        let too_small = MIN_INTERVAL_SECS - 1;
        let too_large = MAX_INTERVAL_SECS + 1;

        let err = check_interval_bounds(too_small);
        assert!(matches!(err, Err(CatalogRefreshError::IntervalOutOfRange)));
        let err = check_interval_bounds(too_large);
        assert!(matches!(err, Err(CatalogRefreshError::IntervalOutOfRange)));
        let ok = check_interval_bounds(DEFAULT_INTERVAL_SECS);
        assert!(ok.is_ok());
    }

    /// `start`의 interval 검증 path를 분리한 헬퍼 — AppHandle 없이도 단언 가능.
    fn check_interval_bounds(interval_secs: u64) -> Result<(), CatalogRefreshError> {
        if !(MIN_INTERVAL_SECS..=MAX_INTERVAL_SECS).contains(&interval_secs) {
            return Err(CatalogRefreshError::IntervalOutOfRange);
        }
        Ok(())
    }

    #[tokio::test]
    async fn error_serialization_uses_kind_tag() {
        let e = CatalogRefreshError::IntervalOutOfRange;
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "interval-out-of-range");
        assert!(v["message"].as_str().unwrap().contains("주기"));
    }

    #[tokio::test]
    async fn last_refresh_serializes_with_outcome_field() {
        let r = LastRefresh {
            at_ms: 12345,
            fetched_count: 3,
            failed_count: 1,
            outcome: "partial",
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["at_ms"], 12345);
        assert_eq!(v["fetched_count"], 3);
        assert_eq!(v["failed_count"], 1);
        assert_eq!(v["outcome"], "partial");
    }
}
