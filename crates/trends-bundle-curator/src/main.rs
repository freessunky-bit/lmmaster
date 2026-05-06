//! `trends-bundle-curator` — Phase 22'.b prototype (ADR-0060).
//!
//! 흐름:
//! 1. `source::fetch_arxiv` — arXiv RSS (cs.LG/CL/AI/CV) atom feed.
//! 2. `source::fetch_hf_daily_papers` — HuggingFace Daily Papers JSON API.
//! 3. `source::fetch_company_blogs` — OpenAI / TechCrunch / The Verge / VentureBeat / NVIDIA RSS.
//! 4. `bundle::build` — fetched items → trends-bundle.json (kind tagged enum 6종).
//! 5. `report::review_queue_md` — 큐레이터 GitHub Issue 본문 (한국어 해요체).
//! 6. GHA workflow (Phase 22'.d)가 매일 cron으로 본 binary 실행 + JasonEtco/create-an-issue.
//!
//! 본 sub-phase는 **CLI 골격 + struct 정의 + 단위 테스트**까지. 실 fetch는 22'.c.

use anyhow::Result;
use tracing::info;

mod bundle;
mod source;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().init();

    info!("trends-bundle-curator 시작 — Phase 22'.b prototype");
    info!(
        "다음 sub-phase: 22'.c (실 fetch + review queue Issue) → 22'.d (GHA cron) → 22'.e (큐레이터 가이드)"
    );

    // Phase 22'.c에서 채울 흐름:
    //   let arxiv_items = source::fetch_arxiv().await?;
    //   let hf_items = source::fetch_hf_daily_papers().await?;
    //   let blog_items = source::fetch_company_blogs().await?;
    //   let bundle = bundle::build(arxiv_items, hf_items, blog_items);
    //   bundle::write(&bundle, "manifests/apps/trends-bundle.json")?;

    Ok(())
}
