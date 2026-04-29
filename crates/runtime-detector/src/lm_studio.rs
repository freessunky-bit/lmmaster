//! LM Studio daemon 감지 — OpenAI-compatible `/v1/models` HTTP probe.
//!
//! LM Studio의 default 서버 포트는 `1234`. REST API는 daemon 버전을 직접 노출하지 않으므로
//! Phase 1A.1의 HTTP probe만으로는 version=None — Phase 1A.2에서 `lms version` CLI 보강.

use reqwest::Client;
use serde::Deserialize;
use shared_types::RuntimeKind;

use crate::DetectResult;

pub const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:1234";

#[derive(Deserialize)]
struct ModelsResp {
    #[serde(default)]
    object: String,
    #[serde(default)]
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    #[serde(default)]
    id: String,
}

pub async fn probe(client: &Client) -> anyhow::Result<DetectResult> {
    probe_at(client, DEFAULT_ENDPOINT).await
}

pub async fn probe_at(client: &Client, base_url: &str) -> anyhow::Result<DetectResult> {
    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/v1/models");

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            // body가 OpenAI 호환 형태인지 가볍게 검증 — `object: "list"`가 아니어도 200이면 LM Studio로 본다.
            let body: ModelsResp = resp.json().await?;
            let model_count = body.data.len();
            let first_model = body
                .data
                .first()
                .map(|m| m.id.clone())
                .filter(|s| !s.is_empty());
            tracing::info!(
                base_url = %base,
                models = model_count,
                first_model = ?first_model,
                object = %body.object,
                "LM Studio probe ok"
            );
            Ok(DetectResult::running(
                RuntimeKind::LmStudio,
                base.to_string(),
                None, // REST endpoint는 daemon 버전을 노출하지 않음.
            ))
        }
        Ok(resp) => {
            let status = resp.status();
            tracing::debug!(http_status = %status, base_url = %base, "LM Studio probe non-2xx");
            Ok(DetectResult::error(
                RuntimeKind::LmStudio,
                format!("unexpected HTTP status {status} from {url}"),
            ))
        }
        Err(e) if is_connect_or_timeout(&e) => {
            tracing::debug!(error = %e, base_url = %base, "LM Studio not reachable");
            Ok(DetectResult::not_installed(RuntimeKind::LmStudio))
        }
        Err(e) => Err(e.into()),
    }
}

fn is_connect_or_timeout(e: &reqwest::Error) -> bool {
    e.is_connect() || e.is_timeout() || e.is_request()
}
