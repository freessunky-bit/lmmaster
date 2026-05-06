//! 데이터 소스 fetcher 골격 — Phase 21'.b에 채워요.
//!
//! 정책 (ADR-0059 §3):
//! - HF Trending: `huggingface.co/api/models?sort=trending&library=gguf&pipeline_tag=text-generation&limit=200`
//! - Open LLM Leaderboard 2: `datasets-server.huggingface.co/rows?dataset=open-llm-leaderboard/contents`
//! - Arena 미러: `raw.githubusercontent.com/oolong-tea-2026/arena-ai-leaderboards/main/data/latest.json`
//! - KMMLU: model card 정규식 `KMMLU.*\d+\.\d+`
//! - 익명 호출. Rate limit 5분 윈도우 500 (HF).

// Phase 21'.a 골격 — 본 struct들은 21'.b에서 fetch 함수에 의해 사용돼요.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// HF Trending 응답 1 entry (필요 필드만).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HfModel {
    pub id: String,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub likes: u64,
    #[serde(rename = "trendingScore", default)]
    pub trending_score: f64,
    #[serde(default)]
    pub pipeline_tag: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub library_name: Option<String>,
    #[serde(default)]
    pub gated: serde_json::Value,
}

/// Open LLM Leaderboard 2 dataset 1 row (필요 필드만).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenLlmRow {
    #[serde(rename = "Model")]
    pub model: String,
    #[serde(rename = "Average ⬆️", default)]
    pub avg: f64,
    #[serde(rename = "#Params (B)", default)]
    pub params_b: f64,
    #[serde(rename = "Hub License", default)]
    pub license: Option<String>,
    #[serde(rename = "Chat Template", default)]
    pub chat_template: bool,
}

// fetch 함수는 Phase 21'.b에서 추가:
//   pub async fn fetch_hf_trending(client: &reqwest::Client) -> Result<Vec<HfModel>>;
//   pub async fn fetch_open_llm_leaderboard(client: &reqwest::Client) -> Result<Vec<OpenLlmRow>>;
//   pub async fn fetch_arena_mirror(client: &reqwest::Client) -> Result<...>;
