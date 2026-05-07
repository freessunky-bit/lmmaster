//! System / user prompt builder + cache key — Phase 22'.e.1.
//!
//! 정책 (ADR-0060 §6 + reinforcement §7):
//! - System: 한국어 해요체 + "당신은 한국 AI 동향 큐레이터예요" + 1~2문장 + 영어 단어 풀이.
//! - User: 카테고리별 묶음 + 각 item title + summary_ko + source.
//! - cache_key: sha256(system_prompt + items_canonical_json + model_kind) 32 hex.

use sha2::{Digest, Sha256};

use crate::types::{SummaryInput, SummaryKind};

/// 한국어 해요체 system prompt — fixed.
///
/// 정책:
/// - "당신은 ... 큐레이터예요" + 한국어 해요체 + 1~2문장 + 영어 단어 풀이.
/// - 절대 원문 재출판 X — `summary_ko` 기반 *메타 요약*만.
/// - 카테고리 라벨 6종 한국어 (`SummaryKind::label_ko`) 명시.
pub fn build_system_prompt() -> String {
    "당신은 한국 AI 동향을 정리해 사용자에게 전달하는 큐레이터예요.\n\
     입력으로 들어온 트렌드 항목들을 카테고리별로 묶어 \
     *한국어 해요체*로 1~2문장씩 요약해 주세요.\n\n\
     규칙:\n\
     - 카테고리: 논문 / 블로그 / 뉴스 / 영상 / 오픈소스 / SNS 6종.\n\
     - 1~2문장 해요체 (예: \"이번 주는 ~ 논문이 많이 나왔어요\").\n\
     - 영어 단어는 한국어 풀이를 추가해 주세요 (예: \"LLM(대규모 언어모델)\").\n\
     - 원문 재출판 금지 — 입력의 `summary_ko`을 기반으로 *메타 요약*만.\n\
     - 인용 시 출처 매체 이름을 자연스럽게 삽입 (예: \"OpenAI 블로그에 따르면\").\n\
     - 빈 카테고리는 \"이번에는 ~ 소식이 없었어요\"로 짧게.\n\
     - 광고성 표현 금지. 사실 진술 위주.\n"
        .to_string()
}

/// User prompt — 카테고리별 묶음 + 각 item title/summary/source.
///
/// 출력 형식:
/// ```text
/// ## 논문 (3건)
/// - "Title 1" (summary_ko_1) — outlet 1
/// - "Title 2" (summary_ko_2) — outlet 2
/// ...
/// ## 블로그 (2건)
/// ...
/// ```
pub fn build_user_prompt(items: &[SummaryInput]) -> String {
    use std::collections::BTreeMap;
    let mut grouped: BTreeMap<SummaryKind, Vec<&SummaryInput>> = BTreeMap::new();
    for item in items {
        grouped.entry(item.kind).or_default().push(item);
    }

    let mut s = String::new();
    s.push_str(
        "아래 트렌드 항목들을 카테고리별로 묶어 한국어 해요체로 1~2문장씩 요약해 주세요.\n\n",
    );

    for kind in [
        SummaryKind::Paper,
        SummaryKind::Blog,
        SummaryKind::News,
        SummaryKind::Video,
        SummaryKind::Github,
        SummaryKind::Sns,
    ] {
        let bucket = grouped.get(&kind);
        let count = bucket.map(|v| v.len()).unwrap_or(0);
        s.push_str(&format!("## {} ({}건)\n", kind.label_ko(), count));
        if let Some(items) = bucket {
            for it in items {
                s.push_str(&format!(
                    "- \"{}\" ({}) — {}\n",
                    it.title, it.summary_ko, it.source
                ));
            }
        } else {
            s.push_str("(없음)\n");
        }
        s.push('\n');
    }
    s
}

