//! Intent 사전 — 의도 기반 모델 추천 1차 축. (Phase 11'.a, ADR-0048)
//!
//! 정책:
//! - `IntentId`는 자유 태그(`String`)지만 manifest validator는 본 사전
//!   (`INTENT_VOCABULARY`)에 등록된 ID만 통과시킴. 자유 태그의 유연함과
//!   큐레이션 무결성을 동시에 잡는 게이트.
//! - kebab-case ID. 한국어 라벨은 UI(Catalog IntentBoard 등)에서 fallback.
//! - 카테고리(`ModelCategory`)와 직교. 한 모델이 여러 intent 가능 (N:N).
//! - v1.x 시드 11종. v2 확장 시 사용자 신호(텔레메트리) 누적 후 추가.

/// Intent 식별자 — kebab-case 자유 태그. Validator가 `INTENT_VOCABULARY`로 게이트.
pub type IntentId = String;

/// `(id, 한국어 라벨)` — UI 노출용. v1.x 시드 11종.
pub const INTENT_VOCABULARY: &[(&str, &str)] = &[
    ("vision-image", "이미지 분석"),
    ("vision-multimodal", "이미지+텍스트 멀티모달"),
    ("translation-ko-en", "한↔영 번역"),
    ("translation-multi", "다국어 번역"),
    ("coding-general", "코딩"),
    ("coding-fim", "코드 자동완성 (FIM)"),
    ("agent-tool-use", "에이전트 / 도구 사용"),
    ("roleplay-narrative", "롤플레이 / 서사"),
    ("ko-conversation", "한국어 대화"),
    ("ko-rag", "한국어 RAG"),
    ("voice-stt", "음성 인식"),
];

/// `id`가 사전에 등록됐는지 검사. Manifest validator가 사용.
pub fn is_registered_intent(id: &str) -> bool {
    INTENT_VOCABULARY.iter().any(|(k, _)| *k == id)
}

/// 한국어 라벨 룩업. UI 카피 토큰 fallback. 미등록 ID는 None.
pub fn intent_label_ko(id: &str) -> Option<&'static str> {
    INTENT_VOCABULARY
        .iter()
        .find_map(|(k, label)| (*k == id).then_some(*label))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vocabulary_seed_size_v1x() {
        // 추가/삭제 시 본 테스트가 의도 변경을 가시화.
        assert_eq!(INTENT_VOCABULARY.len(), 11);
    }

    #[test]
    fn vocabulary_ids_are_kebab_case() {
        for (id, _) in INTENT_VOCABULARY {
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "non-kebab-case intent id: {id}"
            );
            assert!(!id.starts_with('-') && !id.ends_with('-'));
        }
    }

    #[test]
    fn vocabulary_has_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for (id, _) in INTENT_VOCABULARY {
            assert!(seen.insert(*id), "duplicate intent id: {id}");
        }
    }

    #[test]
    fn vocabulary_labels_have_hangul() {
        for (id, label) in INTENT_VOCABULARY {
            let has_hangul = label
                .chars()
                .any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c));
            assert!(has_hangul, "intent {id} label '{label}' lacks Hangul");
        }
    }

    #[test]
    fn is_registered_known() {
        assert!(is_registered_intent("vision-image"));
        assert!(is_registered_intent("ko-rag"));
        assert!(is_registered_intent("voice-stt"));
    }

    #[test]
    fn is_registered_unknown() {
        assert!(!is_registered_intent("unknown-intent"));
        assert!(!is_registered_intent(""));
        // case sensitive — kebab-case 강제
        assert!(!is_registered_intent("Vision-Image"));
    }

    #[test]
    fn intent_label_ko_round_trip() {
        assert_eq!(intent_label_ko("vision-image"), Some("이미지 분석"));
        assert_eq!(intent_label_ko("ko-rag"), Some("한국어 RAG"));
        assert_eq!(intent_label_ko("nope"), None);
    }
}
