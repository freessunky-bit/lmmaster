//! `WorkbenchResponder` — workbench-core `Responder` trait의 bench-harness 어댑터.
//!
//! 정책 (Phase 5'.e — phase-5pe-runtime-http-reinforcement.md):
//! - Ollama: POST `/api/generate` + `stream:false` + `options.temperature/num_ctx`. 응답 `response`
//!   필드를 plain text로 반환.
//! - LM Studio: POST `/v1/chat/completions` + `messages:[{role:"user",content:...}]` +
//!   `stream:false`. 사전 `GET /v1/models` 1회로 모델 로드 여부 판단.
//! - Mock: 기존 `MockResponder` 위임 (테스트 + 데모 + UI fallback 보존).
//! - reqwest::Client 단일 Arc 재사용 — connection pool 보존 (research §7.6).
//! - timeout 60s, connect_timeout 5s, no_proxy() — research §4.1.
//! - 5xx / 429 retry x2 (backon ExponentialBuilder + jitter), 4xx 즉시 실패 — research §4.2.
//! - 에러 메시지 한국어 해요체 — CLAUDE.md §4.1.

use std::time::Duration;

use async_trait::async_trait;
use backon::{ExponentialBuilder, Retryable};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use shared_types::RuntimeKind as SharedRuntimeKind;
use url::{Host, Url};
use workbench_core::{MockResponder, Responder, WorkbenchError};

/// Workbench Validate stage가 dispatch할 런타임 종류.
///
/// `shared_types::RuntimeKind`와 분리된 이유: Workbench는 Validate 단계에서 *현재*
/// `Ollama` / `LmStudio` / `Mock`(테스트·데모) 3가지만 의미가 있고, `LlamaCpp` / `KoboldCpp` /
/// `Vllm` 등은 향후 v1.x 확장 영역이라 별도 enum으로 의도를 좁힘. From 변환을 통해
/// 상호운용 가능.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeKind {
    Ollama,
    LmStudio,
    Mock,
}

impl RuntimeKind {
    /// `shared_types::RuntimeKind`로의 매핑 — Mock은 LlamaCpp(가장 보수적 placeholder)로.
    pub fn to_shared(self) -> SharedRuntimeKind {
        match self {
            RuntimeKind::Ollama => SharedRuntimeKind::Ollama,
            RuntimeKind::LmStudio => SharedRuntimeKind::LmStudio,
            RuntimeKind::Mock => SharedRuntimeKind::LlamaCpp,
        }
    }
}

impl From<SharedRuntimeKind> for RuntimeKind {
    fn from(s: SharedRuntimeKind) -> Self {
        match s {
            SharedRuntimeKind::Ollama => RuntimeKind::Ollama,
            SharedRuntimeKind::LmStudio => RuntimeKind::LmStudio,
            // 다른 런타임은 v1에서 mock으로 fallback (Phase 5'.e 범위 밖).
            _ => RuntimeKind::Mock,
        }
    }
}

/// 호출별 generation parameter — 단일 source.
///
/// research §1.1 / §2.1의 권장 기본값을 그대로 채택.
#[derive(Debug, Clone)]
pub struct ResponderConfig {
    /// 전체 request timeout. cold load 포함 60s.
    pub timeout: Duration,
    /// 5xx / 429 시 재시도 횟수 (4xx는 retry 안 함).
    pub max_retries: u32,
    /// `temperature` — Validate는 deterministic 비교 우선 → 0.0.
    pub temperature: f32,
    /// `num_ctx` — 한국어 BPE는 영어 대비 2~3배 토큰 → 2048 안전 기본.
    pub num_ctx: u32,
    /// `num_predict` (Ollama) / `max_tokens` (LM Studio) 등가.
    pub max_tokens: u32,
}

impl Default for ResponderConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(60),
            max_retries: 2,
            temperature: 0.0,
            num_ctx: 2048,
            max_tokens: 256,
        }
    }
}

/// Workbench Validate stage용 responder 어댑터.
///
/// - `RuntimeKind::Mock`이면 `MockResponder`로 위임 (테스트 + UI fallback).
/// - `RuntimeKind::Ollama`면 `/api/generate` POST.
/// - `RuntimeKind::LmStudio`면 `GET /v1/models` 사전체크 후 `/v1/chat/completions` POST.
#[derive(Debug, Clone)]
pub struct WorkbenchResponder {
    /// 어떤 런타임으로 dispatch할지.
    runtime_kind: RuntimeKind,
    /// 모델 식별자 — Ollama tag / LM Studio loaded model id.
    model_id: String,
    /// HTTP base URL (예: `http://localhost:11434`).
    base_url: String,
    /// reqwest::Client — connection pool 재사용.
    client: reqwest::Client,
    /// generation parameter.
    config: ResponderConfig,
}

