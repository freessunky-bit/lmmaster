//! Phase 3'.a — Gateway 라우팅 통합 테스트.
//!
//! 검증 invariant (ADR-0022):
//! - byte-perfect SSE relay (chunk 순서 + 개행 정확).
//! - model_not_found OpenAI 호환 envelope.
//! - 업스트림 5xx 시 502/503 + envelope 보존.
//! - /v1/models 합산 + owned_by 정확.
//! - semaphore 직렬화 (동시 2 request → 두 번째 block).
//! - x-lmmaster-queue-wait-ms 헤더 존재.
//! - body.model 누락 → 400 invalid_request_error.

use std::sync::Arc;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use core_gateway::{
    build_router, AppState, GatewayConfig, ModelDescriptor, StaticProvider, UpstreamProvider,
    UpstreamRoute,
};
use http_body_util::BodyExt;
use shared_types::RuntimeKind;
use tower::util::ServiceExt;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn route(rt: RuntimeKind, base: &str) -> UpstreamRoute {
    UpstreamRoute {
        runtime: rt,
        base_url: base.into(),
    }
}

fn make_app(provider: Arc<dyn UpstreamProvider>) -> axum::Router {
    let state = AppState::new(provider);
    build_router(GatewayConfig::default(), state)
}

#[tokio::test]
async fn chat_completions_invalid_body_no_model_returns_400() {
    let app = make_app(Arc::new(StaticProvider::default()));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"messages":[]}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"]["type"], "invalid_request_error");
    assert_eq!(v["error"]["code"], "invalid_request_body");
}

#[tokio::test]
async fn chat_completions_unknown_model_returns_404() {
    let app = make_app(Arc::new(StaticProvider::default()));
    let response = app
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
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"]["type"], "not_found_error");
    assert_eq!(v["error"]["code"], "model_not_found");
    assert!(v["error"]["message"].as_str().unwrap().contains("missing"));
}

#[tokio::test]
async fn chat_completions_forwards_to_upstream_and_returns_json() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("content-type", "application/json"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "chatcmpl-x",
                "object": "chat.completion",
                "model": "test",
                "choices": [{"index":0,"message":{"role":"assistant","content":"안녕"},"finish_reason":"stop"}]
            })),
        )
        .mount(&upstream)
        .await;

    let provider = StaticProvider::new(vec![(
        "test".into(),
        route(RuntimeKind::Ollama, &upstream.uri()),
    )]);
    let app = make_app(Arc::new(provider));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"model":"test","messages":[{"role":"user","content":"hi"}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    // queue-wait 헤더 노출 검증.
    assert!(response.headers().get("x-lmmaster-queue-wait-ms").is_some());
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["choices"][0]["message"]["content"], "안녕");
}

#[tokio::test]
async fn chat_completions_propagates_upstream_5xx() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(500).set_body_string(
            r#"{"error":{"message":"upstream boom","type":"server_error","code":"internal"}}"#,
        ))
        .mount(&upstream)
        .await;

    let provider = StaticProvider::new(vec![(
        "test".into(),
        route(RuntimeKind::LmStudio, &upstream.uri()),
    )]);
    let app = make_app(Arc::new(provider));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"model":"test","messages":[{"role":"user","content":"hi"}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"]["type"], "upstream_error");
    assert!(v["error"]["message"]
        .as_str()
        .unwrap()
        .contains("upstream boom"));
}

#[tokio::test]
async fn chat_completions_unreachable_returns_502() {
    let provider = StaticProvider::new(vec![(
        "test".into(),
        route(RuntimeKind::Ollama, "http://127.0.0.1:65000"),
    )]);
    let app = make_app(Arc::new(provider));
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"model":"test","messages":[{"role":"user","content":"x"}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"]["type"], "upstream_error");
    assert_eq!(v["error"]["code"], "upstream_unreachable");
}

#[tokio::test]
async fn chat_completions_stream_byte_perfect_relay() {
    // 업스트림이 SSE bytes를 그대로 흘려보내고 클라이언트는 받은 chunk를 합쳐 검증.
    let upstream = MockServer::start().await;
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

    let provider = StaticProvider::new(vec![(
        "test".into(),
        route(RuntimeKind::Ollama, &upstream.uri()),
    )]);
    let app = make_app(Arc::new(provider));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"model":"test","stream":true,"messages":[{"role":"user","content":"hi"}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    let bytes = to_bytes(response.into_body(), 8192).await.unwrap();
    let received = std::str::from_utf8(&bytes).unwrap();
    // byte-perfect — chunk 순서 + 개행 정확 일치.
    assert_eq!(received, body);
}

#[tokio::test]
async fn list_models_aggregates_provider_results() {
    let provider = StaticProvider::new(vec![
        ("exaone".into(), route(RuntimeKind::Ollama, "http://x")),
        ("qwen".into(), route(RuntimeKind::LmStudio, "http://y")),
    ]);
    let app = make_app(Arc::new(provider));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/models")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["object"], "list");
    let data = v["data"].as_array().unwrap();
    assert_eq!(data.len(), 2);
    assert_eq!(data[0]["id"], "exaone");
    assert_eq!(data[0]["owned_by"], "ollama");
    assert_eq!(data[0]["object"], "model");
    assert_eq!(data[1]["id"], "qwen");
    assert_eq!(data[1]["owned_by"], "lmstudio");
}

#[tokio::test]
async fn retrieve_model_returns_404_when_missing() {
    let app = make_app(Arc::new(StaticProvider::default()));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/models/missing")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"]["code"], "model_not_found");
}

