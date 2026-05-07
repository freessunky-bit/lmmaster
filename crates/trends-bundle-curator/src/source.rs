//! 데이터 소스 fetcher — Phase 22'.d (ADR-0060 §2).
//!
//! 정책:
//! - HF Daily Papers: `huggingface.co/api/daily_papers?date=YYYY-MM-DD&limit=100` (anonymous).
//! - arXiv RSS: `https://rss.arxiv.org/atom/cs.LG+cs.CL+cs.AI+cs.CV` (User-Agent 필수, 3 req/sec).
//! - 외부 통신 화이트리스트: `huggingface.co` + `arxiv.org` (큐레이터 GHA runner 측만, 사용자 PC 무관).
//! - User-Agent: `lmmaster-trends-bundle-curator/<ver>`.
//! - rate limit 회피: 매일 cron이라 호출 ≤5회/일, 안전 마진 충분.

#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// trends-bundle 1 item (kind tagged enum 6종).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrendItem {
    pub id: String,
    pub kind: TrendKind,
    pub title: String,
    pub summary_ko: String,
    pub source: String,
    pub source_url: String,
    pub attribution: String,
    pub published_at: String,
    pub tags: Vec<String>,
    pub score: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TrendKind {
    Paper,
    Blog,
    News,
    Video,
    Github,
    Sns,
}

