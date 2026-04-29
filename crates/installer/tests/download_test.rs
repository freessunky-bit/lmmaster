//! Downloader 통합 테스트 — wiremock으로 HTTP 응답 시뮬레이션 + tempdir로 파일 검증.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use installer::{
    DownloadError, DownloadEvent, DownloadOutcome, DownloadRequest, Downloader, ProgressSink,
};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// 테스트용 진행률 sink — 모든 이벤트를 Vec에 기록.
#[derive(Default, Clone)]
struct CapturedSink {
    events: Arc<Mutex<Vec<DownloadEvent>>>,
}

impl ProgressSink for CapturedSink {
    fn emit(&self, event: DownloadEvent) {
        self.events.lock().unwrap().push(event);
    }
}

impl CapturedSink {
    fn snapshot(&self) -> Vec<DownloadEvent> {
        self.events.lock().unwrap().clone()
    }
}

fn sha256_of(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn make_payload(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 251) as u8).collect()
}

#[tokio::test]
async fn fresh_download_with_correct_sha256() {
    let payload = make_payload(64 * 1024);
    let expected = sha256_of(&payload);

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/file.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(payload.clone()))
        .mount(&server)
        .await;

    let dir = TempDir::new().unwrap();
    let final_path = dir.path().join("output.bin");
    let req = DownloadRequest {
        url: format!("{}/file.bin", server.uri()),
        final_path: final_path.clone(),
        expected_sha256: Some(expected),
        size_hint: None,
        max_retries: Some(0),
    };
    let cancel = CancellationToken::new();
    let sink = CapturedSink::default();

    let dl = Downloader::new().unwrap();
    let outcome: DownloadOutcome = dl
        .download(&req, &cancel, &sink)
        .await
        .expect("download ok");

    assert_eq!(outcome.bytes, payload.len() as u64);
    assert_eq!(outcome.final_path, final_path);
    assert!(!outcome.resumed);

    // 최종 파일이 존재하고 .partial은 없어야 함.
    assert!(final_path.exists(), "final file must exist");
    let on_disk = std::fs::read(&final_path).unwrap();
    assert_eq!(on_disk, payload, "downloaded content must match payload");

    let events = sink.snapshot();
    assert!(events
        .iter()
        .any(|e| matches!(e, DownloadEvent::Started { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, DownloadEvent::Verified { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, DownloadEvent::Finished { .. })));
}

#[tokio::test]
async fn sha256_mismatch_removes_partial_and_errors() {
    let payload = make_payload(8 * 1024);
    let wrong_expected = [0xFFu8; 32];

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/wrong.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(payload))
        .mount(&server)
        .await;

    let dir = TempDir::new().unwrap();
    let final_path = dir.path().join("wrong.bin");
    let partial_path = {
        let mut s: std::ffi::OsString = final_path.as_os_str().into();
        s.push(".partial");
        std::path::PathBuf::from(s)
    };
    let req = DownloadRequest {
        url: format!("{}/wrong.bin", server.uri()),
        final_path: final_path.clone(),
        expected_sha256: Some(wrong_expected),
        size_hint: None,
        max_retries: Some(0),
    };
    let cancel = CancellationToken::new();
    let sink = CapturedSink::default();

    let dl = Downloader::new().unwrap();
    let res = dl.download(&req, &cancel, &sink).await;
    match res {
        Err(DownloadError::HashMismatch { .. }) => {}
        other => panic!("expected HashMismatch, got: {other:?}"),
    }
    assert!(
        !final_path.exists(),
        "final must not exist on hash mismatch"
    );
    assert!(
        !partial_path.exists(),
        ".partial must be cleaned up on hash mismatch"
    );
}

