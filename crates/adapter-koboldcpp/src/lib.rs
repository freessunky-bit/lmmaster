//! adapter-koboldcpp — character/RP 카테고리 옵션.
//!
//! 라이선스 주의 (docs/oss-dependencies.md L2):
//! - KoboldCpp는 AGPLv3.
//! - v1 통합 방식은 attach 우선(사용자가 별도 설치한 인스턴스에 HTTP 연결).
//! - 자동 다운로드 형태 결합은 법무 검토 후 결정.

use async_trait::async_trait;
use runtime_manager::{
    DetectResult, HealthReport, InstallOpts, LocalModel, ProgressSink, RuntimeAdapter, RuntimeCfg,
    RuntimeHandle,
};
use shared_types::{CapabilityMatrix, ModelRef, RuntimeKind};

pub struct KoboldCppAdapter;

impl KoboldCppAdapter {
    pub fn new() -> Self {
        Self
    }
}
impl Default for KoboldCppAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RuntimeAdapter for KoboldCppAdapter {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::KoboldCpp
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
            vision: false,
            tools: false,
            structured_output: false,
            embeddings: false,
        }
    }
}
