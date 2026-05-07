//! Parquet streaming — Phase 23'.c.2.b (ADR-0063 §2).
//!
//! 정책:
//! - HF endpoint: `huggingface.co/api/datasets/{repo}/parquet/{config}/{split}` (ADR-0026 적중).
//! - reqwest Range header로 row group 단위 lazy fetch — 1.8GB 안전.
//! - User-Agent 명시 — `lmmaster-dataset-importer/<ver>`.
//! - 429 응답 시 `RateLimit-Retry-After` 헤더 honor (`backon` 호환).
//!
//! **본 sub-phase 23'.c.2.a 코드는 *struct + URL resolver*만**. 실 `AsyncFileReader` impl + ParquetRecordBatchStream은 **23'.c.2.b**에서 채워요. 골격만 두는 게 아니라 *URL 해석 + reqwest Range 호출 검증된 함수* 1차 도착.

#![allow(dead_code)]

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::error::{DatasetImportError, DatasetImportResult};

const HF_PARQUET_ENDPOINT: &str = "https://huggingface.co/api/datasets";
const USER_AGENT: &str = concat!("lmmaster-dataset-importer/", env!("CARGO_PKG_VERSION"));

/// HF `/api/datasets/{repo}/parquet/{config}/{split}` 응답 schema.
///
/// 응답은 *URL 배열의 배열* — 각 outer 항목이 split의 shard.
/// ```json
/// {
///   "default": {
///     "train": ["https://huggingface.co/datasets/.../resolve/.../0000.parquet", ...]
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HfParquetIndex {
    /// `config` (보통 `default`) → `split` (보통 `train`) → URL 배열.
    #[serde(flatten)]
    pub configs:
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, Vec<String>>>,
}

/// HF parquet URL resolver — repo + config + split 받아 URL 배열 반환.
pub struct ParquetUrlResolver {
    client: reqwest::Client,
}

impl ParquetUrlResolver {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// `huggingface.co/api/datasets/{repo}/parquet/{config}/{split}` 호출 + URL 배열 파싱.
    ///
    /// 정책:
    /// - 익명 GET. User-Agent 필수.
    /// - 429 → `Err(RateLimited { retry_after_secs })`.
    /// - 404 → `Err(HfApiUnreachable)`.
    /// - 응답 schema는 *URL 배열* (간단한 1차 형태).
    pub async fn resolve(
        &self,
        repo: &str,
        config: &str,
        split: &str,
    ) -> DatasetImportResult<Vec<String>> {
        let url = format!("{HF_PARQUET_ENDPOINT}/{repo}/parquet/{config}/{split}");
        let resp = self
            .client
            .get(&url)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await
            .map_err(|e| DatasetImportError::HfApiUnreachable(format!("{url}: {e}")))?;

        if resp.status() == 429 {
            let retry_after = resp
                .headers()
                .get("RateLimit-Retry-After")
                .or_else(|| resp.headers().get("Retry-After"))
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse().ok())
                .unwrap_or(60);
            return Err(DatasetImportError::RateLimited {
                retry_after_secs: retry_after,
            });
        }
        if !resp.status().is_success() {
            return Err(DatasetImportError::HfApiUnreachable(format!(
                "HTTP {} for {url}",
                resp.status()
            )));
        }

        let text = resp
            .text()
            .await
            .map_err(|e| DatasetImportError::HfApiUnreachable(format!("body read: {e}")))?;
        parse_parquet_url_list(&text)
    }
}

/// 응답 JSON parse — 단위 테스트 가능.
///
/// HF API는 *flat URL 배열* 또는 *nested config/split* 두 형태 가능. 두 가지 시도.
pub fn parse_parquet_url_list(json: &str) -> DatasetImportResult<Vec<String>> {
    // 1차: flat URL 배열 (가장 흔한 형태).
    if let Ok(urls) = serde_json::from_str::<Vec<String>>(json) {
        return Ok(urls);
    }

    // 2차: nested {config: {split: [urls]}}.
    if let Ok(idx) = serde_json::from_str::<HfParquetIndex>(json) {
        let mut all: Vec<String> = idx
            .configs
            .into_values()
            .flat_map(|splits| splits.into_values())
            .flatten()
            .collect();
        all.sort();
        return Ok(all);
    }

    Err(DatasetImportError::ParquetReadFailed(format!(
        "응답 schema를 인식할 수 없어요: {}",
        &json.chars().take(200).collect::<String>()
    )))
}

