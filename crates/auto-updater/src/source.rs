//! `UpdateSource` trait + `GitHubReleasesSource` 실 구현 + `MockSource` 테스트용.
//!
//! 정책 (ADR-0025, phase-6p-updater-pipelines-decision.md §4):
//! - GitHub Releases API: `https://api.github.com/repos/{repo}/releases/latest`.
//! - User-Agent 헤더 필수 (GitHub API 정책).
//! - 응답에서 `tag_name`/`html_url`/`published_at`/`body` 4 필드만 추출.
//! - 실패 시 `UpdaterError::Network` 또는 `UpdaterError::Parse` 또는 `UpdaterError::SourceFailure`.
//! - `MockSource`는 `set_release(release)`로 런타임 mutate 가능 (Poller 테스트 시 사용).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::error::UpdaterError;

/// 릴리스 메타데이터 (사용자 향 토스트 + 다운로드 결정에 충분한 표면).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseInfo {
    /// `1.2.3` 또는 `v1.2.3` (`is_outdated`가 prefix tolerant).
    pub version: String,
    /// 릴리스 발행 시점 — 토스트에서 "3일 전" 표시 등에 사용.
    #[serde(with = "time::serde::rfc3339")]
    pub published_at: time::OffsetDateTime,
    /// 사용자가 직접 열 수 있는 릴리스 페이지 URL.
    pub url: String,
    /// markdown 릴리스 노트. None일 수 있음.
    pub notes: Option<String>,
}

/// 업데이트 소스 추상화. GitHub Releases / 자체 호스팅 / mock 모두 동일 인터페이스.
#[async_trait]
pub trait UpdateSource: Send + Sync {
    async fn latest_version(&self) -> Result<ReleaseInfo, UpdaterError>;
}

// --- GitHub Releases 실 구현 ----------------------------------------------------

/// GitHub Releases `latest` 엔드포인트 1순위 소스.
pub struct GitHubReleasesSource {
    /// "owner/repo" 형식 (예: "lmmaster/lmmaster").
    repo: String,
    client: reqwest::Client,
    /// 단위 테스트에서 wiremock 등으로 base URL을 갈아끼울 수 있도록 분리.
    /// 기본값은 `https://api.github.com`.
    base_url: String,
    /// 요청 타임아웃 — 6h 폴 사이클을 막지 않도록 8s 보수.
    timeout: Duration,
}

impl GitHubReleasesSource {
    pub fn new(repo: impl Into<String>) -> Self {
        Self::with_base("https://api.github.com", repo)
    }

    /// base URL을 사용자가 지정 (테스트용). 마지막 슬래시 없이 입력.
    pub fn with_base(base_url: impl Into<String>, repo: impl Into<String>) -> Self {
        Self {
            repo: repo.into(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(8))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            base_url: base_url.into(),
            timeout: Duration::from_secs(8),
        }
    }

    /// 사용자 정의 client 주입 (이미 user-agent 등 설정 끝난 경우).
    pub fn with_client(repo: impl Into<String>, client: reqwest::Client) -> Self {
        Self {
            repo: repo.into(),
            client,
            base_url: "https://api.github.com".into(),
            timeout: Duration::from_secs(8),
        }
    }
}

#[async_trait]
impl UpdateSource for GitHubReleasesSource {
    async fn latest_version(&self) -> Result<ReleaseInfo, UpdaterError> {
        let url = format!("{}/repos/{}/releases/latest", self.base_url, self.repo);
        tracing::debug!(repo = %self.repo, %url, "GitHub Releases latest 요청");

        let resp = self
            .client
            .get(&url)
            .header(
                reqwest::header::USER_AGENT,
                concat!("LMmaster-auto-updater/", env!("CARGO_PKG_VERSION")),
            )
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .timeout(self.timeout)
            .send()
            .await?;

        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            // GitHub은 릴리스가 0개일 때 404를 반환. 명확히 NoReleases로 매핑.
            return Err(UpdaterError::NoReleases);
        }
        if !status.is_success() {
            return Err(UpdaterError::SourceFailure(format!(
                "HTTP {} from {url}",
                status.as_u16()
            )));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| UpdaterError::SourceFailure(format!("응답 본문 읽기 실패: {e}")))?;
        parse_github_release(&body)
    }
}