/// HF Daily Papers API 응답 1 item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HfDailyPaper {
    #[serde(default)]
    pub paper: HfPaperMeta,
    #[serde(default, rename = "publishedAt")]
    pub published_at: Option<String>,
    #[serde(default)]
    pub upvotes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HfPaperMeta {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub authors: Vec<HfAuthor>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HfAuthor {
    #[serde(default)]
    pub name: String,
}

const HF_DAILY_PAPERS_URL: &str = "https://huggingface.co/api/daily_papers";
const ARXIV_FEED_URL: &str = "https://rss.arxiv.org/atom/cs.LG+cs.CL+cs.AI+cs.CV";
const USER_AGENT: &str = concat!("lmmaster-trends-bundle-curator/", env!("CARGO_PKG_VERSION"));

/// HF Daily Papers JSON parse — 단위 테스트 가능.
pub fn parse_hf_daily_papers(json: &str) -> Result<Vec<HfDailyPaper>> {
    serde_json::from_str(json).context("HF Daily Papers 응답 파싱 실패")
}

/// HF Daily Papers fetch — 익명 GET. 날짜 미지정 시 today (UTC).
pub async fn fetch_hf_daily_papers(
    client: &reqwest::Client,
    date: Option<&str>,
) -> Result<Vec<HfDailyPaper>> {
    let url = match date {
        Some(d) => format!("{HF_DAILY_PAPERS_URL}?date={d}&limit=100"),
        None => format!("{HF_DAILY_PAPERS_URL}?limit=100"),
    };
    let body = client
        .get(&url)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .send()
        .await
        .context("HF Daily Papers 요청 실패")?
        .error_for_status()
        .context("HF Daily Papers HTTP 에러")?
        .text()
        .await
        .context("HF Daily Papers 본문 읽기 실패")?;
    parse_hf_daily_papers(&body)
}

/// arXiv Atom RSS fetch — 익명 GET. parse는 호출자 책임 (feed-rs 도입 검토 후 22'.d.2).
pub async fn fetch_arxiv_atom(client: &reqwest::Client) -> Result<String> {
    let body = client
        .get(ARXIV_FEED_URL)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .send()
        .await
        .context("arXiv RSS 요청 실패")?
        .error_for_status()
        .context("arXiv RSS HTTP 에러")?
        .text()
        .await
        .context("arXiv RSS 본문 읽기 실패")?;
    Ok(body)
}

/// HfDailyPaper → TrendItem 변환 (큐레이터 한국어 요약은 GHA review queue에서 사람 손).
pub fn hf_paper_to_trend_item(paper: &HfDailyPaper) -> TrendItem {
    let id = format!("hf-paper-{}", paper.paper.id);
    let attribution = if paper.paper.authors.is_empty() {
        "AK on HuggingFace".to_string()
    } else {
        paper
            .paper
            .authors
            .iter()
            .take(3)
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let summary_en = paper.paper.summary.clone().unwrap_or_default();
    let summary_ko = if summary_en.is_empty() {
        "(큐레이터 한국어 요약 작성 대기)".to_string()
    } else {
        // 1차 placeholder — 22'.d.2에서 LLM 요약 또는 큐레이터 손길.
        format!(
            "(영문 요약 {} 자) — 큐레이터 한국어 번역 대기",
            summary_en.len()
        )
    };
    let score = (paper.upvotes as f64 / 100.0).min(1.0);
    TrendItem {
        id,
        kind: TrendKind::Paper,
        title: paper.paper.title.clone(),
        summary_ko,
        source: "huggingface-daily-papers".into(),
        source_url: format!("https://arxiv.org/abs/{}", paper.paper.id),
        attribution,
        published_at: paper.published_at.clone().unwrap_or_default(),
        tags: vec!["llm".into()],
        score,
    }
}

/// 외부 호출용 reqwest::Client — `.no_proxy()` + rustls TLS (ADR-0055 정합).
pub fn make_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .no_proxy()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("reqwest::Client builder 실패 (TLS init)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trend_kind_round_trip_kebab_case() {
        for (kind, expected) in [
            (TrendKind::Paper, "paper"),
            (TrendKind::Blog, "blog"),
            (TrendKind::News, "news"),
            (TrendKind::Video, "video"),
            (TrendKind::Github, "github"),
            (TrendKind::Sns, "sns"),
        ] {
            let v = serde_json::to_value(kind).unwrap();
            assert_eq!(v.as_str(), Some(expected));
            let parsed: TrendKind = serde_json::from_value(v).unwrap();
            assert_eq!(parsed, kind);
        }
    }

    #[test]
    fn parse_hf_daily_papers_minimal() {
        let json = r#"[
            {
                "paper": {
                    "id": "2405.12345",
                    "title": "Test Paper",
                    "summary": "Lorem ipsum"
                },
                "publishedAt": "2026-05-07T12:00:00Z",
                "upvotes": 42
            }
        ]"#;
        let papers = parse_hf_daily_papers(json).unwrap();
        assert_eq!(papers.len(), 1);
        assert_eq!(papers[0].paper.id, "2405.12345");
        assert_eq!(papers[0].upvotes, 42);
        assert_eq!(
            papers[0].published_at.as_deref(),
            Some("2026-05-07T12:00:00Z")
        );
    }

    #[test]
    fn parse_hf_daily_papers_empty() {
        let papers = parse_hf_daily_papers("[]").unwrap();
        assert_eq!(papers.len(), 0);
    }

    #[test]
    fn parse_hf_daily_papers_with_authors() {
        let json = r#"[
            {
                "paper": {
                    "id": "test/x",
                    "title": "T",
                    "authors": [{"name": "A"}, {"name": "B"}, {"name": "C"}, {"name": "D"}]
                }
            }
        ]"#;
        let papers = parse_hf_daily_papers(json).unwrap();
        assert_eq!(papers[0].paper.authors.len(), 4);
        let item = hf_paper_to_trend_item(&papers[0]);
        // top 3 attribution.
        assert!(item.attribution.contains("A"));
        assert!(item.attribution.contains("C"));
        assert!(!item.attribution.contains("D"));
    }

    #[test]
    fn parse_hf_daily_papers_no_authors_fallback_attribution() {
        let json = r#"[{"paper": {"id": "x", "title": "T"}}]"#;
        let papers = parse_hf_daily_papers(json).unwrap();
        let item = hf_paper_to_trend_item(&papers[0]);
        assert_eq!(item.attribution, "AK on HuggingFace");
    }

    #[test]
    fn hf_paper_to_trend_item_id_format() {
        let json = r#"[{"paper": {"id": "2405.12345", "title": "T"}}]"#;
        let papers = parse_hf_daily_papers(json).unwrap();
        let item = hf_paper_to_trend_item(&papers[0]);
        assert_eq!(item.id, "hf-paper-2405.12345");
        assert_eq!(item.kind, TrendKind::Paper);
        assert_eq!(item.source, "huggingface-daily-papers");
        assert_eq!(item.source_url, "https://arxiv.org/abs/2405.12345");
    }

    #[test]
    fn hf_paper_score_normalized_by_upvotes() {
        let json = r#"[{"paper": {"id": "x", "title": "T"}, "upvotes": 50}]"#;
        let item = hf_paper_to_trend_item(&parse_hf_daily_papers(json).unwrap()[0]);
        assert!((item.score - 0.5).abs() < 0.001);

        let json_max = r#"[{"paper": {"id": "x", "title": "T"}, "upvotes": 200}]"#;
        let item_max = hf_paper_to_trend_item(&parse_hf_daily_papers(json_max).unwrap()[0]);
        assert_eq!(item_max.score, 1.0);
    }

    #[test]
    fn make_client_succeeds() {
        let client = make_client();
        assert!(client.is_ok());
    }

    #[test]
    fn trend_item_round_trip() {
        let item = TrendItem {
            id: "hf-paper-2405.12345".into(),
            kind: TrendKind::Paper,
            title: "Test".into(),
            summary_ko: "한국어 요약이에요.".into(),
            source: "huggingface-daily-papers".into(),
            source_url: "https://arxiv.org/abs/2405.12345".into(),
            attribution: "AK on HuggingFace".into(),
            published_at: "2026-05-07T12:00:00Z".into(),
            tags: vec!["llm".into(), "korean".into()],
            score: 0.84,
        };
        let v = serde_json::to_value(&item).unwrap();
        assert_eq!(v["kind"], "paper");
        assert_eq!(v["summary_ko"], "한국어 요약이에요.");
        let parsed: TrendItem = serde_json::from_value(v).unwrap();
        assert_eq!(parsed, item);
    }
}
