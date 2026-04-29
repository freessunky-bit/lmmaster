//! GET /v1/models, GET /v1/models/:id — OpenAI 호환 모델 목록.
//!
//! 정책 (ADR-0022 §1):
//! - UpstreamProvider.list_all_models() 합산 → OpenAI shape.
//! - response: `{ object: "list", data: [{ id, object: "model", owned_by, created }] }`.
//! - GET /v1/models/:id — 단일 모델 메타 (owned_by + created).

use axum::{
    extract::{Path, State},
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use serde_json::json;

use crate::openai_error::model_not_found;
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/models", get(list_models))
        .route("/v1/models/{id}", get(retrieve_model))
        .with_state(state)
}

async fn list_models(State(state): State<AppState>) -> Response {
    let models = state.provider.list_all_models().await;
    let data: Vec<_> = models
        .into_iter()
        .map(|m| {
            json!({
                "id": m.id,
                "object": "model",
                "owned_by": m.owned_by,
                "created": 0u64,
            })
        })
        .collect();
    Json(json!({
        "object": "list",
        "data": data,
    }))
    .into_response()
}

async fn retrieve_model(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let models = state.provider.list_all_models().await;
    match models.into_iter().find(|m| m.id == id) {
        Some(m) => Json(json!({
            "id": m.id,
            "object": "model",
            "owned_by": m.owned_by,
            "created": 0u64,
        }))
        .into_response(),
        None => model_not_found(&id),
    }
}
