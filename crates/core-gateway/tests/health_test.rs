//! gateway 통합 테스트.
//! tower::ServiceExt::oneshot 으로 router에 직접 요청을 흘려본다.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use core_gateway::{build_router, AppState, GatewayConfig, StaticProvider};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::util::ServiceExt;

#[tokio::test]
async fn health_returns_ok_envelope() {
    let state = AppState::new(Arc::new(StaticProvider::default()));
    let app = build_router(GatewayConfig::default(), state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("oneshot");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("collect body")
        .to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(value["status"], "ok");
    assert!(value["version"].is_string(), "version should be a string");
}

#[tokio::test]
async fn capabilities_returns_streaming_true() {
    let state = AppState::new(Arc::new(StaticProvider::default()));
    let app = build_router(GatewayConfig::default(), state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/capabilities")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("oneshot");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("collect")
        .to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(value["streaming"], serde_json::Value::Bool(true));
    assert_eq!(value["chat_completions"], serde_json::Value::Bool(false));
}

#[tokio::test]
async fn unknown_route_404() {
    let state = AppState::new(Arc::new(StaticProvider::default()));
    let app = build_router(GatewayConfig::default(), state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/no-such-route")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("oneshot");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