/// `make_client` — `.no_proxy()` + rustls + 30s timeout (ADR-0055 정합).
pub fn make_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .no_proxy()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("reqwest::Client builder 실패 (TLS init)")
}

/// Phase 23'.c.2.b 후속 — `AsyncFileReader` impl + `ParquetRecordBatchStream`.
/// 현재는 placeholder.
pub struct HfParquetReader {
    pub url: String,
    pub client: reqwest::Client,
    pub total_size: Option<u64>,
}

impl HfParquetReader {
    pub fn new(url: String, client: reqwest::Client) -> Self {
        Self {
            url,
            client,
            total_size: None,
        }
    }

    /// HEAD 요청으로 Content-Length 확인 — 23'.c.2.b에서 footer fetch 시 활용.
    pub async fn fetch_total_size(&mut self) -> DatasetImportResult<u64> {
        let resp = self
            .client
            .head(&self.url)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(DatasetImportError::HfApiUnreachable(format!(
                "HEAD {} = HTTP {}",
                self.url,
                resp.status()
            )));
        }
        let size = resp
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| DatasetImportError::ParquetReadFailed("Content-Length 누락".into()))?;
        self.total_size = Some(size);
        Ok(size)
    }

    /// Range request — Phase 23'.c.2.b에서 `AsyncFileReader::get_bytes` 호출.
    pub async fn fetch_range(
        &self,
        start: u64,
        end_exclusive: u64,
    ) -> DatasetImportResult<bytes::Bytes> {
        let resp = self
            .client
            .get(&self.url)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .header(
                reqwest::header::RANGE,
                format!("bytes={}-{}", start, end_exclusive - 1),
            )
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(DatasetImportError::HfApiUnreachable(format!(
                "Range {}-{} = HTTP {}",
                start,
                end_exclusive,
                resp.status()
            )));
        }
        let body = resp.bytes().await?;
        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_flat_url_list() {
        let json = r#"[
            "https://huggingface.co/datasets/x/resolve/refs%2Fconvert%2Fparquet/y/0000.parquet",
            "https://huggingface.co/datasets/x/resolve/refs%2Fconvert%2Fparquet/y/0001.parquet"
        ]"#;
        let urls = parse_parquet_url_list(json).unwrap();
        assert_eq!(urls.len(), 2);
        assert!(urls[0].contains("0000.parquet"));
    }

    #[test]
    fn parse_nested_config_split() {
        let json = r#"{
            "default": {
                "train": [
                    "https://huggingface.co/datasets/x/resolve/.../0000.parquet"
                ]
            }
        }"#;
        let urls = parse_parquet_url_list(json).unwrap();
        assert_eq!(urls.len(), 1);
        assert!(urls[0].contains("0000.parquet"));
    }

    #[test]
    fn parse_empty_array() {
        let urls = parse_parquet_url_list("[]").unwrap();
        assert_eq!(urls.len(), 0);
    }

    #[test]
    fn parse_invalid_json_errors() {
        let result = parse_parquet_url_list("not json");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DatasetImportError::ParquetReadFailed(_)
        ));
    }

    #[test]
    fn parse_nested_multiple_splits_sorted() {
        let json = r#"{
            "default": {
                "validation": ["https://x/v.parquet"],
                "train": ["https://x/t.parquet"]
            }
        }"#;
        let urls = parse_parquet_url_list(json).unwrap();
        // 정렬됨 (BTreeMap 자체로).
        assert_eq!(urls, vec!["https://x/t.parquet", "https://x/v.parquet"]);
    }

    #[test]
    fn make_client_succeeds() {
        let client = make_client();
        assert!(client.is_ok());
    }

    #[test]
    fn hf_parquet_reader_new() {
        let client = make_client().unwrap();
        let reader = HfParquetReader::new("https://x/y.parquet".into(), client);
        assert_eq!(reader.url, "https://x/y.parquet");
        assert!(reader.total_size.is_none());
    }
}
