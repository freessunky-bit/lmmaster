//! Upstream provider — gateway가 어댑터에 직접 의존하지 않도록 thin trait.
//!
//! 정책 (ADR-0022 §4):
//! - desktop app이 OllamaAdapter/LmStudioAdapter를 wrap한 `RegistryProvider`를 등록.
//! - gateway는 이 trait를 통해 모델 → 업스트림 URL dispatch.
//! - 5초 TTL 캐시는 provider 측에서 책임 (gateway는 stateless).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use shared_types::RuntimeKind;

/// 단일 모델의 업스트림 라우트 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamRoute {
    pub runtime: RuntimeKind,
    /// 업스트림 base URL (예: `http://127.0.0.1:11434`). path는 gateway가 붙임.
    pub base_url: String,
}

/// `/v1/models` 응답용 모델 메타.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDescriptor {
    pub id: String,
    pub owned_by: String,
}

#[async_trait]
pub trait UpstreamProvider: Send + Sync {
    /// 모델 id → 업스트림 라우트. 매핑 없으면 None.
    async fn upstream_for(&self, model: &str) -> Option<UpstreamRoute>;

    /// 모든 어댑터에서 모델 목록 합산. OpenAI 호환 응답에 사용.
    async fn list_all_models(&self) -> Vec<ModelDescriptor>;
}

/// 테스트용 in-memory provider — 모델 → URL 정적 매핑.
#[derive(Debug, Clone, Default)]
pub struct StaticProvider {
    pub routes: Vec<(String, UpstreamRoute)>,
}

impl StaticProvider {
    pub fn new(routes: Vec<(String, UpstreamRoute)>) -> Self {
        Self { routes }
    }
}

#[async_trait]
impl UpstreamProvider for StaticProvider {
    async fn upstream_for(&self, model: &str) -> Option<UpstreamRoute> {
        self.routes
            .iter()
            .find(|(id, _)| id == model)
            .map(|(_, r)| r.clone())
    }

    async fn list_all_models(&self) -> Vec<ModelDescriptor> {
        self.routes
            .iter()
            .map(|(id, r)| ModelDescriptor {
                id: id.clone(),
                owned_by: runtime_label(r.runtime).to_string(),
            })
            .collect()
    }
}

pub fn runtime_label(rk: RuntimeKind) -> &'static str {
    match rk {
        RuntimeKind::Ollama => "ollama",
        RuntimeKind::LmStudio => "lmstudio",
        RuntimeKind::LlamaCpp => "llama-cpp",
        RuntimeKind::KoboldCpp => "kobold",
        RuntimeKind::Vllm => "vllm",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route(rt: RuntimeKind, base: &str) -> UpstreamRoute {
        UpstreamRoute {
            runtime: rt,
            base_url: base.into(),
        }
    }

    #[tokio::test]
    async fn static_provider_lookup_hit() {
        let p = StaticProvider::new(vec![(
            "exaone".into(),
            route(RuntimeKind::Ollama, "http://127.0.0.1:11434"),
        )]);
        let r = p.upstream_for("exaone").await.unwrap();
        assert_eq!(r.runtime, RuntimeKind::Ollama);
    }

    #[tokio::test]
    async fn static_provider_lookup_miss() {
        let p = StaticProvider::default();
        assert!(p.upstream_for("missing").await.is_none());
    }

    #[tokio::test]
    async fn list_all_models_includes_owned_by_runtime_label() {
        let p = StaticProvider::new(vec![
            ("exaone".into(), route(RuntimeKind::Ollama, "http://x")),
            ("llama".into(), route(RuntimeKind::LmStudio, "http://y")),
        ]);
        let models = p.list_all_models().await;
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].owned_by, "ollama");
        assert_eq!(models[1].owned_by, "lmstudio");
    }
}
