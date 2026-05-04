//! Workspace fingerprint repair IPC — Phase 3'.c.
//! Workspace export/import IPC — Phase 11'.
//! Workspace cancellation scope — Phase R-E.7.

pub mod cancel_scope;
pub mod commands;
pub mod portable;

pub use cancel_scope::WorkspaceCancellationScope;
pub use commands::{
    check_workspace_repair, get_repair_history, get_workspace_fingerprint, RepairHistoryEntry,
};
pub use portable::{
    cancel_workspace_export, cancel_workspace_import, start_workspace_export,
    start_workspace_import, verify_workspace_archive, PortableRegistry,
};
