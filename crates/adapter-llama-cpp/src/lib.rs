//! adapter-llama-cpp — `llama-server` HTTP client wrapping (Phase 13'.h.2.b/c, ADR-0051).
//!
//! 책임 분리 (ADR-0051):
//! - **runner-llama-cpp** = process lifecycle (spawn/port/health/stderr_map).
//! - **adapter-llama-cpp** = HTTP client wrapping (OpenAI compat `/v1/chat/completions` SSE).
//!
//! 정책:
//! - `RuntimeAdapter::start`은 외부에서 spawn된 endpoint에 attach (chat IPC가 LlamaServerHandle 보관).
//! - `chat_stream` — adapter-lmstudio OpenAI compat 패턴 재사용. vision content array 변환 포함.
//! - 외부 통신 0 — localhost-only base_url.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::StreamExt;
use openai_compat_dto::{
    ChatChunk as OpenAIChatChunk, ChatRequest as OpenAIChatRequest, ChatTurn as OpenAIChatTurn,
    Content as OpenAIContent, ContentPart as OpenAIContentPart, ImageUrl as OpenAIImageUrl,
};
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use runtime_manager::{
    DetectResult, HealthReport, InstallOpts, LocalModel, ProgressSink, RuntimeAdapter, RuntimeCfg,
    RuntimeHandle,
};
use shared_types::{CapabilityMatrix, ModelRef, RuntimeKind, RuntimeState};

const PROBE_TIMEOUT: Duration = Duration::from_millis(1500);

/// `llama-server` HTTP wrapper. endpoint(base_url) 주입형 — runner-llama-cpp가 spawn 후
/// LlamaServerHandle::endpoint를 받아 본 어댑터에 base_url 전달.
///
/// `LlamaCppAdapter::new()`는 default endpoint(http://127.0.0.1:8080)로 생성 — 사용자가 외부에서
/// 직접 띄운 server에 attach 시 사용. 자동 spawn 흐름은 chat IPC가 with_endpoint 사용.
#[derive(Clone)]
pub struct LlamaCppAdapter {
    endpoint: String,
    http: reqwest::Client,
}

impl LlamaCppAdapter {
    /// default endpoint (http://127.0.0.1:8080) — 외부에서 직접 띄운 server attach용.
    pub fn new() -> Self {
        Self::with_endpoint("http://127.0.0.1:8080")
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

impl Default for LlamaCppAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// ── DTO (OpenAI compat) ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
    #[allow(dead_code)]
    #[serde(default)]
    object: String,
}

// ── RuntimeAdapter impl ──────────────────────────────────────────────
//
// 정책 (Phase 13'.h.2.b 핵심):
// - install/update/pull_model/remove_model = bail (사용자 직접 build/download).
// - start = endpoint detect만 (자동 spawn은 chat IPC가 LlamaServerHandle 사용).
// - stop/restart = no-op (drop이 처리).
// - warmup = `/v1/chat/completions { max_tokens: 1 }`.

#[async_trait]
impl RuntimeAdapter for LlamaCppAdapter {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::LlamaCpp
    }

    async fn detect(&self) -> anyhow::Result<DetectResult> {
        let resp = self
            .http
            .get(self.url("/health"))
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
            "llama.cpp는 자동 설치를 지원하지 않아요. ggml-org/llama.cpp Releases에서 직접 받으신 후 LMMASTER_LLAMA_SERVER_PATH 환경변수에 경로를 지정해 주세요."
        )
    }

    async fn update(&self) -> anyhow::Result<()> {
        anyhow::bail!("llama.cpp는 자체 빌드를 사용해 주세요.")
    }

    async fn start(&self, _cfg: RuntimeCfg) -> anyhow::Result<RuntimeHandle> {
        let detect = self.detect().await?;
        if !detect.installed {
            anyhow::bail!(
                "llama-server가 응답하지 않아요. Settings에서 자동 시작을 활성화하거나 외부에서 직접 띄워 주세요."
            );
        }
        // Port는 endpoint URL에서 추출 — 단순 문자열 파싱.
        let port = parse_port_from_endpoint(&self.endpoint).unwrap_or(8080);
        Ok(RuntimeHandle {
            kind: RuntimeKind::LlamaCpp,
            instance_id: "llama-cpp-attach".into(),
            internal_port: port,
        })
    }

    async fn stop(&self, _h: &RuntimeHandle) -> anyhow::Result<()> {
        // process lifecycle은 LlamaServerHandle drop이 처리 — 본 어댑터는 attach 모드.
        Ok(())
    }

    async fn restart(&self, _h: &RuntimeHandle) -> anyhow::Result<()> {
        Ok(())
    }

    async fn health(&self, _h: &RuntimeHandle) -> HealthReport {
        let started = Instant::now();
        let resp = self
            .http
            .get(self.url("/health"))
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
            anyhow::bail!("llama-server /v1/models HTTP {}", resp.status());
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
            "llama.cpp 모델은 카탈로그에서 받거나 HuggingFace에서 직접 GGUF 파일을 받아 주세요."
        )
    }

    async fn remove_model(&self, _m: &ModelRef) -> anyhow::Result<()> {
        anyhow::bail!("llama.cpp 모델은 GGUF 파일을 직접 삭제해 주세요.")
    }

    async fn warmup(&self, _h: &RuntimeHandle, m: &ModelRef) -> anyhow::Result<()> {
        let body = serde_json::json!({
            "model": m.id,
            "messages": [{"role": "user", "content": "."}],
            "max_tokens": 1,
            "stream": false
        });
        let resp = self
            .http
            .post(self.url("/v1/chat/completions"))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            anyhow::bail!("llama-server warmup 실패: HTTP {}", resp.status());
        }
        Ok(())
    }

    fn capability_matrix(&self) -> CapabilityMatrix {
        CapabilityMatrix {
            // mmproj가 적용된 server라면 vision 작동. capability는 *server-side* 결정 — 여기선 true 표시
            // 후 manifest의 vision_support로 게이팅.
            vision: true,
            tools: true,
            structured_output: true,
            embeddings: true,
        }
    }
}