impl WorkbenchResponder {
    /// 새 어댑터. base_url은 localhost-only allowlist 검증 + trailing slash 정규화.
    ///
    /// Phase R-F+R-G hotfix (ADR-0064 §2): cloud-zero 정체성 보존.
    /// 비-localhost host / `https` / userinfo / 잘못된 포맷은 `WorkbenchError::InvalidBaseUrl`로 거부.
    pub fn new(
        runtime_kind: RuntimeKind,
        model_id: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Result<Self, WorkbenchError> {
        let base = base_url.into();
        let url = validate_localhost_url(&base)?;
        // url::Url을 다시 직렬화하면 trailing slash가 path에 추가될 수 있어 명시 trim.
        let normalized = url.as_str().trim_end_matches('/').to_string();
        let client = default_client();
        Ok(Self {
            runtime_kind,
            model_id: model_id.into(),
            base_url: normalized,
            client,
            config: ResponderConfig::default(),
        })
    }

    /// Ollama 기본값 — default URL은 localhost라 항상 안전.
    pub fn ollama(model_id: impl Into<String>) -> Self {
        Self::new(RuntimeKind::Ollama, model_id, "http://localhost:11434")
            .expect("default Ollama base_url is localhost — cannot fail")
    }

    /// LM Studio 기본값 — default URL은 localhost라 항상 안전.
    pub fn lm_studio(model_id: impl Into<String>) -> Self {
        Self::new(RuntimeKind::LmStudio, model_id, "http://localhost:1234")
            .expect("default LM Studio base_url is localhost — cannot fail")
    }

    /// 테스트/UI 데모용 mock — 외부 통신 0.
    pub fn mock() -> Self {
        Self {
            runtime_kind: RuntimeKind::Mock,
            model_id: String::new(),
            base_url: String::new(),
            client: default_client(),
            config: ResponderConfig::default(),
        }
    }

    /// generation 파라미터 override.
    pub fn with_config(mut self, config: ResponderConfig) -> Self {
        self.config = config;
        self
    }

    /// 외부에서 만든 client 주입 — wiremock + connection pool 공유 시.
    pub fn with_client(mut self, client: reqwest::Client) -> Self {
        self.client = client;
        self
    }

    /// runtime kind 노출 — UI 라벨 / 테스트.
    pub fn runtime_kind(&self) -> RuntimeKind {
        self.runtime_kind
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

/// Phase R-F+R-G hotfix (ADR-0064 §2) — base_url localhost-only allowlist.
///
/// 허용:
/// - `http` 스킴 한정 (https는 외부 강제 의미라 거부).
/// - host: hostname `localhost` (대소문자 무시 정확 매치), IPv4 loopback (127.0.0.0/8 전체),
///   IPv6 loopback `::1`.
/// - port 자유 (사용자 PC 안이라).
///
/// 거부:
/// - https / public IP / private LAN (10/192.168/172.16-31) / link-local (169.254 cloud metadata)
/// - `localhost.evil.com` suffix attack (정확 매치만).
/// - userinfo embedded (`http://attacker@127.0.0.1`).
/// - 0.0.0.0 / `[::]` (server bind-sentinel — client dial 대상 아님).
/// - 빈 host / parse 실패.
pub(crate) fn validate_localhost_url(input: &str) -> Result<Url, WorkbenchError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(invalid_base_url("주소를 입력해 주세요."));
    }
    let url = Url::parse(trimmed)
        .map_err(|_| invalid_base_url("주소 형식이 올바르지 않아요. 예: http://localhost:11434"))?;

    if url.scheme() != "http" {
        return Err(invalid_base_url(
            "http만 사용할 수 있어요. https는 외부 서비스라 차단했어요.",
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(invalid_base_url(
            "주소에 사용자명/비밀번호를 넣을 수 없어요.",
        ));
    }

    let host = url
        .host()
        .ok_or_else(|| invalid_base_url("주소를 입력해 주세요."))?;
    let allowed = match host {
        // 정확 매치 — `localhost.evil.com` suffix attack 차단.
        Host::Domain(name) => name.eq_ignore_ascii_case("localhost"),
        // 127.0.0.0/8 전체 허용. 0.0.0.0 (unspec) 차단.
        Host::Ipv4(ip) => ip.is_loopback(),
        // ::1 만 허용. ::, link-local fe80::/10 등 차단.
        Host::Ipv6(ip) => ip.is_loopback(),
    };
    if !allowed {
        return Err(invalid_base_url(
            "내 PC 안에서 돌아가는 모델만 평가할 수 있어요. http://localhost 주소로 입력해 주세요.",
        ));
    }
    Ok(url)
}

fn invalid_base_url(msg: &str) -> WorkbenchError {
    WorkbenchError::InvalidBaseUrl {
        message: msg.into(),
    }
}

/// 모듈 기본 client — timeout 60s + connect 5s + no_proxy.
/// Phase R-C (ADR-0055) — 폴백 제거. fail-fast on TLS init issue (rogue proxy 우회 방지).
fn default_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(5))
        .no_proxy()
        .pool_idle_timeout(Duration::from_secs(60))
        .build()
        .expect("reqwest Client builder must succeed (TLS init)")
}

// ── HTTP DTOs ────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct OllamaGenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
    options: OllamaOptions,
    keep_alive: &'a str,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_ctx: u32,
    num_predict: u32,
}

