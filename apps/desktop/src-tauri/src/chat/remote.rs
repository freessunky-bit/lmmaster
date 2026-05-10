//! 원격 LMmaster 게이트웨이 채팅 — OpenAI /v1/chat/completions SSE 스트리밍.
//!
//! 정책:
//! - `endpoint_id`로 settings에서 연결 정보(base_url, api_key) 조회.
//! - reqwest SSE 스트리밍 → ChatEvent Channel.
//! - CancellationToken으로 중단 가능 (기존 ChatRegistry 재사용).
//! - HTTP 상태 코드별 한국어 에러 안내.
//! - delta 1건 이상 수신 후 연결 끊김 → graceful Completed (부분 응답 보존, ADR-0055 패턴).

use std::sync::Arc;
use std::time::Instant;

use chat_protocol::{ChatEvent, ChatMessage, FinishReason};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use super::{ChatApiError, ChatOutcomeIpc};
use crate::chat::registry::ChatRegistry;

// ── OpenAI SSE 청크 타입 ─────────────────────────────────────────────

#[derive(Serialize)]
struct RemoteChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    stream: bool,
}

#[derive(Deserialize)]
struct OpenAIChunk {
    choices: Vec<ChunkChoice>,
}

#[derive(Deserialize)]
struct ChunkChoice {
    delta: Delta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize, Default)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
}

// ── IPC command ──────────────────────────────────────────────────────

