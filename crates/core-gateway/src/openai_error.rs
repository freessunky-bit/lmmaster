//! OpenAI 호환 에러 envelope 헬퍼.
//!
//! 정책 (ADR-0022 §4 — 검증 invariant):
//! - 모든 4xx/5xx는 `{"error":{"message","type","code"}}` 형식.
//! - 미준수 시 openai-python 등 클라이언트가 generic Error로 던져 디버깅 어려움.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde_json::json;

/// type 필드 값.
pub mod error_type {
    pub const INVALID_REQUEST: &str = "invalid_request_error";
    pub const NOT_FOUND: &str = "not_found_error";
    pub const UPSTREAM: &str = "upstream_error";
    pub const TIMEOUT: &str = "timeout_error";
    pub const QUEUE_TIMEOUT: &str = "queue_timeout_error";
}

pub fn error_response(status: StatusCode, error_type: &str, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({
            "error": {
                "message": message,
                "type": error_type,
                "code": code,
            }
        })),
    )
        .into_response()
}

pub fn model_not_found(model: &str) -> Response {
    error_response(
        StatusCode::NOT_FOUND,
        error_type::NOT_FOUND,
        "model_not_found",
        &format!(
            "모델 '{model}'을(를) 찾을 수 없어요. /v1/models로 사용 가능한 목록을 확인해 주세요."
        ),
    )
}

pub fn upstream_unreachable(message: &str) -> Response {
    error_response(
        StatusCode::BAD_GATEWAY,
        error_type::UPSTREAM,
        "upstream_unreachable",
        message,
    )
}

pub fn upstream_status(status: StatusCode, message: &str) -> Response {
    error_response(
        status,
        error_type::UPSTREAM,
        "upstream_status_error",
        message,
    )
}

pub fn invalid_body(message: &str) -> Response {
    error_response(
        StatusCode::BAD_REQUEST,
        error_type::INVALID_REQUEST,
        "invalid_request_body",
        message,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[tokio::test]
    async fn model_not_found_envelope_has_oai_shape() {
        let resp = model_not_found("foo");
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let body = to_bytes(resp.into_body(), 4096).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"]["type"], "not_found_error");
        assert_eq!(v["error"]["code"], "model_not_found");
        assert!(v["error"]["message"].as_str().unwrap().contains("foo"));
    }

    #[tokio::test]
    async fn upstream_unreachable_returns_502() {
        let resp = upstream_unreachable("connection refused");
        assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
    }

    #[tokio::test]
    async fn invalid_body_returns_400() {
        let resp = invalid_body("missing model field");
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
