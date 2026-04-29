//! crate: portable-workspace — workspace manifest + 경로 + 3-tier fingerprint repair + export/import.
//!
//! 정책 (ADR-0009, ADR-0022 §8, ADR-0039):
//! - 모든 경로는 워크스페이스 루트 기준 상대.
//! - host_fingerprint mismatch 시 3-tier repair (green/yellow/red) 자동 진입.
//! - 모델 파일(`models/`)은 GGUF agnostic이라 모든 tier에서 보존.
//! - export/import는 zip 8.x + AES-GCM(키 옵션) + 단일 .zip — Windows NTFS 가정.

pub mod export;
pub mod fingerprint;
pub mod import;
pub mod manifest;
pub mod paths;
pub mod repair;

pub use export::{
    export_workspace, ExportError, ExportEvent, ExportOptions, ExportSink, ExportSummary,
};
pub use fingerprint::{classify, GpuClass, RepairTier, WorkspaceFingerprint};
pub use import::{
    import_workspace, verify_archive, ArchivePreview, ConflictPolicy, ImportError, ImportEvent,
    ImportOptions, ImportSink, ImportSummary,
};
pub use manifest::{ModelRecord, PortMap, RuntimeRecord, WorkspaceManifest};
pub use paths::Workspace;
pub use repair::{
    apply_repair, evaluate_and_repair, fingerprint_path, load_fingerprint, save_fingerprint,
    RepairError, RepairReport,
};