/// 캐시 키 — `(system_prompt + canonical_items + model_kind)`의 sha256 hex 64자.
///
/// 정책:
/// - canonical_items: items를 sorted by id로 정렬해 JSON 직렬화 (deterministic).
/// - model_kind 변경 시 키 자동 변경 (모델 교체하면 cache miss).
/// - system_prompt 변경 시도 마찬가지 (prompt 정책 갱신 시 cache invalidate).
pub fn cache_key(items: &[SummaryInput], model_kind: &str) -> String {
    let mut sorted: Vec<&SummaryInput> = items.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));

    let canonical_items = serde_json::to_string(&sorted).unwrap_or_else(|_| "[]".to_string());
    let system = build_system_prompt();

    let mut hasher = Sha256::new();
    hasher.update(b"v1|");
    hasher.update(system.as_bytes());
    hasher.update(b"|");
    hasher.update(canonical_items.as_bytes());
    hasher.update(b"|");
    hasher.update(model_kind.as_bytes());
    let digest = hasher.finalize();
    hex::encode(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_input(id: &str, kind: SummaryKind, title: &str) -> SummaryInput {
        SummaryInput {
            id: id.into(),
            kind,
            title: title.into(),
            summary_ko: format!("{} 한국어 한 줄 요약", title),
            source: "OpenAI 블로그".into(),
            source_url: "https://example.com".into(),
        }
    }

    #[test]
    fn system_prompt_is_korean_haeyo_che() {
        let p = build_system_prompt();
        assert!(p.contains("당신은"));
        assert!(p.contains("해요체"));
        assert!(p.contains("논문") && p.contains("블로그"));
        // 영어 단어 풀이 명시.
        assert!(p.contains("한국어 풀이"));
        // 원문 재출판 금지 정책 명시.
        assert!(p.contains("원문 재출판 금지"));
    }

    #[test]
    fn user_prompt_groups_all_six_categories() {
        let inputs = vec![
            sample_input("1", SummaryKind::Paper, "Paper Title"),
            sample_input("2", SummaryKind::Blog, "Blog Title"),
        ];
        let p = build_user_prompt(&inputs);
        // 6 카테고리 모두 헤더 등장.
        for label in ["논문", "블로그", "뉴스", "영상", "오픈소스", "SNS"] {
            assert!(p.contains(label), "label {label} missing");
        }
        // 카테고리 카운트.
        assert!(p.contains("논문 (1건)"));
        assert!(p.contains("뉴스 (0건)"));
        // 빈 카테고리 (없음) 표기.
        assert!(p.contains("(없음)"));
    }

    #[test]
    fn user_prompt_includes_title_and_summary() {
        let inputs = vec![sample_input("1", SummaryKind::Paper, "Test Paper")];
        let p = build_user_prompt(&inputs);
        assert!(p.contains("Test Paper"));
        assert!(p.contains("Test Paper 한국어 한 줄 요약"));
        assert!(p.contains("OpenAI 블로그"));
    }

    #[test]
    fn cache_key_is_deterministic_100x() {
        let inputs = vec![
            sample_input("a", SummaryKind::Paper, "A"),
            sample_input("b", SummaryKind::Blog, "B"),
        ];
        let first = cache_key(&inputs, "gemma3:4b");
        for _ in 0..100 {
            assert_eq!(cache_key(&inputs, "gemma3:4b"), first);
        }
    }

    #[test]
    fn cache_key_invariant_under_input_order() {
        // items 순서가 다르더라도 같은 키 (sorted by id 내부 정규화).
        let inputs1 = vec![
            sample_input("a", SummaryKind::Paper, "A"),
            sample_input("b", SummaryKind::Blog, "B"),
        ];
        let inputs2 = vec![
            sample_input("b", SummaryKind::Blog, "B"),
            sample_input("a", SummaryKind::Paper, "A"),
        ];
        assert_eq!(
            cache_key(&inputs1, "gemma3:4b"),
            cache_key(&inputs2, "gemma3:4b")
        );
    }

    #[test]
    fn cache_key_changes_with_model() {
        let inputs = vec![sample_input("a", SummaryKind::Paper, "A")];
        let key_gemma = cache_key(&inputs, "gemma3:4b");
        let key_exaone = cache_key(&inputs, "exaone3.5:7.8b");
        assert_ne!(key_gemma, key_exaone, "모델 변경 시 cache miss 필수");
    }

    #[test]
    fn cache_key_changes_with_items() {
        let inputs1 = vec![sample_input("a", SummaryKind::Paper, "A")];
        let inputs2 = vec![sample_input("b", SummaryKind::Paper, "B")];
        assert_ne!(cache_key(&inputs1, "x"), cache_key(&inputs2, "x"));
    }

    #[test]
    fn cache_key_is_64_hex_chars() {
        let inputs = vec![sample_input("a", SummaryKind::Paper, "A")];
        let key = cache_key(&inputs, "x");
        assert_eq!(key.len(), 64);
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