#[derive(Debug, Deserialize)]
struct OllamaGenerateResponse {
    #[serde(default)]
    response: String,
    #[serde(default)]
    done: bool,
}

#[derive(Debug, Serialize)]
struct LmStudioChatRequest<'a> {
    model: &'a str,
    messages: Vec<LmStudioMessage<'a>>,
    stream: bool,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Debug, Serialize)]
struct LmStudioMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct LmStudioChatResponse {
    #[serde(default)]
    choices: Vec<LmStudioChoice>,
}

#[derive(Debug, Deserialize)]
struct LmStudioChoice {
    message: LmStudioResponseMessage,
}

#[derive(Debug, Deserialize)]
struct LmStudioResponseMessage {
    #[serde(default)]
    content: String,
}

#[derive(Debug, Deserialize)]
struct LmStudioModelsResponse {
    #[serde(default)]
    data: Vec<LmStudioModelEntry>,
}

#[derive(Debug, Deserialize)]
struct LmStudioModelEntry {
    #[serde(default)]
    id: String,
}

// ── Responder impl ───────────────────────────────────────────────────

#[async_trait]
impl Responder for WorkbenchResponder {
    async fn respond(&self, prompt: &str) -> Result<String, WorkbenchError> {
        match self.runtime_kind {
            RuntimeKind::Mock => MockResponder.respond(prompt).await,
            RuntimeKind::Ollama => self.call_ollama_generate(prompt).await,
            RuntimeKind::LmStudio => self.call_lmstudio_chat(prompt).await,
        }
    }
}

impl WorkbenchResponder {
    fn retry_policy(&self) -> ExponentialBuilder {
        ExponentialBuilder::default()
            .with_min_delay(Duration::from_millis(200))
            .with_max_delay(Duration::from_secs(5))
            .with_factor(2.0)
            .with_max_times(self.config.max_retries as usize)
            .with_jitter()
    }

    /// Ollama `/api/generate` non-stream. 5xx/429 retry x2.
    async fn call_ollama_generate(&self, prompt: &str) -> Result<String, WorkbenchError> {
        let url = format!("{}/api/generate", self.base_url);
        let model = self.model_id.clone();
        let body = OllamaGenerateRequest {
            model: &model,
            prompt,
            stream: false,
            options: OllamaOptions {
                temperature: self.config.temperature,
                num_ctx: self.config.num_ctx,
                num_predict: self.config.max_tokens,
            },
            keep_alive: "5m",
        };
        let body_json = serde_json::to_string(&body).map_err(WorkbenchError::from)?;
        let client = self.client.clone();
        let url_for_retry = url.clone();

        let attempt = || {
            let client = client.clone();
            let url = url_for_retry.clone();
            let body_json = body_json.clone();
            let model = model.clone();
            async move {
                let resp = client
                    .post(&url)
                    .header(reqwest::header::CONTENT_TYPE, "application/json")
                    .body(body_json)
                    .send()
                    .await
                    .map_err(map_reqwest_error)?;
                let status = resp.status();
                if status.is_success() {
                    let parsed: OllamaGenerateResponse =
                        resp.json().await.map_err(|e| WorkbenchError::Internal {
                            message: format!("Ollama 응답을 해석하지 못했어요: {e}"),
                        })?;
                    if !parsed.done && parsed.response.is_empty() {
                        return Err(WorkbenchError::Internal {
                            message: "Ollama가 빈 응답을 보냈어요. 데몬 상태를 확인해 주세요."
                                .into(),
                        });
                    }
                    Ok(parsed.response)
                } else if is_retryable(status) {
                    Err(retryable_status_error("Ollama", status, &model))
                } else {
                    let body = resp.text().await.unwrap_or_default();
                    Err(non_retryable_status_error("Ollama", status, &model, &body))
                }
            }
        };

        attempt
            .retry(self.retry_policy())
            .when(is_workbench_retryable)
            .notify(|err, dur| {
                tracing::warn!(
                    error = %err,
                    delay_ms = dur.as_millis() as u64,
                    "ollama generate retry"
                );
            })
            .await
    }

