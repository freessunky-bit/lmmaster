//! `run_install()` — manifest + cache_dir + cancel + sink을 받아 InstallEvent stream을 흘리며
//! ActionExecutor로 실제 설치를 수행하는 순수 함수.
//!
//! 정책 (Phase 1A.3.c 보강 리서치):
//! - Tauri command와 분리된 순수 함수 — 테스트 용이성 + IPC 의존성 격리.
//! - sink가 close됐다면 caller가 `cancel.cancel()`을 trigger하고 우리는 `InstallRunnerError::SinkClosed` 반환.
//! - DownloadEvent → InstallEvent::Download wrapping은 본 모듈 내 brigde sink로 처리.
//! - Extract 단계는 starting/done 2-checkpoint만 emit (extracting의 fine-grained progress는 후순위).
//! - method-specific 분기는 ActionExecutor가 책임. 본 모듈은 lifecycle 이벤트 + 실패 변환.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use thiserror::Error;
use tokio_util::sync::CancellationToken;

use runtime_detector::manifest::{AppManifest, PlatformInstall};

use crate::action::{ActionError, ActionExecutor, ActionOutcome};
use crate::error::DownloadError;
use crate::install_event::{
    ExtractPhase, InstallEvent, InstallSink, InstallSinkClosed, PostCheckStatus,
};
use crate::progress::{DownloadEvent, ProgressSink};

/// 설치 오케스트레이터 에러. ActionError + manifest/sink 관련 변형 추가.
#[derive(Debug, Error)]
pub enum InstallRunnerError {
    #[error("매니페스트에 install 섹션이 없어요")]
    NoInstallSection,

    #[error("이 OS에서 지원하지 않는 매니페스트예요 (현재 platform install 없음)")]
    NoPlatformBranch,

    #[error("설치 실행기 초기화 실패: {0}")]
    Init(#[from] DownloadError),

    #[error("설치 실패: {0}")]
    Action(#[from] ActionError),

    #[error("이벤트 채널이 닫혔어요 — 설치를 중단했습니다")]
    SinkClosed,
}

impl InstallRunnerError {
    /// 사용자/UI에 노출할 i18n key (kebab-case).
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoInstallSection => "no-install-section",
            Self::NoPlatformBranch => "no-platform-branch",
            Self::Init(_) => "init-failed",
            Self::Action(ActionError::Download(_)) => "download-failed",
            Self::Action(ActionError::Extract(_)) => "extract-failed",
            Self::Action(ActionError::Io(_)) => "io-error",
            Self::Action(ActionError::ExitCode(_)) => "installer-exit-nonzero",
            Self::Action(ActionError::NoExitCode) => "installer-killed",
            Self::Action(ActionError::Timeout(_)) => "installer-timeout",
            Self::Action(ActionError::Cancelled) => "cancelled",
            Self::Action(ActionError::OpenUrl { .. }) => "open-url-failed",
            Self::Action(ActionError::Unsupported(_)) => "unsupported",
            Self::Action(ActionError::InvalidSpec(_)) => "invalid-spec",
            Self::SinkClosed => "sink-closed",
        }
    }
}

