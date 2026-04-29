//! crate: knowledge-stack — Phase 4.5' RAG (G1).
//!
//! per-workspace SQLite + Korean-aware chunker + Embedder trait + IngestService.
//!
//! 정책 (ADR-0024, phase-4p5-rag-decision.md):
//! - per-workspace 격리: 모든 query에 `WHERE workspace_id = ?` 강제.
//! - NFC 정규화 + 단락 → 문장 → 글자 윈도 fallback chunker.
//! - Embedder trait + MockEmbedder (sha256 deterministic).
//! - 외부 통신 0 — rusqlite bundled, 모든 임베딩 로직은 trait inject.
//! - 한국어 1차 — 모든 사용자 향 에러 메시지 해요체.

pub mod chunker;
pub mod embed;
pub mod embed_download;
#[cfg(feature = "embed-onnx")]
pub mod embed_onnx;
pub mod error;
pub mod ingest;
pub mod store;

pub use chunker::{chunk_text, normalize_korean, Chunk};
pub use embed::{default_embedder, Embedder, MockEmbedder};
pub use embed_download::{
    is_downloaded, model_dir, model_file_path, tokenizer_file_path, verify_files_sha256,
    DownloadEvent, ModelDownloader, ModelManifest, OnnxModelKind,
};
#[cfg(feature = "embed-onnx")]
pub use embed_onnx::{try_load_from_dir, OnnxEmbedder};
pub use error::KnowledgeError;
pub use ingest::{
    CancelToken, IngestProgress, IngestService, IngestStage, IngestSummary, ProgressTx,
};
pub use store::{ChunkRow, DocumentRow, KnowledgeStore, SearchHit, WorkspaceRow};