/// GitHub Releases JSON → `ReleaseInfo` 파서.
///
/// `tag_name` 누락은 `Parse` (스키마 비호환), `published_at` 누락은 unix epoch fallback.
fn parse_github_release(body: &str) -> Result<ReleaseInfo, UpdaterError> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| UpdaterError::Parse(format!("JSON: {e}")))?;

    let tag = v
        .get("tag_name")
        .and_then(|x| x.as_str())
        .ok_or_else(|| UpdaterError::Parse("tag_name 필드가 없어요".to_string()))?
        .to_string();

    let url = v
        .get("html_url")
        .and_then(|x| x.as_str())
        .map(str::to_string)
        .unwrap_or_default();

    let notes = v.get("body").and_then(|x| x.as_str()).map(str::to_string);

    let published_at = v
        .get("published_at")
        .and_then(|x| x.as_str())
        .and_then(|s| {
            time::OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339).ok()
        })
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH);

    Ok(ReleaseInfo {
        version: tag,
        published_at,
        url,
        notes,
    })
}

// --- Mock source -----------------------------------------------------------------

/// 단위 테스트 + 통합 테스트에서 사용하는 mock 소스.
///
/// `set_release`로 런타임에 응답을 갈아끼울 수 있어 Poller cancel/poll 테스트에 필수.
#[derive(Clone, Default)]
pub struct MockSource {
    release: Arc<Mutex<Option<ReleaseInfo>>>,
    /// 호출 횟수 — Poller가 interval 내에 정확히 N번 호출했는지 검증용.
    call_count: Arc<Mutex<usize>>,
    /// `latest_version` 호출 시 반환할 강제 에러 (테스트용). Some이면 release 무시.
    force_error: Arc<Mutex<Option<UpdaterError>>>,
}

