use axum::{routing::get, Json, Router};
use serde::Serialize;

#[derive(Serialize)]
struct Health<'a> {
    status: &'a str,
    version: &'a str,
}

#[derive(Serialize, Default)]
struct Capabilities {
    chat_completions: bool,
    embeddings: bool,
    streaming: bool,
}

pub fn router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/capabilities", get(capabilities))
}

async fn health() -> Json<Health<'static>> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn capabilities() -> Json<Capabilities> {
    // Phase 0: 인프라가 stream을 지원하지만 실제 chat/embeddings 라우트는 아직 미연결.
    // Phase 2(M2)에서 어댑터 capability matrix를 집계해 동적으로 응답한다.
    Json(Capabilities {
        chat_completions: false,
        embeddings: false,
        streaming: true,
    })
}
