//! `FetcherError` — 사용자 메시지는 한국어 해요체, debug 정보는 structured로.
//!
//! Phase 1' 결정: Korean tracing/error strings + English structured fields.

use crate::source::SourceTier;

#[derive(Debug, thiserror::Error)]
pub enum FetcherError {
    #[error("네트워크 오류: {0}")]
    Network(#[from] reqwest::Error),

    #[error("모든 미러에서 받지 못했어요 (manifest_id={id}, tried={tried:?})")]
    AllSourcesFailed { id: String, tried: Vec<SourceTier> },

    #[error("HTTP {status} 응답을 받았어요 (tier={tier:?})")]
    HttpStatus { status: u16, tier: SourceTier },

    #[error("JSON 파싱에 실패했어요: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("캐시 데이터가 손상됐어요 — 자동으로 다시 받을게요")]
    CacheCorrupt,

    #[error("캐시 DB 오류: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("내장 매니페스트 파일을 찾지 못했어요: {0}")]
    BundledMissing(String),

    #[error("스키마 버전 {found}는 지원하지 않아요 (지원 최대 {max})")]
    SchemaMismatch { found: u32, max: u32 },

    #[error("내부 작업 실패: {0}")]
    JoinError(#[from] tokio::task::JoinError),

    #[error("IO 오류: {0}")]
    Io(#[from] std::io::Error),

    #[error("매니페스트 ID가 비어있어요")]
    EmptyManifestId,

    #[error("URL 템플릿 치환 실패: {0}")]
    UrlTemplate(String),
}