/// 설치 1회 실행. caller는 `manifest.install`이 비어있지 않음을 보장하지 않아도 된다 — 본 함수가 검사.
///
/// 동작 순서:
/// 1. `Started` emit → method 결정.
/// 2. ActionExecutor::execute() 호출 — 내부 DownloadEvent는 `bridge_sink`로 InstallEvent::Download로 변환.
/// 3. 추출 단계가 있는 method면 `Extract { Starting }` / `Extract { Done }` 보강 emit.
/// 4. 정상 종료 시 `PostCheck { ... }` + `Finished { outcome }`.
/// 5. 실패 시 `Failed { code, message }` 또는 `Cancelled`.
///
/// 반환은 ActionOutcome — UI에 그대로 보여줄 수 있다 (이미 `InstallEvent::Finished`로도 emit됨).
pub async fn run_install<S: InstallSink + 'static>(
    manifest: &AppManifest,
    cache_dir: &Path,
    cancel: &CancellationToken,
    sink: Arc<S>,
) -> Result<ActionOutcome, InstallRunnerError> {
    // 1. 매니페스트 검증 + platform 분기.
    let install = manifest
        .install
        .as_ref()
        .ok_or(InstallRunnerError::NoInstallSection)?;
    let method = install
        .for_current_platform()
        .ok_or(InstallRunnerError::NoPlatformBranch)?;
    let method_name = method_short_name(method);
    let is_extract_method = matches!(method, PlatformInstall::DownloadAndExtract(_));

    // 2. Started.
    emit_or_cancel(
        sink.as_ref(),
        cancel,
        InstallEvent::Started {
            id: manifest.id.clone(),
            method: method_name.into(),
            display_name: manifest.display_name.clone(),
        },
    )?;

    // 3. ActionExecutor 준비.
    let executor = ActionExecutor::new(cache_dir.to_path_buf())?;

    // 4. DownloadEvent → InstallEvent::Download bridge sink.
    let bridge = DownloadBridge::new(sink.clone(), cancel.clone());

    // 5. Extract starting checkpoint (해당 method 한정).
    if is_extract_method {
        emit_or_cancel(
            sink.as_ref(),
            cancel,
            InstallEvent::Extract {
                phase: ExtractPhase::Starting,
                entries: 0,
                total_bytes: 0,
            },
        )?;
    }

    // 6. 실제 실행. cancel은 tokio::select!에 직접 동기화 — bridge 안에서 추가 cancel 호출 가능.
    let result = executor.execute(method, cancel, &bridge).await;

    // bridge 안에서 sink가 닫혔다면 — emit_or_cancel이 InstallRunnerError::SinkClosed로 반환됨.
    if bridge.was_closed() {
        return Err(InstallRunnerError::SinkClosed);
    }

    match result {
        Ok(outcome) => {
            // 추출 method면 done checkpoint.
            if is_extract_method {
                // ActionOutcome::Success.method가 download_and_extract.* — 우리는 entries/bytes를 모름 (현재).
                // future: extract.rs가 ExtractEvent를 emit하면 cumulative하게 누적. 이번 sub-phase는 0.
                emit_or_cancel(
                    sink.as_ref(),
                    cancel,
                    InstallEvent::Extract {
                        phase: ExtractPhase::Done,
                        entries: 0,
                        total_bytes: 0,
                    },
                )?;
            }
            // post_install_check 단계 emit — outcome에서 결과 읽어 mapping.
            let post_status = match &outcome {
                ActionOutcome::Success {
                    post_install_check_passed: Some(true),
                    ..
                } => PostCheckStatus::Passed,
                ActionOutcome::Success {
                    post_install_check_passed: Some(false),
                    ..
                } => PostCheckStatus::Failed,
                ActionOutcome::Success {
                    post_install_check_passed: None,
                    ..
                } => PostCheckStatus::Skipped,
                _ => PostCheckStatus::Skipped,
            };
            emit_or_cancel(
                sink.as_ref(),
                cancel,
                InstallEvent::PostCheck {
                    status: post_status,
                },
            )?;
            // Finished — 단말 이벤트.
            emit_or_cancel(
                sink.as_ref(),
                cancel,
                InstallEvent::Finished {
                    outcome: outcome.clone(),
                },
            )?;
            Ok(outcome)
        }
        Err(ActionError::Cancelled) => {
            // sink_closed가 아니면 Cancelled emit (sink가 살아있을 가능성).
            let _ = sink.emit(InstallEvent::Cancelled);
            Err(InstallRunnerError::Action(ActionError::Cancelled))
        }
        Err(e) => {
            let runner_err = InstallRunnerError::Action(e);
            let code = runner_err.code().to_string();
            let message = runner_err.to_string();
            // 실패 emit은 best-effort — 이미 종료 단계.
            let _ = sink.emit(InstallEvent::Failed { code, message });
            Err(runner_err)
        }
    }
}

fn method_short_name(method: &PlatformInstall) -> &'static str {
    match method {
        PlatformInstall::DownloadAndRun(_) => "download_and_run",
        PlatformInstall::DownloadAndExtract(_) => "download_and_extract",
        PlatformInstall::ShellCurlPipeSh(_) => "shell.curl_pipe_sh",
        PlatformInstall::OpenUrl(_) => "open_url",
    }
}

