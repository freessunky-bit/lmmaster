//! Auto-updater 에러 타입.
//!
//! 정책 (ADR-0025, phase-6p-updater-pipelines-decision.md §1.8):
//! - 사용자 향 메시지는 1차 한국어 해요체. 영어는 fallback.
//! - thiserror 기반, `Display`만으로 사용자에게 노출 가능.
//! - 6 variant 분리: Network / Parse / InvalidVersion / NoReleases / Cancelled / SourceFailure.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum UpdaterError {
    #[error("업데이트 서버에 연결하지 못했어요: {0}")]
    Network(#[from] reqwest::Error),

    #[error("릴리스 정보를 해석하지 못했어요: {0}")]
    Parse(String),

    #[error("버전 형식이 올바르지 않아요: {0}")]
    InvalidVersion(String),

    #[error("릴리스가 아직 없어요")]
    NoReleases,

    #[error("사용자가 업데이트 확인을 취소했어요")]
    Cancelled,

    #[error("업데이트 소스가 응답하지 않아요: {0}")]
    SourceFailure(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_message_korean() {
        let e = UpdaterError::Parse("invalid JSON".into());
        let msg = format!("{e}");
        assert!(msg.contains("릴리스 정보"));
        assert!(msg.contains("해석"));
        assert!(msg.contains("invalid JSON"));
    }

    #[test]
    fn invalid_version_message_korean() {
        let e = UpdaterError::InvalidVersion("not.a.version".into());
        let msg = format!("{e}");
        assert!(msg.contains("버전 형식"));
        assert!(msg.contains("not.a.version"));
    }

    #[test]
    fn no_releases_message_korean() {
        let e = UpdaterError::NoReleases;
        let msg = format!("{e}");
        assert!(msg.contains("릴리스"));
        assert!(msg.contains("없어요"));
    }

    #[test]
    fn cancelled_message_korean() {
        let e = UpdaterError::Cancelled;
        let msg = format!("{e}");
        assert!(msg.contains("취소"));
        assert!(msg.contains("사용자"));
    }

    #[test]
    fn source_failure_message_korean() {
        let e = UpdaterError::SourceFailure("upstream timeout".into());
        let msg = format!("{e}");
        assert!(msg.contains("업데이트 소스"));
        assert!(msg.contains("응답"));
        assert!(msg.contains("upstream timeout"));
    }

    #[tokio::test]
    async fn network_message_korean_prefix() {
        // reqwest::Error는 직접 생성이 까다로워 실제 호출 실패로 만든다.
        // 의도적으로 비정상 URL을 호출해 실패 케이스를 만든다.
        let client = reqwest::Client::new();
        let req_err = client
            .get("http://127.0.0.1:1/__nope__")
            .timeout(std::time::Duration::from_millis(50))
            .send()
            .await
            .expect_err("invalid request must fail");
        let e: UpdaterError = req_err.into();
        let msg = format!("{e}");
        assert!(msg.contains("업데이트 서버"));
        assert!(msg.contains("연결"));
    }

    #[test]
    fn variants_are_distinct() {
        // 패턴 매칭 가능성 — 각 variant가 enum에 정상 등록됐는지.
        for e in [
            UpdaterError::Parse("p".into()),
            UpdaterError::InvalidVersion("v".into()),
            UpdaterError::NoReleases,
            UpdaterError::Cancelled,
            UpdaterError::SourceFailure("s".into()),
        ] {
            // 각 메시지는 비어 있지 않아야 함.
            let msg = format!("{e}");
            assert!(!msg.is_empty());
        }
    }
}