#[tokio::test]
async fn retrieve_model_returns_metadata_when_found() {
    let provider = StaticProvider::new(vec![(
        "exaone".into(),
        route(RuntimeKind::Ollama, "http://x"),
    )]);
    let app = make_app(Arc::new(provider));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/models/exaone")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["id"], "exaone");
    assert_eq!(v["owned_by"], "ollama");
}

#[tokio::test]
async fn semaphore_serializes_concurrent_requests() {
    // 업스트림이 200ms 지연. permits=1이라면 두 요청은 직렬화.
    // 주의: permit은 body stream이 drop/consume될 때 release되므로 body를 drain해야 측정 정확.
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(200))
                .set_body_json(serde_json::json!({
                    "choices":[{"message":{"content":"ok"}}]
                })),
        )
        .mount(&upstream)
        .await;

    let provider = Arc::new(StaticProvider::new(vec![(
        "test".into(),
        route(RuntimeKind::Ollama, &upstream.uri()),
    )]));
    let state = AppState::new(provider);
    let app = build_router(GatewayConfig::default(), state);

    let body = r#"{"model":"test","messages":[{"role":"user","content":"hi"}]}"#.to_string();
    let make_req = || {
        Request::builder()
            .method("POST")
            .uri("/v1/chat/completions")
            .header("content-type", "application/json")
            .body(Body::from(body.clone()))
            .unwrap()
    };

    async fn drain(app: axum::Router, req: Request<Body>) -> StatusCode {
        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status();
        let _ = resp.into_body().collect().await.unwrap().to_bytes();
        status
    }

    let started = std::time::Instant::now();
    let (a, b) = tokio::join!(
        drain(app.clone(), make_req()),
        drain(app.clone(), make_req()),
    );
    let elapsed = started.elapsed();
    assert!(a.is_success());
    assert!(b.is_success());
    // 두 요청 직렬화 시 ≥ 380ms (200+200 - 20 여유), 병렬이면 ~200ms.
    assert!(
        elapsed >= Duration::from_millis(380),
        "expected serialized (>=380ms), got {:?}",
        elapsed
    );
}

#[tokio::test]
async fn queue_wait_header_is_numeric() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"choices":[{"message":{"content":"x"}}]})),
        )
        .mount(&upstream)
        .await;
    let provider = Arc::new(StaticProvider::new(vec![(
        "test".into(),
        route(RuntimeKind::Ollama, &upstream.uri()),
    )]));
    let app = make_app(provider);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"model":"test","messages":[{"role":"user","content":"x"}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let h = response
        .headers()
        .get("x-lmmaster-queue-wait-ms")
        .unwrap()
        .to_str()
        .unwrap();
    let n: u64 = h.parse().expect("queue wait should be numeric");
    assert!(n < 5000, "queue wait should be small for first request");
}

#[tokio::test]
async fn upstream_404_returns_404_with_envelope() {
    let upstream = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(404).set_body_string(
                r#"{"error":{"message":"model 'x' not found","type":"not_found_error","code":"model_not_found"}}"#,
            ),
        )
        .mount(&upstream)
        .await;
    let provider = Arc::new(StaticProvider::new(vec![(
        "test".into(),
        route(RuntimeKind::Ollama, &upstream.uri()),
    )]));
    let app = make_app(provider);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"model":"test","messages":[{"role":"user","content":"hi"}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn list_all_models_descriptor_is_consistent() {
    // ModelDescriptor가 export되어 있어 외부 wrapping 가능.
    let m = ModelDescriptor {
        id: "x".into(),
        owned_by: "ollama".into(),
    };
    assert_eq!(m.id, "x");
    assert_eq!(m.owned_by, "ollama");
}
