//! `RuntimeManager` — `RuntimeAdapter` trait object registry.
//!
//! 정책 (ADR-0004):
//! - 모든 어댑터는 `Arc<dyn RuntimeAdapter>`로 등록 — `if RuntimeKind == ...` 분기 금지.
//! - `priority()`는 Gateway routing 시 사용 (Phase 3'). v1 우선순위:
//!   Ollama / LM Studio = 1순위 (HTTP attach, 외부 설치형).
//!   llama.cpp / KoboldCpp / vLLM = 2순위+ (Phase 5'+).

use std::collections::HashMap;
use std::sync::Arc;

use shared_types::RuntimeKind;

use crate::RuntimeAdapter;

#[derive(Default)]
pub struct RuntimeManager {
    adapters: HashMap<RuntimeKind, Arc<dyn RuntimeAdapter>>,
}

impl RuntimeManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 어댑터 등록. 같은 kind 재등록 시 교체.
    pub fn register(&mut self, adapter: Arc<dyn RuntimeAdapter>) {
        let kind = adapter.kind();
        self.adapters.insert(kind, adapter);
    }

    /// kind에 등록된 어댑터 조회.
    pub fn get(&self, kind: RuntimeKind) -> Option<Arc<dyn RuntimeAdapter>> {
        self.adapters.get(&kind).cloned()
    }

    /// 등록된 모든 kind 목록.
    pub fn list_kinds(&self) -> Vec<RuntimeKind> {
        self.adapters.keys().copied().collect()
    }

    /// Gateway routing용 priority — 작을수록 우선.
    /// v1: Ollama / LM Studio = 1, llama.cpp = 2, KoboldCpp = 3, vLLM = 4.
    pub fn priority(kind: RuntimeKind) -> u8 {
        match kind {
            RuntimeKind::Ollama | RuntimeKind::LmStudio => 1,
            RuntimeKind::LlamaCpp => 2,
            RuntimeKind::KoboldCpp => 3,
            RuntimeKind::Vllm => 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use shared_types::{CapabilityMatrix, ModelRef};

    use crate::{
        DetectResult, HealthReport, InstallOpts, LocalModel, ProgressSink, RuntimeAdapter,
        RuntimeCfg, RuntimeHandle,
    };

    struct MockAdapter(RuntimeKind);

    #[async_trait]
    impl RuntimeAdapter for MockAdapter {
        fn kind(&self) -> RuntimeKind {
            self.0
        }
        async fn detect(&self) -> anyhow::Result<DetectResult> {
            Ok(DetectResult {
                installed: false,
                version: None,
                build_target: None,
            })
        }
        async fn install(&self, _: InstallOpts) -> anyhow::Result<()> {
            anyhow::bail!("mock")
        }
        async fn update(&self) -> anyhow::Result<()> {
            anyhow::bail!("mock")
        }
        async fn start(&self, _: RuntimeCfg) -> anyhow::Result<RuntimeHandle> {
            anyhow::bail!("mock")
        }
        async fn stop(&self, _: &RuntimeHandle) -> anyhow::Result<()> {
            Ok(())
        }
        async fn restart(&self, _: &RuntimeHandle) -> anyhow::Result<()> {
            Ok(())
        }
        async fn health(&self, _: &RuntimeHandle) -> HealthReport {
            HealthReport::default()
        }
        async fn list_models(&self) -> anyhow::Result<Vec<LocalModel>> {
            Ok(vec![])
        }
        async fn pull_model(&self, _: &ModelRef, _: ProgressSink) -> anyhow::Result<()> {
            Ok(())
        }
        async fn remove_model(&self, _: &ModelRef) -> anyhow::Result<()> {
            Ok(())
        }
        async fn warmup(&self, _: &RuntimeHandle, _: &ModelRef) -> anyhow::Result<()> {
            Ok(())
        }
        fn capability_matrix(&self) -> CapabilityMatrix {
            CapabilityMatrix::default()
        }
    }

    #[test]
    fn register_and_get() {
        let mut m = RuntimeManager::new();
        m.register(Arc::new(MockAdapter(RuntimeKind::Ollama)));
        assert!(m.get(RuntimeKind::Ollama).is_some());
        assert!(m.get(RuntimeKind::LmStudio).is_none());
    }

    #[test]
    fn re_register_replaces() {
        let mut m = RuntimeManager::new();
        m.register(Arc::new(MockAdapter(RuntimeKind::Ollama)));
        m.register(Arc::new(MockAdapter(RuntimeKind::Ollama)));
        assert_eq!(m.list_kinds().len(), 1);
    }

    #[test]
    fn list_kinds_returns_all_registered() {
        let mut m = RuntimeManager::new();
        m.register(Arc::new(MockAdapter(RuntimeKind::Ollama)));
        m.register(Arc::new(MockAdapter(RuntimeKind::LmStudio)));
        let mut kinds = m.list_kinds();
        kinds.sort_by_key(|k| RuntimeManager::priority(*k));
        assert_eq!(kinds.len(), 2);
    }

    #[test]
    fn priority_ollama_lmstudio_first() {
        assert_eq!(RuntimeManager::priority(RuntimeKind::Ollama), 1);
        assert_eq!(RuntimeManager::priority(RuntimeKind::LmStudio), 1);
        assert!(
            RuntimeManager::priority(RuntimeKind::LlamaCpp)
                > RuntimeManager::priority(RuntimeKind::Ollama)
        );
        assert!(
            RuntimeManager::priority(RuntimeKind::Vllm)
                > RuntimeManager::priority(RuntimeKind::KoboldCpp)
        );
    }
}