/// "http://127.0.0.1:8080" → 8080. 파싱 실패 시 None.
fn parse_port_from_endpoint(endpoint: &str) -> Option<u16> {
    // 매우 단순한 파싱 — `:port[/path]?` 패턴.
    let after_scheme = endpoint.split("://").nth(1)?;
    let host_port = after_scheme.split('/').next()?;
    let port_str = host_port.rsplit(':').next()?;
    port_str.parse().ok()
}

// ── Chat streaming (Phase 13'.h.2.b/c, ADR-0051) ─────────────────────
//
// 정책:
// - llama-server는 OpenAI compat `/v1/chat/completions` SSE — adapter-lmstudio와 동일 wire 형식.
// - Vision: `messages[i].content`가 array — `[{type: "text"}, {type: "image_url"}]`. data URL 인라인.
// - ChatMessage/ChatEvent/ChatOutcome은 adapter-ollama re-use — frontend 단일 시그니처.
// - `[DONE]` 마커 또는 finish_reason → ChatEvent::Completed.
// - cancel은 stream drop으로 server abort.
// - vision_support=true + mmproj 미적용 server → server가 422 또는 텍스트 무시 — stderr_map이 매핑.

impl LlamaCppAdapter {
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
                    let msg = format!("llama-server 연결 실패: {e}");
                    on_event(ChatEvent::Failed { message: msg.clone() });
                    return ChatOutcome::Failed(msg);
                }
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            let msg = if status == reqwest::StatusCode::NOT_FOUND || text.contains("not found") {
                format!("이 모델이 llama-server에 로드돼 있지 않아요. (id={model_id})")
            } else {
                format!("llama-server HTTP {status}: {text}")
            };
            on_event(ChatEvent::Failed {
                message: msg.clone(),
            });
            return ChatOutcome::Failed(msg);
        }

        let mut stream = resp.bytes_stream();
        let mut buffer: Vec<u8> = Vec::new();
        // Phase R-C (ADR-0055) — adapter-ollama / adapter-lmstudio와 동일 정책.
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
                                        tracing::debug!(error = %e, "llama-server sse parse skip");
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
                                tracing::warn!(error = %e, "llama-server 스트림 중단 — 부분 응답으로 마감");
                                on_event(ChatEvent::Completed {
                                    took_ms: started.elapsed().as_millis() as u64,
                                });
                                return ChatOutcome::Completed;
                            }
                            let msg = format!("llama-server 응답 읽기 실패: {e}");
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
/// images 비어있으면 plain text content. 있으면 content array (text + image_url parts).
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
// adapter-lmstudio와 같은 DTO 공유.

