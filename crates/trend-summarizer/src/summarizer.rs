//! Summarizer trait + MockSummarizer + summarize_bundle — Phase 22'.e.1.
//!
//! 정책 (ADR-0060 §6):
//! - `Summarizer` async trait — 호출자가 ollama / lm-studio adapter inject (.e.3).
//! - `MockSummarizer` — deterministic placeholder (test + first-time fallback).
//! - `summarize_bundle` — items 정규화 + LLM 호출 + 결과 grouping + cache_key 부착.

use async_trait::async_trait;
use std::collections::BTreeMap;

use crate::error::{SummarizerError, SummarizerResult};
use crate::prompt::{build_system_prompt, build_user_prompt, cache_key};
use crate::types::{SummaryInput, SummaryKind, TrendsSummary};

/// 단일 LLM 호출 추상 — 호출자가 ollama / lm-studio adapter로 impl.
///
/// `non_exhaustive` 매크로는 X — 향후 streaming variant 추가 시 wrapper trait 분리 (.e.2).
#[async_trait]
pub trait Summarizer: Send + Sync {
    /// 모델 식별자 — `gemma3:4b` / `exaone3.5:7.8b` 등.
    fn model_kind(&self) -> String;

    /// (system, user) prompt → 한국어 응답 텍스트.
    async fn complete(&self, system: &str, user: &str) -> SummarizerResult<String>;
}

/// Deterministic 더미 — `summary_ko` 한 줄을 카테고리별로 줄바꿈으로 묶어 반환.
///
/// 용도:
/// - 단위 테스트 (LLM 호출 없이 흐름 검증).
/// - 첫 실행 / 모델 미설치 / Tauri 시작 시 즉시 응답 (사용자가 캐시된 자료 보는 동안 백그라운드로 실 LLM).
pub struct MockSummarizer {
    /// 사용자에게 노출되는 식별자. 실 모델 미설치 시 "mock-summary"로 표시.
    pub model_kind: String,
}

impl MockSummarizer {
    pub fn new() -> Self {
        Self {
            model_kind: "mock-summary".to_string(),
        }
    }
}

impl Default for MockSummarizer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Summarizer for MockSummarizer {
    fn model_kind(&self) -> String {
        self.model_kind.clone()
    }

    async fn complete(&self, _system: &str, user: &str) -> SummarizerResult<String> {
        // user prompt에서 헤더(`## 논문 (N건)`)와 bullet들 그대로 echo + "샘플 요약" 한 줄 추가.
        let mut out = String::new();
        for line in user.lines() {
            if line.starts_with("## ") {
                out.push_str(line);
                out.push('\n');
                out.push_str("이번에는 샘플 요약이에요. (실 모델 설치 시 자동으로 갱신돼요.)\n\n");
            }
        }
        if out.is_empty() {
            return Err(SummarizerError::ParseFailed(
                "user prompt에 카테고리 헤더가 없어요".into(),
            ));
        }
        Ok(out)
    }
}

/// `Summarizer`의 응답을 카테고리별 1~2문장으로 분리.
///
/// 정책:
/// - LLM 응답이 `## 논문` `## 블로그` 등 카테고리 헤더로 구분된 markdown이라 가정.
/// - 헤더 다음 줄(또는 빈 줄까지)을 그 카테고리 요약으로 채택.
/// - 잘 구분 안 되면 *전체 응답*을 `paper` 섹션에 통째로 (graceful fallback).
pub fn parse_response(response: &str) -> BTreeMap<SummaryKind, String> {
    let mut sections: BTreeMap<SummaryKind, String> = BTreeMap::new();
    let mut current_kind: Option<SummaryKind> = None;
    let mut buf = String::new();

    let label_to_kind = |label: &str| -> Option<SummaryKind> {
        match label.trim() {
            "논문" => Some(SummaryKind::Paper),
            "블로그" => Some(SummaryKind::Blog),
            "뉴스" => Some(SummaryKind::News),
            "영상" => Some(SummaryKind::Video),
            "오픈소스" => Some(SummaryKind::Github),
            "SNS" => Some(SummaryKind::Sns),
            _ => None,
        }
    };

    for line in response.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            // 이전 섹션 flush.
            if let Some(kind) = current_kind {
                let text = buf.trim().to_string();
                if !text.is_empty() {
                    sections.insert(kind, text);
                }
            }
            buf.clear();
            // 헤더에서 한국어 라벨 추출 — `## 논문 (3건)` → `논문`.
            let label = rest.split_whitespace().next().unwrap_or("");
            current_kind = label_to_kind(label);
        } else if !line.trim().is_empty() {
            buf.push_str(line.trim());
            buf.push(' ');
        }
    }
    // 마지막 섹션 flush.
    if let Some(kind) = current_kind {
        let text = buf.trim().to_string();
        if !text.is_empty() {
            sections.insert(kind, text);
        }
    }

    sections
}

