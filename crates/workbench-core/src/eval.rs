//! Korean QA evals — deterministic substring matching.
//!
//! 정책 (phase-5p-workbench-decision.md §1.6, ADR-0023 §Decision 5):
//! - LLM-as-judge 거부 (비결정적 + 외부 통신 회피).
//! - case-insensitive substring 매칭 (ASCII lowercase로 비교).
//! - baseline 10 case = factuality 4 + instruction-following 3 + tone-korean 3.
//!
//! Phase 5'.c 보강:
//! - `Responder` trait — Validate stage가 한 case마다 호출하는 응답 생성기 인터페이스.
//! - `MockResponder` — baseline 10 case에 대해 deterministic mapping. 테스트 + 초기 통합.
//! - `run_eval_suite` — cancel-aware 평가 오케스트레이터. cancel 시 즉시 중단.
//! - 실 HTTP 런타임 wiring은 Phase 5'.e (`bench-harness::workbench_responder` 어댑터).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

use crate::error::WorkbenchError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvalCase {
    pub id: String,
    pub user: String,
    pub expected_substring: Option<String>,
    pub forbidden_substrings: Vec<String>,
    /// "factuality" / "instruction-following" / "tone-korean".
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvalResult {
    pub case_id: String,
    pub passed: bool,
    pub failure_reason: Option<String>,
    pub model_response: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvalReport {
    pub model_id: String,
    pub passed_count: usize,
    pub total: usize,
    /// category → (passed, total).
    pub by_category: HashMap<String, (usize, usize)>,
    pub cases: Vec<EvalResult>,
}

/// 단일 case 평가 — expected (있으면) 매칭 + forbidden 검출. case-insensitive.
pub fn evaluate_response(case: &EvalCase, response: &str) -> EvalResult {
    let response_lower = response.to_lowercase();

    if let Some(expected) = &case.expected_substring {
        if !response_lower.contains(&expected.to_lowercase()) {
            return EvalResult {
                case_id: case.id.clone(),
                passed: false,
                failure_reason: Some(format!("expected substring '{expected}' 없음")),
                model_response: response.to_string(),
            };
        }
    }

    for forbidden in &case.forbidden_substrings {
        if response_lower.contains(&forbidden.to_lowercase()) {
            return EvalResult {
                case_id: case.id.clone(),
                passed: false,
                failure_reason: Some(format!("forbidden substring '{forbidden}' 포함")),
                model_response: response.to_string(),
            };
        }
    }

    EvalResult {
        case_id: case.id.clone(),
        passed: true,
        failure_reason: None,
        model_response: response.to_string(),
    }
}

/// case 메타 없이 카테고리 unknown으로 묶음 — placeholder.
/// 실제 카테고리 집계는 `aggregate_with_cases` 사용.
pub fn aggregate(model_id: &str, results: Vec<EvalResult>) -> EvalReport {
    let total = results.len();
    let mut by_category: HashMap<String, (usize, usize)> = HashMap::new();
    let mut passed_count = 0;
    for r in &results {
        let entry = by_category.entry("unknown".to_string()).or_insert((0, 0));
        entry.1 += 1;
        if r.passed {
            entry.0 += 1;
            passed_count += 1;
        }
    }
    EvalReport {
        model_id: model_id.to_string(),
        passed_count,
        total,
        by_category,
        cases: results,
    }
}

/// case 메타와 함께 집계 — case_id로 카테고리 lookup.
pub fn aggregate_with_cases(
    model_id: &str,
    results: Vec<EvalResult>,
    cases: &[EvalCase],
) -> EvalReport {
    let category_lookup: HashMap<&str, &str> = cases
        .iter()
        .map(|c| (c.id.as_str(), c.category.as_str()))
        .collect();
    let mut by_category: HashMap<String, (usize, usize)> = HashMap::new();
    let mut passed_count = 0;
    for r in &results {
        let cat = category_lookup
            .get(r.case_id.as_str())
            .copied()
            .unwrap_or("unknown")
            .to_string();
        let entry = by_category.entry(cat).or_insert((0, 0));
        entry.1 += 1;
        if r.passed {
            entry.0 += 1;
            passed_count += 1;
        }
    }
    let total = results.len();
    EvalReport {
        model_id: model_id.to_string(),
        passed_count,
        total,
        by_category,
        cases: results,
    }
}

/// baseline 10 case — 한국어 factuality / instruction-following / tone-korean 3 카테고리 커버.
pub fn baseline_korean_eval_cases() -> Vec<EvalCase> {
    vec![
        // ── factuality 4건 ──────────────────────────────────────────
        EvalCase {
            id: "fact-capital".into(),
            user: "한국의 수도는?".into(),
            expected_substring: Some("서울".into()),
            forbidden_substrings: vec![],
            category: "factuality".into(),
        },
        EvalCase {
            id: "fact-hangul".into(),
            user: "세종대왕이 만든 글자는?".into(),
            expected_substring: Some("한글".into()),
            forbidden_substrings: vec![],
            category: "factuality".into(),
        },
        EvalCase {
            id: "fact-last-king".into(),
            user: "조선왕조의 마지막 왕은?".into(),
            expected_substring: Some("순종".into()),
            forbidden_substrings: vec![],
            category: "factuality".into(),
        },
        EvalCase {
            id: "fact-1950".into(),
            user: "1950년에 한국에서 일어난 큰 전쟁의 이름은?".into(),
            expected_substring: Some("한국 전쟁".into()),
            forbidden_substrings: vec![],
            category: "factuality".into(),
        },
        // ── instruction-following 3건 ───────────────────────────────
        EvalCase {
            id: "inst-haeyo".into(),
            user: "한국어 해요체로 자기 소개를 두 문장으로 해 주세요.".into(),
            expected_substring: Some("요".into()),
            forbidden_substrings: vec!["입니다".into(), "합니다".into()],
            category: "instruction-following".into(),
        },
        EvalCase {
            id: "inst-numbers-only".into(),
            user: "다음에서 숫자만 한국어로 적어 주세요: apple 3, banana 5, cherry 7. 답: ".into(),
            expected_substring: None,
            forbidden_substrings: vec!["apple".into(), "banana".into(), "cherry".into()],
            category: "instruction-following".into(),
        },
        EvalCase {
            id: "inst-translate-ko-only".into(),
            user: "다음을 한국어로만 번역해 주세요. 'Hello world.'".into(),
            expected_substring: None,
            forbidden_substrings: vec!["hello".into(), "world".into()],
            category: "instruction-following".into(),
        },
        // ── tone-korean 3건 ─────────────────────────────────────────
        EvalCase {
            id: "tone-no-question-pohoming".into(),
            user: "내가 어떤 모델을 골라야 좋을까요? 추천해 주세요.".into(),
            expected_substring: None,
            forbidden_substrings: vec!["좋으세요".into(), "어떠세요".into()],
            category: "tone-korean".into(),
        },
        EvalCase {
            id: "tone-no-formal".into(),
            user: "지금 시각을 알려 주세요.".into(),
            expected_substring: None,
            forbidden_substrings: vec!["하시겠습니까".into(), "확인하십시오".into()],
            category: "tone-korean".into(),
        },
        EvalCase {
            id: "tone-no-bank-english".into(),
            user: "로딩 중이라고 한국어로 표현해 주세요.".into(),
            expected_substring: None,
            forbidden_substrings: vec!["loading".into(), "click".into()],
            category: "tone-korean".into(),
        },
    ]
}

// ───────────────────────────────────────────────────────────────────
// Responder trait + MockResponder — Phase 5'.c
// ───────────────────────────────────────────────────────────────────

/// Validate stage가 한 EvalCase마다 호출하는 모델 응답 생성기.
///
/// 실 구현은 Phase 5'.e에서 bench-harness 어댑터(Ollama/LM Studio)로 wire.
/// 본 trait는 v1에서는 `MockResponder`로 deterministic 응답을 만들어 베이스라인이 통과하도록 함.
#[async_trait]
pub trait Responder: Send + Sync {
    /// 한 prompt에 대한 모델 응답을 한국어로 반환.
    /// cancel 발동 시 즉시 `WorkbenchError::Cancelled` 반환.
    async fn respond(&self, prompt: &str) -> Result<String, WorkbenchError>;
}

/// 한국어 baseline 10 case에 대해 expected substring을 그대로 포함하고 forbidden은
/// 회피하도록 deterministic mapping을 갖는 mock responder.
///
/// 알 수 없는 prompt는 한국어 일반 응답("네, 도와드릴게요.")을 반환.
/// Phase 5'.e 이후로도 테스트 픽스처 + UI 데모용으로 보존.
#[derive(Debug, Default, Clone)]
pub struct MockResponder;

impl MockResponder {
    pub fn new() -> Self {
        Self
    }

    /// prompt → deterministic 한국어 응답. case 매칭은 prompt 전체 substring으로 수행.
    fn map_prompt(prompt: &str) -> String {
        // baseline_korean_eval_cases의 user prompt와 1:1 매핑.
        // 매핑 키 = case의 `user` 필드 substring (안정 매칭).
        if prompt.contains("한국의 수도") {
            "한국의 수도는 서울이에요.".into()
        } else if prompt.contains("세종대왕") {
            "세종대왕이 만든 글자는 한글이에요.".into()
        } else if prompt.contains("조선왕조의 마지막 왕") {
            "조선의 마지막 왕은 순종이에요.".into()
        } else if prompt.contains("1950년") {
            "1950년에 일어난 큰 전쟁은 한국 전쟁이에요.".into()
        } else if prompt.contains("해요체로 자기 소개") {
            "안녕하세요. 저는 한국어 도우미예요. 잘 도와드릴게요.".into()
        } else if prompt.contains("숫자만 한국어로") {
            "삼, 오, 칠이에요.".into()
        } else if prompt.contains("Hello world") || prompt.contains("한국어로만 번역") {
            "안녕 세상.".into()
        } else if prompt.contains("어떤 모델을 골라야") {
            "여러 모델 중 한국어와 잘 맞는 걸 추천해 드릴게요.".into()
        } else if prompt.contains("지금 시각을 알려") {
            "지금 시각을 알려드릴게요.".into()
        } else if prompt.contains("로딩 중") {
            "받고 있어요.".into()
        } else {
            "네, 도와드릴게요.".into()
        }
    }
}

#[async_trait]
impl Responder for MockResponder {
    async fn respond(&self, prompt: &str) -> Result<String, WorkbenchError> {
        Ok(Self::map_prompt(prompt))
    }
}

// ───────────────────────────────────────────────────────────────────
// run_eval_suite — cancel-aware orchestrator
// ───────────────────────────────────────────────────────────────────

/// baseline cases 전체를 responder로 평가 후 EvalReport 반환.
///
/// 동작:
/// 1. 각 case마다 cancel 체크 (cancelled면 `WorkbenchError::Cancelled`).
/// 2. responder.respond(prompt) 호출.
/// 3. evaluate_response로 pass/fail 판정 → results 누적.
/// 4. 마지막에 aggregate_with_cases로 카테고리 집계.
///
/// 빈 cases 리스트도 정상 처리 (passed=0, total=0, by_category 비어있음).
pub async fn run_eval_suite(
    responder: &dyn Responder,
    cases: &[EvalCase],
    cancel: &CancellationToken,
    model_id: &str,
) -> Result<EvalReport, WorkbenchError> {
    let mut results: Vec<EvalResult> = Vec::with_capacity(cases.len());

    for case in cases {
        if cancel.is_cancelled() {
            return Err(WorkbenchError::Cancelled);
        }
        let response = responder.respond(&case.user).await?;
        let result = evaluate_response(case, &response);
        results.push(result);
    }

    Ok(aggregate_with_cases(model_id, results, cases))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_match_passes() {
        let case = EvalCase {
            id: "t1".into(),
            user: "한국의 수도는?".into(),
            expected_substring: Some("서울".into()),
            forbidden_substrings: vec![],
            category: "factuality".into(),
        };
        let result = evaluate_response(&case, "수도는 서울입니다.");
        assert!(result.passed);
        assert!(result.failure_reason.is_none());
    }

    #[test]
    fn expected_missing_fails() {
        let case = EvalCase {
            id: "t2".into(),
            user: "한국의 수도는?".into(),
            expected_substring: Some("서울".into()),
            forbidden_substrings: vec![],
            category: "factuality".into(),
        };
        let result = evaluate_response(&case, "잘 모르겠어요.");
        assert!(!result.passed);
        assert!(result
            .failure_reason
            .as_ref()
            .unwrap()
            .contains("'서울' 없음"));
    }

    #[test]
    fn forbidden_present_fails() {
        let case = EvalCase {
            id: "t3".into(),
            user: "한국어로만 번역".into(),
            expected_substring: None,
            forbidden_substrings: vec!["hello".into()],
            category: "instruction-following".into(),
        };
        let result = evaluate_response(&case, "Hello 안녕하세요");
        assert!(!result.passed);
        assert!(result
            .failure_reason
            .as_ref()
            .unwrap()
            .contains("'hello' 포함"));
    }

    #[test]
    fn case_insensitive_matching() {
        let case = EvalCase {
            id: "t4".into(),
            user: "x".into(),
            expected_substring: Some("Seoul".into()),
            forbidden_substrings: vec![],
            category: "factuality".into(),
        };
        let result = evaluate_response(&case, "the capital is seoul.");
        assert!(result.passed);
    }

    #[test]
    fn forbidden_case_insensitive() {
        let case = EvalCase {
            id: "t5".into(),
            user: "x".into(),
            expected_substring: None,
            forbidden_substrings: vec!["LOADING".into()],
            category: "tone-korean".into(),
        };
        let result = evaluate_response(&case, "loading 중이에요");
        assert!(!result.passed);
    }

    #[test]
    fn no_expected_no_forbidden_passes() {
        let case = EvalCase {
            id: "t6".into(),
            user: "x".into(),
            expected_substring: None,
            forbidden_substrings: vec![],
            category: "tone-korean".into(),
        };
        let result = evaluate_response(&case, "any response");
        assert!(result.passed);
    }

    #[test]
    fn aggregate_with_cases_groups_correctly() {
        let cases = vec![
            EvalCase {
                id: "a".into(),
                user: "x".into(),
                expected_substring: None,
                forbidden_substrings: vec![],
                category: "factuality".into(),
            },
            EvalCase {
                id: "b".into(),
                user: "x".into(),
                expected_substring: None,
                forbidden_substrings: vec![],
                category: "factuality".into(),
            },
            EvalCase {
                id: "c".into(),
                user: "x".into(),
                expected_substring: None,
                forbidden_substrings: vec![],
                category: "tone-korean".into(),
            },
        ];
        let results = vec![
            EvalResult {
                case_id: "a".into(),
                passed: true,
                failure_reason: None,
                model_response: "ok".into(),
            },
            EvalResult {
                case_id: "b".into(),
                passed: false,
                failure_reason: Some("x".into()),
                model_response: "no".into(),
            },
            EvalResult {
                case_id: "c".into(),
                passed: true,
                failure_reason: None,
                model_response: "ok".into(),
            },
        ];
        let report = aggregate_with_cases("test-model", results, &cases);
        assert_eq!(report.passed_count, 2);
        assert_eq!(report.total, 3);
        let fact = report.by_category.get("factuality").unwrap();
        assert_eq!(*fact, (1, 2));
        let tone = report.by_category.get("tone-korean").unwrap();
        assert_eq!(*tone, (1, 1));
    }

    #[test]
    fn aggregate_no_case_meta_uses_unknown() {
        let results = vec![EvalResult {
            case_id: "a".into(),
            passed: true,
            failure_reason: None,
            model_response: "ok".into(),
        }];
        let report = aggregate("model", results);
        assert_eq!(report.passed_count, 1);
        assert_eq!(report.total, 1);
        assert!(report.by_category.contains_key("unknown"));
    }

    #[test]
    fn baseline_10_cases_3_categories() {
        let cases = baseline_korean_eval_cases();
        assert_eq!(cases.len(), 10);
        let categories: std::collections::HashSet<_> =
            cases.iter().map(|c| c.category.as_str()).collect();
        assert_eq!(categories.len(), 3);
        assert!(categories.contains("factuality"));
        assert!(categories.contains("instruction-following"));
        assert!(categories.contains("tone-korean"));
    }

    #[test]
    fn baseline_factuality_count_4() {
        let cases = baseline_korean_eval_cases();
        let fact_count = cases.iter().filter(|c| c.category == "factuality").count();
        assert_eq!(fact_count, 4);
    }

    #[test]
    fn baseline_instruction_following_count_3() {
        let cases = baseline_korean_eval_cases();
        let inst_count = cases
            .iter()
            .filter(|c| c.category == "instruction-following")
            .count();
        assert_eq!(inst_count, 3);
    }

    #[test]
    fn baseline_tone_korean_count_3() {
        let cases = baseline_korean_eval_cases();
        let tone_count = cases.iter().filter(|c| c.category == "tone-korean").count();
        assert_eq!(tone_count, 3);
    }

    #[test]
    fn baseline_case_ids_unique() {
        let cases = baseline_korean_eval_cases();
        let ids: std::collections::HashSet<_> = cases.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids.len(), cases.len());
    }

    // ── Responder + run_eval_suite tests (Phase 5'.c) ───────────────

    #[tokio::test]
    async fn mock_responder_respond_returns_korean_for_capital() {
        let r = MockResponder::new();
        let resp = r.respond("한국의 수도는?").await.unwrap();
        assert!(resp.contains("서울"));
    }

    #[tokio::test]
    async fn mock_responder_unknown_prompt_returns_default_korean() {
        let r = MockResponder::new();
        let resp = r.respond("완전히 처음 보는 질문이에요").await.unwrap();
        assert!(resp.contains("도와드릴게요"));
        // 안전장치 — 한글이 포함되어 있어야 함.
        assert!(resp
            .chars()
            .any(|c| (0xAC00..=0xD7A3).contains(&(c as u32))));
    }

    #[tokio::test]
    async fn mock_responder_passes_all_baseline_cases() {
        let r = MockResponder::new();
        for case in baseline_korean_eval_cases() {
            let resp = r.respond(&case.user).await.unwrap();
            let result = evaluate_response(&case, &resp);
            assert!(
                result.passed,
                "case {} 통과해야 함 — 응답='{}', 사유={:?}",
                case.id, resp, result.failure_reason
            );
        }
    }

    #[tokio::test]
    async fn run_eval_suite_baseline_all_pass_with_mock() {
        let r = MockResponder::new();
        let cases = baseline_korean_eval_cases();
        let cancel = CancellationToken::new();
        let report = run_eval_suite(&r, &cases, &cancel, "qwen-test")
            .await
            .unwrap();
        assert_eq!(report.total, 10);
        assert_eq!(report.passed_count, 10);
        assert_eq!(report.model_id, "qwen-test");
        assert_eq!(report.cases.len(), 10);
    }

    #[tokio::test]
    async fn run_eval_suite_categories_three() {
        let r = MockResponder::new();
        let cases = baseline_korean_eval_cases();
        let cancel = CancellationToken::new();
        let report = run_eval_suite(&r, &cases, &cancel, "m").await.unwrap();
        assert_eq!(report.by_category.len(), 3);
        assert!(report.by_category.contains_key("factuality"));
        assert!(report.by_category.contains_key("instruction-following"));
        assert!(report.by_category.contains_key("tone-korean"));
    }

    #[tokio::test]
    async fn run_eval_suite_pre_cancelled_returns_cancelled_err() {
        let r = MockResponder::new();
        let cases = baseline_korean_eval_cases();
        let cancel = CancellationToken::new();
        cancel.cancel();
        let err = run_eval_suite(&r, &cases, &cancel, "m").await.unwrap_err();
        assert!(matches!(err, WorkbenchError::Cancelled));
    }

    #[tokio::test]
    async fn run_eval_suite_empty_cases_returns_empty_report() {
        let r = MockResponder::new();
        let cancel = CancellationToken::new();
        let report = run_eval_suite(&r, &[], &cancel, "model-x").await.unwrap();
        assert_eq!(report.total, 0);
        assert_eq!(report.passed_count, 0);
        assert!(report.by_category.is_empty());
        assert_eq!(report.model_id, "model-x");
    }

    /// 응답을 항상 빈 문자열로 반환하는 responder — 모든 case 실패 시뮬레이션.
    struct EmptyResponder;

    #[async_trait]
    impl Responder for EmptyResponder {
        async fn respond(&self, _prompt: &str) -> Result<String, WorkbenchError> {
            Ok(String::new())
        }
    }

    #[tokio::test]
    async fn run_eval_suite_failing_responder_marks_cases_failed() {
        let r = EmptyResponder;
        let cases = baseline_korean_eval_cases();
        let cancel = CancellationToken::new();
        let report = run_eval_suite(&r, &cases, &cancel, "m").await.unwrap();
        // expected_substring 있는 case는 모두 실패해야 함 (factuality 4 + inst-haeyo 1 = 5).
        let has_expected_count = cases
            .iter()
            .filter(|c| c.expected_substring.is_some())
            .count();
        let failed_count = report.total - report.passed_count;
        assert!(failed_count >= has_expected_count);
    }

    /// 첫 호출 후 cancel을 트리거하는 responder.
    struct CancellingResponder {
        token: CancellationToken,
    }

    #[async_trait]
    impl Responder for CancellingResponder {
        async fn respond(&self, _prompt: &str) -> Result<String, WorkbenchError> {
            self.token.cancel();
            Ok("응답이에요".into())
        }
    }

    #[tokio::test]
    async fn run_eval_suite_cancel_mid_run_returns_cancelled() {
        let cancel = CancellationToken::new();
        let r = CancellingResponder {
            token: cancel.clone(),
        };
        let cases = baseline_korean_eval_cases();
        // 첫 case 후 cancel 발동 → 두 번째 iteration에서 Cancelled.
        let err = run_eval_suite(&r, &cases, &cancel, "m").await.unwrap_err();
        assert!(matches!(err, WorkbenchError::Cancelled));
    }

    /// 항상 IO 에러를 반환하는 responder.
    struct FailingResponder;

    #[async_trait]
    impl Responder for FailingResponder {
        async fn respond(&self, _prompt: &str) -> Result<String, WorkbenchError> {
            Err(WorkbenchError::Internal {
                message: "런타임 죽음".into(),
            })
        }
    }

    #[tokio::test]
    async fn run_eval_suite_responder_error_propagates() {
        let r = FailingResponder;
        let cases = baseline_korean_eval_cases();
        let cancel = CancellationToken::new();
        let err = run_eval_suite(&r, &cases, &cancel, "m").await.unwrap_err();
        assert!(matches!(err, WorkbenchError::Internal { .. }));
    }

    #[tokio::test]
    async fn run_eval_suite_results_in_order_of_cases() {
        let r = MockResponder::new();
        let cases = baseline_korean_eval_cases();
        let cancel = CancellationToken::new();
        let report = run_eval_suite(&r, &cases, &cancel, "m").await.unwrap();
        // case 순서가 그대로 result 순서로 보존돼야 함.
        for (i, c) in cases.iter().enumerate() {
            assert_eq!(report.cases[i].case_id, c.id);
        }
    }

    #[tokio::test]
    async fn run_eval_suite_model_id_preserved() {
        let r = MockResponder::new();
        let cases = baseline_korean_eval_cases();
        let cancel = CancellationToken::new();
        let report = run_eval_suite(&r, &cases, &cancel, "exaone-3.5-7.8b")
            .await
            .unwrap();
        assert_eq!(report.model_id, "exaone-3.5-7.8b");
    }
}
