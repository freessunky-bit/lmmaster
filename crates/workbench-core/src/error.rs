//! Workbench top-level 에러 타입.
//!
//! 정책 (phase-5p-workbench-decision.md §1.8):
//! - 사용자 향 메시지는 1차 한국어 해요체. 영어는 fallback.
//! - bench-harness `BenchError`와 동일 결.
//! - `Cancelled` / `ToolMissing` / `UnsupportedDataFormat` / `EvalFailed` / `Internal`을 명시 분리.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkbenchError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("입력 데이터 형식을 알 수 없어요: {0}")]
    UnsupportedDataFormat(String),

    #[error("CLI 도구가 설치되어 있지 않아요: {tool}")]
    ToolMissing { tool: String },

    #[error("측정 단계가 취소됐어요")]
    Cancelled,

    #[error("Korean QA evals 실패: {message}")]
    EvalFailed { message: String },

    #[error("내부 오류: {message}")]
    Internal { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_message_korean() {
        assert!(format!("{}", WorkbenchError::Cancelled).contains("취소"));
    }

    #[test]
    fn tool_missing_includes_tool_name() {
        let e = WorkbenchError::ToolMissing {
            tool: "llama-quantize".into(),
        };
        let msg = format!("{e}");
        assert!(msg.contains("llama-quantize"));
        assert!(msg.contains("CLI"));
    }

    #[test]
    fn unsupported_data_format_includes_detail() {
        let e = WorkbenchError::UnsupportedDataFormat("4 포맷 모두 불일치".into());
        let msg = format!("{e}");
        assert!(msg.contains("입력 데이터 형식"));
        assert!(msg.contains("4 포맷 모두 불일치"));
    }

    #[test]
    fn eval_failed_includes_message() {
        let e = WorkbenchError::EvalFailed {
            message: "expected substring 누락".into(),
        };
        let msg = format!("{e}");
        assert!(msg.contains("Korean QA evals"));
        assert!(msg.contains("expected substring 누락"));
    }

    #[test]
    fn internal_includes_korean_label() {
        let e = WorkbenchError::Internal {
            message: "panic chained".into(),
        };
        let msg = format!("{e}");
        assert!(msg.contains("내부 오류"));
        assert!(msg.contains("panic chained"));
    }

    #[test]
    fn io_from_std_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing file");
        let e: WorkbenchError = io_err.into();
        assert!(matches!(e, WorkbenchError::Io(_)));
    }

    #[test]
    fn json_from_serde_error() {
        let bad = "not json";
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(bad);
        let serde_err = parsed.unwrap_err();
        let e: WorkbenchError = serde_err.into();
        assert!(matches!(e, WorkbenchError::Json(_)));
    }
}
