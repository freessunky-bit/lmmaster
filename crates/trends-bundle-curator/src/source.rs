//! 데이터 소스 fetcher 골격 — Phase 22'.b → 22'.c에서 실 fetch.
//!
//! 정책 (ADR-0060 §2):
//! - arXiv RSS: `https://rss.arxiv.org/atom/cs.LG+cs.CL+cs.AI+cs.CV` — User-Agent 필수, 3 req/sec.
//! - HF Daily Papers: `https://huggingface.co/api/daily_papers?date=YYYY-MM-DD&limit=100`.
//! - 회사 블로그 RSS: OpenAI / TechCrunch AI / The Verge / VentureBeat / NVIDIA / 한국 매체.
//! - YouTube Data API v3 (옵션, secret YOUTUBE_API_KEY).
//! - 외부 통신: huggingface.co + github.com + arxiv.org + techcrunch.com 등 화이트리스트 (GHA runner 측만).

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// trends-bundle 1 item (kind tagged enum 6종).
///
/// ADR-0060 §1 schema:
/// ```json
/// {
///   "id": "...",
///   "kind": "paper" | "blog" | "news" | "video" | "github" | "sns",
///   "title": "...",
///   "summary_ko": "한국어 1~2문장 요약",
///   "source": "huggingface-daily-papers" | "arxiv" | "openai-blog" | ...,
///   "source_url": "...",
///   "attribution": "...",
///   "published_at": "RFC3339",
///   "tags": ["llm", "korean"],
///   "score": 0.84
/// }
/// ```
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

/// trends-bundle item 종류 (ADR-0060 §1 tagged enum).
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
    /// HF API 원본 키 `publishedAt` — Rust 필드는 snake_case.
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

/// HF Daily Papers JSON parse — 단위 테스트 가능.
pub fn parse_hf_daily_papers(json: &str) -> anyhow::Result<Vec<HfDailyPaper>> {
    Ok(serde_json::from_str(json)?)
}

// 실 fetch 함수는 Phase 22'.c에서 추가:
//   pub async fn fetch_arxiv(client: &reqwest::Client) -> Result<Vec<TrendItem>>;
//   pub async fn fetch_hf_daily_papers(client: &reqwest::Client, date: &str) -> Result<Vec<TrendItem>>;
//   pub async fn fetch_company_blogs(client: &reqwest::Client) -> Result<Vec<TrendItem>>;
//   pub async fn fetch_youtube(client: &reqwest::Client, api_key: &str, channel_ids: &[&str]) -> Result<Vec<TrendItem>>;

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
