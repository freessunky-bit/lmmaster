//! 사용자 in-app chat IPC — Ollama `/api/chat` 스트리밍 wrapper.
//!
//! 정책 (사용자 모델 검증/체험 — 2026-04-30):
//! - 카탈로그/측정 끝난 모델을 데스크톱 안에서 바로 채팅으로 검증할 수 있게.
//! - 외부 웹앱은 여전히 gateway `/v1/chat/completions` (with API key) 사용 — 별개.
//! - 동일 model_id 다중 동시 채팅 허용. cancel은 token 기반.

pub mod registry;

use std::sync::Arc;

use adapter_lmstudio::LmStudioAdapter;
use adapter_ollama::OllamaAdapter;
use chat_protocol::{ChatEvent, ChatMessage, ChatOutcome};
use serde::Serialize;
use shared_types::RuntimeKind;
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use thiserror::Error;
use uuid::Uuid;

use registry::ChatRegistry;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ChatApiError {
    #[error("아직 지원하지 않는 런타임이에요: {runtime}")]
    UnsupportedRuntime { runtime: String },

    #[error("채팅 중 내부 오류: {message}")]
    Internal { message: String },
}

/// 채팅 시작 — 매 호출 신규 chat_id 발급. 같은 chat_id로 cancel 가능.
#[tauri::command]
pub async fn start_chat(
    _app: AppHandle,
    registry: State<'_, Arc<ChatRegistry>>,
    runtime_kind: RuntimeKind,
    model_id: String,
    messages: Vec<ChatMessage>,
    channel: Channel<ChatEvent>,
) -> Result<ChatOutcomeIpc, ChatApiError> {
    let registry: Arc<ChatRegistry> = (*registry).clone();
    let chat_id = Uuid::new_v4().to_string();
    let cancel = registry.start(&chat_id);

    // RAII guard — Drop으로 finish 보장.
    struct Guard {
        registry: Arc<ChatRegistry>,
        chat_id: String,
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            self.registry.finish(&self.chat_id);
        }
    }
    let _g = Guard {
        registry: registry.clone(),
        chat_id: chat_id.clone(),
    };

    match runtime_kind {
        RuntimeKind::Ollama => {
            let adapter = OllamaAdapter::new();
            let channel_tx = channel.clone();
            // Phase R-E.6 (ADR-0058) — Channel send 실패 = 사용자 화면 닫음 → 즉시 cancel cascade.
            // backend stream 자원(reqwest connection / GPU 추론)을 다음 chunk 대기 없이 drop.
            let cancel_for_emit = cancel.clone();
            let outcome = adapter
                .chat_stream(
                    &model_id,
                    &messages,
                    move |event| {
                        if channel_tx.send(event).is_err() {
                            tracing::debug!("chat channel closed — cancelling backend stream");
                            cancel_for_emit.cancel();
                        }
                    },
                    &cancel,
                )
                .await;
            Ok(outcome.into())
        }
        RuntimeKind::LmStudio => {
            // Phase 13'.h.2.a (ADR-0050) — LM Studio OpenAI compat /v1/chat/completions SSE.
            // adapter-lmstudio::chat_stream가 ChatMessage(images 포함) 그대로 받아 vision content array로 변환.
            let adapter = LmStudioAdapter::new();
            let channel_tx = channel.clone();
            // Phase R-E.6 (ADR-0058) — Channel close → cancel cascade (Ollama branch와 동일).
            let cancel_for_emit = cancel.clone();
            let outcome = adapter
                .chat_stream(
                    &model_id,
                    &messages,
                    move |event| {
                        if channel_tx.send(event).is_err() {
                            tracing::debug!(
                                "lm-studio chat channel closed — cancelling backend stream"
                            );
                            cancel_for_emit.cancel();
                        }
                    },
                    &cancel,
                )
                .await;
            Ok(outcome.into())
        }
        other => Err(ChatApiError::UnsupportedRuntime {
            runtime: format!("{other:?}").to_lowercase(),
        }),
    }
}

/// 진행 중인 모든 채팅 cancel — 같은 model_id 그룹은 모두 abort. UI는 Cancelled 이벤트로 정리.
#[tauri::command]
pub fn cancel_all_chats(registry: State<'_, Arc<ChatRegistry>>) {
    registry.cancel_all();
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ChatOutcomeIpc {
    Completed,
    Cancelled,
    Failed { message: String },
}

impl From<ChatOutcome> for ChatOutcomeIpc {
    fn from(o: ChatOutcome) -> Self {
        match o {
            ChatOutcome::Completed => Self::Completed,
            ChatOutcome::Cancelled => Self::Cancelled,
            ChatOutcome::Failed(m) => Self::Failed { message: m },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_serializes_kebab() {
        let v = serde_json::to_value(ChatOutcomeIpc::Completed).unwrap();
        assert_eq!(v["kind"], "completed");
    }

    #[test]
    fn api_error_serializes_with_kind_tag() {
        let e = ChatApiError::UnsupportedRuntime {
            runtime: "vllm".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "unsupported-runtime");
        assert_eq!(v["runtime"], "vllm");
    }
}
