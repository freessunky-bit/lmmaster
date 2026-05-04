//! OpenAI compat chat completion DTO — `adapter-lmstudio` + `adapter-llama-cpp` 공유.
//!
//! Phase R-E.2 (ADR-0057). 두 어댑터가 동일 DTO를 inline 정의하던 중복 제거.
//! OpenAI `/v1/chat/completions` SSE 스트리밍 와이어 포맷 표준.
//!
//! 정책:
//! - **Serialize 전용 request DTO** — caller가 owned String으로 messages 빌드. `<'a>` lifetime은 model id만.
//! - **Deserialize 전용 response DTO** — `#[serde(default)]`로 LM Studio / llama-server 둘 다 호환.
//! - **content array 패턴** — vision 모델은 `[{type: text}, {type: image_url}]` content array.
//! - **확장 필드 무시** — `tool_choice`, `usage`, `system_fingerprint` 등은 v1 미사용 → DTO 미정의 (deserialize 시 ignored).
//!
//! References: <https://platform.openai.com/docs/api-reference/chat>

use serde::{Deserialize, Serialize};

/// `POST /v1/chat/completions` request body.
#[derive(Debug, Serialize)]
pub struct ChatRequest<'a> {
    pub model: &'a str,
    pub messages: Vec<ChatTurn>,
    pub stream: bool,
}

/// `messages[i]` — 한 turn (system / user / assistant). content는 plain text 또는 array.
#[derive(Debug, Serialize)]
pub struct ChatTurn {
    pub role: String,
    pub content: Content,
}

/// `messages[i].content` — plain text 또는 multimodal content array (vision).
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Content {
    Text(String),
    Array(Vec<ContentPart>),
}

/// `messages[i].content[k]` — vision multimodal part.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
}

/// `image_url` 객체 — `data:image/jpeg;base64,...` URL 또는 외부 https URL.
#[derive(Debug, Serialize)]
pub struct ImageUrl {
    pub url: String,
}

/// SSE chunk — `data: {json}` 한 줄. `[DONE]` 마커는 caller가 검사.
#[derive(Debug, Deserialize)]
pub struct ChatChunk {
    #[serde(default)]
    pub choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
pub struct ChatChoice {
    #[serde(default)]
    pub delta: ChatDelta,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

/// `delta.content` — 토큰 스트림 단위 텍스트.
#[derive(Debug, Default, Deserialize)]
pub struct ChatDelta {
    #[serde(default)]
    pub content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_text_serializes_as_string() {
        let c = Content::Text("hi".into());
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v, serde_json::json!("hi"));
    }

    #[test]
    fn content_array_serializes_as_array() {
        let c = Content::Array(vec![
            ContentPart::Text {
                text: "describe".into(),
            },
            ContentPart::ImageUrl {
                image_url: ImageUrl {
                    url: "data:image/jpeg;base64,AAAA".into(),
                },
            },
        ]);
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(
            v,
            serde_json::json!([
                {"type": "text", "text": "describe"},
                {"type": "image_url", "image_url": {"url": "data:image/jpeg;base64,AAAA"}}
            ])
        );
    }

    #[test]
    fn chunk_deserializes_with_default_choices() {
        let json = r#"{"choices":[{"delta":{"content":"hi"}}]}"#;
        let c: ChatChunk = serde_json::from_str(json).unwrap();
        assert_eq!(c.choices.len(), 1);
        assert_eq!(c.choices[0].delta.content.as_deref(), Some("hi"));
    }

    #[test]
    fn chunk_deserializes_empty_choices() {
        let json = r#"{}"#;
        let c: ChatChunk = serde_json::from_str(json).unwrap();
        assert!(c.choices.is_empty());
    }

    #[test]
    fn chunk_with_finish_reason_round_trip() {
        let json = r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#;
        let c: ChatChunk = serde_json::from_str(json).unwrap();
        assert_eq!(c.choices[0].finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn request_serializes_with_stream_true() {
        let req = ChatRequest {
            model: "gemma-3-4b",
            messages: vec![ChatTurn {
                role: "user".into(),
                content: Content::Text("ping".into()),
            }],
            stream: true,
        };
        let v = serde_json::to_value(&req).unwrap();
        assert_eq!(v["model"], "gemma-3-4b");
        assert_eq!(v["stream"], true);
        assert_eq!(v["messages"][0]["content"], "ping");
    }
}
