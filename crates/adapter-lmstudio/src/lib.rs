//! adapter-lmstudio — 외부 설치형 attach. OpenAI 호환 endpoint.
//!
//! 정책 (Phase 1' 결정):
//! - **Wrap-not-replace**: LM Studio 바이너리 임베드 안 함. EULA 상 재배포 금지.
//! - `start/stop/restart`은 no-op — 외부 데몬은 사용자가 통제.
//! - `install/update`는 bail — `crates/installer`의 `open_url`만 (manifests/apps/lm-studio.json).
//! - `pull_model/remove_model`은 bail — LM Studio UI에서만 가능 (EULA).
//! - `warmup`은 OpenAI 호환 `/v1/chat/completions { max_tokens: 1 }`.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use bench_harness::{BenchAdapter, BenchError, BenchMetricsSource, BenchSample};
use futures::StreamExt;
use openai_compat_dto::{
    ChatChunk as OpenAIChatChunk, ChatRequest as OpenAIChatRequest, ChatTurn as OpenAIChatTurn,
    Content as OpenAIContent, ContentPart as OpenAIContentPart, ImageUrl as OpenAIImageUrl,
};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use runtime_manager::{
    DetectResult, HealthReport, InstallOpts, LocalModel, ProgressSink, RuntimeAdapter, RuntimeCfg,
    RuntimeHandle,
};
use shared_types::{CapabilityMatrix, ModelRef, RuntimeKind, RuntimeState};

const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:1234";
const PROBE_TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Clone)]
pub struct LmStudioAdapter {
    endpoint: String,
    http: reqwest::Client,
}

impl LmStudioAdapter {
    pub fn new() -> Self {
        Self::with_endpoint(DEFAULT_ENDPOINT)
    }

    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        // Phase R-C (ADR-0055) — 폴백 제거. fail-fast on TLS init issue.
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_millis(500))
            .timeout(Duration::from_secs(60))
            .pool_idle_timeout(Duration::from_secs(30))
            .no_proxy()
            .build()
            .expect("reqwest Client builder must succeed (TLS init)");
        Self {
            endpoint: endpoint.into(),
            http,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.endpoint.trim_end_matches('/'), path)
    }
}

impl Default for LmStudioAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// ── DTO ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
    /// LM Studio 응답엔 항상 "model"이지만 unused — 향후 OpenAI batch / file 등 분기 시 사용.
    #[allow(dead_code)]
    #[serde(default)]
    object: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    max_tokens: u32,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

// ── RuntimeAdapter impl ───────────────────────────────────────────────