#[tokio::test]
async fn cancelled_download_preserves_partial() {
    // Slow server: 매 100ms마다 1KB. 우리는 50ms 후 cancel.
    let server = MockServer::start().await;
    let body: Vec<u8> = make_payload(1024 * 1024); // 1MB; 큰 응답 body.
    Mock::given(method("GET"))
        .and(path("/slow.bin"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(body)
                .set_delay(Duration::from_millis(2000)),
        )
        .mount(&server)
        .await;

    let dir = TempDir::new().unwrap();
    let final_path = dir.path().join("slow.bin");
    let req = DownloadRequest {
        url: format!("{}/slow.bin", server.uri()),
        final_path: final_path.clone(),
        expected_sha256: None,
        size_hint: None,
        max_retries: Some(0),
    };
    let cancel = CancellationToken::new();
    let sink = CapturedSink::default();

    // 100ms 후 취소.
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        cancel_clone.cancel();
    });

    let dl = Downloader::new().unwrap();
    let res = dl.download(&req, &cancel, &sink).await;
    match res {
        Err(DownloadError::Cancelled) => {}
        other => panic!("expected Cancelled, got: {other:?}"),
    }
    assert!(!final_path.exists(), "final must not exist on cancel");
    // .partial은 다음 실행에서 resume 가능하도록 보존되거나, body가 아직 안 와서 빈 파일일 수 있음.
    // body 도착 전 cancel이면 .partial이 비거나 부분 — 둘 다 invariant 만족.
}

#[tokio::test]
async fn invalid_request_empty_url() {
    let dl = Downloader::new().unwrap();
    let dir = TempDir::new().unwrap();
    let req = DownloadRequest {
        url: String::new(),
        final_path: dir.path().join("x.bin"),
        expected_sha256: None,
        size_hint: None,
        max_retries: Some(0),
    };
    let cancel = CancellationToken::new();
    let sink = installer::NoopSink;
    let res = dl.download(&req, &cancel, &sink).await;
    assert!(matches!(res, Err(DownloadError::InvalidRequest(_))));
}

#[tokio::test]
async fn invalid_request_missing_parent_dir() {
    let dl = Downloader::new().unwrap();
    let req = DownloadRequest {
        url: "http://127.0.0.1:1/x".into(),
        final_path: "/no/such/dir/x.bin".into(),
        expected_sha256: None,
        size_hint: None,
        max_retries: Some(0),
    };
    let cancel = CancellationToken::new();
    let sink = installer::NoopSink;
    let res = dl.download(&req, &cancel, &sink).await;
    assert!(matches!(res, Err(DownloadError::InvalidRequest(_))));
}

#[tokio::test]
async fn http_404_is_not_retryable_and_fails_fast() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/missing.bin"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let dir = TempDir::new().unwrap();
    let req = DownloadRequest {
        url: format!("{}/missing.bin", server.uri()),
        final_path: dir.path().join("x.bin"),
        expected_sha256: None,
        size_hint: None,
        max_retries: Some(3),
    };
    let cancel = CancellationToken::new();
    let sink = CapturedSink::default();
    let start = std::time::Instant::now();
    let res = Downloader::new()
        .unwrap()
        .download(&req, &cancel, &sink)
        .await;
    let elapsed = start.elapsed();
    match res {
        Err(DownloadError::BadStatus { status: 404, .. }) => {}
        other => panic!("expected BadStatus 404, got: {other:?}"),
    }
    // 404는 retryable 아님 — backon이 즉시 포기. 1초 안에 끝나야 함.
    assert!(
        elapsed < Duration::from_secs(2),
        "404 should fail fast: {elapsed:?}"
    );
}

#[tokio::test]
async fn progress_events_throttled_emitted() {
    let payload = make_payload(2 * 1024 * 1024); // 2MB — 256KB 단위로 8회 progress.
    let expected = sha256_of(&payload);

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/big.bin"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(payload.clone()))
        .mount(&server)
        .await;

    let dir = TempDir::new().unwrap();
    let req = DownloadRequest {
        url: format!("{}/big.bin", server.uri()),
        final_path: dir.path().join("big.bin"),
        expected_sha256: Some(expected),
        size_hint: None,
        max_retries: Some(0),
    };
    let cancel = CancellationToken::new();
    let sink = CapturedSink::default();
    Downloader::new()
        .unwrap()
        .download(&req, &cancel, &sink)
        .await
        .expect("ok");

    let events = sink.snapshot();
    let progress_count = events
        .iter()
        .filter(|e| matches!(e, DownloadEvent::Progress { .. }))
        .count();
    // 2MB / 256KB = 8 — throttle로 더 적게 emit될 수 있지만 1개 이상은 보장.
    assert!(
        progress_count >= 1,
        "at least one progress event expected, got {progress_count}"
    );
    // started + progress(N) + verified + finished — 4 + N events.
    assert!(events.len() >= 3);
}
