use serde::{Deserialize, Serialize};
use shared_types::HostFingerprint;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceManifest {
    pub schema_version: u32,
    pub workspace_id: String,
    pub host_fingerprint: HostFingerprint,
    pub runtimes_installed: Vec<RuntimeRecord>,
    pub models_installed: Vec<ModelRecord>,
    pub ports: PortMap,
    pub created_at: String,
    pub last_repaired_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeRecord {
    pub id: String,
    pub kind: shared_types::RuntimeKind,
    pub version: String,
    pub build_target: String,
    pub install_dir_rel: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRecord {
    pub id: String,
    pub runtime_id: String,
    pub quantization: String,
    pub file_rel_path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PortMap {
    pub gateway: Option<u16>,
    pub ml_worker: Option<u16>,
}
