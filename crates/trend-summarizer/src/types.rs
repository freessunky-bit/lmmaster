//! Domain types — Phase 22'.e.1.
//!
//! `trends-bundle-curator::TrendKind` / `TrendItem`과 *호환되는 minimal mirror*.
//! coupling 회피 — production은 `serde_json::from_value::<SummaryInput>()`로 변환 후 호출.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 카테고리 — `trends-bundle-curator::TrendKind` 6종과 1:1 매핑 (kebab-case serde).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum SummaryKind {
    Paper,
    Blog,
    News,
    Video,
    Github,
    Sns,
}

impl SummaryKind {
    /// 한국어 라벨 (system prompt + UI에 사용).
    pub fn label_ko(self) -> &'static str {
        match self {
            Self::Paper => "논문",
            Self::Blog => "블로그",
            Self::News => "뉴스",
            Self::Video => "영상",
            Self::Github => "오픈소스",
            Self::Sns => "SNS",
        }
    }
}

/// Summarizer 입력 단위 — `TrendItem`의 minimal subset.
///
/// 정책:
/// - `summary_ko`는 *큐레이터 작성 한국어 한 줄 요약* (≤ 200자, fair use 정합).
/// - LLM은 이 단위 텍스트들을 *카테고리별로 묶어 1~2문장 메타 요약*만 생성.
/// - 원문 재출판 금지 — `summary_ko` + `title` + `source_url`만 prompt로.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SummaryInput {
    pub id: String,
    pub kind: SummaryKind,
    pub title: String,
    pub summary_ko: String,
    pub source: String,
    pub source_url: String,
}

/// LLM 호출 결과 — 카테고리별 1~2문장 한국어 요약.
///
/// `BTreeMap`을 사용해 직렬화 시 키 순서가 결정적 (deterministic 캐시).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrendsSummary {
    pub schema_version: u32,
    /// 카테고리 → "1~2문장 해요체 한국어".
    pub sections: BTreeMap<SummaryKind, String>,
    /// 사용한 모델 식별자 (예: "gemma3:4b", "exaone3.5:7.8b").
    pub model_kind: String,
    /// `(bundle_hash + items_hash + system_prompt_hash + model_kind)` 32 hex.
    pub cache_key: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_kind_korean_labels() {
        assert_eq!(SummaryKind::Paper.label_ko(), "논문");
        assert_eq!(SummaryKind::Blog.label_ko(), "블로그");
        assert_eq!(SummaryKind::News.label_ko(), "뉴스");
        assert_eq!(SummaryKind::Video.label_ko(), "영상");
        assert_eq!(SummaryKind::Github.label_ko(), "오픈소스");
        assert_eq!(SummaryKind::Sns.label_ko(), "SNS");
    }

    #[test]
    fn summary_kind_kebab_round_trip() {
        for (kind, expected) in [
            (SummaryKind::Paper, "paper"),
            (SummaryKind::Blog, "blog"),
            (SummaryKind::News, "news"),
            (SummaryKind::Video, "video"),
            (SummaryKind::Github, "github"),
            (SummaryKind::Sns, "sns"),
        ] {
            let v = serde_json::to_value(kind).unwrap();
            assert_eq!(v.as_str(), Some(expected));
            let parsed: SummaryKind = serde_json::from_value(v).unwrap();
            assert_eq!(parsed, kind);
        }
    }

    #[test]
    fn summary_kind_ord_is_stable() {
        let mut kinds = [
            SummaryKind::Sns,
            SummaryKind::Paper,
            SummaryKind::Github,
            SummaryKind::Blog,
        ];
        kinds.sort();
        // Paper < Blog < News < Video < Github < Sns (declaration order).
        assert_eq!(kinds[0], SummaryKind::Paper);
        assert_eq!(kinds[1], SummaryKind::Blog);
        assert_eq!(kinds[2], SummaryKind::Github);
        assert_eq!(kinds[3], SummaryKind::Sns);
    }
}
