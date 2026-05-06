//! Deterministic 필터 매트릭스 — Phase 21'.c에 채워요.
//!
//! 정책 (ADR-0059 §4):
//!
//! ```text
//! score = 0.35·norm(Open_LLM_Avg)
//!       + 0.20·log10(downloads_30d)
//!       + 0.20·korean_signal
//!       + 0.15·license_score
//!       + 0.10·gguf_present
//! ```
//!
//! - License score: apache-2/mit = 1.0, llama3.x-community/gemma = 0.7, exaone/nvidia-open = 0.4, *other = 0.0 (자동 제외)*.
//! - Korean signal: cardData.language=ko 1.0 / 본문 정규식 `(한국어|Korean|한글|EXAONE|HyperCLOVA|HCX)` hit count 0~3 → 0.3·count cap 1.0 / 미언급 0.0.
//! - GGUF present: 같은 author OR `unsloth|bartowski|lmstudio-community|TheBloke|MaziyarPanahi` 미러 hit = 1.0.
//! - 사이즈 게이트: 3B~14B만 정식 큐.
//! - 다운로드 임계: ≥ 1k 1차, ≥ 10k 추천.
//!
//! LLM judge 거부 — 모든 가중치 코드 상수 (deterministic 100회 동일 결과 invariant).

#![allow(dead_code)]

/// 한 후보 모델의 score 산출 결과.
#[derive(Debug, Clone)]
pub struct Scored {
    pub hub_id: String,
    pub score: f64,
    pub reason: String,
}

/// 라이선스 → score 매핑.
pub fn license_score(license: &str) -> f64 {
    match license.to_ascii_lowercase().as_str() {
        "apache-2.0" | "mit" | "bsd-2-clause" | "bsd-3-clause" => 1.0,
        "llama3" | "llama3.1" | "llama3.2" | "llama3.3" | "llama-community" | "gemma" => 0.7,
        "exaone" | "nvidia-open-model" | "exaone-custom" => 0.4,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn license_score_apache_or_mit_full() {
        assert_eq!(license_score("apache-2.0"), 1.0);
        assert_eq!(license_score("MIT"), 1.0);
    }

    #[test]
    fn license_score_llama_community_partial() {
        assert_eq!(license_score("llama3.1"), 0.7);
        assert_eq!(license_score("gemma"), 0.7);
    }

    #[test]
    fn license_score_exaone_lower() {
        assert_eq!(license_score("exaone"), 0.4);
        assert_eq!(license_score("nvidia-open-model"), 0.4);
    }

    #[test]
    fn license_score_unknown_zero_excluded() {
        // ADR-0059 — other license는 자동 제외 신호.
        assert_eq!(license_score("CC-BY-NC-4.0"), 0.0);
        assert_eq!(license_score(""), 0.0);
    }
}
