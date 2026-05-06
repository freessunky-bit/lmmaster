//! 미성년자 보호 — Phase 23'.b (ADR-0062 §2).
//!
//! 정책:
//! - deterministic 키워드 리스트 (LLM judge 거부, ADR-0048 정신).
//! - 영문 / 일본어 / 한국어 키워드 매트릭스.
//! - case-insensitive 매칭.
//! - 큐레이터 등록 시점에 자동 scan — hit 1건이라도 PR 자동 거부.

/// 미성년 키워드 하드 거부 리스트.
///
/// 출처:
/// - HuggingFace Content Policy (CSAM 금지)
/// - OpenRAIL-M license clause ("exploit, harm or attempting to exploit or harm minors")
/// - 한국 청소년보호법 §4
/// - PROTECT Act / EU CSAM regulation 일관 키워드.
pub const MINOR_KEYWORDS_REJECT: &[&str] = &[
    // 영문
    "loli",
    "lolicon",
    "shota",
    "shotacon",
    "age-regression",
    "ageplay",
    "underage",
    "preteen",
    // 일본어 (가나)
    "ロリ",
    "ショタ",
    // 한국어
    "미성년",
    "아동",
];

/// 데이터셋 메타 (제목 / 설명 / 태그) 또는 본문 sample에 미성년 키워드가 있는지 검사.
///
/// case-insensitive ASCII + 한글/일본어 그대로 매칭.
pub fn dataset_has_minor_keywords(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    MINOR_KEYWORDS_REJECT.iter().any(|k| {
        let needle = k.to_ascii_lowercase();
        // ASCII는 lower 매칭, 비-ASCII (한글/일본어)는 원본 그대로.
        if k.is_ascii() {
            lower.contains(&needle)
        } else {
            text.contains(*k)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_string_no_hit() {
        assert!(!dataset_has_minor_keywords(""));
    }

    #[test]
    fn loli_hit_case_insensitive() {
        assert!(dataset_has_minor_keywords("contains LOLI character"));
        assert!(dataset_has_minor_keywords("loli"));
        assert!(dataset_has_minor_keywords("LolI"));
    }

    #[test]
    fn shota_hit() {
        assert!(dataset_has_minor_keywords("shota arc"));
        assert!(dataset_has_minor_keywords("shotacon stories"));
    }

    #[test]
    fn ageplay_age_regression_hit() {
        assert!(dataset_has_minor_keywords("age-regression theme"));
        assert!(dataset_has_minor_keywords("ageplay scenarios"));
        assert!(dataset_has_minor_keywords("AGE-REGRESSION"));
    }

    #[test]
    fn underage_hit() {
        assert!(dataset_has_minor_keywords("contains underage characters"));
    }

    #[test]
    fn preteen_hit() {
        assert!(dataset_has_minor_keywords("preteen romance"));
    }

    #[test]
    fn japanese_kana_hit() {
        assert!(dataset_has_minor_keywords("ロリ"));
        assert!(dataset_has_minor_keywords("ショタ"));
    }

    #[test]
    fn korean_hit() {
        assert!(dataset_has_minor_keywords("미성년 인물"));
        assert!(dataset_has_minor_keywords("아동 등장"));
    }

    #[test]
    fn safe_text_no_hit() {
        assert!(!dataset_has_minor_keywords(
            "adult roleplay between consenting"
        ));
        assert!(!dataset_has_minor_keywords("Korean conversation dataset"));
        assert!(!dataset_has_minor_keywords("성인 대화 페르소나"));
    }

    #[test]
    fn deterministic_100x() {
        let text = "loli character in romantic scene";
        let first = dataset_has_minor_keywords(text);
        for _ in 0..100 {
            assert_eq!(first, dataset_has_minor_keywords(text));
        }
    }

    #[test]
    fn safe_text_deterministic_100x() {
        let text = "adult Korean RP dataset";
        let first = dataset_has_minor_keywords(text);
        for _ in 0..100 {
            assert_eq!(first, dataset_has_minor_keywords(text));
        }
        assert!(!first);
    }
}
