//! Deterministic 한국어 요약 템플릿 — LLM 미사용 / 실패 시 fallback.
//!
//! 정책:
//! - 해요체 일관.
//! - 검증 결과(`Vec<CheckResult>`)에서 severity별로 묶어 1~3 문장 요약.
//! - 사용자에게 "AI 요약 실패"를 노출하지 않고 자연스럽게 보임.

use crate::checks::{CheckResult, Severity};

/// CheckResult 리스트 → 자연어 요약 (한국어 해요체).
pub fn render_summary(checks: &[CheckResult]) -> String {
    let total = checks.len();
    let errors: Vec<&CheckResult> = checks
        .iter()
        .filter(|c| c.severity == Severity::Error)
        .collect();
    let warns: Vec<&CheckResult> = checks
        .iter()
        .filter(|c| c.severity == Severity::Warn)
        .collect();

    let mut sentences: Vec<String> = Vec::new();
    sentences.push(format!("{total}개 항목을 점검했어요."));

    if !errors.is_empty() {
        let titles = join_titles(&errors);
        sentences.push(format!(
            "{}개 항목은 꼭 확인해 주세요: {titles}.",
            errors.len()
        ));
    }

    if !warns.is_empty() {
        let titles = join_titles(&warns);
        sentences.push(format!(
            "{}개 항목이 권장사항이에요: {titles}.",
            warns.len()
        ));
    }

    if errors.is_empty() && warns.is_empty() {
        sentences.push("환경이 모두 정상이에요. 마음껏 사용해 주세요.".into());
    } else if errors.is_empty() {
        sentences.push("심각한 문제는 없어요.".into());
    }

    sentences.join(" ")
}

fn join_titles(items: &[&CheckResult]) -> String {
    items
        .iter()
        .map(|c| c.title_ko.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(id: &str, sev: Severity, title: &str) -> CheckResult {
        CheckResult {
            id: id.into(),
            severity: sev,
            title_ko: title.into(),
            detail_ko: "detail".into(),
            recommendation: None,
        }
    }

    #[test]
    fn empty_list_says_all_normal() {
        let s = render_summary(&[]);
        assert!(s.contains("0개"));
        assert!(s.contains("정상"));
    }

    #[test]
    fn errors_appear_first() {
        let checks = vec![
            check("a", Severity::Warn, "RAM 낮음"),
            check("b", Severity::Error, "WebView2 없음"),
        ];
        let s = render_summary(&checks);
        assert!(s.contains("WebView2"));
        assert!(s.contains("꼭 확인"));
    }

    #[test]
    fn warns_only_no_errors_text() {
        let checks = vec![check("a", Severity::Warn, "디스크 낮음")];
        let s = render_summary(&checks);
        assert!(s.contains("권장사항"));
        assert!(!s.contains("꼭 확인"));
    }

    #[test]
    fn info_only_says_no_serious() {
        let checks = vec![check("a", Severity::Info, "정상")];
        let s = render_summary(&checks);
        // errors=0 + warns=0이라 "환경이 모두 정상이에요" 분기.
        assert!(s.contains("정상이에요"));
    }
}
