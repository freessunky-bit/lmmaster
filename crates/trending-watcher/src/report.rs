//! report.md 출력 골격 — Phase 21'.d에 채워요.
//!
//! 정책 (ADR-0059 §5):
//! - 제목 fingerprint: `[trending] <hub_id> (Avg: <score>)` — `hub_id` unique key.
//! - Body: 핵심 메타 + 룰 통과 내역 + manifest PR 체크리스트 (한국어 해요체).
//! - 라벨: `auto-curate` + `trending-watcher` + `needs-review`.
//! - Assignee: 큐레이터 1인 (`freessunky-bit`).
//! - dedupe: JasonEtco/create-an-issue `update_existing: true` + `search_existing: open`.

#![allow(dead_code)]

/// report.md body 렌더 — 사용자가 issue 본문에 그대로 게시 (한국어).
pub fn render_body(_hub_id: &str, _score: f64) -> String {
    // Phase 21'.d에서 채워요. 현재는 placeholder.
    String::from("# Trending watcher report\n\n_Phase 21'.d에서 채워질 예정이에요._\n")
}
