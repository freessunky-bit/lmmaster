//! `LiveRegistryProvider` — gateway가 실제 외부 런타임으로 dispatch하는 `UpstreamProvider` 구현.
//!
//! 정책 (ADR-0006, ADR-0014, ADR-0022):
//! - **Wrap-not-replace**: Ollama / LM Studio 데몬에 HTTP attach. 우리는 모델 라우팅만 해요.
//! - **Local-first dispatch**: 같은 모델 ID가 양쪽에 있으면 Ollama 우선 (priority 1, 같지만 코드상 ollama_first
//!   tie-break). MIT + 자동 설치 가능 + 한국어 모델 가용성 우위.
//! - **5초 모델 목록 TTL 캐시**: `list_all_models` 매 호출마다 어댑터를 두드리지 않도록.
//! - **Graceful degrade**: 어댑터 list_models 실패 시 빈 리스트로 폴백 (다른 어댑터는 살아있음).
//! - **외부 통신 0**: localhost:11434 / localhost:1234만 노출. cloud fallback 절대 없음.
//!
//! 라우팅 알고리즘 (ADR-0022 §2):
//! 1. 캐시(또는 fresh fetch)에서 어댑터별 모델 목록 가져옴.
//! 2. 모델 ID exact match — 둘 다 보유하면 Ollama 우선.
//! 3. 매치 없으면 None → 게이트웨이가 OpenAI 호환 404 envelope 반환.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use core_gateway::{ModelDescriptor, UpstreamProvider, UpstreamRoute};
use runtime_manager::RuntimeAdapter;
use shared_types::RuntimeKind;
use tokio::sync::RwLock;

/// 어댑터별 base URL 매핑. Ollama / LM Studio는 외부 설치형이라 고정 포트가 일반적.
#[derive(Debug, Clone)]
pub struct RuntimeEndpoints {
    pub ollama: Option<String>,
    pub lmstudio: Option<String>,
}

impl RuntimeEndpoints {
    /// 기본 — Ollama 11434 / LM Studio 1234.
    pub fn defaults() -> Self {
        Self {
            ollama: Some("http://127.0.0.1:11434".to_string()),
            lmstudio: Some("http://127.0.0.1:1234".to_string()),
        }
    }
}

/// 5초 TTL 모델 목록 캐시 — 매 OpenAI request마다 list_models 호출하면 업스트림 부하 + 지연.
#[derive(Default)]
struct ModelListCache {
    /// (runtime, fetched_at, ids).
    fetched_at: Option<Instant>,
    by_runtime: HashMap<RuntimeKind, Vec<String>>,
}

impl ModelListCache {
    fn is_fresh(&self, ttl: Duration) -> bool {
        self.fetched_at.map(|t| t.elapsed() < ttl).unwrap_or(false)
    }
}

const CACHE_TTL: Duration = Duration::from_secs(5);

/// gateway에 주입되는 실시간 provider.
pub struct LiveRegistryProvider {
    /// 등록된 어댑터 — RuntimeKind → 어댑터 + base URL.
    adapters: Vec<RegisteredAdapter>,
    cache: RwLock<ModelListCache>,
}

struct RegisteredAdapter {
    kind: RuntimeKind,
    adapter: Arc<dyn RuntimeAdapter>,
    base_url: String,
}

impl LiveRegistryProvider {
    /// 명시적 어댑터 목록으로 빌드 — 테스트에서 wiremock URL 주입 시 사용.
    pub fn with_adapters(adapters: Vec<(Arc<dyn RuntimeAdapter>, String)>) -> Self {
        let registered = adapters
            .into_iter()
            .map(|(adapter, base_url)| RegisteredAdapter {
                kind: adapter.kind(),
                adapter,
                base_url,
            })
            .collect();
        Self {
            adapters: registered,
            cache: RwLock::new(ModelListCache::default()),
        }
    }

