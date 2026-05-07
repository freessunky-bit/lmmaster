//! Parquet streaming — Phase 23'.c.2.b (ADR-0063 §2 + 보강 리서치 §1).
//!
//! 정책:
//! - HF endpoint: `huggingface.co/api/datasets/{repo}/parquet/{config}/{split}` (ADR-0026 적중).
//! - reqwest Range header로 row group 단위 lazy fetch — 1.8GB 안전.
//! - User-Agent 명시 — `lmmaster-dataset-importer/<ver>`.
//! - 429 응답 시 `RateLimit-Retry-After` 헤더 honor.
//! - `arrow-rs parquet::arrow::async_reader::AsyncFileReader` trait impl.
//! - `ParquetRecordBatchStreamBuilder` + projection mask로 *5개 컬럼만* 메모리 절약.
//! - row group 단위 lazy stream — `Stream<Item = Result<RecordBatch>>`.

#![allow(dead_code)]

use std::ops::Range;
use std::sync::Arc;

use anyhow::{Context, Result};
use bytes::Bytes;
use futures::future::BoxFuture;
use futures::FutureExt;
use parquet::arrow::async_reader::{
    AsyncFileReader, ParquetRecordBatchStream, ParquetRecordBatchStreamBuilder,
};
use parquet::arrow::ProjectionMask;
use parquet::errors::{ParquetError, Result as ParquetResult};
use parquet::file::metadata::ParquetMetaData;
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

    /// `{base}/api/datasets/{repo}/parquet/{config}/{split}` 호출 + URL 배열 파싱.
    ///
    /// 정책:
    /// - 익명 GET. User-Agent 필수.
    /// - 429 → `Err(RateLimited { retry_after_secs })`.
    /// - 404 → `Err(HfApiUnreachable)`.
    /// - 응답 schema는 *URL 배열 (flat)* 또는 *nested config/split*.
    pub async fn resolve(
        &self,
        repo: &str,
        config: &str,
        split: &str,
    ) -> DatasetImportResult<Vec<String>> {
        self.resolve_with_base(HF_PARQUET_ENDPOINT, repo, config, split)
            .await
    }

    /// 테스트용 — base URL 주입 (wiremock 등).
    pub async fn resolve_with_base(
        &self,
        base: &str,
        repo: &str,
        config: &str,
        split: &str,
    ) -> DatasetImportResult<Vec<String>> {
        let url = format!("{base}/{repo}/parquet/{config}/{split}");
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
pub fn parse_parquet_url_list(json: &str) -> DatasetImportResult<Vec<String>> {
    if let Ok(urls) = serde_json::from_str::<Vec<String>>(json) {
        return Ok(urls);
    }

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

/// HfParquetReader — `arrow-rs parquet::arrow::async_reader::AsyncFileReader` impl.
///
/// 정책:
/// - `get_bytes`: reqwest Range header로 byte 범위 fetch.
/// - `get_metadata`: parquet footer 8 bytes에서 metadata length 추출 → 전체 metadata fetch + decode.
/// - 모든 호출은 idempotent (Cache 가능 — 23'.c.2.c에서 구현).
#[derive(Debug, Clone)]
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

    /// HEAD 요청으로 Content-Length 확인 — caching for AsyncFileReader.
    pub async fn fetch_total_size(&mut self) -> DatasetImportResult<u64> {
        if let Some(size) = self.total_size {
            return Ok(size);
        }
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

    /// Range request — `AsyncFileReader::get_bytes` 백엔드.
    pub async fn fetch_range(&self, start: u64, end_exclusive: u64) -> DatasetImportResult<Bytes> {
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

/// `AsyncFileReader` trait impl — arrow-rs parquet 53.x.
///
/// `get_bytes` + `get_metadata` 두 함수만 구현 필수.
/// `get_byte_ranges`는 default impl (각 range 직렬 호출)이 충분 — 23'.c.2.c에서 병렬 최적화 검토.
impl AsyncFileReader for HfParquetReader {
    fn get_bytes(&mut self, range: Range<usize>) -> BoxFuture<'_, ParquetResult<Bytes>> {
        let start = range.start as u64;
        let end = range.end as u64;
        async move {
            self.fetch_range(start, end)
                .await
                .map_err(|e| ParquetError::External(Box::new(std::io::Error::other(e.to_string()))))
        }
        .boxed()
    }

    fn get_metadata(&mut self) -> BoxFuture<'_, ParquetResult<Arc<ParquetMetaData>>> {
        async move {
            let total = self.fetch_total_size().await.map_err(|e| {
                ParquetError::External(Box::new(std::io::Error::other(e.to_string())))
            })?;
            // 마지막 8 bytes — [4 bytes footer length, 4 bytes "PAR1" magic].
            if total < 8 {
                return Err(ParquetError::EOF(format!(
                    "parquet 파일이 너무 작아요: {total} bytes"
                )));
            }
            let footer_tail = self.fetch_range(total - 8, total).await.map_err(|e| {
                ParquetError::External(Box::new(std::io::Error::other(e.to_string())))
            })?;
            // [0..4] = metadata length (little-endian u32), [4..8] = "PAR1".
            let magic = &footer_tail[4..8];
            if magic != b"PAR1" {
                return Err(ParquetError::General(format!("PAR1 magic 누락: {magic:?}")));
            }
            let metadata_len = u32::from_le_bytes(
                footer_tail[0..4]
                    .try_into()
                    .map_err(|_| ParquetError::General("footer length 파싱 실패".into()))?,
            ) as u64;
            if metadata_len + 8 > total {
                return Err(ParquetError::General(format!(
                    "metadata length {metadata_len} > total {total} - 8"
                )));
            }
            let metadata_offset = total - 8 - metadata_len;
            let metadata_bytes =
                self.fetch_range(metadata_offset, total - 8)
                    .await
                    .map_err(|e| {
                        ParquetError::External(Box::new(std::io::Error::other(e.to_string())))
                    })?;
            let metadata =
                parquet::file::metadata::ParquetMetaDataReader::decode_metadata(&metadata_bytes)?;
            Ok(Arc::new(metadata))
        }
        .boxed()
    }
}

/// `HfParquetReader` → `ParquetRecordBatchStream` builder.
///
/// 정책:
/// - `column_names`: 읽을 컬럼 이름 목록 (예: ["persona", "age", "province", "occupation"]).
///   parquet schema에서 leaf 인덱스를 자동 매핑 → ProjectionMask. 메모리 절약 핵심 (Personas-Korea 26 → 5 컬럼).
/// - `batch_size`: row group 안에서 N row씩 RecordBatch 반환 (기본 256, RAG 임베딩 batch 크기와 정렬).
///
/// 반환된 `ParquetRecordBatchStream<HfParquetReader>`는 `futures::Stream<Item = ParquetResult<RecordBatch>>`로
/// row group 단위 lazy iterator. 23'.c.2.c `DatasetIngestService`가 consume.
pub async fn open_stream(
    reader: HfParquetReader,
    column_names: &[&str],
    batch_size: usize,
) -> ParquetResult<ParquetRecordBatchStream<HfParquetReader>> {
    let builder = ParquetRecordBatchStreamBuilder::new(reader).await?;

    // 컬럼 이름 → leaf index 매핑. `parquet_schema()`는 `&SchemaDescriptor` —
    // scope로 borrow를 닫아 builder consume(`with_projection` → `build`) 가능하게 한다.
    let indices: Vec<usize> = {
        let schema = builder.parquet_schema();
        let mut idxs: Vec<usize> = Vec::with_capacity(column_names.len());
        for name in column_names {
            let idx = schema
                .columns()
                .iter()
                .position(|c| c.name() == *name)
                .ok_or_else(|| {
                    ParquetError::General(format!("컬럼 '{name}'를 schema에서 찾을 수 없어요"))
                })?;
            idxs.push(idx);
        }
        idxs
    };

    let mask = ProjectionMask::leaves(builder.parquet_schema(), indices);
    let stream = builder
        .with_projection(mask)
        .with_batch_size(batch_size)
        .build()?;
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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
                "train": ["https://huggingface.co/datasets/x/resolve/.../0000.parquet"]
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

    // ── wiremock 통합 테스트 — 23'.c.2.b 신규 ───────────────────────

    #[tokio::test]
    async fn url_resolver_returns_url_array() {
        let server = MockServer::start().await;
        // wiremock path matcher — repo의 슬래시가 unencoded로 전달.
        Mock::given(method("GET"))
            .and(path("/api/datasets/test/dataset/parquet/default/train"))
            .and(header("user-agent", USER_AGENT))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"["https://x/0.parquet","https://x/1.parquet"]"#),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let resolver = ParquetUrlResolver::new(client);
        let urls = resolver
            .resolve_with_base(
                &format!("{}/api/datasets", server.uri()),
                "test/dataset",
                "default",
                "train",
            )
            .await
            .unwrap();
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0], "https://x/0.parquet");
    }

    #[tokio::test]
    async fn url_resolver_429_returns_rate_limited() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(429).insert_header("RateLimit-Retry-After", "120"))
            .mount(&server)
            .await;

        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let resolver = ParquetUrlResolver::new(client);
        let result = resolver
            .resolve_with_base(
                &format!("{}/api/datasets", server.uri()),
                "x",
                "default",
                "train",
            )
            .await;
        assert!(matches!(
            result,
            Err(DatasetImportError::RateLimited {
                retry_after_secs: 120
            })
        ));
    }

    #[tokio::test]
    async fn url_resolver_404_returns_unreachable() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let resolver = ParquetUrlResolver::new(client);
        let result = resolver
            .resolve_with_base(
                &format!("{}/api/datasets", server.uri()),
                "x",
                "default",
                "train",
            )
            .await;
        assert!(matches!(
            result,
            Err(DatasetImportError::HfApiUnreachable(_))
        ));
    }

    #[tokio::test]
    async fn fetch_total_size_caches_result() {
        let server = MockServer::start().await;
        Mock::given(method("HEAD"))
            .and(path("/y.parquet"))
            .respond_with(ResponseTemplate::new(200).insert_header("Content-Length", "12345"))
            .expect(1) // 단 1회만 호출 — caching 확인.
            .mount(&server)
            .await;

        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let url = format!("{}/y.parquet", server.uri());
        let mut reader = HfParquetReader::new(url, client);

        let size1 = reader.fetch_total_size().await.unwrap();
        let size2 = reader.fetch_total_size().await.unwrap(); // cached.
        assert_eq!(size1, 12345);
        assert_eq!(size2, 12345);
    }

    #[tokio::test]
    async fn fetch_range_sends_range_header() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/y.parquet"))
            .and(header("range", "bytes=100-199"))
            .respond_with(ResponseTemplate::new(206).set_body_bytes(vec![0u8; 100]))
            .mount(&server)
            .await;

        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let url = format!("{}/y.parquet", server.uri());
        let reader = HfParquetReader::new(url, client);

        let bytes = reader.fetch_range(100, 200).await.unwrap();
        assert_eq!(bytes.len(), 100);
    }

    #[tokio::test]
    async fn async_file_reader_get_bytes_via_range() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/y.parquet"))
            .and(header("range", "bytes=0-99"))
            .respond_with(ResponseTemplate::new(206).set_body_bytes(vec![42u8; 100]))
            .mount(&server)
            .await;

        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let url = format!("{}/y.parquet", server.uri());
        let mut reader = HfParquetReader::new(url, client);

        // AsyncFileReader trait 호출.
        let bytes = AsyncFileReader::get_bytes(&mut reader, 0..100)
            .await
            .unwrap();
        assert_eq!(bytes.len(), 100);
        assert_eq!(bytes[0], 42);
    }

    #[tokio::test]
    async fn get_metadata_parses_footer_magic() {
        // 작은 parquet 파일 mock — footer 8 bytes 구조만 검증.
        // 실 parquet은 너무 복잡해서 *magic 검증 실패 케이스*만 단위 테스트.
        let server = MockServer::start().await;
        Mock::given(method("HEAD"))
            .respond_with(ResponseTemplate::new(200).insert_header("Content-Length", "16"))
            .mount(&server)
            .await;
        // 마지막 8 bytes: [0, 0, 0, 0, 'X', 'X', 'X', 'X'] — magic mismatch.
        Mock::given(method("GET"))
            .and(header("range", "bytes=8-15"))
            .respond_with(
                ResponseTemplate::new(206).set_body_bytes(vec![0, 0, 0, 0, b'X', b'X', b'X', b'X']),
            )
            .mount(&server)
            .await;

        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let url = format!("{}/y.parquet", server.uri());
        let mut reader = HfParquetReader::new(url, client);

        let result = AsyncFileReader::get_metadata(&mut reader).await;
        assert!(result.is_err(), "magic mismatch should error");
    }
}
