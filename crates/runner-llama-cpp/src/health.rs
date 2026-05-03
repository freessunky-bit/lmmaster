//! `/health` polling — exponential backoff + cancel-aware.
//!
//! 정책 (보강 리서치 §1.4):
//! - 초기 200ms × 배수 1.5 + 최대 2초 간격. 60초 timeout.
//! - 503 (Loading model) → 계속, 200 (status: ok) → ready.
//! - cancel 시 즉시 종료 + 호출자가 child kill.

use std::time::{Duration, Instant};

use tokio_util::sync::CancellationToken;

use crate::RunnerError;

const HEALTH_INITIAL_MS: u64 = 200;
const HEALTH_MAX_MS: u64 = 2_000;
const HEALTH_MULTIPLIER: f64 = 1.5;
const HEALTH_TIMEOUT_SECS: u64 = 60;

/// `/health` 200까지 대기. timeout 또는 cancel 시 에러.
pub async fn wait_for_ready(
    health_url: &str,
    cancel: &CancellationToken,
) -> Result<(), RunnerError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .connect_timeout(Duration::from_millis(500))
        .no_proxy()
        .build()
        .map_err(|e| RunnerError::Internal {
            message: format!("reqwest build: {e}"),
        })?;

    let started = Instant::now();
    let mut delay_ms = HEALTH_INITIAL_MS;

    loop {
        if started.elapsed().as_secs() >= HEALTH_TIMEOUT_SECS {
            return Err(RunnerError::HealthcheckTimeout);
        }

        tokio::select! {
            biased;
            () = cancel.cancelled() => {
                return Err(RunnerError::HealthcheckTimeout);
            }
            resp = client.get(health_url).send() => {
                match resp {
                    Ok(r) if r.status().as_u16() == 200 => {
                        tracing::debug!(url = %health_url, "llama-server health ok");
                        return Ok(());
                    }
                    Ok(r) => {
                        // 503 (Loading model) 등 — 계속 polling.
                        tracing::debug!(status = %r.status(), "llama-server health pending");
                    }
                    Err(e) => {
                        // connect refused 등 — 계속 polling (초기 시작 단계).
                        tracing::debug!(error = %e, "llama-server health connect retry");
                    }
                }
            }
        }

        // 다음 polling까지 대기. cancel-aware.
        tokio::select! {
            biased;
            () = cancel.cancelled() => {
                return Err(RunnerError::HealthcheckTimeout);
            }
            () = tokio::time::sleep(Duration::from_millis(delay_ms)) => {
                delay_ms = ((delay_ms as f64) * HEALTH_MULTIPLIER) as u64;
                if delay_ms > HEALTH_MAX_MS {
                    delay_ms = HEALTH_MAX_MS;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn ready_immediately_on_200() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"status":"ok"}"#))
            .mount(&server)
            .await;
        let url = format!("{}/health", server.uri());
        let cancel = CancellationToken::new();
        wait_for_ready(&url, &cancel)
            .await
            .expect("/health 200 즉시 ready");
    }

    #[tokio::test]
    async fn cancellation_returns_timeout() {
        // 응답 안 오는 endpoint — 0.5초 후 cancel.
        let server = MockServer::start().await;
        // 응답을 영원히 안 주도록 설정 X — wiremock에서 mount 안 한 path는 404. 그래서 별도 endpoint.
        let url = format!("{}/health", server.uri());
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            cancel_clone.cancel();
        });
        let result = wait_for_ready(&url, &cancel).await;
        // wiremock이 404를 즉시 반환 → 503 처리 → 계속 polling → cancel 시 timeout.
        // 단, mount되지 않은 path는 404를 줘서 *200 아님*으로 polling 계속됨.
        assert!(matches!(result, Err(RunnerError::HealthcheckTimeout)));
    }

    #[tokio::test]
    async fn loading_then_ready() {
        // 처음 2회는 503, 3회째 200.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(
                ResponseTemplate::new(503)
                    .set_body_string(r#"{"error":{"code":503,"message":"Loading model"}}"#),
            )
            .up_to_n_times(2)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"status":"ok"}"#))
            .mount(&server)
            .await;
        let url = format!("{}/health", server.uri());
        let cancel = CancellationToken::new();
        wait_for_ready(&url, &cancel)
            .await
            .expect("loading 후 ready");
    }
}
