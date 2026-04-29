//! `run_install` 통합 테스트 — wiremock + tempdir로 실제 다운로드 경로를 타고
//! DownloadEvent → InstallEvent::Download bridge가 정상 동작하는지 확인.
//!
//! 주의: download_and_run 또는 download_and_extract method를 만들려면 실제 실행 가능한 binary가
//! 필요하지만, 이 테스트는 download 단계까지만 보고 그 이후 spawn 실패는 Failed 이벤트로
//! 흘러도 OK — `Started` + `Download(*)` + `Failed`/`Finished` 시퀀스만 검증.

use std::sync::{Arc, Mutex};

use installer::{
    run_install, DownloadEvent, InstallEvent, InstallRunnerError, InstallSink, InstallSinkClosed,
};
use runtime_detector::manifest::AppManifest;
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(Default)]
struct CapturedSink {
    events: Mutex<Vec<InstallEvent>>,
}

impl InstallSink for CapturedSink {
    fn emit(&self, event: InstallEvent) -> Result<(), InstallSinkClosed> {
        self.events.lock().unwrap().push(event);
        Ok(())
    }
}

impl CapturedSink {
    fn snapshot(&self) -> Vec<InstallEvent> {
        self.events.lock().unwrap().clone()
    }
}

fn make_payload(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 251) as u8).collect()
}

fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

fn build_download_and_run_manifest(url: &str, sha256: &str) -> AppManifest {
    // download_and_run을 모든 OS에 부여 — 다운로드는 OS 무관, 이후 spawn은 Win에선 비-exe라 Failed가 정상.
    let json = format!(
        r#"{{
            "schema_version": 1,
            "id": "fake-runner",
            "display_name": "FakeRunner",
            "license": "MIT",
            "detect": [],
            "install": {{
                "windows": {{
                    "method": "download_and_run",
                    "url_template": "{url}",
                    "sha256": "{sha256}",
                    "args": [],
                    "timeout_seconds": 5
                }},
                "macos": {{
                    "method": "download_and_run",
                    "url_template": "{url}",
                    "sha256": "{sha256}",
                    "args": [],
                    "timeout_seconds": 5
                }},
                "linux": {{
                    "method": "download_and_run",
                    "url_template": "{url}",
                    "sha256": "{sha256}",
                    "args": [],
                    "timeout_seconds": 5
                }}
            }}
        }}"#
    );
    serde_json::from_str(&json).expect("manifest parse")
}

#[tokio::test]
async fn run_install_download_phase_emits_started_and_download_events() {
    let payload = make_payload(8 * 1024); // 8KB — 적어도 1회 progress 가능.
    let sha = sha256_hex(&payload);

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/installer.exe"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(payload.clone()))
        .mount(&server)
        .await;

    let url = format!("{}/installer.exe", server.uri());
    let manifest = build_download_and_run_manifest(&url, &sha);

    let cancel = CancellationToken::new();
    let sink: Arc<CapturedSink> = Arc::new(CapturedSink::default());
    let dir = TempDir::new().unwrap();

    // download_and_run은 download 후 .exe spawn — 실제 .exe가 아니므로 Win에선 Failed가 정상,
    // Linux/mac에선 chmod 후 exec attempt에서 Failed가 정상. 우리는 이벤트 시퀀스만 본다.
    let _ = run_install(&manifest, dir.path(), &cancel, sink.clone()).await;

    let events = sink.snapshot();
    assert!(
        !events.is_empty(),
        "최소 Started는 emit돼야 함; got {events:?}"
    );

    // 1) Started.
    assert!(
        matches!(events.first(), Some(InstallEvent::Started { id, .. }) if id == "fake-runner")
    );

    // 2) 적어도 한 번의 Download(Started) 이벤트.
    let saw_download_started = events.iter().any(|e| {
        matches!(
            e,
            InstallEvent::Download {
                download: DownloadEvent::Started { .. }
            }
        )
    });
    assert!(saw_download_started, "Download::Started가 보여야 함");

    // 3) Download(Verified) — sha256 일치.
    let saw_verified = events.iter().any(|e| {
        matches!(
            e,
            InstallEvent::Download {
                download: DownloadEvent::Verified { .. }
            }
        )
    });
    assert!(saw_verified, "Download::Verified가 보여야 함");

    // 4) Download(Finished) — atomic rename 성공.
    let saw_finished = events.iter().any(|e| {
        matches!(
            e,
            InstallEvent::Download {
                download: DownloadEvent::Finished { .. }
            }
        )
    });
    assert!(saw_finished, "Download::Finished가 보여야 함");

    // 5) 마지막은 Finished 또는 Failed (spawn 결과에 따라).
    assert!(
        matches!(
            events.last(),
            Some(InstallEvent::Finished { .. }) | Some(InstallEvent::Failed { .. })
        ),
        "마지막 이벤트는 Finished 또는 Failed여야 함; got {:?}",
        events.last()
    );
}

#[tokio::test]
async fn run_install_download_404_yields_failed_event() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/missing.exe"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    // 가짜 sha (어차피 다운로드 실패).
    let manifest = build_download_and_run_manifest(
        &format!("{}/missing.exe", server.uri()),
        "0000000000000000000000000000000000000000000000000000000000000000",
    );

    let cancel = CancellationToken::new();
    let sink: Arc<CapturedSink> = Arc::new(CapturedSink::default());
    let dir = TempDir::new().unwrap();

    let r = run_install(&manifest, dir.path(), &cancel, sink.clone()).await;
    assert!(matches!(r, Err(InstallRunnerError::Action(_))));

    let events = sink.snapshot();
    // Started + Failed.
    assert!(matches!(events.first(), Some(InstallEvent::Started { .. })));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, InstallEvent::Failed { code, .. } if code == "download-failed")),
        "code=download-failed Failed event 필요; got {events:?}"
    );
}

#[tokio::test]
async fn run_install_cancellation_during_download_yields_cancelled() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/slow.exe"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(vec![0u8; 1024 * 1024])
                .set_delay(std::time::Duration::from_millis(2000)),
        )
        .mount(&server)
        .await;

    // sha는 미사용 (cancel 발생). 빈 string 안 됨 — 임의 64-hex.
    let manifest = build_download_and_run_manifest(
        &format!("{}/slow.exe", server.uri()),
        "0000000000000000000000000000000000000000000000000000000000000000",
    );

    let cancel = CancellationToken::new();
    let sink: Arc<CapturedSink> = Arc::new(CapturedSink::default());
    let dir = TempDir::new().unwrap();

    // 100ms 후 cancel.
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        cancel_clone.cancel();
    });

    let r = run_install(&manifest, dir.path(), &cancel, sink.clone()).await;
    // Cancelled 또는 Action(Cancelled).
    assert!(
        matches!(
            r,
            Err(InstallRunnerError::Action(
                installer::ActionError::Cancelled
            )) | Err(InstallRunnerError::Action(
                installer::ActionError::Download(_)
            ))
        ),
        "cancel 시 Cancelled 또는 Download 에러 기대; got {r:?}"
    );

    let events = sink.snapshot();
    assert!(matches!(events.first(), Some(InstallEvent::Started { .. })));
    // 마지막은 Cancelled 또는 Failed(다운로드 도중 끊김).
    assert!(
        matches!(
            events.last(),
            Some(InstallEvent::Cancelled) | Some(InstallEvent::Failed { .. })
        ),
        "cancel 후 마지막 이벤트는 Cancelled 또는 Failed; got {:?}",
        events.last()
    );
}
