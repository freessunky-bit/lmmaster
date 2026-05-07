//! crate: dataset-importer — Phase 23'.c.2 (ADR-0063).
//!
//! 정책:
//! - HuggingFace `/api/datasets/{ds}/parquet/{config}/{split}` endpoint 사용 (ADR-0026 화이트리스트 적중).
//! - `arrow-rs parquet::arrow::async_reader::ParquetRecordBatchStreamBuilder` + `AsyncFileReader` impl.
//! - reqwest Range header로 row group 단위 lazy stream — 메모리 peak ~500MB (1.8GB 안전).
//! - text-splitter (tokenizers 기반) chunk size 512 / overlap 64 (KURE-v1 native context).
//! - ingest pipeline: Manifest → Downloading → Chunking → Embedding → Writing → Done.
//! - cancel token (Arc<AtomicBool>) + 부분 commit (`partial = true` 플래그).
//!
//! 본 crate는 *fetch + chunk + IngestStage emit*만. 임베딩 + SQLCipher 저장은 호출 측 (knowledge-stack).

pub mod chunker;
pub mod error;
pub mod parquet_stream;
pub mod pipeline;
pub mod service;

pub use chunker::{ChunkConfigParams, DatasetChunk, DatasetChunker};
pub use error::{DatasetImportError, DatasetImportResult};
pub use parquet_stream::{HfParquetReader, ParquetUrlResolver};
pub use pipeline::{DatasetIngestStage, IngestProgress, SampleStrategy};
pub use service::{
    DatasetIngestService, EmbeddedChunk, IngestRequest, IngestStats, DEFAULT_EMBEDDING_BATCH_SIZE,
};
