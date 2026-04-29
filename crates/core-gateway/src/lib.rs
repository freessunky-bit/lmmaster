//! crate: core-gateway — localhost-only HTTP gateway.
//!
//! 정책 (ADR-0001, ADR-0006, ADR-0022):
//! - 127.0.0.1 바인딩. 외부 노출 금지.
//! - OpenAI-compatible REST + SSE를 1차로 노출.
//! - 모든 endpoint에 API key 의무 (단 /health, /capabilities는 무인증).
//! - raw runtime port는 직접 노출하지 않는다.
//!
//! Phase 3'.a 책임 영역:
//! - OpenAI 호환 4 endpoint: `/v1/chat/completions`, `/v1/embeddings`, `/v1/models`, `/v1/models/:id`.
//! - SSE byte-perfect pass-through (axum::Sse 재구성 안 함).
//! - 글로벌 semaphore (permits=1)로 GPU contention 직렬화.
//! - chat/completions 라우트는 별도 600s timeout (장시간 generation 대비).

use std::time::Duration;

use axum::{extract::DefaultBodyLimit, http::StatusCode, middleware, Router};
use tower::ServiceBuilder;
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

pub mod auth;
pub mod config;
pub mod openai_error;
pub mod pipeline_layer;
pub mod routes;
pub mod shutdown;
pub mod state;
pub mod upstream;
pub mod usage_log;

pub use auth::{AuthState, Principal};
pub use config::GatewayConfig;
pub use pipeline_layer::{PipelineLayer, PipelineMiddleware};
pub use state::AppState;
pub use upstream::{ModelDescriptor, StaticProvider, UpstreamProvider, UpstreamRoute};

/// Gateway router 빌드.
///
/// Phase 3'.a 라우트:
/// - `GET /health`, `GET /capabilities` (무인증)
/// - `POST /v1/chat/completions` (stream 지원, 별도 600s timeout)
/// - `GET /v1/models`, `GET /v1/models/:id`
///
/// 미들웨어 스택(외→내): RequestId set → Trace → RequestId propagate →
/// Timeout(30s, 408 status) → CORS(permissive).
/// 채팅/embeddings 라우트는 별도 600s TimeoutLayer로 wrap (장시간 generation).
pub fn build_router(_cfg: GatewayConfig, state: AppState) -> Router {
    // 채팅 라우트는 별도 timeout 600s.
    let mut chat_router = routes::chat::router(state.clone()).layer(
        TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(600)),
    );
    let mut models_router = routes::models::router(state.clone());

    // auth 미들웨어 — KeyManager가 주입되어 있으면 /v1/* 라우트에 mount.
    if let Some(km) = state.key_manager.clone() {
        let auth_state = AuthState::new(km);
        let auth_layer = middleware::from_fn_with_state(auth_state, auth::require_api_key);
        chat_router = chat_router.layer(auth_layer.clone());
        models_router = models_router.layer(auth_layer);
    }

    Router::new()
        .merge(routes::health::router())
        .merge(chat_router)
        .merge(models_router)
        .layer(
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                .layer(TraceLayer::new_for_http())
                .layer(PropagateRequestIdLayer::x_request_id())
                .layer(TimeoutLayer::with_status_code(
                    StatusCode::REQUEST_TIMEOUT,
                    Duration::from_secs(30),
                )),
        )
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
    // CORS는 auth 미들웨어가 책임 — 외부 permissive layer는 ACAO를 *로 덮어쓰므로 제거.
    // OPTIONS preflight + actual request의 ACAO 모두 auth.rs가 origin echo (whitelist 검증된 키만).
}

/// 기존 router에 `PipelineLayer`를 mount — Phase 6'.b opt-in 통합.
///
/// 정책:
/// - 본 함수는 *opt-in* — 기본 `build_router`는 Pipelines 미적용 (호환성 유지).
/// - chain은 `/v1/*` 라우트(특히 chat/completions, embeddings)에만 의미. `/health`/`/capabilities`는
///   짧은 비-JSON 응답이라 Pipeline이 자동 skip.
/// - SSE relay는 PipelineLayer가 content-type으로 자동 감지 → byte-perfect pass-through.
pub fn with_pipelines(router: Router, chain: pipelines::PipelineChain) -> Router {
    router.layer(pipeline_layer::PipelineLayer::new(chain))
}

/// `with_pipelines`의 audit 채널 변형 — Phase 6'.d.
///
/// 정책:
/// - audit_sender는 게이트웨이가 처리하는 매 요청의 `AuditEntry`를 best-effort try_send.
/// - capacity 권장 256 (gateway burst 흡수). 호출자(예: Tauri PipelinesState)가 capacity 결정.
/// - 호환성: `with_pipelines`는 그대로 유지 — 본 함수는 audit 채널 주입이 필요한 빌더.
pub fn with_pipelines_audited(
    router: Router,
    chain: pipelines::PipelineChain,
    audit_sender: tokio::sync::mpsc::Sender<pipelines::AuditEntry>,
) -> Router {
    router.layer(pipeline_layer::PipelineLayer::new(chain).with_audit_channel(audit_sender))
}

/// listener에서 gateway를 동작시키고 cancellation 신호를 받으면 graceful shutdown.
pub async fn serve_with_shutdown(
    listener: tokio::net::TcpListener,
    router: Router,
    cancel: tokio_util::sync::CancellationToken,
) -> std::io::Result<()> {
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            cancel.cancelled().await;
            tracing::info!("gateway received cancellation signal");
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::routing::post;
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tower::ServiceExt;

    /// `with_pipelines`은 호환성 유지 — audit 채널 없이도 chain이 적용돼요.
    #[tokio::test]
    async fn with_pipelines_keeps_existing_behavior() {
        let chain =
            pipelines::PipelineChain::new().add(Arc::new(pipelines::PiiRedactPipeline::new()));
        let router = Router::new().route(
            "/echo",
            post(|body: axum::body::Bytes| async move {
                axum::response::Response::builder()
                    .status(StatusCode::OK)
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap()
            }),
        );
        let router = with_pipelines(router, chain);

        let req = Request::builder()
            .method("POST")
            .uri("/echo")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"messages":[{"role":"user","content":"alice@example.com"}]}"#,
            ))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(v["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("[REDACTED-이메일]"));
    }

    /// `with_pipelines_audited`은 audit_sender를 등록 + 실제 audit entry가 채널로 도착해야 해요.
    #[tokio::test]
    async fn with_pipelines_audited_registers_audit_channel_and_receives_entry() {
        let (tx, mut rx) = mpsc::channel::<pipelines::AuditEntry>(8);
        let chain =
            pipelines::PipelineChain::new().add(Arc::new(pipelines::PiiRedactPipeline::new()));

        let router = Router::new().route(
            "/echo",
            post(|body: axum::body::Bytes| async move {
                axum::response::Response::builder()
                    .status(StatusCode::OK)
                    .header(axum::http::header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap()
            }),
        );
        let router = with_pipelines_audited(router, chain, tx);

        let req = Request::builder()
            .method("POST")
            .uri("/echo")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"messages":[{"role":"user","content":"a@b.com"}]}"#,
            ))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let entry = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .expect("audit recv timeout")
            .expect("channel closed");
        assert_eq!(entry.pipeline_id, "pii-redact");
        assert_eq!(entry.action, "modified");
    }
}