#[async_trait]
impl RuntimeAdapter for LmStudioAdapter {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::LmStudio
    }

    async fn detect(&self) -> anyhow::Result<DetectResult> {
        let resp = self
            .http
            .get(self.url("/v1/models"))
            .timeout(PROBE_TIMEOUT)
            .send()
            .await;
        match resp {
            Ok(r) if r.status().is_success() => Ok(DetectResult {
                installed: true,
                version: None,
                build_target: None,
            }),
            Ok(r) => Ok(DetectResult {
                installed: false,
                version: None,
                build_target: Some(format!("HTTP {}", r.status())),
            }),
            Err(_) => Ok(DetectResult {
                installed: false,
                version: None,
                build_target: None,
            }),
        }
    }

    async fn install(&self, _: InstallOpts) -> anyhow::Result<()> {
        anyhow::bail!(
            "LM Studio는 EULA 상 자동 설치할 수 없어요. crates/installer의 open_url + manifests/apps/lm-studio.json을 사용해 주세요."
        )
    }

    async fn update(&self) -> anyhow::Result<()> {
        anyhow::bail!("LM Studio는 자체 업데이트를 사용해 주세요.")
    }

    async fn start(&self, _cfg: RuntimeCfg) -> anyhow::Result<RuntimeHandle> {
        let detect = self.detect().await?;
        if !detect.installed {
            anyhow::bail!(
                "LM Studio가 실행 중이 아니에요. LM Studio 앱에서 'Start Server'를 눌러 주세요."
            );
        }
        Ok(RuntimeHandle {
            kind: RuntimeKind::LmStudio,
            instance_id: "external-lm-studio".into(),
            internal_port: 1234,
        })
    }

    async fn stop(&self, _h: &RuntimeHandle) -> anyhow::Result<()> {
        Ok(())
    }

    async fn restart(&self, _h: &RuntimeHandle) -> anyhow::Result<()> {
        Ok(())
    }

    async fn health(&self, _h: &RuntimeHandle) -> HealthReport {
        let started = Instant::now();
        let resp = self
            .http
            .get(self.url("/v1/models"))
            .timeout(PROBE_TIMEOUT)
            .send()
            .await;
        let latency_ms = started.elapsed().as_millis() as u32;
        match resp {
            Ok(r) if r.status().is_success() => HealthReport {
                state: Some(RuntimeState::Active),
                latency_ms: Some(latency_ms),
                error: None,
            },
            Ok(r) => HealthReport {
                state: Some(RuntimeState::Failed),
                latency_ms: Some(latency_ms),
                error: Some(format!("HTTP {}", r.status())),
            },
            Err(e) => HealthReport {
                state: Some(RuntimeState::Failed),
                latency_ms: None,
                error: Some(e.to_string()),
            },
        }
    }

    async fn list_models(&self) -> anyhow::Result<Vec<LocalModel>> {
        let resp = self.http.get(self.url("/v1/models")).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("LM Studio /v1/models HTTP {}", resp.status());
        }
        let body: ModelsResponse = resp.json().await?;
        Ok(body
            .data
            .into_iter()
            .map(|m| LocalModel {
                r#ref: None,
                file_rel_path: m.id,
                size_bytes: 0,
                sha256: String::new(),
            })
            .collect())
    }

    async fn pull_model(&self, _m: &ModelRef, _sink: ProgressSink) -> anyhow::Result<()> {
        anyhow::bail!(
            "LM Studio 모델은 EULA 상 LM Studio 앱에서만 받을 수 있어요. 앱을 열어 주세요."
        )
    }

    async fn remove_model(&self, _m: &ModelRef) -> anyhow::Result<()> {
        anyhow::bail!("LM Studio 모델은 LM Studio 앱에서 직접 삭제해 주세요.")
    }

    async fn warmup(&self, _h: &RuntimeHandle, m: &ModelRef) -> anyhow::Result<()> {
        let body = ChatRequest {
            model: &m.id,
            messages: vec![ChatMessage {
                role: "user",
                content: ".",
            }],
            max_tokens: 1,
            stream: false,
        };
        let resp = self
            .http
            .post(self.url("/v1/chat/completions"))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            anyhow::bail!("LM Studio warmup 실패: HTTP {}", resp.status());
        }
        Ok(())
    }

    fn capability_matrix(&self) -> CapabilityMatrix {
        CapabilityMatrix {
            vision: false,
            tools: true,
            structured_output: true,
            embeddings: true,
        }
    }
}

// ── BenchAdapter impl (Phase 2'.c.2) ──────────────────────────────────
//
// 정책 (phase-2pc-bench-decision.md):
// - OpenAI 호환 `/v1/chat/completions { stream: true }` SSE.
// - SSE `data: {...}` 라인 파싱. `[DONE]` 마커로 종료.
// - 첫 non-empty `delta.content` = TTFT.
// - LM Studio는 native ns-counter 없음 → wallclock 기반 추정.
//   tg_tps = completion_tokens / (e2e_ms - ttft_ms) (초). pp_tps는 None.
// - metrics_source = WallclockEst.