    /// 환경 detect 결과 기반 자동 빌드 — Ollama / LM Studio 둘 다 시도해 detected만 등록.
    /// Phase 3'.c+: setup() 시점에 호출. 어댑터가 0개여도 OK — `/v1/models`만 빈 리스트로 응답.
    pub async fn from_environment(endpoints: RuntimeEndpoints) -> Self {
        let mut adapters: Vec<(Arc<dyn RuntimeAdapter>, String)> = Vec::new();

        if let Some(url) = endpoints.ollama {
            let adapter = Arc::new(adapter_ollama::OllamaAdapter::with_endpoint(url.clone()))
                as Arc<dyn RuntimeAdapter>;
            // detect는 비차단 — 실패해도 (cold start 직후 등) 등록은 함. 실제 list_models는
            // 매 요청 시 graceful degrade.
            tracing::info!(url = %url, "registering Ollama adapter for gateway routing");
            adapters.push((adapter, url));
        }

        if let Some(url) = endpoints.lmstudio {
            let adapter = Arc::new(adapter_lmstudio::LmStudioAdapter::with_endpoint(
                url.clone(),
            )) as Arc<dyn RuntimeAdapter>;
            tracing::info!(url = %url, "registering LM Studio adapter for gateway routing");
            adapters.push((adapter, url));
        }

        Self::with_adapters(adapters)
    }

    /// 캐시된 모델 목록 — fresh면 그대로, 아니면 fetch.
    async fn ensure_cache(&self) -> HashMap<RuntimeKind, Vec<String>> {
        {
            let g = self.cache.read().await;
            if g.is_fresh(CACHE_TTL) {
                return g.by_runtime.clone();
            }
        }
        // miss — write lock 잡고 다시 확인 (double-checked).
        let mut g = self.cache.write().await;
        if g.is_fresh(CACHE_TTL) {
            return g.by_runtime.clone();
        }
        let mut by_runtime: HashMap<RuntimeKind, Vec<String>> = HashMap::new();
        for reg in &self.adapters {
            match reg.adapter.list_models().await {
                Ok(models) => {
                    let ids: Vec<String> = models.into_iter().map(|m| m.file_rel_path).collect();
                    tracing::debug!(
                        runtime = ?reg.kind,
                        count = ids.len(),
                        "list_models success"
                    );
                    by_runtime.insert(reg.kind, ids);
                }
                Err(e) => {
                    tracing::warn!(
                        runtime = ?reg.kind,
                        error = %e,
                        "list_models 실패 — 빈 목록으로 폴백해요"
                    );
                    by_runtime.insert(reg.kind, Vec::new());
                }
            }
        }
        g.by_runtime = by_runtime.clone();
        g.fetched_at = Some(Instant::now());
        by_runtime
    }

    /// 어댑터 base URL 조회.
    fn base_url_of(&self, kind: RuntimeKind) -> Option<&str> {
        self.adapters
            .iter()
            .find(|r| r.kind == kind)
            .map(|r| r.base_url.as_str())
    }
}

#[async_trait]
impl UpstreamProvider for LiveRegistryProvider {
    async fn upstream_for(&self, model: &str) -> Option<UpstreamRoute> {
        if model.is_empty() {
            return None;
        }
        let by_runtime = self.ensure_cache().await;

        // Local-first: Ollama 우선, 그 다음 LM Studio. 두 어댑터 모두 보유 시 Ollama가 이김.
        // ADR-0022 §2 invariant: deterministic — 같은 (catalog, request) → 같은 라우트.
        for kind in [RuntimeKind::Ollama, RuntimeKind::LmStudio] {
            if let Some(ids) = by_runtime.get(&kind) {
                if ids.iter().any(|id| id == model) {
                    if let Some(base) = self.base_url_of(kind) {
                        return Some(UpstreamRoute {
                            runtime: kind,
                            base_url: base.to_string(),
                        });
                    }
                }
            }
        }

        // 향후 llama.cpp 자식 프로세스 모드 (Phase 5'+) — 여기에 추가.
        None
    }

