//! adapter-vllm — 고사양 서빙 옵션 (Linux + CUDA 우선).
//! Windows는 후순위. 라이선스 Apache-2.0.

use async_trait::async_trait;
use runtime_manager::{
    DetectResult, HealthReport, InstallOpts, LocalModel, ProgressSink, RuntimeAdapter, RuntimeCfg,
    RuntimeHandle,
};
use shared_types::{CapabilityMatrix, ModelRef, RuntimeKind};

pub struct VllmAdapter;

impl VllmAdapter {
    pub fn new() -> Self {
        Self
    }
}
impl Default for VllmAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RuntimeAdapter for VllmAdapter {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Vllm
    }
    async fn detect(&self) -> anyhow::Result<DetectResult> {
        unimplemented!("M5")
    }
    async fn install(&self, _opts: InstallOpts) -> anyhow::Result<()> {
        unimplemented!("M5")
    }
    async fn update(&self) -> anyhow::Result<()> {
        unimplemented!("M5")
    }
    async fn start(&self, _cfg: RuntimeCfg) -> anyhow::Result<RuntimeHandle> {
        unimplemented!("M5")
    }
    async fn stop(&self, _h: &RuntimeHandle) -> anyhow::Result<()> {
        unimplemented!("M5")
    }
    async fn restart(&self, _h: &RuntimeHandle) -> anyhow::Result<()> {
        unimplemented!("M5")
    }
    async fn health(&self, _h: &RuntimeHandle) -> HealthReport {
        HealthReport::default()
    }
    async fn list_models(&self) -> anyhow::Result<Vec<LocalModel>> {
        Ok(vec![])
    }
    async fn pull_model(&self, _m: &ModelRef, _sink: ProgressSink) -> anyhow::Result<()> {
        unimplemented!("M5")
    }
    async fn remove_model(&self, _m: &ModelRef) -> anyhow::Result<()> {
        unimplemented!("M5")
    }
    async fn warmup(&self, _h: &RuntimeHandle, _m: &ModelRef) -> anyhow::Result<()> {
        unimplemented!("M5")
    }
    fn capability_matrix(&self) -> CapabilityMatrix {
        CapabilityMatrix {
            vision: true,
            tools: true,
            structured_output: true,
            embeddings: true,
        }
    }
}