    /// LM Studio `/v1/chat/completions` non-stream — 사전 `/v1/models` 점검 후 본 호출.
    async fn call_lmstudio_chat(&self, prompt: &str) -> Result<String, WorkbenchError> {
        // 사전 모델 점검 (research §2.3): 빌드 의존성 회피 위해 GET /v1/models 1회 polling.
        self.lmstudio_precheck_model().await?;

        let url = format!("{}/v1/chat/completions", self.base_url);
        let model = self.model_id.clone();
        let body = LmStudioChatRequest {
            model: &model,
            messages: vec![LmStudioMessage {
                role: "user",
                content: prompt,
            }],
            stream: false,
            temperature: self.config.temperature,
            max_tokens: self.config.max_tokens,
        };
        let body_json = serde_json::to_string(&body).map_err(WorkbenchError::from)?;
        let client = self.client.clone();
        let url_for_retry = url.clone();

        let attempt = || {
            let client = client.clone();
            let url = url_for_retry.clone();
            let body_json = body_json.clone();
            let model = model.clone();
            async move {
                let resp = client
                    .post(&url)
                    .header(reqwest::header::CONTENT_TYPE, "application/json")
                    .body(body_json)
                    .send()
                    .await
                    .map_err(map_reqwest_error)?;
                let status = resp.status();
                if status.is_success() {
                    let parsed: LmStudioChatResponse =
                        resp.json().await.map_err(|e| WorkbenchError::Internal {
                            message: format!("LM Studio 응답을 해석하지 못했어요: {e}"),
                        })?;
                    let first = parsed.choices.into_iter().next().ok_or_else(|| {
                        WorkbenchError::Internal {
                            message:
                                "LM Studio가 빈 choices를 보냈어요. 모델 상태를 확인해 주세요."
                                    .into(),
                        }
                    })?;
                    Ok(first.message.content)
                } else if is_retryable(status) {
                    Err(retryable_status_error("LM Studio", status, &model))
                } else {
                    let body = resp.text().await.unwrap_or_default();
                    Err(non_retryable_status_error(
                        "LM Studio",
                        status,
                        &model,
                        &body,
                    ))
                }
            }
        };

        attempt
            .retry(self.retry_policy())
            .when(is_workbench_retryable)
            .notify(|err, dur| {
                tracing::warn!(
                    error = %err,
                    delay_ms = dur.as_millis() as u64,
                    "lm studio chat retry"
                );
            })
            .await
    }

    /// LM Studio `/v1/models` 사전 polling — `data:[]`면 NotReady 반환.
    async fn lmstudio_precheck_model(&self) -> Result<(), WorkbenchError> {
        let url = format!("{}/v1/models", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(map_reqwest_error)?;
        let status = resp.status();
        if !status.is_success() {
            return Err(WorkbenchError::Internal {
                message: format!(
                    "LM Studio가 준비되지 않았어요. (HTTP {status}) 모델 로드 후 다시 시도해 주세요."
                ),
            });
        }
        let parsed: LmStudioModelsResponse =
            resp.json().await.map_err(|e| WorkbenchError::Internal {
                message: format!("LM Studio 모델 목록을 해석하지 못했어요: {e}"),
            })?;
        if parsed.data.is_empty() {
            return Err(WorkbenchError::Internal {
                message:
                    "LM Studio에 모델이 로드되지 않았어요. LM Studio를 열고 모델을 로드해 주세요."
                        .into(),
            });
        }
        // 모델 ID가 비지정(빈 string)이면 — LM Studio가 자동 선택을 허용하므로 통과.
        if !self.model_id.is_empty() {
            let exists = parsed.data.iter().any(|m| m.id == self.model_id);
            if !exists {
                return Err(WorkbenchError::Internal {
                    message: format!(
                        "LM Studio에서 '{}' 모델이 로드되지 않았어요. 다른 모델로 바꾸거나 로드해 주세요.",
                        self.model_id
                    ),
                });
            }
        }
        Ok(())
    }
}

/// reqwest::Error → WorkbenchError 매핑 — timeout / connect / 기타 분리.
fn map_reqwest_error(e: reqwest::Error) -> WorkbenchError {
    if e.is_timeout() {
        WorkbenchError::Internal {
            message: "런타임 응답이 너무 오래 걸렸어요 (60초 초과). 다시 시도해 주세요.".into(),
        }
    } else if e.is_connect() {
        WorkbenchError::Internal {
            message: "런타임에 연결하지 못했어요. 데몬이 실행 중인지 확인해 주세요.".into(),
        }
    } else {
        WorkbenchError::Internal {
            message: format!("런타임 통신 중 오류가 났어요: {e}"),
        }
    }
}

fn is_retryable(status: StatusCode) -> bool {
    status.as_u16() == 429 || status.is_server_error()
}

fn retryable_status_error(label: &str, status: StatusCode, model: &str) -> WorkbenchError {
    WorkbenchError::Internal {
        message: format!(
            "{label}가 일시적으로 응답하지 못했어요 (HTTP {status}). '{model}' 호출을 다시 시도할게요."
        ),
    }
}

fn non_retryable_status_error(
    label: &str,
    status: StatusCode,
    model: &str,
    body: &str,
) -> WorkbenchError {
    let snippet = body.chars().take(160).collect::<String>();
    WorkbenchError::Internal {
        message: format!("{label} 호출이 실패했어요 (HTTP {status}, 모델 '{model}'): {snippet}"),
    }
}

/// backon when() 콜백 — Internal 에러 중 retryable 표지 메시지가 있는 것만 retry.
fn is_workbench_retryable(e: &WorkbenchError) -> bool {
    match e {
        WorkbenchError::Internal { message } => message.contains("일시적"),
        // Cancelled / EvalFailed / 기타는 재시도 의미 없음.
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio_util::sync::CancellationToken;
    use workbench_core::{baseline_korean_eval_cases, run_eval_suite};

    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Respond, ResponseTemplate};

    // ── helpers ──────────────────────────────────────────────────────

    fn fast_config() -> ResponderConfig {
        ResponderConfig {
            timeout: Duration::from_secs(2),
            max_retries: 2,
            temperature: 0.0,
            num_ctx: 2048,
            max_tokens: 64,
        }
    }

    fn fast_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .connect_timeout(Duration::from_millis(500))
            .no_proxy()
            .build()
            .expect("client")
    }

