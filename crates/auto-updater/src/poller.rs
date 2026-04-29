//! 폴러 — 6h interval로 `UpdateSource::latest_version`을 호출하고 outdated일 때만 콜백.
//!
//! 정책 (ADR-0025, phase-6p-updater-pipelines-decision.md §4·§5):
//! - `tokio_util::sync::CancellationToken` 협력 cancel — `run` 루프는 매 sleep 사이에 cancel 체크.
//! - 콜백은 outdated 감지 시점에만 실행. 같은 버전을 반복 감지해도 매 호출마다 emit하지 않도록
//!   "마지막으로 알린 버전"을 보유 (idempotency).
//! - source 실패는 `tracing::warn!`로 로그만 + 다음 polling 간격까지 대기 (chain 중단 X).
//! - `check_once`는 단발성 호출 — Tauri IPC `check_for_update` 핸들러에서 직접 사용.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::error::UpdaterError;
use crate::source::{ReleaseInfo, UpdateSource};
use crate::version::is_outdated;

/// 6시간 — Phase 6'.a 기본값. 사용자 설정으로 1h~24h 범위 조정 가능.
pub const DEFAULT_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// 폴러 — `Arc<dyn UpdateSource>` + 현재 버전 + interval.
///
/// `run`은 cancel 토큰 신호 또는 source가 영구 실패할 때까지 무한 루프.
/// `check_once`는 단발 — `None` = up-to-date, `Some` = 최신 릴리스가 outdated.
pub struct Poller {
    source: Arc<dyn UpdateSource>,
    current_version: String,
    interval: Duration,
    /// 마지막으로 콜백을 emit한 outdated 버전. 같은 outdated 버전을 반복 emit 안 함.
    last_notified: Arc<Mutex<Option<String>>>,
}

impl Poller {
    pub fn new(source: Arc<dyn UpdateSource>, current_version: impl Into<String>) -> Self {
        Self::with_interval(source, current_version, DEFAULT_INTERVAL)
    }

    pub fn with_interval(
        source: Arc<dyn UpdateSource>,
        current_version: impl Into<String>,
        interval: Duration,
    ) -> Self {
        Self {
            source,
            current_version: current_version.into(),
            interval,
            last_notified: Arc::new(Mutex::new(None)),
        }
    }

