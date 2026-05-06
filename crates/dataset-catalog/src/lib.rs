//! crate: dataset-catalog — Phase 23'.a (ADR-0061 + ADR-0062).
//!
//! 정책:
//! - DatasetCategory enum (별도) — ModelCategory와 직교 축.
//! - DatasetEntry manifest schema — model entry parallel structure.
//! - safety 모듈 — 미성년 키워드 deterministic 거부.
//! - validator — minor_safety_attestation + license 화이트리스트 검증.
//! - 외부 통신 0 — 본 crate는 *schema + validator*. 다운로드는 호출 측 (registry-fetcher).

pub mod format;
pub mod manifest;
pub mod safety;
pub mod validator;

pub use format::{ChunkStrategy, DatasetFormat};
pub use manifest::{
    DatasetBundle, DatasetCategory, DatasetEntry, DatasetSource, DatasetUseCase,
    MinorSafetyAttestation,
};
pub use safety::{dataset_has_minor_keywords, MINOR_KEYWORDS_REJECT};
pub use validator::{validate_dataset_entry, DatasetValidationError};
