//! POST /v1/chat/completions — OpenAI 호환 라우팅 + byte-perfect SSE pass-through.
//!
//! 정책 (ADR-0022 §1, §2, §3):
//! - body의 `model` 필드 inspect → UpstreamProvider.upstream_for() lookup.
//! - 없으면 404 model_not_found (OpenAI envelope).
//! - 있으면 semaphore acquire → 업스트림 `/v1/chat/completions`로 forward.
//! - stream=true는 byte-perfect bytes_stream relay (axum::Sse 재포맷 안 함).
//! - x-lmmaster-queue-wait-ms 헤더로 큐 대기 시간 노출.

use std::time::Instant;

use axum::{
    body::Body,
    extract::{Json, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use serde_json::Value;

use crate::openai_error::{invalid_body, model_not_found, upstream_status, upstream_unreachable};
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .with_state(state)
}

async fn chat_completions(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let model = match body.get("model").and_then(|v| v.as_str()) {
        Some(m) if !m.is_empty() => m.to_string(),
        _ => return invalid_body("body.model은 필수예요"),
    };

    let route = match state.provider.upstream_for(&model).await {
        Some(r) => r,
        None => return model_not_found(&model),
    };

    let queue_started = Instant::now();
    let permit = match state.semaphore.clone().acquire_owned().await {
        Ok(p) => p,
        Err(_) => {
            return upstream_unreachable("게이트웨이 내부 큐가 닫혔어요");
        }
    };
    let queue_wait_ms = queue_started.elapsed().as_millis() as u64;

    let upstream_url = format!(
        "{}/v1/chat/completions",
        route.base_url.trim_end_matches('/')
    );

    let upstream = match state.http.post(&upstream_url).json(&body).send().await {
        Ok(r) => r,
        Err(e) => {
            drop(permit);
            return upstream_unreachable(&format!("업스트림 호출 실패: {e}"));
        }
    };

    let upstream_status_code = upstream.status();
    if !upstream_status_code.is_success() {
        let status =
            StatusCode::from_u16(upstream_status_code.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        let text = upstream.text().await.unwrap_or_default();
        drop(permit);
        return upstream_status(status, &text);
    }

    let is_stream = body
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let upstream_headers = upstream.headers().clone();
    let stream = upstream.bytes_stream();

    // permit은 stream이 끝날 때까지 holding — Body::from_stream과 함께 살아있어야 한다.
    // permit Drop은 Body가 Drop될 때 발생.
    let body_stream = ReleaseOnDrop {
        inner: stream,
        _permit: permit,
    };

    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        "x-lmmaster-queue-wait-ms",
        HeaderValue::from_str(&queue_wait_ms.to_string()).expect("ascii numeric"),
    );

    if is_stream {
        response_headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("text/event-stream"),
        );
        response_headers.insert(
            axum::http::header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        );
        response_headers.insert("x-accel-buffering", HeaderValue::from_static("no"));
    } else if let Some(ct) = upstream_headers.get(axum::http::header::CONTENT_TYPE) {
        if let Ok(v) = HeaderValue::from_bytes(ct.as_bytes()) {
            response_headers.insert(axum::http::header::CONTENT_TYPE, v);
        }
    }

    (
        StatusCode::OK,
        response_headers,
        Body::from_stream(body_stream),
    )
        .into_response()
}

/// Body stream wrapper — drop 시 semaphore permit을 자동 반환.
struct ReleaseOnDrop<S> {
    inner: S,
    _permit: tokio::sync::OwnedSemaphorePermit,
}

impl<S> futures::Stream for ReleaseOnDrop<S>
where
    S: futures::Stream + Unpin,
{
    type Item = S::Item;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::pin::Pin::new(&mut self.inner).poll_next(cx)
    }
}