/// sink.emit 결과가 SinkClosed면 cancel + 에러로 즉시 반환. 아니면 Ok.
fn emit_or_cancel<S: InstallSink + ?Sized>(
    sink: &S,
    cancel: &CancellationToken,
    ev: InstallEvent,
) -> Result<(), InstallRunnerError> {
    match sink.emit(ev) {
        Ok(()) => Ok(()),
        Err(InstallSinkClosed) => {
            cancel.cancel();
            Err(InstallRunnerError::SinkClosed)
        }
    }
}

/// `ProgressSink` (downloader 호환) → `InstallSink` 어댑터.
/// channel close 감지 시 cancel을 trigger하고 closed flag 기록 → caller가 후속 단계 short-circuit.
struct DownloadBridge<S: InstallSink + 'static> {
    sink: Arc<S>,
    cancel: CancellationToken,
    closed: Arc<Mutex<bool>>,
}

impl<S: InstallSink + 'static> DownloadBridge<S> {
    fn new(sink: Arc<S>, cancel: CancellationToken) -> Self {
        Self {
            sink,
            cancel,
            closed: Arc::new(Mutex::new(false)),
        }
    }

    fn was_closed(&self) -> bool {
        *self.closed.lock().expect("DownloadBridge.closed poisoned")
    }
}

impl<S: InstallSink + 'static> ProgressSink for DownloadBridge<S> {
    fn emit(&self, event: DownloadEvent) {
        match self.sink.emit(InstallEvent::Download { download: event }) {
            Ok(()) => {}
            Err(InstallSinkClosed) => {
                *self.closed.lock().expect("DownloadBridge.closed poisoned") = true;
                self.cancel.cancel();
            }
        }
    }
}