    pub fn current_version(&self) -> &str {
        &self.current_version
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// 단발 outdated 체크. UI "지금 확인할게요" 버튼이 호출.
    ///
    /// `Ok(Some(release))` = outdated → UI에 알림. `Ok(None)` = up-to-date.
    /// `Err` = source 실패 또는 invalid version (caller가 한국어 토스트로 노출).
    pub async fn check_once(&self) -> Result<Option<ReleaseInfo>, UpdaterError> {
        let release = self.source.latest_version().await?;
        if is_outdated(&self.current_version, &release.version)? {
            Ok(Some(release))
        } else {
            Ok(None)
        }
    }

    /// 무한 polling 루프 — cancel 신호까지 동작.
    ///
    /// `on_update`는 outdated 감지 + 같은 버전을 아직 알리지 않은 경우에만 호출.
    /// 호출자는 콜백 안에서 long-running 작업 금지 (다음 polling cycle을 막음).
    pub async fn run<F>(&self, on_update: F, cancel: CancellationToken)
    where
        F: Fn(ReleaseInfo) + Send + Sync,
    {
        // Phase 8'.a.3 — 기존 호출자 호환을 위해 cycle hook 없이 위임.
        self.run_with_lifecycle(on_update, |_| {}, cancel).await;
    }

    /// `run`의 확장형 — 매 cycle source 호출이 *성공*했을 때 `on_cycle_success(version)` 추가 콜백.
    ///
    /// 정책 (Phase 8'.a.3 last_check_iso 일관성):
    /// - source 호출이 OK면 outdated/uptodate 둘 다 `on_cycle_success` 호출 (last_check 갱신 의도).
    /// - source가 `Err`(network 등)면 `on_cycle_success` 호출 안 함 (실패는 "확인 못 함").
    /// - `on_update`는 기존 의미 그대로 — outdated + dedup 통과 시에만.
    /// - cancel은 매 cycle 시작과 sleep 중 양쪽에서 listen.
    pub async fn run_with_lifecycle<U, C>(
        &self,
        on_update: U,
        on_cycle_success: C,
        cancel: CancellationToken,
    ) where
        U: Fn(ReleaseInfo) + Send + Sync,
        C: Fn(&str) + Send + Sync,
    {
        tracing::info!(
            current = %self.current_version,
            interval_secs = self.interval.as_secs(),
            "자동 갱신 폴러를 시작했어요"
        );
        loop {
            // 매 cycle 시작 시 cancel 체크.
            if cancel.is_cancelled() {
                tracing::info!("자동 갱신 폴러를 멈췄어요 (cancel)");
                return;
            }

            // 한 번 polling — `poll_cycle`은 source 호출 결과 raw release를 함께 노출.
            match self.poll_cycle().await {
                Ok((release, outdated_to_emit)) => {
                    on_cycle_success(release.version.as_str());
                    if let Some(r) = outdated_to_emit {
                        tracing::info!(version = %r.version, "새 버전을 찾았어요");
                        on_update(r);
                    } else {
                        tracing::debug!("새 버전이 없어요");
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "업데이트 확인이 실패했어요. 다음 주기에 다시 시도할게요");
                    // Failed는 last_check 갱신 안 함 — caller policy.
                }
            }

            // sleep — cancel을 동시에 listen해서 즉시 깨어남.
            tokio::select! {
                _ = tokio::time::sleep(self.interval) => {}
                _ = cancel.cancelled() => {
                    tracing::info!("자동 갱신 폴러를 멈췄어요 (sleep cancel)");
                    return;
                }
            }
        }
    }

    /// 한 cycle 실행 — source 호출 + dedup. 성공 시 `(release, outdated_to_emit)` 반환:
    /// - `release` = source가 반환한 가장 최신 release (UpToDate 시도 포함).
    /// - `outdated_to_emit` = `Some` (outdated + dedup 통과) 또는 `None` (uptodate 또는 dedup 차단).
    async fn poll_cycle(&self) -> Result<(ReleaseInfo, Option<ReleaseInfo>), UpdaterError> {
        let release = self.source.latest_version().await?;
        if !is_outdated(&self.current_version, &release.version)? {
            // up-to-date — last_notified 클리어 (다시 outdated 등장 시 재알림 가능).
            let mut last = self.last_notified.lock().await;
            *last = None;
            return Ok((release, None));
        }

        // outdated. dedup.
        let mut last = self.last_notified.lock().await;
        if last.as_deref() == Some(release.version.as_str()) {
            return Ok((release, None));
        }
        *last = Some(release.version.clone());
        Ok((release.clone(), Some(release)))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::source::MockSource;

    fn release(version: &str) -> ReleaseInfo {
        ReleaseInfo {
            version: version.to_string(),
            published_at: time::OffsetDateTime::UNIX_EPOCH,
            url: "https://example.com/r".into(),
            notes: None,
        }
    }

    #[tokio::test]
    async fn check_once_returns_some_when_outdated() {
        let mock = Arc::new(MockSource::with_release(release("1.1.0")));
        let poller = Poller::new(mock, "1.0.0");
        let r = poller.check_once().await.unwrap();
        assert!(r.is_some());
        assert_eq!(r.unwrap().version, "1.1.0");
    }

    #[tokio::test]
    async fn check_once_returns_none_when_same_version() {
        let mock = Arc::new(MockSource::with_release(release("1.0.0")));
        let poller = Poller::new(mock, "1.0.0");
        let r = poller.check_once().await.unwrap();
        assert!(r.is_none());
    }

    #[tokio::test]
    async fn check_once_returns_none_when_downgrade() {
        let mock = Arc::new(MockSource::with_release(release("0.9.0")));
        let poller = Poller::new(mock, "1.0.0");
        let r = poller.check_once().await.unwrap();
        assert!(r.is_none());
    }

    #[tokio::test]
    async fn check_once_propagates_no_releases() {
        let mock = Arc::new(MockSource::new());
        let poller = Poller::new(mock, "1.0.0");
        let err = poller.check_once().await.unwrap_err();
        assert!(matches!(err, UpdaterError::NoReleases));
    }

    #[tokio::test]
    async fn check_once_propagates_invalid_version() {
        let mock = Arc::new(MockSource::with_release(release("not.a.version")));
        let poller = Poller::new(mock, "1.0.0");
        let err = poller.check_once().await.unwrap_err();
        assert!(matches!(err, UpdaterError::InvalidVersion(_)));
    }

    #[tokio::test(start_paused = true)]
    async fn run_cancellable_immediately() {
        let mock = Arc::new(MockSource::with_release(release("1.1.0")));
        let poller = Poller::with_interval(mock, "1.0.0", Duration::from_secs(60));
        let cancel = CancellationToken::new();
        let invoked = Arc::new(AtomicUsize::new(0));
        let invoked_clone = invoked.clone();

        cancel.cancel(); // 시작 전 cancel.
        poller
            .run(
                move |_| {
                    invoked_clone.fetch_add(1, Ordering::SeqCst);
                },
                cancel,
            )
            .await;

        // 시작 전 cancel이라 콜백 emit 0회.
        assert_eq!(invoked.load(Ordering::SeqCst), 0);
    }

    /// 스케줄러 슬롯을 비워주는 helper — `start_paused` 환경에서 spawned task가
    /// 큐에 쌓인 future들을 모두 polling하도록 강제.
    async fn drain_scheduler() {
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test(start_paused = true)]
    async fn run_calls_callback_on_outdated_then_dedups() {
        let mock = Arc::new(MockSource::with_release(release("1.1.0")));
        let poller = Poller::with_interval(mock.clone(), "1.0.0", Duration::from_secs(60));
        let cancel = CancellationToken::new();
        let invoked = Arc::new(AtomicUsize::new(0));
        let invoked_clone = invoked.clone();
        let cancel_for_task = cancel.clone();

        let handle = tokio::spawn(async move {
            poller
                .run(
                    move |_| {
                        invoked_clone.fetch_add(1, Ordering::SeqCst);
                    },
                    cancel_for_task,
                )
                .await;
        });

        // 첫 polling 처리 (initial poll).
        drain_scheduler().await;
        // interval 4번 advance — dedup 때문에 같은 버전은 1번만 emit.
        for _ in 0..4 {
            tokio::time::advance(Duration::from_secs(60)).await;
            drain_scheduler().await;
        }
        cancel.cancel();
        handle.await.unwrap();

        assert_eq!(
            invoked.load(Ordering::SeqCst),
            1,
            "같은 outdated 버전은 1번만 emit해야 해요"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn run_emits_again_after_version_changes() {
        let mock = Arc::new(MockSource::with_release(release("1.1.0")));
        let poller = Poller::with_interval(mock.clone(), "1.0.0", Duration::from_secs(60));
        let cancel = CancellationToken::new();
        let invoked = Arc::new(AtomicUsize::new(0));
        let invoked_clone = invoked.clone();
        let cancel_for_task = cancel.clone();

        let handle = tokio::spawn(async move {
            poller
                .run(
                    move |_| {
                        invoked_clone.fetch_add(1, Ordering::SeqCst);
                    },
                    cancel_for_task,
                )
                .await;
        });

        drain_scheduler().await;
        // 1.1.0 emit (1번째).
        tokio::time::advance(Duration::from_secs(60)).await;
        drain_scheduler().await;
        // 같은 1.1.0 dedup.
        tokio::time::advance(Duration::from_secs(60)).await;
        drain_scheduler().await;
        // 새 버전 1.2.0 게시 → 다음 cycle에 다시 emit.
        mock.set_release(Some(release("1.2.0"))).await;
        tokio::time::advance(Duration::from_secs(60)).await;
        drain_scheduler().await;

        cancel.cancel();
        handle.await.unwrap();

        assert!(
            invoked.load(Ordering::SeqCst) >= 2,
            "1.1.0 + 1.2.0 각각 1번씩, 최소 2번 emit"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn run_skips_callback_on_source_error_but_continues() {
        let mock = Arc::new(MockSource::new()); // None → NoReleases.
        let poller = Poller::with_interval(mock.clone(), "1.0.0", Duration::from_secs(30));
        let cancel = CancellationToken::new();
        let invoked = Arc::new(AtomicUsize::new(0));
        let invoked_clone = invoked.clone();
        let cancel_for_task = cancel.clone();

        let handle = tokio::spawn(async move {
            poller
                .run(
                    move |_| {
                        invoked_clone.fetch_add(1, Ordering::SeqCst);
                    },
                    cancel_for_task,
                )
                .await;
        });

        // source error 동안 콜백 0회 — 루프는 계속 돔.
        drain_scheduler().await;
        for _ in 0..3 {
            tokio::time::advance(Duration::from_secs(30)).await;
            drain_scheduler().await;
        }
        // 이제 outdated release 추가 → 다음 cycle에 emit.
        mock.set_release(Some(release("1.1.0"))).await;
        tokio::time::advance(Duration::from_secs(30)).await;
        drain_scheduler().await;

        cancel.cancel();
        handle.await.unwrap();

        assert!(
            invoked.load(Ordering::SeqCst) >= 1,
            "source error 후 release가 들어오면 emit"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn run_respects_interval_between_polls() {
        let mock = Arc::new(MockSource::with_release(release("0.9.0"))); // up-to-date.
        let poller = Poller::with_interval(mock.clone(), "1.0.0", Duration::from_secs(120));
        let cancel = CancellationToken::new();
        let cancel_for_task = cancel.clone();
        let mock_for_task = mock.clone();

        let handle = tokio::spawn(async move {
            poller.run(|_| {}, cancel_for_task).await;
        });

        drain_scheduler().await;
        // 120초 미만에는 1번 호출만.
        tokio::time::advance(Duration::from_secs(60)).await;
        drain_scheduler().await;
        let count_after_60s = mock_for_task.call_count().await;
        assert_eq!(count_after_60s, 1, "interval=120s 내엔 1번만 호출");

        // 추가 120초 → 2번째 호출 (총 2).
        tokio::time::advance(Duration::from_secs(120)).await;
        drain_scheduler().await;
        let count_after_180s = mock_for_task.call_count().await;
        assert!(
            count_after_180s >= 2,
            "interval 1회 경과 후엔 최소 2번 호출"
        );

        cancel.cancel();
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn poller_exposes_interval_and_version() {
        let mock = Arc::new(MockSource::new());
        let poller = Poller::with_interval(mock, "0.0.1", Duration::from_secs(42));
        assert_eq!(poller.current_version(), "0.0.1");
        assert_eq!(poller.interval(), Duration::from_secs(42));
    }

    #[test]
    fn default_interval_is_six_hours() {
        assert_eq!(DEFAULT_INTERVAL, Duration::from_secs(6 * 60 * 60));
    }

    // ── Phase 8'.a.3 — run_with_lifecycle: cycle-success hook ────────

    #[tokio::test(start_paused = true)]
    async fn run_with_lifecycle_cycle_hook_fires_on_uptodate() {
        // up-to-date 응답이라도 source 호출이 OK이면 cycle hook 호출.
        let mock = Arc::new(MockSource::with_release(release("1.0.0")));
        let poller = Poller::with_interval(mock.clone(), "1.0.0", Duration::from_secs(60));
        let cancel = CancellationToken::new();
        let updates = Arc::new(AtomicUsize::new(0));
        let cycles = Arc::new(AtomicUsize::new(0));
        let updates_clone = updates.clone();
        let cycles_clone = cycles.clone();
        let cancel_for_task = cancel.clone();

        let handle = tokio::spawn(async move {
            poller
                .run_with_lifecycle(
                    move |_| {
                        updates_clone.fetch_add(1, Ordering::SeqCst);
                    },
                    move |_| {
                        cycles_clone.fetch_add(1, Ordering::SeqCst);
                    },
                    cancel_for_task,
                )
                .await;
        });

        drain_scheduler().await;
        // 첫 cycle — UpToDate 분기.
        assert_eq!(updates.load(Ordering::SeqCst), 0);
        assert!(
            cycles.load(Ordering::SeqCst) >= 1,
            "UpToDate에서도 cycle hook 호출돼야 해요"
        );

        cancel.cancel();
        handle.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn run_with_lifecycle_cycle_hook_does_not_fire_on_source_error() {
        // source가 에러(NoReleases)를 반환하면 cycle hook 호출 안 함.
        let mock = Arc::new(MockSource::new()); // None → NoReleases.
        let poller = Poller::with_interval(mock.clone(), "1.0.0", Duration::from_secs(60));
        let cancel = CancellationToken::new();
        let cycles = Arc::new(AtomicUsize::new(0));
        let cycles_clone = cycles.clone();
        let cancel_for_task = cancel.clone();

        let handle = tokio::spawn(async move {
            poller
                .run_with_lifecycle(
                    |_| {},
                    move |_| {
                        cycles_clone.fetch_add(1, Ordering::SeqCst);
                    },
                    cancel_for_task,
                )
                .await;
        });

        drain_scheduler().await;
        // 여러 interval 진행해도 source는 계속 fail.
        for _ in 0..3 {
            tokio::time::advance(Duration::from_secs(60)).await;
            drain_scheduler().await;
        }
        assert_eq!(
            cycles.load(Ordering::SeqCst),
            0,
            "source 실패 시 cycle hook은 호출되면 안 돼요"
        );

        cancel.cancel();
        handle.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn run_with_lifecycle_cycle_hook_fires_on_outdated() {
        // outdated 응답에서도 cycle hook 호출 (source 호출 자체가 OK이므로).
        let mock = Arc::new(MockSource::with_release(release("1.1.0")));
        let poller = Poller::with_interval(mock.clone(), "1.0.0", Duration::from_secs(60));
        let cancel = CancellationToken::new();
        let updates = Arc::new(AtomicUsize::new(0));
        let cycles = Arc::new(AtomicUsize::new(0));
        let updates_clone = updates.clone();
        let cycles_clone = cycles.clone();
        let cancel_for_task = cancel.clone();

        let handle = tokio::spawn(async move {
            poller
                .run_with_lifecycle(
                    move |_| {
                        updates_clone.fetch_add(1, Ordering::SeqCst);
                    },
                    move |_| {
                        cycles_clone.fetch_add(1, Ordering::SeqCst);
                    },
                    cancel_for_task,
                )
                .await;
        });

        drain_scheduler().await;
        assert!(updates.load(Ordering::SeqCst) >= 1);
        assert!(
            cycles.load(Ordering::SeqCst) >= 1,
            "outdated에서도 cycle hook이 호출돼야 해요"
        );

        cancel.cancel();
        handle.await.unwrap();
    }
}
