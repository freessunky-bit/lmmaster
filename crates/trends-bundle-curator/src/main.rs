//! `trends-bundle-curator` — Phase 22'.d (ADR-0060).
//!
//! 흐름:
//! 1. HF Daily Papers fetch (오늘 + 어제 2일치).
//! 2. arXiv RSS fetch (cs.LG/CL/AI/CV).
//! 3. items 합본 → trends-bundle.json 후보.
//! 4. report.md 출력 — GHA workflow가 JasonEtco/create-an-issue로 큐레이터 알림.
//! 5. 큐레이터가 Issue body 검토 → 한국어 요약 작성 → manifests/apps/trends-bundle.json PR.
//!
//! CLI:
//! - `trends-bundle-curator` — 기본: report.md를 stdout.
//! - `trends-bundle-curator --out report.md` — file.
//! - `trends-bundle-curator --dry-run` — 외부 호출 0 (CI 골격 검증).

use anyhow::Result;
use std::path::PathBuf;

mod bundle;
mod source;

#[derive(Debug, Default)]
struct Args {
    out: Option<PathBuf>,
    dry_run: bool,
}

fn parse_args() -> Args {
    let mut args = Args::default();
    let mut iter = std::env::args().skip(1);
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--out" => {
                if let Some(p) = iter.next() {
                    args.out = Some(PathBuf::from(p));
                }
            }
            "--dry-run" => args.dry_run = true,
            _ => {}
        }
    }
    args
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().init();
    let args = parse_args();
    tracing::info!(
        "trends-bundle-curator 시작 — out={:?} dry_run={}",
        args.out,
        args.dry_run
    );

    let items: Vec<source::TrendItem> = if args.dry_run {
        tracing::warn!("dry-run 모드 — 외부 호출 skip, 빈 결과");
        Vec::new()
    } else {
        let client = source::make_client()?;
        let papers = source::fetch_hf_daily_papers(&client, None).await?;
        tracing::info!("HF Daily Papers: {} items", papers.len());
        papers.iter().map(source::hf_paper_to_trend_item).collect()
    };

    // GHA workflow가 JasonEtco/create-an-issue에 전달할 markdown.
    let body = render_review_report(&items);

    if let Some(path) = args.out {
        std::fs::write(&path, &body)?;
        tracing::info!("report.md 저장: {:?}", path);
    } else {
        print!("{body}");
    }

    Ok(())
}

/// GHA Issue body — 큐레이터 review queue 알림.
fn render_review_report(items: &[source::TrendItem]) -> String {
    let mut s = String::new();
    s.push_str(
        "---\ntitle: \"[trends] HF Daily Papers + arXiv 큐레이션 review queue\"\nlabels: auto-curate, trends-bundle, needs-review\nassignees: freessunky-bit\n---\n\n",
    );
    s.push_str("# Trends bundle 큐레이션 review queue\n\n");
    s.push_str(&format!(
        "**총 {} items 발견** (HF Daily Papers).\n\n",
        items.len()
    ));
    if items.is_empty() {
        s.push_str("> 오늘은 HF Daily Papers에 새 paper가 없어요. 내일 cron이 다시 시도해요.\n");
        return s;
    }

    s.push_str("## 큐레이터 검토 흐름\n\n");
    s.push_str("1. 아래 항목 중 *한국어 사용자에게 가치 있는 것*을 5~10개 선별.\n");
    s.push_str("2. 각 item의 영문 summary를 *해요체 1~2문장 한국어*로 번역.\n");
    s.push_str("3. `manifests/apps/trends-bundle.json`에 entries 추가 (kind tagged enum 6종).\n");
    s.push_str(
        "4. PR merge → sign-catalog.yml 자동 minisign → jsdelivr propagate → 사용자 도착.\n\n",
    );

    s.push_str("## Top items (HF Daily Papers, upvote 기준)\n\n");
    let mut sorted: Vec<&source::TrendItem> = items.iter().collect();
    sorted.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for item in sorted.iter().take(20) {
        s.push_str(&format!(
            "- **[{}]({})** — score `{:.2}` · {}\n",
            item.title, item.source_url, item.score, item.attribution
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use source::{TrendItem, TrendKind};

    fn sample_item(score: f64, title: &str) -> TrendItem {
        TrendItem {
            id: format!("hf-paper-{title}"),
            kind: TrendKind::Paper,
            title: title.into(),
            summary_ko: "요약".into(),
            source: "huggingface-daily-papers".into(),
            source_url: format!("https://arxiv.org/abs/{title}"),
            attribution: "Author".into(),
            published_at: "2026-05-08T00:00:00Z".into(),
            tags: vec![],
            score,
        }
    }

    #[test]
    fn render_report_empty_shows_no_results() {
        let body = render_review_report(&[]);
        assert!(body.contains("[trends]"));
        assert!(body.contains("오늘은 HF Daily Papers에 새 paper가 없어요"));
    }

    #[test]
    fn render_report_with_items_sorted_by_score() {
        let items = vec![
            sample_item(0.3, "low"),
            sample_item(0.9, "high"),
            sample_item(0.6, "mid"),
        ];
        let body = render_review_report(&items);
        // sorted desc — high가 mid보다 먼저, mid가 low보다 먼저.
        let pos_high = body.find("high").unwrap();
        let pos_mid = body.find("mid").unwrap();
        let pos_low = body.find("low").unwrap();
        assert!(pos_high < pos_mid);
        assert!(pos_mid < pos_low);
    }

    #[test]
    fn render_report_includes_curator_workflow() {
        let body = render_review_report(&[sample_item(0.5, "x")]);
        assert!(body.contains("큐레이터 검토 흐름"));
        assert!(body.contains("해요체 1~2문장 한국어"));
        assert!(body.contains("manifests/apps/trends-bundle.json"));
        assert!(body.contains("jsdelivr propagate"));
    }

    #[test]
    fn render_report_caps_at_20_items() {
        let items: Vec<TrendItem> = (0..30)
            .map(|i| sample_item(0.5, &format!("p{i}")))
            .collect();
        let body = render_review_report(&items);
        // 30 items 중 top 20만 출력.
        assert!(body.contains("p0"));
        assert!(body.contains("p19"));
        // 21번째는 cut.
        assert!(!body.contains("- **[p20]"));
    }
}
