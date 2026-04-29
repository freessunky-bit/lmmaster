//! crate: installer — 외부 앱/모델 다운로드/검증/설치 실행기.
//!
//! 정책 (ADR-0017, ADR-0021):
//! - 다운로드: streaming + Range resume + sha256 검증 + atomic rename + backon retry.
//! - tauri-plugin-updater 재사용 금지 — Range/resume/sha256 미구현, in-memory buffer.
//! - LM Studio EULA 상 redistribution 금지 — installer는 vendor-managed installer를 trigger만 함.
//! - capability ACL은 `apps/desktop/src-tauri/capabilities/main.json` (Phase 1A.3.b)에서 관리.
//!
//! Phase 1A.3.a 책임 영역 (이 sub-phase):
//! - `Downloader` — resumable + sha256 + retry
//! - `DownloadEvent` / `ProgressSink` — Tauri Channel/mpsc/closure 친화 인터페이스
//! - 통합 테스트 (wiremock + tempdir)
//!
//! Phase 1A.3.b 합류 예정:
//! - Pinokio install action executor (manifest의 install/update 객체 실행)
//! - tauri-plugin-shell 통합 + capability JSON
//! - post_install_check (manifest evaluator 재호출)
//! - InstallEvent (DownloadEvent를 감싸 더 넓은 lifecycle 표현)

pub mod action;
pub mod downloader;
pub mod error;
pub mod extract;
pub mod install_event;
pub mod install_runner;
pub mod progress;

pub use action::{ActionError, ActionExecutor, ActionOutcome};
pub use downloader::{DownloadOutcome, DownloadRequest, Downloader};
pub use error::DownloadError;
pub use extract::{
    detect_format, extract as extract_archive, ExtractError, ExtractFormat, ExtractOutcome,
};
pub use install_event::{
    ExtractPhase, InstallEvent, InstallSink, InstallSinkClosed, NoopInstallSink, PostCheckStatus,
};
pub use install_runner::{manifest_path, run_install, InstallRunnerError};
pub use progress::{DownloadEvent, NoopSink, ProgressSink};