impl MockSource {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_release(release: ReleaseInfo) -> Self {
        Self {
            release: Arc::new(Mutex::new(Some(release))),
            call_count: Arc::new(Mutex::new(0)),
            force_error: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn set_release(&self, release: Option<ReleaseInfo>) {
        let mut guard = self.release.lock().await;
        *guard = release;
    }

    /// 강제로 다음 호출에서 SourceFailure를 반환. 한 번 set한 후 자동 해제는 안 됨 — 명시적으로 None으로 clear.
    pub async fn set_force_error(&self, err: Option<UpdaterError>) {
        let mut guard = self.force_error.lock().await;
        *guard = err;
    }

    pub async fn call_count(&self) -> usize {
        *self.call_count.lock().await
    }
}

#[async_trait]
impl UpdateSource for MockSource {
    async fn latest_version(&self) -> Result<ReleaseInfo, UpdaterError> {
        {
            let mut count = self.call_count.lock().await;
            *count += 1;
        }
        let force_msg = {
            let guard = self.force_error.lock().await;
            guard.as_ref().map(|e| format!("{e}"))
        };
        if let Some(msg) = force_msg {
            // 새 인스턴스로 복제 (UpdaterError는 Clone 미구현 — 메시지 보존하며 새 SourceFailure 생성).
            return Err(UpdaterError::SourceFailure(msg));
        }
        let guard = self.release.lock().await;
        match guard.as_ref() {
            Some(r) => Ok(r.clone()),
            None => Err(UpdaterError::NoReleases),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_release(version: &str) -> ReleaseInfo {
        ReleaseInfo {
            version: version.to_string(),
            published_at: time::OffsetDateTime::UNIX_EPOCH,
            url: format!("https://example.com/{version}"),
            notes: Some("changelog".into()),
        }
    }

    #[tokio::test]
    async fn mock_returns_release() {
        let mock = MockSource::with_release(sample_release("1.2.3"));
        let r = mock.latest_version().await.unwrap();
        assert_eq!(r.version, "1.2.3");
    }

    #[tokio::test]
    async fn mock_returns_no_releases_when_none() {
        let mock = MockSource::new();
        let err = mock.latest_version().await.unwrap_err();
        assert!(matches!(err, UpdaterError::NoReleases));
        let msg = format!("{err}");
        assert!(msg.contains("릴리스"));
    }

    #[tokio::test]
    async fn mock_set_release_mutates_runtime() {
        let mock = MockSource::new();
        // 초기엔 None → NoReleases.
        assert!(mock.latest_version().await.is_err());
        // mutate.
        mock.set_release(Some(sample_release("2.0.0"))).await;
        let r = mock.latest_version().await.unwrap();
        assert_eq!(r.version, "2.0.0");
        // 다시 비우면 NoReleases.
        mock.set_release(None).await;
        assert!(matches!(
            mock.latest_version().await.unwrap_err(),
            UpdaterError::NoReleases
        ));
    }

    #[tokio::test]
    async fn mock_call_count_tracks_invocations() {
        let mock = MockSource::with_release(sample_release("0.1.0"));
        let _ = mock.latest_version().await;
        let _ = mock.latest_version().await;
        let _ = mock.latest_version().await;
        assert_eq!(mock.call_count().await, 3);
    }

    #[tokio::test]
    async fn mock_force_error_overrides_release() {
        let mock = MockSource::with_release(sample_release("1.0.0"));
        mock.set_force_error(Some(UpdaterError::SourceFailure("upstream 503".into())))
            .await;
        let err = mock.latest_version().await.unwrap_err();
        assert!(matches!(err, UpdaterError::SourceFailure(_)));
        let msg = format!("{err}");
        assert!(msg.contains("upstream 503"));
    }

    #[test]
    fn parse_github_release_full_payload() {
        let body = serde_json::json!({
            "tag_name": "v1.2.3",
            "html_url": "https://github.com/owner/repo/releases/tag/v1.2.3",
            "published_at": "2026-04-01T12:34:56Z",
            "body": "## 변경 사항\n- 첫 릴리스"
        })
        .to_string();
        let r = parse_github_release(&body).unwrap();
        assert_eq!(r.version, "v1.2.3");
        assert_eq!(r.url, "https://github.com/owner/repo/releases/tag/v1.2.3");
        assert!(r.notes.unwrap().contains("첫 릴리스"));
        // 정확한 timestamp 대신 epoch 이후라는 정도 + 분/초 일치만 검증.
        assert!(r.published_at.unix_timestamp() > 1_700_000_000);
        assert_eq!(r.published_at.minute(), 34);
        assert_eq!(r.published_at.second(), 56);
    }

    #[test]
    fn parse_github_release_missing_tag_returns_parse_error() {
        let body = r#"{"html_url":"x"}"#;
        let err = parse_github_release(body).unwrap_err();
        assert!(matches!(err, UpdaterError::Parse(_)));
        let msg = format!("{err}");
        assert!(msg.contains("tag_name"));
    }

    #[test]
    fn parse_github_release_invalid_json() {
        let err = parse_github_release("not json").unwrap_err();
        assert!(matches!(err, UpdaterError::Parse(_)));
        let msg = format!("{err}");
        assert!(msg.contains("릴리스 정보"));
    }

    #[test]
    fn parse_github_release_missing_published_at_falls_back_epoch() {
        let body = serde_json::json!({
            "tag_name": "v0.0.1",
            "html_url": "x",
            "body": null
        })
        .to_string();
        let r = parse_github_release(&body).unwrap();
        assert_eq!(r.version, "v0.0.1");
        assert_eq!(r.published_at, time::OffsetDateTime::UNIX_EPOCH);
        assert!(r.notes.is_none());
    }

    #[test]
    fn release_info_serde_round_trip() {
        let r = sample_release("0.5.0");
        let s = serde_json::to_string(&r).unwrap();
        let back: ReleaseInfo = serde_json::from_str(&s).unwrap();
        assert_eq!(r, back);
    }
}
