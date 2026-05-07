//! Dataset import 에러 — Phase 23'.c.2.
//!
//! 정책: 사용자 노출 메시지는 한국어 해요체.

use thiserror::Error;

pub type DatasetImportResult<T> = Result<T, DatasetImportError>;

#[derive(Debug, Error)]
pub enum DatasetImportError {
    #[error("HF API 응답을 받지 못했어요: {0}")]
    HfApiUnreachable(String),

    #[error("HF rate limit 초과 — {retry_after_secs}초 후 다시 시도할게요")]
    RateLimited { retry_after_secs: u64 },

    #[error("Parquet 파일을 읽지 못했어요: {0}")]
    ParquetReadFailed(String),

    #[error("Parquet schema에 텍스트 필드 '{0}'가 없어요")]
    TextFieldMissing(String),

    #[error("토크나이저를 로드하지 못했어요: {0}")]
    TokenizerLoadFailed(String),

    #[error("청크 분할에 실패했어요: {0}")]
    ChunkingFailed(String),

    #[error("임베딩에 실패했어요: {0}")]
    EmbeddingFailed(String),

    #[error("사용자가 import를 취소했어요")]
    Cancelled,

    #[error("미성년 보호 검증 실패 — minor_safety_attestation이 누락됐어요")]
    MinorSafetyAttestationMissing,

    #[error("CC-BY-NC 라이선스 — 비상업 EULA 동의가 필요해요")]
    NoncommercialEulaRequired,

    #[error("내부 에러: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for DatasetImportError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

impl From<reqwest::Error> for DatasetImportError {
    fn from(err: reqwest::Error) -> Self {
        Self::HfApiUnreachable(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_korean() {
        assert!(DatasetImportError::HfApiUnreachable("test".into())
            .to_string()
            .contains("HF API"));
        assert!(DatasetImportError::RateLimited {
            retry_after_secs: 60
        }
        .to_string()
        .contains("60초"));
        assert!(DatasetImportError::Cancelled.to_string().contains("취소"));
        assert!(DatasetImportError::MinorSafetyAttestationMissing
            .to_string()
            .contains("minor_safety_attestation"));
    }

    #[test]
    fn error_serialization_via_display() {
        let err = DatasetImportError::TextFieldMissing("persona".into());
        assert!(err.to_string().contains("persona"));
    }
}