#[cfg(test)]
mod tests {
    use super::*;
    use adapter_ollama::{ChatEvent, ChatMessage, ChatOutcome};
    use std::sync::{Arc, Mutex};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn detect_succeeds_when_health_200() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"status":"ok"}"#))
            .mount(&server)
            .await;
        let a = LlamaCppAdapter::with_endpoint(server.uri());
        let d = a.detect().await.unwrap();
        assert!(d.installed);
    }

    #[tokio::test]
    async fn detect_returns_not_installed_on_unreachable() {
        let a = LlamaCppAdapter::with_endpoint("http://127.0.0.1:65000");
        let d = a.detect().await.unwrap();
        assert!(!d.installed);
    }

    #[test]
    fn capability_vision_true() {
        let a = LlamaCppAdapter::new();
        assert!(a.capability_matrix().vision);
    }

    #[test]
    fn parse_port_extracts_8080() {
        assert_eq!(
            parse_port_from_endpoint("http://127.0.0.1:8080"),
            Some(8080)
        );
        assert_eq!(
            parse_port_from_endpoint("http://localhost:9999/"),
            Some(9999)
        );
        assert_eq!(parse_port_from_endpoint("http://example.com"), None);
    }

    #[test]
    fn convert_plain_text_message() {
        let m = ChatMessage {
            role: "user".into(),
            content: "안녕".into(),
            images: None,
        };
        let turn = convert_message_to_openai(&m);
        assert_eq!(turn.role, "user");
        match turn.content {
            OpenAIContent::Text(s) => assert_eq!(s, "안녕"),
            OpenAIContent::Array(_) => panic!("plain text expected"),
        }
    }

    #[test]
    fn convert_vision_message_to_content_array() {
        let m = ChatMessage {
            role: "user".into(),
            content: "이미지 설명".into(),
            images: Some(vec!["abc123base64".into()]),
        };
        let turn = convert_message_to_openai(&m);
        match turn.content {
            OpenAIContent::Array(parts) => {
                assert_eq!(parts.len(), 2);
                let v = serde_json::to_value(&parts).unwrap();
                assert_eq!(v[0]["type"], "text");
                assert_eq!(v[0]["text"], "이미지 설명");
                assert_eq!(v[1]["type"], "image_url");
                assert!(v[1]["image_url"]["url"]
                    .as_str()
                    .unwrap()
                    .starts_with("data:image/jpeg;base64,"));
            }
            OpenAIContent::Text(_) => panic!("content array expected"),
        }
    }

    #[test]
    fn convert_empty_text_with_images_omits_text_part() {
        let m = ChatMessage {
            role: "user".into(),
            content: "".into(),
            images: Some(vec!["b64".into()]),
        };
        let turn = convert_message_to_openai(&m);
        match turn.content {
            OpenAIContent::Array(parts) => {
                // text part 없이 image_url만.
                assert_eq!(parts.len(), 1);
                let v = serde_json::to_value(&parts).unwrap();
                assert_eq!(v[0]["type"], "image_url");
            }
            OpenAIContent::Text(_) => panic!("array expected"),
        }
    }

    fn build_sse_body(deltas: &[&str]) -> String {
        let mut out = String::new();
        for piece in deltas {
            let chunk = serde_json::json!({
                "id": "x",
                "object": "chat.completion.chunk",
                "choices": [{"index": 0, "delta": {"content": piece}, "finish_reason": null}]
            });
            out.push_str("data: ");
            out.push_str(&serde_json::to_string(&chunk).unwrap());
            out.push_str("\n\n");
        }
        out.push_str("data: [DONE]\n\n");
        out
    }

    #[tokio::test]
    async fn chat_stream_emits_deltas_and_completed() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(build_sse_body(&["안", "녕", "하세요"]))
                    .insert_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;
        let a = LlamaCppAdapter::with_endpoint(server.uri());

        let events: Arc<Mutex<Vec<ChatEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let cancel = CancellationToken::new();
        let outcome = a
            .chat_stream(
                "test-model",
                &[ChatMessage {
                    role: "user".into(),
                    content: "안녕?".into(),
                    images: None,
                }],
                move |e| events_clone.lock().unwrap().push(e),
                &cancel,
            )
            .await;
        assert!(matches!(outcome, ChatOutcome::Completed));
        let evs = events.lock().unwrap();
        let deltas: Vec<&str> = evs
            .iter()
            .filter_map(|e| match e {
                ChatEvent::Delta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(deltas.join(""), "안녕하세요");
        assert!(matches!(evs.last(), Some(ChatEvent::Completed { .. })));
    }

    #[tokio::test]
    async fn chat_stream_model_not_loaded_returns_failed() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(404).set_body_string(
                r#"{"error":{"message":"model 'unknown' not found","type":"not_found_error"}}"#,
            ))
            .mount(&server)
            .await;
        let a = LlamaCppAdapter::with_endpoint(server.uri());
        let cancel = CancellationToken::new();
        let outcome = a
            .chat_stream(
                "unknown",
                &[ChatMessage {
                    role: "user".into(),
                    content: "x".into(),
                    images: None,
                }],
                |_| {},
                &cancel,
            )
            .await;
        match outcome {
            ChatOutcome::Failed(msg) => {
                assert!(msg.contains("로드돼 있지 않아요") || msg.contains("not found"));
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn chat_stream_cancel_returns_cancelled() {
        let server = MockServer::start().await;
        // 응답이 영원히 안 오도록 mount X — connect 실패로 즉시 실패할 수 있어 별도 path.
        // wiremock에서 응답 지연은 set_delay 사용.
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_secs(10))
                    .set_body_string("data: [DONE]\n\n")
                    .insert_header("content-type", "text/event-stream"),
            )
            .mount(&server)
            .await;
        let a = LlamaCppAdapter::with_endpoint(server.uri());
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            cancel_clone.cancel();
        });
        let outcome = a
            .chat_stream(
                "test",
                &[ChatMessage {
                    role: "user".into(),
                    content: "x".into(),
                    images: None,
                }],
                |_| {},
                &cancel,
            )
            .await;
        assert!(matches!(outcome, ChatOutcome::Cancelled));
    }
}
