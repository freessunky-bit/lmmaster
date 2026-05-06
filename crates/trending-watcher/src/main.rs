//! `trending-watcher` — Phase 21'.b/c/d 통합 (ADR-0059).
//!
//! 흐름:
//! 1. `source::fetch_hf_trending` — HF 익명 GET.
//! 2. `source::fetch_open_llm_leaderboard` — Open LLM Leaderboard 2 datasets-server.
//! 3. `filter::rank_candidates` — deterministic score + sort.
//! 4. `report::render_body` × Queue 후보 → stdout 또는 `--out <path>`로 markdown 파일.
//! 5. GHA workflow가 markdown을 JasonEtco/create-an-issue에 전달 (Phase 21'.d).
//!
//! CLI:
//! - `trending-watcher` — 기본: stdout에 batch summary.
//! - `trending-watcher --out report.md` — 첫 Queue 후보의 issue body를 file로.
//! - `trending-watcher --out summary.md --batch` — batch summary를 file로.
//! - `--dry-run` — 외부 호출 X, 빈 결과 보고 (CI 골격 검증용).

use anyhow::{Context, Result};
use std::path::PathBuf;

mod filter;
mod report;
mod source;

#[derive(Debug, Default)]
struct Args {
    out: Option<PathBuf>,
    batch: bool,
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
            "--batch" => args.batch = true,
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
        "trending-watcher 시작 — out={:?} batch={} dry_run={}",
        args.out,
        args.batch,
        args.dry_run
    );

    let scored = if args.dry_run {
        tracing::warn!("dry-run 모드 — 외부 호출 skip, 빈 결과");
        Vec::new()
    } else {
        let client = source::make_client()?;
        let (hf, lb) = tokio::try_join!(
            source::fetch_hf_trending(&client),
            source::fetch_open_llm_leaderboard(&client),
        )?;
        tracing::info!("HF Trending: {} 모델 / Open LLM: {} 행", hf.len(), lb.len());
        filter::rank_candidates(&hf, &lb)
    };

    // 출력 흐름.
    let body = if args.batch {
        report::render_batch_summary(&scored)
    } else {
        // 첫 Queue 후보만 (가장 높은 score) — 동일 issue 갱신 패턴.
        let target = scored
            .iter()
            .find(|s| s.disposition == filter::Disposition::Queue)
            .or(scored.first());
        match target {
            Some(s) => report::render_body(s),
            None => "# Trending watcher\n\n_정식 큐 후보가 없어요._\n".to_string(),
        }
    };

    if let Some(path) = args.out {
        std::fs::write(&path, &body).with_context(|| format!("file 쓰기 실패: {:?}", path))?;
        tracing::info!("report 파일 저장: {:?}", path);
    } else {
        print!("{body}");
    }

    Ok(())
}
