//! crate: auto-updater — Phase 6'.a 자동 갱신 + 폴러 + 소스 추상화.
//!
//! 정책 (ADR-0025, phase-6p-updater-pipelines-decision.md):
//! - GitHub Releases 1순위 소스 + 6h 폴 (사용자가 매번 수동 확인하지 않도록).
//! - 다운로드는 사용자 동의 후만 (silent install 금지 — EULA 정책 + ADR-0016 wrap-not-replace).
//! - 사용자-노출 메시지 1차 한국어 해요체. 영어는 fallback.
//! - 외부 통신 0 정책 — 본 crate는 GitHub `api.github.com` 한 호스트만 호출.
//! - cancel 협력: `tokio_util::sync::CancellationToken`을 외부에서 주입.
//! - semver 비교: `is_outdated(current, latest)` — pre-release / build metadata 표준 처리.
//!
//! 5개 모듈로 분할:
//! - `error` — `UpdaterError` enum (해요체 메시지) + `Display` 단위 테스트.
//! - `version` — `is_outdated(current, latest) -> Result<bool, _>` semver 비교.
//! - `source` — `UpdateSource` trait + `GitHubReleasesSource` impl + `MockSource`.
//! - `poller` — `Poller` 구조체 + `run` (interval + cancel) / `check_once`.

pub mod error;
pub mod poller;
pub mod source;
pub mod version;

pub use error::UpdaterError;
pub use poller::{Poller, DEFAULT_INTERVAL};
pub use source::{GitHubReleasesSource, MockSource, ReleaseInfo, UpdateSource};
pub use version::is_outdated;