/// 원격 LMmaster 게이트웨이로 채팅 스트리밍.
///
/// `endpoint_id` — settings.json의 remote_endpoints[].id.
/// `model_id` — 원격 서버의 Ollama model ID (예: "stheno-l3-8b").
/// `messages` — 전체 대화 history (기존 채팅 IPC와 동일).
/// `channel` — Tauri IPC Channel<ChatEvent> (delta 실시간 전달).
#[tauri::command]
pub async fn start_remote_chat(
    app: AppHandle,
    registry: State<'_, Arc<ChatRegistry>>,
    endpoint_id: String,
    model_id: String,
    messages: Vec<ChatMessage>,
    channel: Channel<ChatEvent>,
) -> Result<ChatOutcomeIpc, ChatApiError> {
    // 1. settings에서 endpoint 조회.
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| ChatApiError::Internal {
            message: format!("app_local_data_dir 해결 실패: {e}"),
        })?;
    let settings = crate::settings::UserSettings::load(&dir);
    let endpoint = settings
        .remote_endpoints
        .into_iter()
        .find(|e| e.id == endpoint_id)
        .ok_or_else(|| ChatApiError::Internal {
            message: format!(
                "원격 연결 정보를 찾을 수 없어요 (id={endpoint_id}). 다시 등록해 주세요."
            ),
        })?;

    // 2. 채팅 ID + cancel token (ChatRegistry로 기존 cancel_all_chats와 호환).
    let chat_id = uuid::Uuid::new_v4().to_string();
    let cancel = registry.start(&chat_id);
    let registry_clone: Arc<ChatRegistry> = (*registry).clone();
    struct Guard {
        registry: Arc<ChatRegistry>,
        id: String,
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            self.registry.finish(&self.id);
        }
    }
    let _guard = Guard {
        registry: registry_clone,
        id: chat_id,
    };

    // 3. reqwest 클라이언트 구성.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| ChatApiError::Internal {
            message: format!("HTTP 클라이언트 생성 실패: {e}"),
        })?;

    // 4. 요청 전송 — cancel 먼저 체크 (select! biased).
    let url = format!(
        "{}/chat/completions",
        endpoint.base_url.trim_end_matches('/')
    );
    let body = RemoteChatRequest {
        model: &model_id,
        messages: &messages,
        stream: true,
    };
    let started = Instant::now();

    let send_fut = client
        .post(&url)
        .bearer_auth(&endpoint.api_key)
        .json(&body)
        .send();

    let resp = tokio::select! {
        biased;
        () = cancel.cancelled() => {
            let _ = channel.send(ChatEvent::Cancelled);
            return Ok(ChatOutcomeIpc::Cancelled);
        }
        r = send_fut => match r {
            Ok(r) => r,
            Err(e) => {
                let msg = format!(
                    "원격 서버에 연결할 수 없어요 ({}). 사용자 A의 LMmaster가 켜져 있고 LAN 노출이 활성화됐는지 확인해 주세요. 오류: {e}",
                    endpoint.alias
                );
                tracing::warn!(endpoint_id, alias = %endpoint.alias, error = %e, "원격 채팅 연결 실패");
                let _ = channel.send(ChatEvent::Failed { message: msg.clone() });
                return Ok(ChatOutcomeIpc::Failed { message: msg });
            }
        }
    };

    // 5. HTTP 상태 코드 검증.
    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        let msg = match status.as_u16() {
            401 | 403 => "API 키가 올바르지 않아요. 키를 다시 확인해 주세요.".to_string(),
            404 => format!(
                "모델을 찾을 수 없어요: {model_id}. 사용자 A가 모델을 설치했는지 확인해 주세요."
            ),
            429 => "요청이 너무 많아요. 잠시 후 다시 시도해 주세요.".to_string(),
            _ => format!("원격 서버 오류 HTTP {status}: {body_text}"),
        };
        tracing::warn!(endpoint_id, %status, "원격 채팅 HTTP 오류");
        let _ = channel.send(ChatEvent::Failed {
            message: msg.clone(),
        });
        return Ok(ChatOutcomeIpc::Failed { message: msg });
    }

    // 6. SSE 스트리밍.
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut delta_emitted = false;

    loop {
        tokio::select! {
            biased;
            () = cancel.cancelled() => {
                let _ = channel.send(ChatEvent::Cancelled);
                return Ok(ChatOutcomeIpc::Cancelled);
            }
            next = stream.next() => match next {
                None => {
                    // 스트림 정상 종료 ([DONE] 없이 닫히는 경우 포함).
                    let _ = channel.send(ChatEvent::Completed {
                        took_ms: started.elapsed().as_millis() as u64,
                        finish_reason: FinishReason::Aborted,
                    });
                    return Ok(ChatOutcomeIpc::Completed);
                }
                Some(Err(e)) => {
                    // delta 수신 후 끊김 → 부분 응답 보존 (ADR-0055 패턴).
                    if delta_emitted {
                        tracing::warn!(endpoint_id, error = %e, "원격 스트림 중단 — 부분 응답으로 마감");
                        let _ = channel.send(ChatEvent::Completed {
                            took_ms: started.elapsed().as_millis() as u64,
                            finish_reason: FinishReason::Aborted,
                        });
                        return Ok(ChatOutcomeIpc::Completed);
                    }
                    let msg = format!("원격 스트림이 끊겼어요: {e}");
                    let _ = channel.send(ChatEvent::Failed { message: msg.clone() });
                    return Ok(ChatOutcomeIpc::Failed { message: msg });
                }
                Some(Ok(bytes)) => {
                    // UTF-8 손실 없이 버퍼 추가.
                    buf.push_str(&String::from_utf8_lossy(&bytes));

                    // 줄 단위로 SSE 청크 처리.
                    while let Some(pos) = buf.find('\n') {
                        let line: String = buf.drain(..=pos).collect();
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        if let Some(data) = line.strip_prefix("data: ") {
                            if data == "[DONE]" {
                                let _ = channel.send(ChatEvent::Completed {
                                    took_ms: started.elapsed().as_millis() as u64,
                                    finish_reason: FinishReason::Unknown,
                                });
                                return Ok(ChatOutcomeIpc::Completed);
                            }
                            match serde_json::from_str::<OpenAIChunk>(data) {
                                Ok(chunk) => {
                                    for choice in chunk.choices {
                                        if let Some(content) = choice.delta.content {
                                            if !content.is_empty() {
                                                delta_emitted = true;
                                                if channel
                                                    .send(ChatEvent::Delta { text: content })
                                                    .is_err()
                                                {
                                                    // 채널 닫힘 = 사용자가 화면 닫음 → cancel.
                                                    cancel.cancel();
                                                }
                                            }
                                        }
                                        if let Some(reason) = choice.finish_reason.as_deref() {
                                            let mapped = match reason {
                                                "stop" => FinishReason::Stop,
                                                "length" => FinishReason::Length,
                                                _ => FinishReason::Unknown,
                                            };
                                            let _ = channel.send(ChatEvent::Completed {
                                                took_ms: started.elapsed().as_millis() as u64,
                                                finish_reason: mapped,
                                            });
                                            return Ok(ChatOutcomeIpc::Completed);
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::debug!(error = %e, data, "SSE 청크 파싱 건너뜀");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
