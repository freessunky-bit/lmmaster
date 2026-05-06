//! `trending-watcher` — Phase 21'.a prototype (ADR-0059).
//!
//! 정책 (ADR-0059 §1~7):
//! - 데이터 소스: HF Trending API + Open LLM Leaderboard 2 dataset + Arena 미러 + KMMLU 정규식.
//! - Deterministic 가중치: 0.35·Open_LLM + 0.20·log10(downloads) + 0.20·korean_signal + 0.15·license + 0.10·gguf_present. LLM judge 0.
//! - 큐레이터 알림: report.md → JasonEtco/create-an-issue dedupe (GHA에서 호출).
//! - 외부 통신 화이트리스트: `huggingface.co` + `github.com` (ADR-0026 정합).
//!
//! 본 prototype은 **본 repo 안 (`crates/trending-watcher/`)** — v1.x 검증 단계. 검증 후 v2.x에 별도 repo
//! `lmmaster-trending-watcher` (public, MIT)로 분리 예정.
//!
//! 현재 단계 (v0.0.3+): main 함수 + 모듈 골격만. 실 fetch / filter / report.md 출력은 Phase 21'.b~e에 채워요.

use anyhow::Result;
use tracing::info;

mod filter;
mod report;
mod source;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().init();

    info!("trending-watcher prototype 시작 — Phase 21'.a (코드 골격)");
    info!(
        "다음 sub-phase: 21'.b (HF + Open LLM fetcher) → 21'.c (deterministic 필터) → 21'.d (Issue dedupe) → 21'.e (CURATION_GUIDE 통합)"
    );

    // Phase 21'.b~e에서 채워질 흐름 placeholder:
    //   let candidates = source::fetch_all().await?;
    //   let scored = filter::score(candidates);
    //   let report = report::render(&scored);
    //   println!("{report}");

    Ok(())
}
