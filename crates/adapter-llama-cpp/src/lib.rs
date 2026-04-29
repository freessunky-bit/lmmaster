//! adapter-llama-cpp — primary portable runtime adapter (ADR-0005).
//!
//! 통합 방식: subprocess. llama.cpp의 `server` 바이너리를 자식 프로세스로 spawn.
//! 모델 포맷: GGUF.
//! 빌드 타깃: CUDA / Vulkan / Metal / ROCm / CPU.

use async_trait::async_trait;
use runtime_manager::{
    DetectResult, HealthReport, InstallOpts, LocalModel, ProgressSink, RuntimeAdapter, RuntimeCfg,
    RuntimeHandle,
};
use shared_types::{CapabilityMatrix, ModelRef, RuntimeKind};

pub struct LlamaCppAdapter {
    // M2: 워크스페이스 경로, 빌드 타깃, 다운로드 정책 보유.
}

impl LlamaCppAdapter {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for LlamaCppAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RuntimeAdapter for LlamaCppAdapter {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::LlamaCpp
    }
    async fn detect(&self) -> anyhow::Result<DetectResult> {
        unimplemented!("M2")
    }
    async fn install(&self, _opts: InstallOpts) -> anyhow::Result<()> {
        unimplemented!("M2")
    }
    async fn update(&self) -> anyhow::Result<()> {
        unimplemented!("M2")
    }
    async fn start(&self, _cfg: RuntimeCfg) -> anyhow::Result<RuntimeHandle> {
        unimplemented!("M2")
    }
    async fn stop(&self, _h: &RuntimeHandle) -> anyhow::Result<()> {
        unimplemented!("M2")
    }
    async fn restart(&self, _h: &RuntimeHandle) -> anyhow::Result<()> {
        unimplemented!("M2")
    }
    async fn health(&self, _h: &RuntimeHandle) -> HealthReport {
        HealthReport::default()
    }
    async fn list_models(&self) -> anyhow::Result<Vec<LocalModel>> {
        Ok(vec![])
    }
    async fn pull_model(&self, _m: &ModelRef, _sink: ProgressSink) -> anyhow::Result<()> {
        unimplemented!("M2")
    }
    async fn remove_model(&self, _m: &ModelRef) -> anyhow::Result<()> {
        unimplemented!("M2")
    }
    async fn warmup(&self, _h: &RuntimeHandle, _m: &ModelRef) -> anyhow::Result<()> {
        unimplemented!("M2")
    }
    fn capability_matrix(&self) -> CapabilityMatrix {
        CapabilityMatrix {
            vision: false,
            tools: true,
            structured_output: true,
            embeddings: true,
        }
    }
}