    async fn list_all_models(&self) -> Vec<ModelDescriptor> {
        let by_runtime = self.ensure_cache().await;
        let mut out = Vec::new();
        // Ollama 먼저 → LM Studio 순으로 dedupe.
        // 같은 모델 ID가 두 어댑터에 있으면 Ollama 게시본만 남김 (라우팅과 일관성).
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for kind in [RuntimeKind::Ollama, RuntimeKind::LmStudio] {
            if let Some(ids) = by_runtime.get(&kind) {
                let label = match kind {
                    RuntimeKind::Ollama => "ollama",
                    RuntimeKind::LmStudio => "lmstudio",
                    _ => continue,
                };
                for id in ids {
                    if seen.insert(id.clone()) {
                        out.push(ModelDescriptor {
                            id: id.clone(),
                            owned_by: label.to_string(),
                        });
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adapter_lmstudio::LmStudioAdapter;
    use adapter_ollama::OllamaAdapter;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Ollama wiremock — `/api/tags` 응답.
    async fn mock_ollama_with_models(server: &MockServer, model_ids: &[&str]) {
        let body = serde_json::json!({
            "models": model_ids.iter().map(|id| serde_json::json!({
                "name": id,
                "size": 1u64,
                "digest": "abc"
            })).collect::<Vec<_>>()
        });
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(server)
            .await;
    }

    /// LM Studio wiremock — `/v1/models` 응답.
    async fn mock_lmstudio_with_models(server: &MockServer, model_ids: &[&str]) {
        let body = serde_json::json!({
            "data": model_ids.iter().map(|id| serde_json::json!({
                "id": id,
                "object": "model"
            })).collect::<Vec<_>>()
        });
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(server)
            .await;
    }

    /// Ollama가 503 등 오류 시 — list_models 자체가 실패.
    async fn mock_ollama_error(server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(503).set_body_string("upstream down"))
            .mount(server)
            .await;
    }

    fn make_provider_with(
        ollama_uri: Option<&str>,
        lmstudio_uri: Option<&str>,
    ) -> LiveRegistryProvider {
        let mut adapters: Vec<(Arc<dyn RuntimeAdapter>, String)> = Vec::new();
        if let Some(u) = ollama_uri {
            let a =
                Arc::new(OllamaAdapter::with_endpoint(u.to_string())) as Arc<dyn RuntimeAdapter>;
            adapters.push((a, u.to_string()));
        }
        if let Some(u) = lmstudio_uri {
            let a =
                Arc::new(LmStudioAdapter::with_endpoint(u.to_string())) as Arc<dyn RuntimeAdapter>;
            adapters.push((a, u.to_string()));
        }
        LiveRegistryProvider::with_adapters(adapters)
    }

    #[tokio::test]
    async fn upstream_for_ollama_only_routes_to_ollama() {
        let server = MockServer::start().await;
        mock_ollama_with_models(&server, &["exaone:1.2b"]).await;

        let provider = make_provider_with(Some(&server.uri()), None);
        let route = provider.upstream_for("exaone:1.2b").await.unwrap();
        assert_eq!(route.runtime, RuntimeKind::Ollama);
        assert_eq!(route.base_url, server.uri());
    }

    #[tokio::test]
    async fn upstream_for_lmstudio_only_routes_to_lmstudio() {
        let server = MockServer::start().await;
        mock_lmstudio_with_models(&server, &["llama-3.5-7b"]).await;

        let provider = make_provider_with(None, Some(&server.uri()));
        let route = provider.upstream_for("llama-3.5-7b").await.unwrap();
        assert_eq!(route.runtime, RuntimeKind::LmStudio);
        assert_eq!(route.base_url, server.uri());
    }

    #[tokio::test]
    async fn upstream_for_model_in_both_prefers_ollama() {
        // ADR-0022 §2 invariant — local-first: Ollama 우선.
        let ollama = MockServer::start().await;
        mock_ollama_with_models(&ollama, &["shared-model"]).await;
        let lms = MockServer::start().await;
        mock_lmstudio_with_models(&lms, &["shared-model"]).await;

        let provider = make_provider_with(Some(&ollama.uri()), Some(&lms.uri()));
        let route = provider.upstream_for("shared-model").await.unwrap();
        assert_eq!(route.runtime, RuntimeKind::Ollama);
        assert_eq!(route.base_url, ollama.uri());
    }

    #[tokio::test]
    async fn upstream_for_unknown_model_returns_none() {
        let server = MockServer::start().await;
        mock_ollama_with_models(&server, &["a"]).await;
        let provider = make_provider_with(Some(&server.uri()), None);
        assert!(provider.upstream_for("nope").await.is_none());
    }

    #[tokio::test]
    async fn upstream_for_empty_id_returns_none() {
        let provider = make_provider_with(None, None);
        assert!(provider.upstream_for("").await.is_none());
    }

    #[tokio::test]
    async fn upstream_for_no_adapters_returns_none() {
        let provider = make_provider_with(None, None);
        assert!(provider.upstream_for("anything").await.is_none());
    }

    #[tokio::test]
    async fn list_all_models_dedupes_keeping_ollama_first() {
        let ollama = MockServer::start().await;
        mock_ollama_with_models(&ollama, &["dup", "ollama-only"]).await;
        let lms = MockServer::start().await;
        mock_lmstudio_with_models(&lms, &["dup", "lms-only"]).await;

        let provider = make_provider_with(Some(&ollama.uri()), Some(&lms.uri()));
        let models = provider.list_all_models().await;

        let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        assert!(ids.contains(&"dup"));
        assert!(ids.contains(&"ollama-only"));
        assert!(ids.contains(&"lms-only"));

        // dup은 Ollama owned_by.
        let dup = models.iter().find(|m| m.id == "dup").unwrap();
        assert_eq!(dup.owned_by, "ollama");
        let lms_only = models.iter().find(|m| m.id == "lms-only").unwrap();
        assert_eq!(lms_only.owned_by, "lmstudio");
    }

    #[tokio::test]
    async fn list_all_models_with_no_adapters_is_empty() {
        let provider = make_provider_with(None, None);
        let models = provider.list_all_models().await;
        assert!(models.is_empty());
    }

    #[tokio::test]
    async fn list_models_failure_in_one_adapter_does_not_fail_others() {
        // Ollama 503, LM Studio OK — LM Studio 모델은 여전히 노출돼야 해요.
        let ollama = MockServer::start().await;
        mock_ollama_error(&ollama).await;
        let lms = MockServer::start().await;
        mock_lmstudio_with_models(&lms, &["x"]).await;

        let provider = make_provider_with(Some(&ollama.uri()), Some(&lms.uri()));
        let models = provider.list_all_models().await;
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "x");
        assert_eq!(models[0].owned_by, "lmstudio");
    }

    #[tokio::test]
    async fn cache_returns_same_results_within_ttl() {
        // 같은 wiremock 인스턴스에 같은 query를 두 번 — 캐시 hit이라면 wiremock의 expect(1) 위반 안 함.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "models": [{"name": "exaone:1.2b", "size": 1u64, "digest": "z"}]
            })))
            .expect(1) // 정확히 1번만 호출되어야 캐시가 작동.
            .mount(&server)
            .await;

