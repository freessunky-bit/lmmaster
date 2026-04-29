//! API key 인증 미들웨어 — Phase 3'.b 본격 활성.
//!
//! 정책 (ADR-0022 §5~§7):
//! - `/v1/*`, `/_admin/*`은 키 의무. `/health`, `/capabilities`, OPTIONS preflight는 무인증.
//! - Authorization: Bearer <key> 추출 → KeyManager.verify(plaintext, origin, path, model).
//! - 거부 시 OpenAI 호환 envelope `{"error":{message,type,code}}`.
//! - 검증 통과 시 `Principal { id, alias }`을 request extensions에 주입.
//! - CORS 응답 헤더는 키의 allowed_origins에서만 echo back — 아래 별도 helper.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderValue, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use key_manager::{AuthOutcome, KeyManager};
use serde_json::json;
use time::OffsetDateTime;

/// 인증 후 request extensions에 주입되는 principal.
#[derive(Debug, Clone)]
pub struct Principal {
    pub id: String,
    pub alias: String,
}

/// auth 미들웨어 layer가 사용하는 state.
#[derive(Clone)]
pub struct AuthState {
    pub key_manager: Arc<KeyManager>,
}

impl AuthState {
    pub fn new(key_manager: Arc<KeyManager>) -> Self {
        Self { key_manager }
    }
}

/// 미들웨어 — 인증 + scope 검증 + Origin 매칭 + CORS 응답.
pub async fn require_api_key(
    State(state): State<AuthState>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    // OPTIONS preflight — 키 없이 origin echo + 204. actual request에서 진짜 검증.
    if req.method() == Method::OPTIONS {
        return preflight_response(&req);
    }

    // /health, /capabilities는 무인증 — 단 본 미들웨어는 /v1/*, /_admin/*에만 mount되므로
    // 여기까지 오면 이미 보호 대상.

    let path = req.uri().path().to_string();

    // Authorization: Bearer <key>.
    let bearer = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_bearer);

    let bearer = match bearer {
        Some(b) => b,
        None => {
            return error_response(
                StatusCode::UNAUTHORIZED,
                "invalid_request_error",
                "missing_api_key",
                "Authorization: Bearer <key> 헤더가 필요해요.",
            )
        }
    };

    // Origin 헤더.
    let origin = req
        .headers()
        .get(axum::http::header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // body의 model 필드 — chat/completions / embeddings에서만 의미. body 추출은 비용 큼.
    // v1 단순화: model scope 검증은 body 파싱 시점(라우트 핸들러)에서 별도 체크하지 않고,
    // 미들웨어에선 endpoint scope만 강제. model scope는 chat 핸들러가 추가로 체크 (KeyManager.verify는 None 전달).
    // 향후 v1.1에서 body peek + model 추출 추가.
    let model = None;

    let outcome = match state.key_manager.verify(
        &bearer,
        origin.as_deref(),
        &path,
        model,
        OffsetDateTime::now_utc(),
    ) {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!(error = %e, "auth verify error");
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "auth_internal",
                "인증 중 내부 오류가 났어요",
            );
        }
    };

    match outcome {
        AuthOutcome::Allowed { id, alias } => {
            req.extensions_mut().insert(Principal { id, alias });

            // CORS 응답 헤더 — 요청 origin이 키의 whitelist에 있을 때만 echo back.
            // (CORS preflight는 OPTIONS에서 별도 처리. 여기는 actual request 응답 갱신.)
            let mut response = next.run(req).await;
            if let Some(o) = origin.as_deref() {
                if let Ok(v) = HeaderValue::from_str(o) {
                    response
                        .headers_mut()
                        .insert(axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN, v);
                    response.headers_mut().insert(
                        axum::http::header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
                        HeaderValue::from_static("true"),
                    );
                }
            }
            response
        }
        AuthOutcome::InvalidKey => error_response(
            StatusCode::UNAUTHORIZED,
            "invalid_request_error",
            "invalid_api_key",
            "유효하지 않은 API 키예요",
        ),
        AuthOutcome::Revoked => error_response(
            StatusCode::UNAUTHORIZED,
            "invalid_request_error",
            "key_revoked",
            "이 키는 회수되었어요",
        ),
        AuthOutcome::Expired => error_response(
            StatusCode::UNAUTHORIZED,
            "invalid_request_error",
            "key_expired",
            "이 키는 만료되었어요",
        ),
        AuthOutcome::OriginDenied => error_response(
            StatusCode::FORBIDDEN,
            "invalid_request_error",
            "origin_denied",
            "이 키는 이 사이트(origin)에서 호출할 수 없어요",
        ),
        AuthOutcome::EndpointDenied => error_response(
            StatusCode::FORBIDDEN,
            "invalid_request_error",
            "endpoint_denied",
            "이 키는 이 endpoint를 호출할 수 없어요",
        ),
        AuthOutcome::ModelDenied => error_response(
            StatusCode::FORBIDDEN,
            "invalid_request_error",
            "model_denied",
            "이 키는 이 모델을 호출할 수 없어요",
        ),
    }
}

/// CORS preflight 응답 — origin echo. browser는 actual request로 진짜 권한 확인.
fn preflight_response(req: &Request<Body>) -> Response {
    let mut response = Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(Body::empty())
        .expect("build preflight");
    if let Some(origin) = req
        .headers()
        .get(axum::http::header::ORIGIN)
        .and_then(|v| v.to_str().ok())
    {
        if let Ok(v) = HeaderValue::from_str(origin) {
            response
                .headers_mut()
                .insert(axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN, v);
        }
    }
    response.headers_mut().insert(
        axum::http::header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, OPTIONS"),
    );
    response.headers_mut().insert(
        axum::http::header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("authorization, content-type, origin, x-request-id"),
    );
    response.headers_mut().insert(
        axum::http::header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
        HeaderValue::from_static("true"),
    );
    response.headers_mut().insert(
        axum::http::header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("600"),
    );
    response
}

fn parse_bearer(header: &str) -> Option<String> {
    let trimmed = header.trim();
    // case-insensitive "Bearer " prefix.
    if trimmed.len() > 7 && trimmed[..7].eq_ignore_ascii_case("bearer ") {
        Some(trimmed[7..].trim().to_string())
    } else {
        None
    }
}

pub fn error_response(status: StatusCode, error_type: &str, code: &str, message: &str) -> Response {
    (
        status,
        [("WWW-Authenticate", "Bearer realm=\"lmmaster\"")],
        axum::Json(json!({
            "error": {
                "message": message,
                "type": error_type,
                "code": code,
            }
        })),
    )
        .into_response()
}

/// 이전 호환 — 기존 호출처에서 사용. v1에서 require_api_key로 대체 권장.
pub fn unauthorized(code: &str, message: &str) -> Response {
    error_response(
        StatusCode::UNAUTHORIZED,
        "invalid_request_error",
        code,
        message,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bearer_strips_prefix() {
        assert_eq!(parse_bearer("Bearer abc123"), Some("abc123".to_string()));
        assert_eq!(parse_bearer("bearer abc123"), Some("abc123".to_string()));
        assert_eq!(parse_bearer("  Bearer   abc  "), Some("abc".to_string()));
    }

    #[test]
    fn parse_bearer_rejects_basic_or_empty() {
        assert!(parse_bearer("Basic dXNlcjpwYXNz").is_none());
        assert!(parse_bearer("").is_none());
    }
}
