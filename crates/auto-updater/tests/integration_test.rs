//! auto-updater 통합 테스트.
//!
//! Phase 6'.a §F — `MockSource` ↔ `Poller` end-to-end + cancel + dedup invariant.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use auto_updater::{MockSource, Poller, ReleaseInfo, UpdaterError};
use tokio_util::sync::CancellationToken;

fn release(version: &str) -> ReleaseInfo {
    ReleaseInfo {
        version: version.to_string(),
        published_at: time::OffsetDateTime::UNIX_EPOCH,
        url: format!("https://example.com/{version}"),
        notes: Some(format!("changelog for {version}")),
    }
}

/// `start_paused = true` 환경에서 spawned task가 큐의 future를 모두 polling하도록.
async fn drain_scheduler() {
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
}

#[tokio::test]
async fn end_to_end_outdated_returns_some() {
    let mock = MockSource::new();
    mock.set_release(Some(release("2.0.0"))).await;

    let poller = Poller::new(Arc::new(mock), "1.0.0");
    let result = poller.check_once().await.unwrap();
    assert!(result.is_some());
    let r = result.unwrap();
    assert_eq!(r.version, "2.0.0");
    assert!(r.notes.unwrap().contains("changelog"));
}

#[tokio::test]
async fn end_to_end_same_version_returns_none() {
    let mock = MockSource::new();
    mock.set_release(Some(release("1.0.0"))).await;

    let poller = Poller::new(Arc::new(mock), "1.0.0");
    let result = poller.check_once().await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn end_to_end_no_releases_returns_error() {
    let mock = MockSource::new();
    // set_release 호출하지 않음 → release == None → NoReleases.
    let poller = Poller::new(Arc::new(mock), "1.0.0");
    let err = poller.check_once().await.unwrap_err();
    assert!(matches!(err, UpdaterError::NoReleases));
    let msg = format!("{err}");
    assert!(msg.contains("릴리스"));
}

#[tokio::test(start_paused = true)]
async fn poller_cancellation_mid_loop() {
    let mock = Arc::new(MockSource::new());
    mock.set_release(Some(release("1.5.0"))).await;
    let poller = Poller::with_interval(mock.clone(), "1.0.0", Duration::from_secs(60));

    let cancel = CancellationToken::new();
    let cancel_for_task = cancel.clone();
    let invoked = Arc::new(AtomicUsize::new(0));
    let invoked_clone = invoked.clone();

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

    // 첫 polling (1.5.0 emit).
    drain_scheduler().await;
    tokio::time::advance(Duration::from_secs(30)).await;
    drain_scheduler().await;

    // mid-loop cancel.
    cancel.cancel();
    handle.await.unwrap();

    // 콜백 1번만 emit (1.5.0). cancel 후엔 더 이상 polling 안 함.
    assert_eq!(invoked.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn poller_emits_callback_exactly_once_per_outdated_event() {
    let mock = Arc::new(MockSource::new());
    mock.set_release(Some(release("1.1.0"))).await;
    let poller = Poller::with_interval(mock.clone(), "1.0.0", Duration::from_secs(60));

    let cancel = CancellationToken::new();
    let cancel_for_task = cancel.clone();
    let invoked = Arc::new(AtomicUsize::new(0));
    let invoked_clone = invoked.clone();

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

    // 4 cycle 동안 같은 1.1.0 release → dedup으로 1번만 emit.
    drain_scheduler().await;
    for _ in 0..4 {
        tokio::time::advance(Duration::from_secs(60)).await;
        drain_scheduler().await;
    }

    cancel.cancel();
    handle.await.unwrap();

    assert_eq!(
        invoked.load(Ordering::SeqCst),
        1,
        "같은 outdated 버전은 polling 횟수와 관계없이 정확히 1번 emit"
    );
}

#[tokio::test]
async fn end_to_end_release_info_serde_round_trip() {
    let r = release("3.0.0-rc.1");
    let s = serde_json::to_string(&r).unwrap();
    let back: ReleaseInfo = serde_json::from_str(&s).unwrap();
    assert_eq!(r, back);
}

#[tokio::test]
async fn end_to_end_invalid_version_propagates() {
    let mock = MockSource::new();
    mock.set_release(Some(release("invalid"))).await;

    let poller = Poller::new(Arc::new(mock), "1.0.0");
    let err = poller.check_once().await.unwrap_err();
    assert!(matches!(err, UpdaterError::InvalidVersion(_)));
    let msg = format!("{err}");
    assert!(msg.contains("invalid"));
}