        let provider = make_provider_with(Some(&server.uri()), None);

        let r1 = provider.upstream_for("exaone:1.2b").await;
        let r2 = provider.upstream_for("exaone:1.2b").await;
        assert!(r1.is_some());
        assert!(r2.is_some());

        // server.verify()는 Drop 시점에 자동으로 expect 검증.
    }

    #[tokio::test]
    async fn from_environment_registers_both_runtimes() {
        let endpoints = RuntimeEndpoints {
            ollama: Some("http://127.0.0.1:11434".into()),
            lmstudio: Some("http://127.0.0.1:1234".into()),
        };
        let provider = LiveRegistryProvider::from_environment(endpoints).await;
        assert_eq!(provider.adapters.len(), 2);
    }

    #[tokio::test]
    async fn from_environment_with_no_endpoints_registers_zero() {
        let endpoints = RuntimeEndpoints {
            ollama: None,
            lmstudio: None,
        };
        let provider = LiveRegistryProvider::from_environment(endpoints).await;
        assert_eq!(provider.adapters.len(), 0);
    }

    #[tokio::test]
    async fn upstream_for_returns_correct_base_url_per_runtime() {
        let ollama = MockServer::start().await;
        mock_ollama_with_models(&ollama, &["m1"]).await;
        let lms = MockServer::start().await;
        mock_lmstudio_with_models(&lms, &["m2"]).await;

        let provider = make_provider_with(Some(&ollama.uri()), Some(&lms.uri()));
        let r1 = provider.upstream_for("m1").await.unwrap();
        let r2 = provider.upstream_for("m2").await.unwrap();
        assert_eq!(r1.base_url, ollama.uri());
        assert_eq!(r2.base_url, lms.uri());
    }

    /// Gateway routing 통합 — wiremock 업스트림 + LiveRegistryProvider + chat completions
    /// 호출 → 업스트림으로 정확하게 forward되는지.
    #[tokio::test]
    async fn provider_integrates_with_gateway_chat_completions() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use core_gateway::{build_router, AppState, GatewayConfig};
        use http_body_util::BodyExt;
        use tower::util::ServiceExt;

        let upstream = MockServer::start().await;
        // /api/tags — provider list_models가 사용.
        mock_ollama_with_models(&upstream, &["my-model"]).await;
        // /v1/chat/completions — gateway가 forward할 endpoint.
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "x",
                "choices": [{"message":{"role":"assistant","content":"안녕하세요"}}]
            })))
            .mount(&upstream)
            .await;

        let provider = Arc::new(make_provider_with(Some(&upstream.uri()), None));
        let state = AppState::new(provider);
        let app = build_router(GatewayConfig::default(), state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/chat/completions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"model":"my-model","messages":[{"role":"user","content":"hi"}]}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            v["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or_default(),
            "안녕하세요"
        );
    }

    /// 미상 모델 → 404 + Korean envelope.
    #[tokio::test]
    async fn provider_integrates_unknown_model_returns_korean_404() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use core_gateway::{build_router, AppState, GatewayConfig};
        use http_body_util::BodyExt;
        use tower::util::ServiceExt;

        let upstream = MockServer::start().await;
        mock_ollama_with_models(&upstream, &["other"]).await;

        let provider = Arc::new(make_provider_with(Some(&upstream.uri()), None));
        let state = AppState::new(provider);
        let app = build_router(GatewayConfig::default(), state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/chat/completions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"model":"missing","messages":[{"role":"user","content":"hi"}]}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let msg = v["error"]["message"].as_str().unwrap();
        // Korean error message invariant — `model_not_found` envelope.
        assert!(msg.contains("찾을 수 없"), "expected Korean error: {msg}");
    }

    /// SSE byte-perfect — provider 경유해도 chunk 누락 없이 그대로 흘려보내야.
    #[tokio::test]
    async fn provider_integrates_sse_byte_perfect_via_provider() {
        use axum::body::{to_bytes, Body};
        use axum::http::{Request, StatusCode};
        use core_gateway::{build_router, AppState, GatewayConfig};
        use tower::util::ServiceExt;

        let upstream = MockServer::start().await;
        mock_ollama_with_models(&upstream, &["m"]).await;
        let body = "data: {\"choices\":[{\"delta\":{\"content\":\"안\"}}]}\n\n\
                    data: {\"choices\":[{\"delta\":{\"content\":\"녕\"}}]}\n\n\
                    data: [DONE]\n\n";
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(body)
                    .insert_header("content-type", "text/event-stream"),
            )
            .mount(&upstream)
            .await;

        let provider = Arc::new(make_provider_with(Some(&upstream.uri()), None));
        let state = AppState::new(provider);
        let app = build_router(GatewayConfig::default(), state);

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/chat/completions")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"model":"m","stream":true,"messages":[{"role":"user","content":"hi"}]}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), 8192).await.unwrap();
        assert_eq!(std::str::from_utf8(&bytes).unwrap(), body);
    }
}
