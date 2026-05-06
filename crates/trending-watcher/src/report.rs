//! report.md 출력 — Phase 21'.d.
//!
//! 정책 (ADR-0059 §5):
//! - 제목 fingerprint: `[trending] <hub_id> (Avg: <score>)` — `hub_id` unique key.
//! - Body: 핵심 메타 + 룰 통과 내역 + manifest PR 체크리스트 (한국어 해요체).
//! - 라벨: `auto-curate` + `trending-watcher` + `needs-review` (workflow 측에서 부여).
//! - Assignee: 큐레이터 1인 (`freessunky-bit`) — workflow 측.
//! - dedupe: JasonEtco/create-an-issue `update_existing: true` + `search_existing: open`.
//!
//! 본 모듈은 *report.md content*만 생성. workflow yaml이 file에 저장 → action에 전달.

use crate::filter::{Disposition, Scored};

const ISSUE_TITLE_PREFIX: &str = "[trending]";

/// Issue 제목 fingerprint — `[trending] <hub_id> (Avg: 0.84)`.
pub fn issue_title(scored: &Scored) -> String {
    format!(
        "{} {} (Avg: {:.2})",
        ISSUE_TITLE_PREFIX, scored.hub_id, scored.score
    )
}

/// report.md body — 한국어 해요체 + manifest PR 체크리스트.
pub fn render_body(scored: &Scored) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "---\ntitle: \"{}\"\nlabels: auto-curate, trending-watcher, needs-review\nassignees: freessunky-bit\n---\n\n",
        issue_title(scored)
    ));

    s.push_str(&format!("# {} 큐레이션 검토 요청\n\n", scored.hub_id));
    s.push_str("## 자동 발견 결과\n\n");
    s.push_str(&format!(
        "- **종합 score**: `{:.3}` (가중치 매트릭스 — ADR-0059 §4)\n",
        scored.score
    ));
    s.push_str(&format!("- **분류**: {:?}\n", scored.disposition));
    s.push_str("\n## 통과/제외 사유\n\n");
    for reason in &scored.reasons {
        s.push_str(&format!("- {reason}\n"));
    }

    if scored.disposition == Disposition::Queue {
        s.push_str("\n## 큐레이터 검토 체크리스트\n\n");
        s.push_str("- [ ] **chat template 한국어 발화 검증** — LMmaster 본 repo의 EXAONE에 동일 prompt 보내 자연스러움 비교.\n");
        s.push_str("- [ ] **라이선스 약관 정밀 확인** — NVIDIA Open Model License / EXAONE Custom 등 footgun 항목 점검.\n");
        s.push_str("- [ ] **GGUF 변종 sha256 검증** — bartowski / unsloth / lmstudio-community 미러 중 하나로 결정.\n");
        s.push_str(
            "- [ ] **Korean signal 실측** — 모델 카드 본문 + r/LocalLLaMA 한국어 후기 확인.\n",
        );
        s.push_str(
            "- [ ] **카탈로그 manifest 작성** — `manifests/snapshot/models/<cat>/<id>.json`.\n",
        );
        s.push_str("- [ ] **build-catalog-bundle.mjs 실행** — `manifests/apps/catalog.json` 갱신 (CLAUDE.md §3 트랩 노트 #9).\n");
        s.push_str("- [ ] **PR 머지 후 jsdelivr propagate** — 사용자 카탈로그에 자동 노출.\n");
    } else if scored.disposition == Disposition::InfoOnly {
        s.push_str("\n> ℹ️ **info-only** — 사이즈 게이트 외이거나 다운로드 임계 미달이에요. 큐레이터 판단으로 강제 등록 가능.\n");
    } else {
        s.push_str("\n> 🚫 **자동 제외** — 라이선스 화이트리스트 외 등 정책 위반. 검토 불필요.\n");
    }

    s.push_str("\n## 정책 출처\n\n");
    s.push_str("- ADR-0059 §4 — Deterministic 가중치 매트릭스.\n");
    s.push_str("- CLAUDE.md §3 — 신규 모델 manifest 추가 흐름 (build-catalog-bundle.mjs 필수).\n");

    s
}

/// 다중 후보 → multi-issue 또는 batch report.
pub fn render_batch_summary(scored_list: &[Scored]) -> String {
    let queue: Vec<&Scored> = scored_list
        .iter()
        .filter(|s| s.disposition == Disposition::Queue)
        .collect();
    let info: Vec<&Scored> = scored_list
        .iter()
        .filter(|s| s.disposition == Disposition::InfoOnly)
        .collect();
    let reject: Vec<&Scored> = scored_list
        .iter()
        .filter(|s| s.disposition == Disposition::Reject)
        .collect();

    let mut s = String::new();
    s.push_str("# Trending watcher 일괄 보고\n\n");
    s.push_str(&format!("- **Queue (정식 검토)**: {}개\n", queue.len()));
    s.push_str(&format!("- **Info-only**: {}개\n", info.len()));
    s.push_str(&format!("- **Reject (자동 제외)**: {}개\n", reject.len()));
    s.push_str("\n## 정식 검토 큐\n\n");
    for sc in queue {
        s.push_str(&format!("- `{}` — score `{:.3}`\n", sc.hub_id, sc.score));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::Disposition;

    fn sample_scored(hub_id: &str, score: f64, dispo: Disposition) -> Scored {
        Scored {
            hub_id: hub_id.into(),
            score,
            reasons: vec!["통과 (테스트)".into()],
            disposition: dispo,
        }
    }

    #[test]
    fn issue_title_format() {
        let s = sample_scored("test/qwen3", 0.842, Disposition::Queue);
        assert_eq!(issue_title(&s), "[trending] test/qwen3 (Avg: 0.84)");
    }

    #[test]
    fn render_body_queue_includes_checklist() {
        let s = sample_scored("test/qwen3", 0.84, Disposition::Queue);
        let body = render_body(&s);
        assert!(body.contains("[ ] **chat template 한국어 발화 검증**"));
        assert!(body.contains("build-catalog-bundle.mjs"));
        assert!(body.contains("auto-curate"));
        assert!(body.contains("freessunky-bit"));
    }

    #[test]
    fn render_body_info_only_no_checklist() {
        let s = sample_scored("test/tiny", 0.4, Disposition::InfoOnly);
        let body = render_body(&s);
        assert!(!body.contains("[ ] **chat template"));
        assert!(body.contains("info-only"));
    }

    #[test]
    fn render_body_reject_minimal() {
        let s = sample_scored("test/cc-bync", 0.3, Disposition::Reject);
        let body = render_body(&s);
        assert!(body.contains("자동 제외"));
        assert!(!body.contains("[ ] **chat template"));
    }

    #[test]
    fn render_batch_summary_counts_categories() {
        let list = vec![
            sample_scored("a/q", 0.8, Disposition::Queue),
            sample_scored("b/q", 0.7, Disposition::Queue),
            sample_scored("c/info", 0.4, Disposition::InfoOnly),
            sample_scored("d/rej", 0.2, Disposition::Reject),
        ];
        let body = render_batch_summary(&list);
        assert!(body.contains("**Queue (정식 검토)**: 2개"));
        assert!(body.contains("**Info-only**: 1개"));
        assert!(body.contains("**Reject (자동 제외)**: 1개"));
    }
}
