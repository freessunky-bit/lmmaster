//! crate: runtime-manager — RuntimeAdapter trait + supervisor + 상태기계.
//!
//! 정책 (ADR-0004):
//! - 모든 런타임은 RuntimeAdapter를 구현한다.
//! - Runtime Manager는 trait 객체만 본다 — 런타임별 if-else 금지.
//! - capability_matrix를 gateway routing에 노출한다.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shared_types::{CapabilityMatrix, ModelRef, RuntimeKind, RuntimeState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeHandle {
    pub kind: RuntimeKind,
    pub instance_id: String,
    pub internal_port: u16,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeCfg {
    pub gpu_layers: Option<u32>,
    pub context: Option<u32>,
    pub extra: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectResult {
    pub installed: bool,
    pub version: Option<String>,
    pub build_target: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstallOpts {
    pub target: Option<String>,
    pub force: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HealthReport {
    pub state: Option<RuntimeState>,
    pub latency_ms: Option<u32>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LocalModel {
    pub r#ref: Option<ModelRef>,
    pub file_rel_path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

pub type ProgressSink = tokio::sync::mpsc::Sender<ProgressUpdate>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressUpdate {
    pub stage: String,
    pub bytes_done: u64,
    pub bytes_total: Option<u64>,
    pub message: Option<String>,
}

#[async_trait]
pub trait RuntimeAdapter: Send + Sync {
    fn kind(&self) -> RuntimeKind;
    async fn detect(&self) -> anyhow::Result<DetectResult>;
    async fn install(&self, opts: InstallOpts) -> anyhow::Result<()>;
    async fn update(&self) -> anyhow::Result<()>;
    async fn start(&self, cfg: RuntimeCfg) -> anyhow::Result<RuntimeHandle>;
    async fn stop(&self, h: &RuntimeHandle) -> anyhow::Result<()>;
    async fn restart(&self, h: &RuntimeHandle) -> anyhow::Result<()>;
    async fn health(&self, h: &RuntimeHandle) -> HealthReport;
    async fn list_models(&self) -> anyhow::Result<Vec<LocalModel>>;
    async fn pull_model(&self, m: &ModelRef, sink: ProgressSink) -> anyhow::Result<()>;
    async fn remove_model(&self, m: &ModelRef) -> anyhow::Result<()>;
    async fn warmup(&self, h: &RuntimeHandle, m: &ModelRef) -> anyhow::Result<()>;
    fn capability_matrix(&self) -> CapabilityMatrix;
}

pub mod manager;

pub use manager::RuntimeManager;

pub mod supervisor {
    //! 자식 프로세스 supervisor — Phase 5'+ (llama.cpp 자식 프로세스 모드).
}

pub mod state {
    //! Runtime 상태기계 — Cold/Warming/Standby/Active. Phase 3' Gateway routing에서 본격 사용.
}