/// 매니페스트 디렉터리 + 앱 id에서 manifest 경로를 만든다.
/// `<base>/<id>.json`. base는 Tauri의 resource_dir/manifests/apps이거나 dev에선 repo의 manifests/apps.
pub fn manifest_path(manifests_dir: &Path, id: &str) -> PathBuf {
    manifests_dir.join(format!("{id}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Vec 캡처 sink — 테스트 전용.
    pub(crate) struct CapturedInstallSink {
        events: Mutex<Vec<InstallEvent>>,
        /// 일정 횟수 emit 후 SinkClosed 반환 (close 시뮬레이션).
        close_after: Option<usize>,
    }

    impl CapturedInstallSink {
        pub fn new() -> Self {
            Self {
                events: Mutex::new(Vec::new()),
                close_after: None,
            }
        }

        pub fn close_after(n: usize) -> Self {
            Self {
                events: Mutex::new(Vec::new()),
                close_after: Some(n),
            }
        }

        pub fn snapshot(&self) -> Vec<InstallEvent> {
            self.events.lock().unwrap().clone()
        }
    }

    impl InstallSink for CapturedInstallSink {
        fn emit(&self, event: InstallEvent) -> Result<(), InstallSinkClosed> {
            let mut events = self.events.lock().unwrap();
            if let Some(n) = self.close_after {
                if events.len() >= n {
                    return Err(InstallSinkClosed);
                }
            }
            events.push(event);
            Ok(())
        }
    }

    #[test]
    fn manifest_path_joins_id_and_json() {
        let p = manifest_path(Path::new("/tmp/manifests/apps"), "ollama");
        assert_eq!(p, Path::new("/tmp/manifests/apps/ollama.json"));
    }

    #[tokio::test]
    async fn run_install_errors_when_no_install_section() {
        let json = r#"{
            "schema_version": 1,
            "id": "naked",
            "display_name": "Naked",
            "license": "MIT",
            "detect": []
        }"#;
        let manifest: AppManifest = serde_json::from_str(json).expect("parse manifest");
        let cancel = CancellationToken::new();
        let sink = Arc::new(CapturedInstallSink::new());
        let dir = tempfile::TempDir::new().unwrap();
        let r = run_install(&manifest, dir.path(), &cancel, sink.clone()).await;
        assert!(matches!(r, Err(InstallRunnerError::NoInstallSection)));
        // sink는 비어있어야 — Started도 emit 안 됨 (검증 전 단계).
        assert!(sink.snapshot().is_empty());
    }

    #[tokio::test]
    async fn run_install_errors_when_no_platform_branch() {
        // 모든 OS에 대한 install이 None인 경우 (install 객체는 있으나 windows/macos/linux 모두 없음).
        let json = r#"{
            "schema_version": 1,
            "id": "alien",
            "display_name": "Alien",
            "license": "MIT",
            "detect": [],
            "install": {}
        }"#;
        let manifest: AppManifest = serde_json::from_str(json).expect("parse manifest");
        let cancel = CancellationToken::new();
        let sink = Arc::new(CapturedInstallSink::new());
        let dir = tempfile::TempDir::new().unwrap();
        let r = run_install(&manifest, dir.path(), &cancel, sink.clone()).await;
        assert!(matches!(r, Err(InstallRunnerError::NoPlatformBranch)));
        assert!(sink.snapshot().is_empty());
    }

    #[tokio::test]
    async fn run_install_open_url_emits_started_finished() {
        // open_url은 webbrowser::open 호출 — CI 환경에선 보통 성공 또는 NotFound.
        // 본 테스트는 webbrowser 동작 의존성을 줄이기 위해 invalid scheme으로 일부러 실패시켜
        // Started + Failed가 emit되는지 확인.
        // 단, OS에 따라 webbrowser는 너그럽게 처리할 수도 있어 outcome 분기 둘 다 허용.

        // 모든 OS에 동일한 open_url을 부여.
        let json = r#"{
            "schema_version": 1,
            "id": "broken-link",
            "display_name": "Broken",
            "license": "MIT",
            "detect": [],
            "install": {
                "windows": {
                    "method": "open_url",
                    "url": "definitely-not-a-real-scheme://nope"
                },
                "macos": {
                    "method": "open_url",
                    "url": "definitely-not-a-real-scheme://nope"
                },
                "linux": {
                    "method": "open_url",
                    "url": "definitely-not-a-real-scheme://nope"
                }
            }
        }"#;
        let manifest: AppManifest = serde_json::from_str(json).expect("parse manifest");
        let cancel = CancellationToken::new();
        let sink = Arc::new(CapturedInstallSink::new());
        let dir = tempfile::TempDir::new().unwrap();
        let _ = run_install(&manifest, dir.path(), &cancel, sink.clone()).await;
        let events = sink.snapshot();
        // 최소 Started는 항상 emit.
        assert!(matches!(events.first(), Some(InstallEvent::Started { .. })));
        // 마지막은 Finished 또는 Failed.
        assert!(matches!(
            events.last(),
            Some(InstallEvent::Finished { .. }) | Some(InstallEvent::Failed { .. })
        ));
    }

    #[tokio::test]
    async fn run_install_sink_close_during_started_returns_sink_closed() {
        // close_after(0) → 첫 emit (Started)부터 SinkClosed.
        let json = r#"{
            "schema_version": 1,
            "id": "closetest",
            "display_name": "CloseTest",
            "license": "MIT",
            "detect": [],
            "install": {
                "windows": { "method": "open_url", "url": "https://example.com/" },
                "macos": { "method": "open_url", "url": "https://example.com/" },
                "linux": { "method": "open_url", "url": "https://example.com/" }
            }
        }"#;
        let manifest: AppManifest = serde_json::from_str(json).expect("parse manifest");
        let cancel = CancellationToken::new();
        let sink = Arc::new(CapturedInstallSink::close_after(0));
        let dir = tempfile::TempDir::new().unwrap();
        let r = run_install(&manifest, dir.path(), &cancel, sink.clone()).await;
        assert!(matches!(r, Err(InstallRunnerError::SinkClosed)));
        // emit_or_cancel이 cancel.cancel()을 호출했어야 함.
        assert!(
            cancel.is_cancelled(),
            "sink close 시 cancel이 trigger돼야 함"
        );
    }

    #[test]
    fn error_codes_are_kebab_case() {
        // 모든 variant의 code()가 kebab-case (lowercase + hyphen)인지 sanity check.
        let codes = [
            InstallRunnerError::NoInstallSection.code(),
            InstallRunnerError::NoPlatformBranch.code(),
            InstallRunnerError::SinkClosed.code(),
        ];
        for code in codes {
            assert!(
                code.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "code {code}는 kebab-case여야 함"
            );
        }
    }
}