/// 통합 — items + summarizer → TrendsSummary.
///
/// 정책:
/// - items 비면 EmptyInput.
/// - summarizer.complete 실패 시 LlmCallFailed.
/// - 응답 파싱 결과가 빈 BTreeMap이면 ParseFailed.
pub async fn summarize_bundle(
    items: &[SummaryInput],
    summarizer: &dyn Summarizer,
) -> SummarizerResult<TrendsSummary> {
    if items.is_empty() {
        return Err(SummarizerError::EmptyInput);
    }

    let system = build_system_prompt();
    let user = build_user_prompt(items);
    let response = summarizer.complete(&system, &user).await?;
    let sections = parse_response(&response);

    if sections.is_empty() {
        return Err(SummarizerError::ParseFailed(
            "응답에서 카테고리 헤더를 찾지 못했어요".into(),
        ));
    }

    let model_kind = summarizer.model_kind();
    let key = cache_key(items, &model_kind);

    Ok(TrendsSummary {
        schema_version: 1,
        sections,
        model_kind,
        cache_key: key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_input(id: &str, kind: SummaryKind, title: &str) -> SummaryInput {
        SummaryInput {
            id: id.into(),
            kind,
            title: title.into(),
            summary_ko: format!("{} 요약", title),
            source: "TestSource".into(),
            source_url: "https://example.com".into(),
        }
    }

    #[tokio::test]
    async fn mock_summarizer_returns_section_for_each_kind_in_input() {
        let inputs = vec![
            sample_input("1", SummaryKind::Paper, "P1"),
            sample_input("2", SummaryKind::Blog, "B1"),
        ];
        let mock = MockSummarizer::new();
        let summary = summarize_bundle(&inputs, &mock).await.unwrap();
        assert_eq!(summary.schema_version, 1);
        assert_eq!(summary.model_kind, "mock-summary");
        assert!(!summary.cache_key.is_empty());
        // 6 카테고리 모두 user_prompt에 등장 → mock이 각 헤더 echo.
        assert!(summary.sections.contains_key(&SummaryKind::Paper));
        assert!(summary.sections.contains_key(&SummaryKind::Blog));
        assert!(summary.sections.contains_key(&SummaryKind::Sns));
    }

    #[tokio::test]
    async fn empty_input_returns_empty_input_error() {
        let mock = MockSummarizer::new();
        let err = summarize_bundle(&[], &mock).await.unwrap_err();
        assert!(matches!(err, SummarizerError::EmptyInput));
    }

    #[test]
    fn parse_response_extracts_3_sections() {
        let resp = "## 논문 (2건)\n이번 주는 모달 학습 논문이 많이 나왔어요.\n\
                    \n## 블로그 (1건)\nOpenAI 블로그에 따르면 신모델이 출시됐어요.\n\
                    \n## SNS (0건)\n이번에는 SNS 소식이 없었어요.\n";
        let sections = parse_response(resp);
        assert_eq!(sections.len(), 3);
        assert!(sections[&SummaryKind::Paper].contains("모달 학습"));
        assert!(sections[&SummaryKind::Blog].contains("OpenAI"));
        assert!(sections[&SummaryKind::Sns].contains("없었어요"));
    }

    #[test]
    fn parse_response_unknown_header_skipped() {
        let resp = "## 알수없는카테고리 (1건)\n무시될 내용.\n\
                    ## 논문 (1건)\n실제 논문 요약.\n";
        let sections = parse_response(resp);
        assert_eq!(sections.len(), 1);
        assert!(sections[&SummaryKind::Paper].contains("실제 논문"));
    }

    #[test]
    fn summarizer_trait_is_object_safe() {
        // dyn Summarizer로 사용 가능 (Box<dyn Summarizer>) — Tauri command에서 inject 시 필수.
        fn _assert_object_safe(_: &dyn Summarizer) {}
        let mock = MockSummarizer::new();
        _assert_object_safe(&mock);
    }

    #[tokio::test]
    async fn summarize_includes_cache_key_64_hex() {
        let inputs = vec![sample_input("1", SummaryKind::Paper, "P1")];
        let mock = MockSummarizer::new();
        let summary = summarize_bundle(&inputs, &mock).await.unwrap();
        assert_eq!(summary.cache_key.len(), 64);
        assert!(summary.cache_key.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
