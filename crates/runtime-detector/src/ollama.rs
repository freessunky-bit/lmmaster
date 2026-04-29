//! Ollama daemon 감지 — `/api/version` HTTP probe.
//!
//! 표준 endpoint는 `127.0.0.1:11434`. `OLLAMA_HOST` 환경변수로 override된 환경에서는
//! `probe_at`을 직접 호출.

use reqwest::Client;
use serde::Deserialize;
use shared_types::RuntimeKind;

use crate::DetectResult;

pub const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:11434";

#[derive(Deserialize)]
struct VersionResp {
    version: String,
}

/// 기본 endpoint(`127.0.0.1:11434`)에서 Ollama probe.
pub async fn probe(client: &Client) -> anyhow::Result<DetectResult> {
    probe_at(client, DEFAULT_ENDPOINT).await
}

/// 임의의 base URL에서 Ollama probe. base URL은 trailing slash 무관.
///
/// 응답 분류:
/// - 2xx + 유효한 JSON → `Status::Running` + version.
/// - 비-2xx → `Status::Error` (서비스가 떠 있지만 응답이 비표준).
/// - connect/timeout 에러 → `Status::NotInstalled`.
/// - 그 외 → `anyhow::Error` 반환 → 호출자가 `Status::Error`로 변환.
pub async fn probe_at(client: &Client, base_url: &str) -> anyhow::Result<DetectResult> {
    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/api/version");

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body: VersionResp = resp.json().await?;
            tracing::info!(version = %body.version, base_url = %base, "Ollama probe ok");
            Ok(DetectResult::running(
                RuntimeKind::Ollama,
                base.to_string(),
                Some(body.version),
            ))
        }
        Ok(resp) => {
            let status = resp.status();
            tracing::debug!(http_status = %status, base_url = %base, "Ollama probe non-2xx");
            Ok(DetectResult::error(
                RuntimeKind::Ollama,
                format!("unexpected HTTP status {status} from {url}"),
            ))
        }
        Err(e) if is_connect_or_timeout(&e) => {
            tracing::debug!(error = %e, base_url = %base, "Ollama not reachable");
            Ok(DetectResult::not_installed(RuntimeKind::Ollama))
        }
        Err(e) => Err(e.into()),
    }
}

fn is_connect_or_timeout(e: &reqwest::Error) -> bool {
    e.is_connect() || e.is_timeout() || e.is_request()
}
