//! Chat 프로토콜 공통 타입 — 모든 RuntimeAdapter (`adapter-ollama` / `adapter-lmstudio` /
//! `adapter-llama-cpp`)와 frontend IPC가 공유.
//!
//! Phase R-E.3 (ADR-0058). 기존엔 `adapter-ollama`에 정의된 `ChatMessage` / `ChatEvent` /
//! `ChatOutcome`을 다른 두 어댑터가 역의존하던 구조 → 의존 방향 정상화.
//!
//! 정책:
//! - 와이어 호환은 `serde(tag/rename_all)` annotation으로 보존 — 기존 frontend / IPC 동작 0 변경.
//! - `ChatMessage.images: Option<Vec<String>>` — Phase 13'.h(ADR-0050) 멀티모달 vision 페이로드.
//! - `serde(skip_serializing_if = "Option::is_none", default)` 백워드 호환.

use serde::{Deserialize, Serialize};

/// 한 chat turn 메시지 — Ollama `/api/chat::messages[i]` 미러 + OpenAI compat content array
/// 변환은 어댑터 측 로직(`convert_message_to_openai`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// "system" / "user" / "assistant".
    pub role: String,
    pub content: String,
    /// Phase 13'.h (ADR-0050) — 멀티모달 이미지. base64 인코딩 string 배열.
    /// `None` 또는 빈 vec이면 텍스트 전용 (기존 호환).
    /// `vision_support: true` 모델만 의미 — 그 외 모델은 어댑터가 무시 또는 에러.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub images: Option<Vec<String>>,
}

/// Chat 스트림 이벤트 — UI에 실시간 token chunk 전달.
///
/// kebab-case tag (frontend TypeScript narrow 친화).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ChatEvent {
    /// 토큰 단위 추가 텍스트 (delta). UI는 누적 표시.
    Delta {
        text: String,
    },
    /// 정상 종료. 마지막 chunk 후 emit. R-C.2(ADR-0055)의 graceful early disconnect도 동일 emit.
    Completed {
        /// 총 응답 ms — 호출 측 elapsed 측정용 hint.
        took_ms: u64,
    },
    Cancelled,
    Failed {
        message: String,
    },
}

/// Chat 호출 결과 — 어댑터 함수 반환 + IPC outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatOutcome {
    Completed,
    Cancelled,
    Failed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_message_without_images_serializes_without_images_field() {
        let m = ChatMessage {
            role: "user".into(),
            content: "안녕".into(),
            images: None,
        };
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["role"], "user");
        assert_eq!(v["content"], "안녕");
        assert!(v.get("images").is_none(), "None images는 직렬화 skip");
    }

    #[test]
    fn chat_message_with_images_serializes() {
        let m = ChatMessage {
            role: "user".into(),
            content: "이미지".into(),
            images: Some(vec!["data:image/jpeg;base64,AAAA".into()]),
        };
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["images"][0], "data:image/jpeg;base64,AAAA");
    }

    #[test]
    fn chat_message_legacy_without_images_field_deserializes() {
        // 기존 (Phase 13'.h 이전) 와이어 — images 필드 없음.
        let json = r#"{"role":"user","content":"hi"}"#;
        let m: ChatMessage = serde_json::from_str(json).unwrap();
        assert!(m.images.is_none());
        assert_eq!(m.role, "user");
    }

    #[test]
    fn chat_event_delta_serializes_kebab_case_tag() {
        let e = ChatEvent::Delta {
            text: "안".to_string(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "delta");
        assert_eq!(v["text"], "안");
    }

    #[test]
    fn chat_event_completed_round_trip() {
        let e = ChatEvent::Completed { took_ms: 1234 };
        let json = serde_json::to_string(&e).unwrap();
        let back: ChatEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ChatEvent::Completed { took_ms: 1234 });
    }

    #[test]
    fn chat_event_failed_serializes_message() {
        let e = ChatEvent::Failed {
            message: "에러".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "failed");
        assert_eq!(v["message"], "에러");
    }

    #[test]
    fn chat_event_cancelled_serializes_unit() {
        let e = ChatEvent::Cancelled;
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "cancelled");
    }
}