    /// 호출 횟수 카운터 — 5xx-then-200 retry 시나리오 검증.
    struct SequenceResponder {
        calls: Arc<AtomicUsize>,
        first_status: u16,
        first_body: serde_json::Value,
        eventual_status: u16,
        eventual_body: serde_json::Value,
    }

    impl Respond for SequenceResponder {
        fn respond(&self, _: &wiremock::Request) -> ResponseTemplate {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                ResponseTemplate::new(self.first_status).set_body_json(self.first_body.clone())
            } else {
                ResponseTemplate::new(self.eventual_status)
                    .set_body_json(self.eventual_body.clone())
            }
        }
    }

    // ── Default config / RuntimeKind ─────────────────────────────────

    #[test]
    fn responder_config_defaults_match_research() {
        let c = ResponderConfig::default();
        assert_eq!(c.timeout, Duration::from_secs(60));
        assert_eq!(c.max_retries, 2);
        assert_eq!(c.temperature, 0.0);
        assert_eq!(c.num_ctx, 2048);
        assert_eq!(c.max_tokens, 256);
    }

    #[test]
    fn runtime_kind_kebab_serde_round_trip() {
        for kind in [
            RuntimeKind::Ollama,
            RuntimeKind::LmStudio,
            RuntimeKind::Mock,
        ] {
            let s = serde_json::to_string(&kind).unwrap();
            let back: RuntimeKind = serde_json::from_str(&s).unwrap();
            assert_eq!(kind, back);
        }
        let s = serde_json::to_string(&RuntimeKind::LmStudio).unwrap();
        assert_eq!(s, r#""lm-studio""#);
    }

    #[test]
    fn shared_runtime_kind_round_trip() {
        let r: RuntimeKind = SharedRuntimeKind::Ollama.into();
        assert_eq!(r, RuntimeKind::Ollama);
        let r: RuntimeKind = SharedRuntimeKind::LmStudio.into();
        assert_eq!(r, RuntimeKind::LmStudio);
        let r: RuntimeKind = SharedRuntimeKind::LlamaCpp.into();
        assert_eq!(r, RuntimeKind::Mock);
        // 역방향
        assert!(matches!(
            RuntimeKind::Ollama.to_shared(),
            SharedRuntimeKind::Ollama
        ));
    }

    #[test]
    fn base_url_trims_trailing_slash() {
        let r = WorkbenchResponder::new(RuntimeKind::Ollama, "model", "http://localhost:11434/")
            .unwrap();
        assert_eq!(r.base_url(), "http://localhost:11434");
    }

    // ── Mock variant fallback ────────────────────────────────────────

    #[tokio::test]
    async fn mock_responder_fallback_uses_mock_responder_default_text() {
        let r = WorkbenchResponder::mock();
        assert_eq!(r.runtime_kind(), RuntimeKind::Mock);
        let resp = r.respond("한국의 수도는?").await.unwrap();
        assert!(resp.contains("서울"));
    }

    #[tokio::test]
    async fn mock_responder_passes_baseline_evals() {
        let r = WorkbenchResponder::mock();
        let cases = baseline_korean_eval_cases();
        let cancel = CancellationToken::new();
        let report = run_eval_suite(&r, &cases, &cancel, "mock-model")
            .await
            .unwrap();
        assert_eq!(report.passed_count, 10);
        assert_eq!(report.total, 10);
    }

    #[tokio::test]
    async fn mock_responder_pre_cancelled_returns_cancelled() {
        let r = WorkbenchResponder::mock();
        let cases = baseline_korean_eval_cases();
        let cancel = CancellationToken::new();
        cancel.cancel();
        let err = run_eval_suite(&r, &cases, &cancel, "mock")
            .await
            .unwrap_err();
        assert!(matches!(err, WorkbenchError::Cancelled));
    }

    // ── Ollama happy path / errors / retry ───────────────────────────

    #[tokio::test]
    async fn ollama_happy_path_returns_response_text() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "response": "한국의 수도는 서울이에요.",
                "done": true,
            })))
            .mount(&server)
            .await;
        let r = WorkbenchResponder::new(RuntimeKind::Ollama, "exaone:7b", server.uri())
            .unwrap()
            .with_config(fast_config())
            .with_client(fast_client());
        let resp = r.respond("한국의 수도는?").await.unwrap();
        assert!(resp.contains("서울"));
    }

    #[tokio::test]
    async fn ollama_5xx_then_200_retries_until_success() {
        let server = MockServer::start().await;
        let calls = Arc::new(AtomicUsize::new(0));
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(SequenceResponder {
                calls: calls.clone(),
                first_status: 503,
                first_body: json!({"error": "warmup"}),
                eventual_status: 200,
                eventual_body: json!({"response": "재시도 후 성공이에요.", "done": true}),
            })
            .mount(&server)
            .await;
        let r = WorkbenchResponder::new(RuntimeKind::Ollama, "m", server.uri())
            .unwrap()
            .with_config(fast_config())
            .with_client(fast_client());
        let resp = r.respond("hi").await.unwrap();
        assert!(resp.contains("재시도"));
        // 2회 이상 호출 (1회 실패 + 1회 성공).
        assert!(calls.load(Ordering::SeqCst) >= 2);
    }

    #[tokio::test]
    async fn ollama_5xx_all_retries_exhausted_returns_korean_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(500).set_body_json(json!({"error": "down"})))
            .mount(&server)
            .await;
        let r = WorkbenchResponder::new(RuntimeKind::Ollama, "m", server.uri())
            .unwrap()
            .with_config(fast_config())
            .with_client(fast_client());
        let err = r.respond("hi").await.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("일시적") || msg.contains("Ollama"));
    }

    #[tokio::test]
    async fn ollama_4xx_no_retry_immediate_failure() {
        let server = MockServer::start().await;
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_clone = calls.clone();
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(move |_: &wiremock::Request| {
                calls_clone.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(400).set_body_json(json!({"error": "bad request"}))
            })
            .mount(&server)
            .await;
        let r = WorkbenchResponder::new(RuntimeKind::Ollama, "m", server.uri())
            .unwrap()
            .with_config(fast_config())
            .with_client(fast_client());
        let err = r.respond("hi").await.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("Ollama"));
        // 4xx는 1회만 호출 (no retry).
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn ollama_timeout_returns_korean_message() {
        let server = MockServer::start().await;
        // 응답을 5초 지연시켜 client timeout 2s를 트리거.
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"response": "x", "done": true}))
                    .set_delay(Duration::from_secs(5)),
            )
            .mount(&server)
            .await;
        let r = WorkbenchResponder::new(RuntimeKind::Ollama, "m", server.uri())
            .unwrap()
            .with_config(fast_config())
            .with_client(fast_client());
        let err = r.respond("hi").await.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("오래") || msg.contains("60초") || msg.contains("일시적"),
            "expected timeout or transient korean message, got: {msg}"
        );
    }

    #[tokio::test]
    async fn ollama_empty_response_returns_korean_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"response": "", "done": false})),
            )
            .mount(&server)
            .await;
        let r = WorkbenchResponder::new(RuntimeKind::Ollama, "m", server.uri())
            .unwrap()
            .with_config(fast_config())
            .with_client(fast_client());
        let err = r.respond("hi").await.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("빈 응답") || msg.contains("Ollama"));
    }

    // ── LM Studio happy path / errors / precheck ─────────────────────

    #[tokio::test]
    async fn lmstudio_happy_path_returns_message_content() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{"id": "hermes-3"}],
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "choices": [{"message": {"role": "assistant", "content": "안녕하세요!"}}],
            })))
            .mount(&server)
            .await;
        let r = WorkbenchResponder::new(RuntimeKind::LmStudio, "hermes-3", server.uri())
            .unwrap()
            .with_config(fast_config())
            .with_client(fast_client());
        let resp = r.respond("안녕").await.unwrap();
        assert!(resp.contains("안녕하세요"));
    }

    #[tokio::test]
    async fn lmstudio_no_model_loaded_returns_specific_korean_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": []})))
            .mount(&server)
            .await;
        let r = WorkbenchResponder::new(RuntimeKind::LmStudio, "any", server.uri())
            .unwrap()
            .with_config(fast_config())
            .with_client(fast_client());
        let err = r.respond("hi").await.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("LM Studio"));
        assert!(msg.contains("로드"));
    }

    #[tokio::test]
    async fn lmstudio_model_id_not_in_list_returns_korean_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{"id": "another-model"}],
            })))
            .mount(&server)
            .await;
        let r = WorkbenchResponder::new(RuntimeKind::LmStudio, "missing-model", server.uri())
            .unwrap()
            .with_config(fast_config())
            .with_client(fast_client());
        let err = r.respond("hi").await.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("missing-model"));
    }

    #[tokio::test]
    async fn lmstudio_4xx_no_retry() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{"id": "x"}],
            })))
            .mount(&server)
            .await;
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_clone = calls.clone();
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(move |_: &wiremock::Request| {
                calls_clone.fetch_add(1, Ordering::SeqCst);
                ResponseTemplate::new(401).set_body_json(json!({"error": "unauthorized"}))
            })
            .mount(&server)
            .await;
        let r = WorkbenchResponder::new(RuntimeKind::LmStudio, "x", server.uri())
            .unwrap()
            .with_config(fast_config())
            .with_client(fast_client());
        let err = r.respond("hi").await.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("LM Studio"));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn lmstudio_5xx_then_200_retries_to_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{"id": "x"}],
            })))
            .mount(&server)
            .await;
        let calls = Arc::new(AtomicUsize::new(0));
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(SequenceResponder {
                calls: calls.clone(),
                first_status: 503,
                first_body: json!({"error": "warming"}),
                eventual_status: 200,
                eventual_body: json!({
                    "choices": [{"message": {"role": "assistant", "content": "두 번째에 성공이에요"}}],
                }),
            })
            .mount(&server)
            .await;
        let r = WorkbenchResponder::new(RuntimeKind::LmStudio, "x", server.uri())
            .unwrap()
            .with_config(fast_config())
            .with_client(fast_client());
        let resp = r.respond("hi").await.unwrap();
        assert!(resp.contains("두 번째"));
        assert!(calls.load(Ordering::SeqCst) >= 2);
    }

    #[tokio::test]
    async fn lmstudio_models_endpoint_5xx_returns_korean_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(500).set_body_string("down"))
            .mount(&server)
            .await;
        let r = WorkbenchResponder::new(RuntimeKind::LmStudio, "x", server.uri())
            .unwrap()
            .with_config(fast_config())
            .with_client(fast_client());
        let err = r.respond("hi").await.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("LM Studio"));
    }

    // ── with_client + with_config wiring ─────────────────────────────

    #[tokio::test]
    async fn with_client_uses_injected_client() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "response": "ok", "done": true,
            })))
            .mount(&server)
            .await;
        let custom = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let r = WorkbenchResponder::new(RuntimeKind::Ollama, "m", server.uri())
            .unwrap()
            .with_config(fast_config())
            .with_client(custom);
        let resp = r.respond("p").await.unwrap();
        assert_eq!(resp, "ok");
    }

    #[test]
    fn with_config_overrides_defaults() {
        // Phase R-F+R-G hotfix: 비-localhost host("http://x")는 거부되므로 localhost 사용.
        let r = WorkbenchResponder::new(RuntimeKind::Ollama, "m", "http://localhost:11434")
            .unwrap()
            .with_config(ResponderConfig {
                timeout: Duration::from_secs(10),
                max_retries: 0,
                temperature: 0.7,
                num_ctx: 4096,
                max_tokens: 1024,
            });
        assert_eq!(r.config.max_retries, 0);
        assert_eq!(r.config.temperature, 0.7);
        assert_eq!(r.config.num_ctx, 4096);
        assert_eq!(r.config.max_tokens, 1024);
    }

    #[test]
    fn helper_constructors_set_runtime_kind() {
        assert_eq!(
            WorkbenchResponder::ollama("m").runtime_kind(),
            RuntimeKind::Ollama
        );
        assert_eq!(
            WorkbenchResponder::lm_studio("m").runtime_kind(),
            RuntimeKind::LmStudio
        );
        assert_eq!(WorkbenchResponder::mock().runtime_kind(), RuntimeKind::Mock);
    }

    // ── Phase R-F+R-G hotfix (ADR-0064 §2) — base_url localhost-only allowlist ───

    fn assert_ok(input: &str) {
        let r = validate_localhost_url(input);
        assert!(r.is_ok(), "expected OK for {input:?}, got {:?}", r.err());
    }

    fn assert_reject(input: &str, korean_substr: &str) {
        let err = validate_localhost_url(input).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains(korean_substr),
            "expected '{korean_substr}' in error for {input:?}, got: {msg}"
        );
    }

    #[test]
    fn base_url_ok_localhost_default_port() {
        assert_ok("http://localhost:11434");
    }
    #[test]
    fn base_url_ok_127_loopback() {
        assert_ok("http://127.0.0.1:11434");
    }
    #[test]
    fn base_url_ok_127_loopback_high() {
        // 127.0.0.0/8 전체 허용 (RFC 1122 loopback).
        assert_ok("http://127.5.5.5:11434");
    }
    #[test]
    fn base_url_ok_ipv6_loopback() {
        assert_ok("http://[::1]:11434");
    }
    #[test]
    fn base_url_ok_lm_studio_port() {
        assert_ok("http://localhost:1234");
    }
    #[test]
    fn base_url_ok_localhost_uppercase() {
        assert_ok("http://LOCALHOST:11434");
    }

    #[test]
    fn base_url_reject_https() {
        assert_reject("https://localhost:11434", "https는 외부");
    }
    #[test]
    fn base_url_reject_private_lan_192() {
        assert_reject("http://192.168.0.10:11434", "내 PC 안에서");
    }
    #[test]
    fn base_url_reject_private_lan_10() {
        assert_reject("http://10.0.0.1:11434", "내 PC 안에서");
    }
    #[test]
    fn base_url_reject_link_local_cloud_metadata() {
        // 169.254/16 — AWS/GCP cloud metadata range. SSRF 방지.
        assert_reject("http://169.254.169.254/latest/meta-data", "내 PC 안에서");
    }
    #[test]
    fn base_url_reject_suffix_attack() {
        // `host_str().contains("localhost")` 식 검증의 즉시 우회. 정확 매치로 차단.
        assert_reject("http://localhost.evil.com:11434", "내 PC 안에서");
    }
    #[test]
    fn base_url_reject_userinfo_bypass() {
        // url crate가 username과 host를 분리해 주므로 검증 가능.
        assert_reject("http://attacker@127.0.0.1:11434", "사용자명");
    }
    #[test]
    fn base_url_reject_public_host() {
        assert_reject("http://example.com", "내 PC 안에서");
    }
    #[test]
    fn base_url_reject_empty() {
        assert_reject("", "입력");
    }
    #[test]
    fn base_url_reject_malformed() {
        assert_reject("not-a-url", "형식");
    }
    #[test]
    fn base_url_reject_unspec_v4() {
        // 0.0.0.0은 server bind-sentinel — client dial 대상 아님. Windows winsock 거부.
        assert_reject("http://0.0.0.0:11434", "내 PC 안에서");
    }
    #[test]
    fn base_url_reject_unspec_v6() {
        assert_reject("http://[::]:11434", "내 PC 안에서");
    }

    #[test]
    fn responder_new_returns_err_on_https() {
        let r = WorkbenchResponder::new(RuntimeKind::Ollama, "m", "https://localhost:11434");
        assert!(r.is_err());
    }

    #[test]
    fn responder_new_returns_err_on_public_host() {
        let r = WorkbenchResponder::new(RuntimeKind::Ollama, "m", "http://example.com");
        assert!(r.is_err());
    }

    #[test]
    fn responder_new_normalizes_trailing_slash_via_url() {
        let r =
            WorkbenchResponder::new(RuntimeKind::Ollama, "m", "http://localhost:11434/").unwrap();
        assert_eq!(r.base_url(), "http://localhost:11434");
    }

    #[test]
    fn responder_default_helpers_are_safe_and_match_kind() {
        // expect()가 정상 작동 — default URL은 항상 localhost.
        let o = WorkbenchResponder::ollama("m");
        assert_eq!(o.runtime_kind(), RuntimeKind::Ollama);
        assert_eq!(o.base_url(), "http://localhost:11434");
        let l = WorkbenchResponder::lm_studio("m");
        assert_eq!(l.runtime_kind(), RuntimeKind::LmStudio);
        assert_eq!(l.base_url(), "http://localhost:1234");
    }
}
