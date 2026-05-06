//! 데이터 소스 fetcher — Phase 21'.b (HF + Open LLM Leaderboard).
//!
//! 정책 (ADR-0059 §3):
//! - HF Trending: `huggingface.co/api/models?sort=trending&library=gguf&pipeline_tag=text-generation&limit=200`
//! - Open LLM Leaderboard 2: `datasets-server.huggingface.co/rows?dataset=open-llm-leaderboard/contents&config=default&split=train&offset=N&length=100`
//! - 익명 호출. Rate limit 5분 윈도우 500 (HF).
//! - 외부 통신 화이트리스트: `huggingface.co` + `datasets-server.huggingface.co` (HF 서브도메인, ADR-0026 정합).
//! - User-Agent 명시 — `lmmaster-trending-watcher/0.0.x` (HF 정책 준수).
//!
//! 단위 테스트는 *parse 함수*만 (실 HTTP 호출은 통합 테스트로 분리).

#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// HF Trending 응답 1 entry — 필요 필드만.
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

/// Open LLM Leaderboard 2 dataset 1 row.
///
/// `datasets-server.huggingface.co`의 응답은 `{rows: [{row: {Model: ..., "Average ⬆️": ...}}]}` 구조.
/// 본 struct는 *내부 row* 매핑 — `OpenLlmResponse`가 wrapping.
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

/// `datasets-server.huggingface.co` 응답 wrapper.
#[derive(Debug, Deserialize)]
struct OpenLlmResponse {
    rows: Vec<OpenLlmRowWrapper>,
}

#[derive(Debug, Deserialize)]
struct OpenLlmRowWrapper {
    row: OpenLlmRow,
}

const HF_TRENDING_URL: &str =
    "https://huggingface.co/api/models?sort=trending&library=gguf&pipeline_tag=text-generation&limit=200";
const OPEN_LLM_URL: &str = "https://datasets-server.huggingface.co/rows?dataset=open-llm-leaderboard%2Fcontents&config=default&split=train&offset=0&length=100";

const USER_AGENT: &str = concat!("lmmaster-trending-watcher/", env!("CARGO_PKG_VERSION"));

/// HF Trending 응답 JSON parse — 단위 테스트 가능.
pub fn parse_hf_trending(json: &str) -> Result<Vec<HfModel>> {
    serde_json::from_str(json).context("HF trending 응답 파싱 실패")
}

/// Open LLM Leaderboard 응답 JSON parse — wrapper 풀어 row만 반환.
pub fn parse_open_llm(json: &str) -> Result<Vec<OpenLlmRow>> {
    let resp: OpenLlmResponse =
        serde_json::from_str(json).context("Open LLM Leaderboard 응답 파싱 실패")?;
    Ok(resp.rows.into_iter().map(|w| w.row).collect())
}

/// HF Trending fetch — 익명 GET.
pub async fn fetch_hf_trending(client: &reqwest::Client) -> Result<Vec<HfModel>> {
    let body = client
        .get(HF_TRENDING_URL)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .send()
        .await
        .context("HF Trending 요청 실패")?
        .error_for_status()
        .context("HF Trending HTTP 에러")?
        .text()
        .await
        .context("HF Trending 본문 읽기 실패")?;
    parse_hf_trending(&body)
}

/// Open LLM Leaderboard 2 fetch — 익명 GET.
pub async fn fetch_open_llm_leaderboard(client: &reqwest::Client) -> Result<Vec<OpenLlmRow>> {
    let body = client
        .get(OPEN_LLM_URL)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .send()
        .await
        .context("Open LLM Leaderboard 요청 실패")?
        .error_for_status()
        .context("Open LLM Leaderboard HTTP 에러")?
        .text()
        .await
        .context("Open LLM Leaderboard 본문 읽기 실패")?;
    parse_open_llm(&body)
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
    fn parse_hf_trending_minimal() {
        let json = r#"[
            {"id": "Qwen/Qwen3-7B-Instruct", "downloads": 50000, "likes": 1200, "trendingScore": 0.85,
             "pipeline_tag": "text-generation", "tags": ["transformers", "korean"], "library_name": "gguf"}
        ]"#;
        let models = parse_hf_trending(json).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "Qwen/Qwen3-7B-Instruct");
        assert_eq!(models[0].downloads, 50000);
        assert!(models[0].tags.contains(&"korean".to_string()));
    }

    #[test]
    fn parse_hf_trending_empty_array() {
        let models = parse_hf_trending("[]").unwrap();
        assert_eq!(models.len(), 0);
    }

    #[test]
    fn parse_hf_trending_missing_optional_fields() {
        // pipeline_tag / library_name / gated 누락도 graceful.
        let json = r#"[{"id": "minimal/model"}]"#;
        let models = parse_hf_trending(json).unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "minimal/model");
        assert_eq!(models[0].downloads, 0);
        assert!(models[0].pipeline_tag.is_none());
    }

    #[test]
    fn parse_open_llm_row_wrapper() {
        // raw string r##"..."## — 내부 `"#Params` 의 `"#`이 close marker로 오인되지 않게.
        let json = r##"{
            "rows": [
                {"row": {"Model": "test/model", "Average ⬆️": 67.5, "#Params (B)": 7.0,
                         "Hub License": "apache-2.0", "Chat Template": true}},
                {"row": {"Model": "test/other", "Average ⬆️": 45.2, "#Params (B)": 3.5,
                         "Hub License": "mit", "Chat Template": false}}
            ]
        }"##;
        let rows = parse_open_llm(json).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].model, "test/model");
        assert!((rows[0].avg - 67.5).abs() < 0.001);
        assert!(rows[0].chat_template);
        assert!(!rows[1].chat_template);
    }

    #[test]
    fn parse_hf_trending_invalid_json_errors() {
        let result = parse_hf_trending("not json");
        assert!(result.is_err());
    }

    #[test]
    fn make_client_succeeds() {
        // reqwest::Client builder + .no_proxy() + TLS init이 succeed해야 해요.
        let client = make_client();
        assert!(client.is_ok());
    }
}
