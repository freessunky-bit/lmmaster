//! Installer 에러 — 모든 사용자 가시 메시지는 한국어로 풀어쓰는 책임은 UI 층에 둔다.
//! 이 레벨은 진단 친화적 영문 + 구조화된 variant 유지.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("HTTP status not success: {status} for {url}")]
    BadStatus { status: u16, url: String },

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("sha256 mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },

    #[error("download cancelled by caller")]
    Cancelled,

    #[error("retries exhausted: {0}")]
    RetriesExhausted(String),

    #[error("invalid request: {0}")]
    InvalidRequest(String),
}

impl DownloadError {
    /// retry 가능한 일시적 실패인지 판정. 5xx + 네트워크 에러만 retry. cancel/hash mismatch는 fatal.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Http(e) => e.is_timeout() || e.is_connect() || e.is_request(),
            Self::BadStatus { status, .. } => *status >= 500 && *status < 600,
            Self::Io(e) => matches!(
                e.kind(),
                std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::Interrupted
                    | std::io::ErrorKind::WouldBlock
            ),
            Self::HashMismatch { .. }
            | Self::Cancelled
            | Self::RetriesExhausted(_)
            | Self::InvalidRequest(_) => false,
        }
    }
}