#[derive(Debug, Serialize)]
struct StreamChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    max_tokens: u32,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<StreamChoice>,
    #[serde(default)]
    usage: Option<UsageInfo>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    #[serde(default)]
    delta: DeltaInfo,
    /// "stop" 등이 들어오면 응답 종료 신호 — 현재는 미사용(스트림 EOF로 종료 판단).
    #[allow(dead_code)]
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct DeltaInfo {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UsageInfo {
    /// OpenAI 호환 — 우리는 LM Studio 측에서 prompt_eval_duration을 못 받기에 미사용.
    /// 추후 LM Studio가 native counter를 노출하면 활용 예정.
    #[allow(dead_code)]
    #[serde(default)]
    prompt_tokens: Option<u64>,
    #[serde(default)]
    completion_tokens: Option<u64>,
}

const BENCH_MAX_TOKENS: u32 = 256;

#[async_trait]
impl BenchAdapter for LmStudioAdapter {
    fn runtime_label(&self) -> &'static str {
        "lmstudio"
    }

    async fn run_prompt(
        &self,
        model_id: &str,
        prompt_id: &str,
        prompt_text: &str,
        _keep_alive: &str, // LM Studio는 keep_alive 미지원 — 무시.
        cancel: &CancellationToken,
    ) -> Result<BenchSample, BenchError> {
        let body = StreamChatRequest {
            model: model_id,
            messages: vec![ChatMessage {
                role: "user",
                content: prompt_text,
            }],
            max_tokens: BENCH_MAX_TOKENS,
            stream: true,
        };

        let req_started = Instant::now();
        let resp = self
            .http
            .post(self.url("/v1/chat/completions"))
            .json(&body)
            .send()
            .await
            .map_err(|e| BenchError::RuntimeUnreachable(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            // OpenAI shape: {"error":{"message":"model 'X' not found"}}
            if text.contains("not found") || text.contains("not loaded") {
                return Err(BenchError::ModelNotLoaded(model_id.to_string()));
            }
            return Err(BenchError::Internal(format!(
                "lmstudio HTTP {status}: {text}"
            )));
        }

        let mut stream = resp.bytes_stream();
        let mut buffer: Vec<u8> = Vec::new();

        let mut first_chunk_at: Option<Instant> = None;
        let mut accumulated_text = String::new();
        let mut completion_tokens_estimate: u64 = 0;
        let mut usage: Option<UsageInfo> = None;

        loop {
            tokio::select! {
                () = cancel.cancelled() => {
                    return Err(BenchError::Cancelled);
                }
                next = stream.next() => {
                    match next {
                        Some(Ok(bytes)) => {
                            buffer.extend_from_slice(&bytes);
                            // SSE — `data: ...\n\n` 또는 `data: ...\n`.
                            while let Some(pos) = buffer.iter().position(|b| *b == b'\n') {
                                let raw_line: Vec<u8> = buffer.drain(..=pos).collect();
                                let line = String::from_utf8_lossy(&raw_line);
                                let line = line.trim();
                                if line.is_empty() || !line.starts_with("data:") {
                                    continue;
                                }
                                let payload = line.trim_start_matches("data:").trim();
                                if payload == "[DONE]" {
                                    continue;
                                }
                                let chunk: StreamChunk = match serde_json::from_str(payload) {
                                    Ok(c) => c,
                                    Err(e) => {
                                        tracing::debug!(error = %e, "lmstudio sse parse skip");
                                        continue;
                                    }
                                };
                                if let Some(u) = chunk.usage {
                                    usage = Some(u);
                                }
                                for choice in chunk.choices {
                                    if let Some(content) = choice.delta.content {
                                        if !content.is_empty() && first_chunk_at.is_none() {
                                            first_chunk_at = Some(Instant::now());
                                        }
                                        if !content.is_empty() {
                                            accumulated_text.push_str(&content);
                                            // 토큰 카운터 추정 — usage 안 오면 chunk 수 fallback.
                                            completion_tokens_estimate += 1;
                                        }
                                    }
                                }
                            }
                        }
                        Some(Err(e)) => {
                            return Err(BenchError::RuntimeUnreachable(e.to_string()));
                        }
                        None => break,
                    }
                }
            }
        }

        let e2e = req_started.elapsed();
        let ttft = first_chunk_at.unwrap_or_else(Instant::now) - req_started;
        let gen_duration = e2e.saturating_sub(ttft);

        let completion_tokens = usage
            .as_ref()
            .and_then(|u| u.completion_tokens)
            .unwrap_or(completion_tokens_estimate);

        let tg_tps = if gen_duration.as_secs_f64() > 0.0 && completion_tokens > 0 {
            completion_tokens as f64 / gen_duration.as_secs_f64()
        } else {
            0.0
        };

        let excerpt = if accumulated_text.is_empty() {
            None
        } else {
            Some(accumulated_text.chars().take(80).collect())
        };

        Ok(BenchSample {
            tg_tps,
            pp_tps: None, // LM Studio는 prompt processing 분리 불가.
            ttft_ms: ttft.as_millis() as u32,
            e2e_ms: e2e.as_millis() as u32,
            load_ms: None,
            sample_text_excerpt: excerpt,
            prompt_id: prompt_id.to_string(),
            metrics_source: BenchMetricsSource::WallclockEst,
        })
    }
}

// ── Chat streaming (Phase 13'.h.2.a, ADR-0050) ───────────────────────
//
// 정책:
// - LM Studio는 OpenAI 호환 `/v1/chat/completions` SSE — Ollama와 다른 wire 형식.
// - Vision: messages[i].content가 array — `[{type: "text"}, {type: "image_url"}]`. data URL 인라인.
// - ChatMessage/ChatEvent/ChatOutcome은 adapter-ollama re-use — frontend 단일 시그니처.
// - `[DONE]` 마커 감지 → ChatEvent::Completed.
// - cancel은 stream drop으로 server abort.

impl LmStudioAdapter {
    /// 사용자 in-app 채팅 — OpenAI 호환 SSE 스트리밍.
    pub async fn chat_stream(
        &self,
        model_id: &str,
        messages: &[adapter_ollama::ChatMessage],
        on_event: impl Fn(adapter_ollama::ChatEvent),
        cancel: &CancellationToken,
    ) -> adapter_ollama::ChatOutcome {
        use adapter_ollama::{ChatEvent, ChatOutcome};

        let started = Instant::now();
        let openai_messages: Vec<OpenAIChatTurn> =
            messages.iter().map(convert_message_to_openai).collect();

        let body = OpenAIChatRequest {
            model: model_id,
            messages: openai_messages,
            stream: true,
        };
        let send_fut = self
            .http
            .post(self.url("/v1/chat/completions"))
            .json(&body)
            .send();
        let resp = tokio::select! {
            biased;
            () = cancel.cancelled() => {
                on_event(ChatEvent::Cancelled);
                return ChatOutcome::Cancelled;
            }
            r = send_fut => match r {
                Ok(r) => r,
                Err(e) => {
                    let msg = format!("LM Studio 연결 실패: {e}");
                    on_event(ChatEvent::Failed { message: msg.clone() });
                    return ChatOutcome::Failed(msg);
                }
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            let msg = if status == reqwest::StatusCode::NOT_FOUND || text.contains("not found") {
                format!("이 모델이 LM Studio에 로드돼 있지 않아요. (id={model_id})")
            } else {
                format!("LM Studio HTTP {status}: {text}")
            };
            on_event(ChatEvent::Failed {
                message: msg.clone(),
            });
            return ChatOutcome::Failed(msg);
        }

        let mut stream = resp.bytes_stream();
        let mut buffer: Vec<u8> = Vec::new();
        // Phase R-C (ADR-0055) — adapter-ollama와 동일 정책 (delta_emitted 추적).
        let mut delta_emitted = false;

        loop {
            tokio::select! {
                () = cancel.cancelled() => {
                    on_event(ChatEvent::Cancelled);
                    return ChatOutcome::Cancelled;
                }
                next = stream.next() => {
                    match next {
                        Some(Ok(bytes)) => {
                            buffer.extend_from_slice(&bytes);
                            // SSE — 라인 단위 파싱.
                            while let Some(pos) = buffer.iter().position(|b| *b == b'\n') {
                                let line: Vec<u8> = buffer.drain(..=pos).collect();
                                let trimmed = line.trim_ascii();
                                if trimmed.is_empty() {
                                    continue;
                                }
                                let payload = match trimmed.strip_prefix(b"data: ") {
                                    Some(p) => p.trim_ascii(),
                                    None => match trimmed.strip_prefix(b"data:") {
                                        Some(p) => p.trim_ascii(),
                                        None => continue,
                                    },
                                };
                                if payload == b"[DONE]" {
                                    on_event(ChatEvent::Completed {
                                        took_ms: started.elapsed().as_millis() as u64,
                                    });
                                    return ChatOutcome::Completed;
                                }
                                let chunk: OpenAIChatChunk = match serde_json::from_slice(payload) {
                                    Ok(c) => c,
                                    Err(e) => {
                                        tracing::debug!(error = %e, "lm-studio sse parse skip");
                                        continue;
                                    }
                                };
                                if let Some(choice) = chunk.choices.first() {
                                    if let Some(text) = choice.delta.content.as_deref() {
                                        if !text.is_empty() {
                                            delta_emitted = true;
                                            on_event(ChatEvent::Delta { text: text.to_string() });
                                        }
                                    }
                                    if choice.finish_reason.is_some() {
                                        on_event(ChatEvent::Completed {
                                            took_ms: started.elapsed().as_millis() as u64,
                                        });
                                        return ChatOutcome::Completed;
                                    }
                                }
                            }
                        }
                        Some(Err(e)) => {
                            // Phase R-C — delta 1건 이상 emit됐으면 graceful early disconnect.
                            if delta_emitted {
                                tracing::warn!(error = %e, "lm-studio 스트림 중단 — 부분 응답으로 마감");
                                on_event(ChatEvent::Completed {
                                    took_ms: started.elapsed().as_millis() as u64,
                                });
                                return ChatOutcome::Completed;
                            }
                            let msg = format!("LM Studio 응답 읽기 실패: {e}");
                            on_event(ChatEvent::Failed { message: msg.clone() });
                            return ChatOutcome::Failed(msg);
                        }
                        None => {
                            on_event(ChatEvent::Completed {
                                took_ms: started.elapsed().as_millis() as u64,
                            });
                            return ChatOutcome::Completed;
                        }
                    }
                }
            }
        }
    }
}

/// adapter_ollama::ChatMessage → OpenAI compat turn 변환.
/// images 필드가 비어있으면 plain text content. 있으면 content array (text + image_url parts).
fn convert_message_to_openai(m: &adapter_ollama::ChatMessage) -> OpenAIChatTurn {
    if let Some(images) = m.images.as_ref() {
        if !images.is_empty() {
            let mut parts: Vec<OpenAIContentPart> = Vec::with_capacity(images.len() + 1);
            if !m.content.is_empty() {
                parts.push(OpenAIContentPart::Text {
                    text: m.content.clone(),
                });
            }
            for img_b64 in images {
                parts.push(OpenAIContentPart::ImageUrl {
                    image_url: OpenAIImageUrl {
                        url: format!("data:image/jpeg;base64,{img_b64}"),
                    },
                });
            }
            return OpenAIChatTurn {
                role: m.role.clone(),
                content: OpenAIContent::Array(parts),
            };
        }
    }
    OpenAIChatTurn {
        role: m.role.clone(),
        content: OpenAIContent::Text(m.content.clone()),
    }
}

// Phase R-E.2 (ADR-0057) — OpenAI compat DTO는 `openai-compat-dto` crate로 추출.
// adapter-llama-cpp와 같은 DTO 공유.

#[cfg(test)]
mod tests {
    use super::*;
    use shared_types::ModelCategory;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn model_ref(id: &str) -> ModelRef {
        ModelRef {
            id: id.into(),
            display_name: id.into(),
            category: ModelCategory::AgentGeneral,
        }
    }

    #[tokio::test]
    async fn detect_succeeds_when_running() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"data": [], "object": "list"})),
            )
            .mount(&server)
            .await;
        let a = LmStudioAdapter::with_endpoint(server.uri());
        let d = a.detect().await.unwrap();
        assert!(d.installed);
    }

    #[tokio::test]
    async fn detect_returns_not_installed_on_unreachable() {
        let a = LmStudioAdapter::with_endpoint("http://127.0.0.1:65000");
        let d = a.detect().await.unwrap();
        assert!(!d.installed);
    }

    #[tokio::test]
    async fn list_models_parses_openai_data() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [
                    {"id": "exaone-3.5-7.8b", "object": "model"},
                    {"id": "qwen2.5-7b-instruct", "object": "model"}
                ]
            })))
            .mount(&server)
            .await;
        let a = LmStudioAdapter::with_endpoint(server.uri());
        let models = a.list_models().await.unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].file_rel_path, "exaone-3.5-7.8b");
    }

    #[tokio::test]
    async fn warmup_posts_chat_completion() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "x",
                "object": "chat.completion",
                "created": 0,
                "model": "exaone-3.5-7.8b",
                "choices": [{
                    "index": 0,
                    "message": {"role": "assistant", "content": "."},
                    "finish_reason": "stop"
                }]
            })))
            .expect(1)
            .mount(&server)
            .await;
        let a = LmStudioAdapter::with_endpoint(server.uri());
        a.warmup(
            &RuntimeHandle {
                kind: RuntimeKind::LmStudio,
                instance_id: "x".into(),
                internal_port: 1234,
            },
            &model_ref("exaone-3.5-7.8b"),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn pull_bails_with_eula_guidance() {
        let a = LmStudioAdapter::new();
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let err = a.pull_model(&model_ref("x"), tx).await.unwrap_err();
        assert!(format!("{err}").contains("EULA"));
    }

    #[tokio::test]
    async fn install_bails_with_eula_guidance() {
        let a = LmStudioAdapter::new();
        let err = a.install(InstallOpts::default()).await.unwrap_err();
        assert!(format!("{err}").contains("EULA"));
    }

    #[tokio::test]
    async fn health_active_on_200() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": []})))
            .mount(&server)
            .await;
        let a = LmStudioAdapter::with_endpoint(server.uri());
        let h = a
            .health(&RuntimeHandle {
                kind: RuntimeKind::LmStudio,
                instance_id: "x".into(),
                internal_port: 1234,
            })
            .await;
        assert_eq!(h.state, Some(RuntimeState::Active));
    }

    // ── BenchAdapter 통합 테스트 (Phase 2'.c.2) ──────────────────────

    use bench_harness::BenchAdapter;
    use tokio_util::sync::CancellationToken;

    /// SSE streaming 응답 — 3 token chunk + final usage chunk + [DONE].
    fn lmstudio_sse_body(with_usage: bool) -> String {
        let mut out = String::new();
        for piece in ["안", "녕", "하세요"] {
            let chunk = serde_json::json!({
                "id": "x",
                "object": "chat.completion.chunk",
                "choices": [{"index": 0, "delta": {"content": piece}, "finish_reason": null}]
            });
            out.push_str("data: ");
            out.push_str(&serde_json::to_string(&chunk).unwrap());
            out.push_str("\n\n");
        }
        if with_usage {
            // 일부 LM Studio 빌드는 마지막 chunk에 usage 포함.
            let final_chunk = serde_json::json!({
                "id": "x",
                "object": "chat.completion.chunk",
                "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
                "usage": {"prompt_tokens": 12, "completion_tokens": 30, "total_tokens": 42}
            });
            out.push_str("data: ");
            out.push_str(&serde_json::to_string(&final_chunk).unwrap());
            out.push_str("\n\n");
        }
        out.push_str("data: [DONE]\n\n");
        out
    }

    #[tokio::test]
    async fn run_prompt_uses_wallclock_with_chunk_count_fallback() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(lmstudio_sse_body(false))
                    .insert_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;
        let a = LmStudioAdapter::with_endpoint(server.uri());
        let cancel = CancellationToken::new();
        let sample = a
            .run_prompt("test-model", "bench-ko-chat", "안녕하세요?", "5m", &cancel)
            .await
            .unwrap();
        assert!(matches!(
            sample.metrics_source,
            BenchMetricsSource::WallclockEst
        ));
        assert!(sample.pp_tps.is_none());
        // 3 chunk가 누적된 응답.
        assert!(sample
            .sample_text_excerpt
            .as_deref()
            .unwrap()
            .contains("안녕"));
        assert_eq!(sample.prompt_id, "bench-ko-chat");
    }

    #[tokio::test]
    async fn run_prompt_prefers_usage_completion_tokens() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(lmstudio_sse_body(true))
                    .insert_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;
        let a = LmStudioAdapter::with_endpoint(server.uri());
        let cancel = CancellationToken::new();
        let sample = a
            .run_prompt("test-model", "bench-ko-chat", "안녕?", "5m", &cancel)
            .await
            .unwrap();
        // usage.completion_tokens=30이라 e2e_ms 매우 짧아 tg_tps 큰 값.
        assert!(sample.tg_tps >= 0.0);
        assert!(sample.e2e_ms > 0);
    }

    #[tokio::test]
    async fn run_prompt_returns_model_not_loaded_on_error_text() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(404).set_body_string(
                r#"{"error":{"message":"model 'unknown' not found","type":"not_found_error"}}"#,
            ))
            .mount(&server)
            .await;
        let a = LmStudioAdapter::with_endpoint(server.uri());
        let cancel = CancellationToken::new();
        let err = a
            .run_prompt("unknown", "p", "x", "5m", &cancel)
            .await
            .unwrap_err();
        assert!(matches!(err, BenchError::ModelNotLoaded(_)));
    }

    #[tokio::test]
    async fn run_prompt_unreachable_when_endpoint_dead() {
        let a = LmStudioAdapter::with_endpoint("http://127.0.0.1:65000");
        let cancel = CancellationToken::new();
        let err = a
            .run_prompt("x", "p", "x", "5m", &cancel)
            .await
            .unwrap_err();
        assert!(matches!(err, BenchError::RuntimeUnreachable(_)));
    }

    #[tokio::test]
    async fn run_prompt_label_is_lmstudio() {
        let a = LmStudioAdapter::new();
        assert_eq!(a.runtime_label(), "lmstudio");
    }

    // ── Phase R-E.1 (T3, ADR-0058) — chat_stream graceful early disconnect ──
    //
    // adapter-ollama의 R-E.1 패턴을 LM Studio SSE wire 형식에 맞춰 적용.

    use adapter_ollama::{ChatEvent as OllamaChatEvent, ChatMessage, ChatOutcome as OllamaChatOutcome};
    use std::sync::{Arc as TestArc, Mutex as TestMutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// 응답 헤더 + 부분 SSE body 만 보내고 socket drop. Content-Length 미달성 → reqwest::Error.
    async fn spawn_partial_sse_server(payload: String) -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                let mut buf = [0u8; 4096];
                let _ = socket.read(&mut buf).await;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: 99999\r\n\r\n{}",
                    payload
                );
                let _ = socket.write_all(response.as_bytes()).await;
                drop(socket);
            }
        });
        addr
    }

    #[tokio::test]
    async fn chat_stream_graceful_completed_after_delta_when_disconnect() {
        // OpenAI SSE 한 줄 — `data: {json}\n\n`
        let body = format!(
            "data: {}\n\n",
            serde_json::to_string(&serde_json::json!({
                "choices": [{"delta": {"content": "hi"}, "finish_reason": null}]
            }))
            .unwrap()
        );
        let addr = spawn_partial_sse_server(body).await;
        let endpoint = format!("http://{}", addr);
        let a = LmStudioAdapter::with_endpoint(endpoint);

        let events: TestArc<TestMutex<Vec<OllamaChatEvent>>> =
            TestArc::new(TestMutex::new(Vec::new()));
        let events_for_cb = events.clone();
        let on_event = move |e: OllamaChatEvent| {
            events_for_cb.lock().unwrap().push(e);
        };

        let cancel = CancellationToken::new();
        let messages = vec![ChatMessage {
            role: "user".into(),
            content: "ping".into(),
            images: None,
        }];
        let outcome = a.chat_stream("test", &messages, on_event, &cancel).await;

        assert!(
            matches!(outcome, OllamaChatOutcome::Completed),
            "delta가 emit된 후 disconnect는 Completed여야 (got {outcome:?})"
        );
        let events = events.lock().unwrap();
        let delta_count = events
            .iter()
            .filter(|e| matches!(e, OllamaChatEvent::Delta { .. }))
            .count();
        assert!(delta_count >= 1, "1건 이상 Delta가 emit돼야");
    }

    #[tokio::test]
    async fn chat_stream_failed_when_disconnect_before_any_delta() {
        let addr = spawn_partial_sse_server(String::new()).await;
        let endpoint = format!("http://{}", addr);
        let a = LmStudioAdapter::with_endpoint(endpoint);

        let events: TestArc<TestMutex<Vec<OllamaChatEvent>>> =
            TestArc::new(TestMutex::new(Vec::new()));
        let events_for_cb = events.clone();
        let on_event = move |e: OllamaChatEvent| {
            events_for_cb.lock().unwrap().push(e);
        };

        let cancel = CancellationToken::new();
        let messages = vec![ChatMessage {
            role: "user".into(),
            content: "ping".into(),
            images: None,
        }];
        let outcome = a.chat_stream("test", &messages, on_event, &cancel).await;

        assert!(
            matches!(outcome, OllamaChatOutcome::Failed(_)),
            "delta 0건 + disconnect → Failed (got {outcome:?})"
        );
    }
}
