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
        /// v0.8.4 추가 — 잘림(`Length`) / 정상 종료(`Stop`) / 메타(`Meta`) / 끊김(`Aborted`) 구분.
        /// 백워드 호환: 누락 시 `Unknown`.
        #[serde(default = "default_finish_reason")]
        finish_reason: FinishReason,
    },
    Cancelled,
    Failed {
        message: String,
    },
}

fn default_finish_reason() -> FinishReason {
    FinishReason::Unknown
}

/// 응답 종료 원인 — 토큰 한계 도달 감지(`Length`)와 정상 종료(`Stop`) 등을 구분.
///
/// v0.8.4 추가. Ollama `done_reason` / llama.cpp `finish_reason` 또는 `stop_type`을 통합 매핑.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FinishReason {
    /// 정상 종료 — EOS / stop word / 자연 마침.
    Stop,
    /// 토큰 한계 도달 — `num_predict` / `max_tokens` 임계 도달. UI 안내 트리거.
    Length,
    /// 사용자 cancel 또는 외부 abort (graceful early disconnect 등). UI는 "잘렸어요" 카피 X.
    Aborted,
    /// 모델 로드/언로드 등 메타 이벤트 (Ollama "load"/"unload"). 일반 응답 아님.
    Meta,
    /// 알 수 없음 — 필드 누락 / 신규 값. 안전 디폴트로 `Stop`과 동일 처리.
    Unknown,
}

/// Chat 호출 결과 — 어댑터 함수 반환 + IPC outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatOutcome {
    Completed,
    Cancelled,
    Failed(String),
}

/// 사용자 조절 가능한 추론 파라미터 — v0.8.4.
///
/// 정책:
/// - 모든 필드 `Option` — `None`은 어댑터/모델 디폴트 사용.
/// - 어댑터별 wire 매핑은 각 어댑터 내부 (Ollama options vs OpenAI request fields).
/// - `serde(skip_serializing_if = "Option::is_none")`로 백워드 호환 보존.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SamplingParams {
    /// 최대 응답 토큰 수. None이면 모델/서버 디폴트 (Ollama -1, llama.cpp -1 = unlimited).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub max_tokens: Option<u32>,
    /// 0.0 (deterministic) ~ 1.5+ (creative). None이면 어댑터 디폴트 (대개 0.7~0.8).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub temperature: Option<f32>,
    /// nucleus sampling. 0.0~1.0.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub top_p: Option<f32>,
    /// 1.0이면 페널티 없음. 1.1~1.3이 일반적.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub repeat_penalty: Option<f32>,
    /// 재현성용. None이면 서버 랜덤 (Ollama -1, llama.cpp -1).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub seed: Option<i64>,
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
        let e = ChatEvent::Completed {
            took_ms: 1234,
            finish_reason: FinishReason::Stop,
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: ChatEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back,
            ChatEvent::Completed {
                took_ms: 1234,
                finish_reason: FinishReason::Stop,
            }
        );
    }

    // v0.8.4 — finish_reason / SamplingParams 테스트.

    #[test]
    fn finish_reason_round_trip_kebab() {
        for r in [
            FinishReason::Stop,
            FinishReason::Length,
            FinishReason::Aborted,
            FinishReason::Meta,
            FinishReason::Unknown,
        ] {
            let json = serde_json::to_string(&r).unwrap();
            let back: FinishReason = serde_json::from_str(&json).unwrap();
            assert_eq!(back, r);
            // kebab-case: "length", "aborted" 등 lowercase.
            assert!(json.chars().filter(|c| *c != '"').all(|c| !c.is_uppercase()));
        }
    }

    #[test]
    fn legacy_completed_without_finish_reason_deserializes_unknown() {
        // v0.8.3 와이어 — finish_reason 필드 없음.
        let json = r#"{"kind":"completed","took_ms":1234}"#;
        let e: ChatEvent = serde_json::from_str(json).unwrap();
        assert_eq!(
            e,
            ChatEvent::Completed {
                took_ms: 1234,
                finish_reason: FinishReason::Unknown,
            }
        );
    }

    #[test]
    fn chat_event_completed_with_length_serializes_kebab() {
        let e = ChatEvent::Completed {
            took_ms: 500,
            finish_reason: FinishReason::Length,
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["finish_reason"], "length");
    }

    #[test]
    fn sampling_params_default_serializes_empty_object() {
        // 모든 필드 None → JSON `{}` (skip 적용).
        let p = SamplingParams::default();
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v, serde_json::json!({}), "모든 필드 None → 빈 객체");
    }

    #[test]
    fn sampling_params_partial_serialize_skips_none() {
        let p = SamplingParams {
            max_tokens: Some(512),
            temperature: Some(0.7),
            top_p: None,
            repeat_penalty: None,
            seed: None,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["max_tokens"], 512);
        assert!((v["temperature"].as_f64().unwrap() - 0.7).abs() < 0.001);
        assert!(v.get("top_p").is_none());
        assert!(v.get("repeat_penalty").is_none());
        assert!(v.get("seed").is_none());
    }

    #[test]
    fn sampling_params_round_trip_full() {
        let p = SamplingParams {
            max_tokens: Some(2048),
            temperature: Some(0.5),
            top_p: Some(0.95),
            repeat_penalty: Some(1.1),
            seed: Some(42),
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: SamplingParams = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
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
