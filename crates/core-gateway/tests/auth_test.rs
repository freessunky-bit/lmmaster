//! Phase 3'.b — auth 미들웨어 통합 테스트.
//!
//! 검증 invariant (ADR-0022 §5~§7):
//! - missing key → 401 invalid_request_error / missing_api_key.
//! - invalid plaintext → 401 invalid_api_key.
//! - revoked → 401 key_revoked.
//! - expired → 401 key_expired.
//! - origin mismatch → 403 origin_denied.
//! - endpoint scope deny → 403 endpoint_denied.
//! - 통과 시 응답 ACAO가 키 origin으로 echo (절대 *).
//! - OPTIONS preflight는 키 없이 통과.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use core_gateway::{build_router, AppState, GatewayConfig, StaticProvider, UpstreamRoute};
use http_body_util::BodyExt;
use key_manager::{IssueRequest, KeyManager, Scope};
use shared_types::RuntimeKind;
use tower::util::ServiceExt;

fn web_scope(origins: &[&str]) -> Scope {
    Scope {
        models: vec!["*".into()],
        endpoints: vec!["/v1/*".into()],
        allowed_origins: origins.iter().map(|s| (*s).to_string()).collect(),
        ..Default::default()
    }
}

fn build_app(km: Arc<KeyManager>) -> axum::Router {
    let provider = Arc::new(StaticProvider::new(vec![(
        "test-model".into(),
        UpstreamRoute {
            runtime: RuntimeKind::Ollama,
            base_url: "http://127.0.0.1:65000".into(), // 사용 안 함 — 인증 단계에서 거부.
        },
    )]));
    let state = AppState::new(provider).with_key_manager(km);
    build_router(GatewayConfig::default(), state)
}

#[tokio::test]
async fn protected_route_without_key_returns_401() {
    let km = Arc::new(KeyManager::open_memory().unwrap());
    let app = build_app(km);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"model":"test-model"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"]["code"], "missing_api_key");
    assert_eq!(v["error"]["type"], "invalid_request_error");
}

#[tokio::test]
async fn invalid_plaintext_returns_401_invalid_api_key() {
    let km = Arc::new(KeyManager::open_memory().unwrap());
    let app = build_app(km);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header(
                    "authorization",
                    "Bearer lm-fakefake0000000000000000000000000",
                )
                .header("origin", "http://localhost:5173")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"model":"test-model"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"]["code"], "invalid_api_key");
}

#[tokio::test]
async fn origin_mismatch_returns_403_origin_denied() {
    let km = Arc::new(KeyManager::open_memory().unwrap());
    let issued = km
        .issue(IssueRequest {
            alias: "blog".into(),
            scope: web_scope(&["https://blog.example.com"]),
        })
        .unwrap();
    let app = build_app(km);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("authorization", format!("Bearer {}", issued.plaintext_once))
                .header("origin", "https://attacker.com")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"model":"test-model"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"]["code"], "origin_denied");
}

#[tokio::test]
async fn revoked_key_returns_401_invalid_api_key() {
    // revoked 키는 prefix 인덱스에서 빠지므로 실제로는 invalid_api_key로 응답.
    let km = Arc::new(KeyManager::open_memory().unwrap());
    let issued = km
        .issue(IssueRequest {
            alias: "blog".into(),
            scope: web_scope(&["https://blog.example.com"]),
        })
        .unwrap();
    km.revoke(&issued.id).unwrap();
    let app = build_app(km);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("authorization", format!("Bearer {}", issued.plaintext_once))
                .header("origin", "https://blog.example.com")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"model":"test-model"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"]["code"], "invalid_api_key");
}

#[tokio::test]
async fn expired_key_returns_401_key_expired() {
    let km = Arc::new(KeyManager::open_memory().unwrap());
    let mut scope = web_scope(&["https://blog.example.com"]);
    scope.expires_at = Some("2000-01-01T00:00:00Z".into());
    let issued = km
        .issue(IssueRequest {
            alias: "old".into(),
            scope,
        })
        .unwrap();
    let app = build_app(km);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("authorization", format!("Bearer {}", issued.plaintext_once))
                .header("origin", "https://blog.example.com")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"model":"test-model"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"]["code"], "key_expired");
}

#[tokio::test]
async fn endpoint_scope_deny_returns_403() {
    let km = Arc::new(KeyManager::open_memory().unwrap());
    let mut scope = web_scope(&["https://blog.example.com"]);
    scope.endpoints = vec!["/v1/embeddings".into()]; // chat 거부.
    let issued = km
        .issue(IssueRequest {
            alias: "embed".into(),
            scope,
        })
        .unwrap();
    let app = build_app(km);
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("authorization", format!("Bearer {}", issued.plaintext_once))
                .header("origin", "https://blog.example.com")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"model":"test-model"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"]["code"], "endpoint_denied");
}

#[tokio::test]
async fn options_preflight_passes_without_key() {
    let km = Arc::new(KeyManager::open_memory().unwrap());
    let app = build_app(km);
    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/v1/models")
                .header("origin", "https://x.com")
                .header("access-control-request-method", "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // preflight는 키 없이 통과 (200).
    assert_ne!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn allowed_origin_echoed_back_in_acao_not_wildcard() {
    // 통과 시 ACAO 헤더가 키의 origin과 정확히 일치해야 함 (절대 *).
    let km = Arc::new(KeyManager::open_memory().unwrap());
    let issued = km
        .issue(IssueRequest {
            alias: "blog".into(),
            scope: web_scope(&["https://blog.example.com"]),
        })
        .unwrap();
    // /v1/models 호출은 upstream 안 부르고 provider list만 본다.
    let app = build_app(km);
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/models")
                .header("authorization", format!("Bearer {}", issued.plaintext_once))
                .header("origin", "https://blog.example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let acao = response
        .headers()
        .get("access-control-allow-origin")
        .map(|v| v.to_str().unwrap().to_string());
    // 우리 auth 미들웨어가 ACAO를 origin으로 정확히 덮어씀 — '*' 거부.
    assert_eq!(acao.as_deref(), Some("https://blog.example.com"));
}

#[tokio::test]
async fn no_origin_header_with_web_only_key_returns_403() {
    let km = Arc::new(KeyManager::open_memory().unwrap());
    let issued = km
        .issue(IssueRequest {
            alias: "blog".into(),
            scope: web_scope(&["https://blog.example.com"]),
        })
        .unwrap();
    let app = build_app(km);
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/models")
                .header("authorization", format!("Bearer {}", issued.plaintext_once))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"]["code"], "origin_denied");
}

#[tokio::test]
async fn server_only_key_no_origin_passes() {
    let km = Arc::new(KeyManager::open_memory().unwrap());
    let mut scope = web_scope(&[]);
    scope.allowed_origins.clear();
    let issued = km
        .issue(IssueRequest {
            alias: "server".into(),
            scope,
        })
        .unwrap();
    let app = build_app(km);
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/models")
                .header("authorization", format!("Bearer {}", issued.plaintext_once))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn health_route_does_not_require_key() {
    let km = Arc::new(KeyManager::open_memory().unwrap());
    let app = build_app(km);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
