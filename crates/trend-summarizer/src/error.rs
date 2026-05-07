//! Trend Summarizer 에러 — 한국어 해요체 (CLAUDE.md §4.1).

use thiserror::Error;

pub type SummarizerResult<T> = Result<T, SummarizerError>;

#[derive(Debug, Error)]
pub enum SummarizerError {
    #[error("로컬 LLM 호출에 실패했어요: {0}")]
    LlmCallFailed(String),

    #[error("응답에서 한국어 요약을 찾지 못했어요: {0}")]
    ParseFailed(String),

    #[error("입력 데이터가 비었어요 — 요약할 트렌드 항목이 없어요")]
    EmptyInput,

    #[error("내부 에러: {0}")]
    Internal(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_korean() {
        assert!(SummarizerError::LlmCallFailed("x".into())
            .to_string()
            .contains("LLM"));
        assert!(SummarizerError::EmptyInput.to_string().contains("비었어요"));
        assert!(SummarizerError::ParseFailed("x".into())
            .to_string()
            .contains("한국어 요약"));
    }
}
