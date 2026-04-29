//! BenchHarness top-level 에러 + 어댑터 측 IO 에러 변환.
//!
//! 정책: 모든 에러는 한국어 메시지를 가짐. UI는 `BenchErrorReport`(JSON tagged enum) 직접 사용.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BenchError {
    #[error("런타임 HTTP에 닿을 수 없어요: {0}")]
    RuntimeUnreachable(String),

    #[error("모델이 런타임에 등록되어 있지 않아요: {0}")]
    ModelNotLoaded(String),

    #[error("VRAM이 부족해요. {need_mb}MB 필요한데 {have_mb}MB만 있어요.")]
    InsufficientVram { need_mb: u32, have_mb: u32 },

    #[error("측정이 취소됐어요")]
    Cancelled,

    #[error("30초 안에 측정을 마치지 못했어요 (timeout)")]
    Timeout,

    #[error("HTTP/serde 등 내부 에러: {0}")]
    Internal(String),
}

impl From<serde_json::Error> for BenchError {
    fn from(e: serde_json::Error) -> Self {
        Self::Internal(format!("json: {e}"))
    }
}

impl From<std::io::Error> for BenchError {
    fn from(e: std::io::Error) -> Self {
        Self::Internal(format!("io: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_message_korean() {
        let e = BenchError::Timeout;
        assert!(format!("{e}").contains("30초"));
    }

    #[test]
    fn insufficient_vram_includes_numbers() {
        let e = BenchError::InsufficientVram {
            need_mb: 12000,
            have_mb: 6000,
        };
        let msg = format!("{e}");
        assert!(msg.contains("12000"));
        assert!(msg.contains("6000"));
    }
}
