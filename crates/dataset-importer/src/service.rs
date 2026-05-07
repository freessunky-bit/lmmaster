//! DatasetIngestService runner — Phase 23'.c.2.c (ADR-0063 §4).
//!
//! 책임:
//! - 카탈로그 entry → HF parquet URL 목록 → row-group lazy stream → row 텍스트 합성 → chunker → IngestProgress emit.
//! - SampleStrategy 적용 (`Full` / `First {n}` / `Stratified {n, by}` — `Stratified`는 .c.2.d에서 by 컬럼 활용 정교화).
//! - cancel token (`Arc<AtomicBool>`)으로 사용자 취소 지원 — row 단위 폴링.
//!
//! **본 sub-phase 범위 (.c.2.c)**:
//! - Manifest / Downloading / Chunking 3단계까지 emit.
//! - row 컬럼 dtype은 *Utf8 (StringArray)* 우선 — 다른 dtype은 `.c.2.d` (Personas-Korea province/occupation/age 합성)에서 확장.
//! - Embedding / Writing 2단계는 `.c.2.d` (OnnxEmbedder + SQLCipher schema 2→3) 후속.

#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use arrow_array::{Array, RecordBatch, StringArray};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::chunker::{DatasetChunk, DatasetChunker};
use crate::error::{DatasetImportError, DatasetImportResult};
use crate::parquet_stream::{make_client, open_stream, HfParquetReader, ParquetUrlResolver};
use crate::pipeline::{DatasetIngestStage, IngestProgress, SampleStrategy};

/// Service 결과 통계.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IngestStats {
    pub rows_processed: u64,
    pub chunks_emitted: u64,
    pub urls_fetched: u32,
}

/// Service 입력 — 카탈로그 entry에서 매핑 (.d Tauri IPC handler 책임).
#[derive(Debug, Clone)]
pub struct IngestRequest {
    /// HF dataset repo (예: "Mxode/Personas-Korea").
    pub repo: String,
    /// HF parquet config — 보통 `default`.
    pub config: String,
    /// HF parquet split — 보통 `train`.
    pub split: String,
    /// row 텍스트 합성에 사용할 컬럼 (예: ["persona"] 또는 ["persona", "province", "occupation"]).
    pub text_columns: Vec<String>,
    /// 샘플 전략 (10K stratified default).
    pub sample: SampleStrategy,
}

/// Dataset ingest orchestrator. .c.2.d에서 Tauri command가 이 service를 hold.
pub struct DatasetIngestService {
    resolver: ParquetUrlResolver,
    client: reqwest::Client,
    chunker: DatasetChunker,
    /// HF base URL — production은 `https://huggingface.co/api/datasets`, 테스트는 wiremock base.
    base_url: String,
    /// Chunk 누적 buffer size (정기 progress emit interval). 기본 100 row.
    progress_emit_interval: u64,
}

impl DatasetIngestService {
    /// Production constructor — HF endpoint + 기본 chunker.
    pub fn new(chunker: DatasetChunker) -> DatasetImportResult<Self> {
        let client = make_client().map_err(|e| DatasetImportError::Internal(e.to_string()))?;
        Ok(Self {
            resolver: ParquetUrlResolver::new(client.clone()),
            client,
            chunker,
            base_url: "https://huggingface.co/api/datasets".into(),
            progress_emit_interval: 100,
        })
    }

    /// 테스트용 — wiremock base URL 주입.
    pub fn with_base_url(
        chunker: DatasetChunker,
        client: reqwest::Client,
        base_url: String,
    ) -> Self {
        Self {
            resolver: ParquetUrlResolver::new(client.clone()),
            client,
            chunker,
            base_url,
            progress_emit_interval: 1, // 테스트는 매 row emit.
        }
    }

    /// 메인 흐름 — Manifest / Downloading / Chunking 단계 emit.
    pub async fn run(
        &self,
        request: IngestRequest,
        progress: mpsc::Sender<IngestProgress>,
        cancel: Arc<AtomicBool>,
        chunks_out: mpsc::Sender<DatasetChunk>,
    ) -> DatasetImportResult<IngestStats> {
        if cancel.load(Ordering::Relaxed) {
            return Err(DatasetImportError::Cancelled);
        }

        // ---- Stage 1: Manifest ----
        let _ = progress
            .send(IngestProgress::new(
                DatasetIngestStage::Manifest,
                "데이터셋 정보 받고 있어요",
            ))
            .await;

        let urls = self
            .resolver
            .resolve_with_base(
                &self.base_url,
                &request.repo,
                &request.config,
                &request.split,
            )
            .await?;

        if urls.is_empty() {
            return Err(DatasetImportError::ParquetReadFailed(
                "응답에 parquet URL이 없어요".into(),
            ));
        }

        // ---- Stage 2 + 3: Downloading + Chunking ----
        let max_rows = match &request.sample {
            SampleStrategy::Full => u64::MAX,
            SampleStrategy::First { n } => *n,
            SampleStrategy::Stratified { n, .. } => *n,
        };

        let mut stats = IngestStats::default();

        'outer: for url in &urls {
            if cancel.load(Ordering::Relaxed) {
                return Err(DatasetImportError::Cancelled);
            }
            stats.urls_fetched += 1;

            let _ = progress
                .send(IngestProgress {
                    stage: DatasetIngestStage::Downloading,
                    current: stats.rows_processed,
                    total: max_rows,
                    eta_secs: None,
                    chunks_written: stats.chunks_emitted,
                    message_ko: format!(
                        "parquet 받고 있어요 ({}/{})",
                        stats.urls_fetched,
                        urls.len()
                    ),
                })
                .await;

            let reader = HfParquetReader::new(url.clone(), self.client.clone());
            let column_refs: Vec<&str> = request.text_columns.iter().map(String::as_str).collect();
            let mut stream = open_stream(reader, &column_refs, 256)
                .await
                .map_err(|e| DatasetImportError::ParquetReadFailed(e.to_string()))?;

            while let Some(batch_res) = stream.next().await {
                if cancel.load(Ordering::Relaxed) {
                    return Err(DatasetImportError::Cancelled);
                }
                if stats.rows_processed >= max_rows {
                    break 'outer;
                }
                let batch =
                    batch_res.map_err(|e| DatasetImportError::ParquetReadFailed(e.to_string()))?;

                for row_idx in 0..batch.num_rows() {
                    if stats.rows_processed >= max_rows {
                        break;
                    }
                    let text = compose_row_text(&batch, row_idx, &request.text_columns)?;
                    if text.trim().is_empty() {
                        stats.rows_processed += 1;
                        continue;
                    }
                    let chunks = self.chunker.chunks(stats.rows_processed, &text);
                    for chunk in chunks {
                        // 채널 닫혀 있으면 receiver drop — Cancelled 등가 처리.
                        if chunks_out.send(chunk).await.is_err() {
                            return Err(DatasetImportError::Cancelled);
                        }
                        stats.chunks_emitted += 1;
                    }
                    stats.rows_processed += 1;

                    if stats.rows_processed % self.progress_emit_interval == 0 {
                        let _ = progress
                            .send(IngestProgress {
                                stage: DatasetIngestStage::Chunking,
                                current: stats.rows_processed,
                                total: max_rows,
                                eta_secs: None,
                                chunks_written: stats.chunks_emitted,
                                message_ko: format!(
                                    "{} row / {} chunk 처리했어요",
                                    stats.rows_processed, stats.chunks_emitted
                                ),
                            })
                            .await;
                    }
                }
            }
        }

        // ---- Stage Done (Embedding/Writing은 .c.2.d) ----
        let _ = progress
            .send(IngestProgress {
                stage: DatasetIngestStage::Done,
                current: stats.rows_processed,
                total: stats.rows_processed,
                eta_secs: Some(0),
                chunks_written: stats.chunks_emitted,
                message_ko: format!(
                    "{} row → {} chunk 처리 완료했어요",
                    stats.rows_processed, stats.chunks_emitted
                ),
            })
            .await;

        Ok(stats)
    }
}

/// RecordBatch의 row를 N개 컬럼 string concat으로 합성.
///
/// 정책 (.c.2.c):
/// - 모든 컬럼 *Utf8 (StringArray)* 가정. 비-Utf8 dtype은 `.c.2.d`에서 확장.
/// - null 값은 skip — chunk 텍스트에서 제거.
/// - 합성 형식: `{col_name}: {value}\n` join.
pub(crate) fn compose_row_text(
    batch: &RecordBatch,
    row_idx: usize,
    columns: &[String],
) -> DatasetImportResult<String> {
    let mut parts = Vec::with_capacity(columns.len());
    for col_name in columns {
        let array = batch
            .column_by_name(col_name)
            .ok_or_else(|| DatasetImportError::TextFieldMissing(col_name.clone()))?;
        let arr = array
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| {
                DatasetImportError::ParquetReadFailed(format!(
                    "컬럼 '{col_name}'이 Utf8 타입이 아니에요 (현 sub-phase는 Utf8만 지원)"
                ))
            })?;
        if arr.is_null(row_idx) {
            continue;
        }
        parts.push(format!("{}: {}", col_name, arr.value(row_idx)));
    }
    Ok(parts.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunker::ChunkConfigParams;
    use arrow_array::ArrayRef;
    use arrow_schema::{DataType, Field, Schema};
    use parquet::arrow::ArrowWriter;
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    /// In-memory parquet 생성 — Utf8 단일 컬럼.
    fn make_test_parquet(rows: &[&str]) -> Vec<u8> {
        let schema = Arc::new(Schema::new(vec![Field::new(
            "persona",
            DataType::Utf8,
            true,
        )]));
        let array: ArrayRef = Arc::new(StringArray::from(rows.to_vec()));
        let batch = RecordBatch::try_new(schema.clone(), vec![array]).unwrap();

        let mut buf = Vec::new();
        let mut writer = ArrowWriter::try_new(&mut buf, schema, None).unwrap();
        writer.write(&batch).unwrap();
        writer.close().unwrap();
        buf
    }

    /// Range header에 따라 해당 byte slice를 206으로 반환하는 wiremock responder.
    /// reqwest의 Range request → AsyncFileReader.get_bytes/get_metadata 통합 검증 핵심.
    struct ParquetRangeResponder {
        body: Vec<u8>,
    }

    impl Respond for ParquetRangeResponder {
        fn respond(&self, req: &Request) -> ResponseTemplate {
            if let Some(range_h) = req.headers.get("range") {
                if let Ok(range_str) = range_h.to_str() {
                    if let Some(rest) = range_str.strip_prefix("bytes=") {
                        if let Some((s, e)) = rest.split_once('-') {
                            if let (Ok(start), Ok(end)) = (s.parse::<usize>(), e.parse::<usize>()) {
                                let end_inc = end.min(self.body.len() - 1);
                                if start <= end_inc {
                                    let slice = self.body[start..=end_inc].to_vec();
                                    return ResponseTemplate::new(206).set_body_bytes(slice);
                                }
                            }
                        }
                    }
                }
            }
            // Range 없으면 200 + 전체.
            ResponseTemplate::new(200).set_body_bytes(self.body.clone())
        }
    }

    /// HEAD/GET 한 쌍 mount — Content-Length + Range respond.
    async fn mount_parquet_endpoint(server: &MockServer, mount_path: &str, body: Vec<u8>) {
        let len = body.len();
        Mock::given(method("HEAD"))
            .and(path(mount_path))
            .respond_with(
                ResponseTemplate::new(200).insert_header("Content-Length", len.to_string()),
            )
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path(mount_path))
            .respond_with(ParquetRangeResponder { body })
            .mount(server)
            .await;
    }

    #[test]
    fn compose_row_text_concats_columns() {
        let schema = Arc::new(Schema::new(vec![
            Field::new("persona", DataType::Utf8, true),
            Field::new("province", DataType::Utf8, true),
        ]));
        let persona: ArrayRef = Arc::new(StringArray::from(vec!["김민지", "이서준"]));
        let province: ArrayRef = Arc::new(StringArray::from(vec!["서울", "부산"]));
        let batch = RecordBatch::try_new(schema, vec![persona, province]).unwrap();

        let columns = vec!["persona".to_string(), "province".to_string()];
        let text = compose_row_text(&batch, 0, &columns).unwrap();
        assert!(text.contains("persona: 김민지"));
        assert!(text.contains("province: 서울"));
    }

    #[test]
    fn compose_row_text_missing_column_errors() {
        let schema = Arc::new(Schema::new(vec![Field::new(
            "persona",
            DataType::Utf8,
            true,
        )]));
        let array: ArrayRef = Arc::new(StringArray::from(vec!["x"]));
        let batch = RecordBatch::try_new(schema, vec![array]).unwrap();

        let columns = vec!["nonexistent".to_string()];
        let err = compose_row_text(&batch, 0, &columns).unwrap_err();
        match err {
            DatasetImportError::TextFieldMissing(c) => assert_eq!(c, "nonexistent"),
            other => panic!("expected TextFieldMissing, got {other:?}"),
        }
    }

    /// 통합 — 전체 pipeline (Manifest → Downloading → Chunking → Done) wiremock + parquet writer로 검증.
    #[tokio::test]
    async fn service_runs_full_pipeline() {
        let server = MockServer::start().await;
        let parquet_bytes = make_test_parquet(&[
            "안녕하세요. 가상 한국인 페르소나입니다.",
            "두 번째 페르소나. 부산 거주 30대 직장인.",
            "세 번째 페르소나. 제주도 농부 60대.",
        ]);

        let parquet_url = format!("{}/files/0000.parquet", server.uri());
        let url_list = serde_json::to_string(&vec![parquet_url]).unwrap();

        Mock::given(method("GET"))
            .and(path("/datasets/test/repo/parquet/default/train"))
            .respond_with(ResponseTemplate::new(200).set_body_string(url_list))
            .mount(&server)
            .await;
        mount_parquet_endpoint(&server, "/files/0000.parquet", parquet_bytes).await;

        let chunker =
            DatasetChunker::with_char_fallback(ChunkConfigParams::default_kure_v1()).unwrap();
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let service = DatasetIngestService::with_base_url(
            chunker,
            client,
            format!("{}/datasets", server.uri()),
        );

        let request = IngestRequest {
            repo: "test/repo".into(),
            config: "default".into(),
            split: "train".into(),
            text_columns: vec!["persona".into()],
            sample: SampleStrategy::Full,
        };

        let (progress_tx, mut progress_rx) = mpsc::channel(64);
        let (chunks_tx, mut chunks_rx) = mpsc::channel(64);
        let cancel = Arc::new(AtomicBool::new(false));

        let stats = service
            .run(request, progress_tx, cancel, chunks_tx)
            .await
            .expect("service.run");

        assert_eq!(stats.rows_processed, 3);
        assert!(stats.chunks_emitted >= 3, "min 1 chunk per row");
        assert_eq!(stats.urls_fetched, 1);

        let mut stages = Vec::new();
        while let Some(p) = progress_rx.recv().await {
            stages.push(p.stage);
        }
        assert!(stages.contains(&DatasetIngestStage::Manifest));
        assert!(stages.contains(&DatasetIngestStage::Downloading));
        assert!(stages.contains(&DatasetIngestStage::Done));

        let mut chunks = Vec::new();
        while let Some(c) = chunks_rx.recv().await {
            chunks.push(c);
        }
        assert!(chunks.len() >= 3);
        assert!(chunks.iter().any(|c| c.text.contains("페르소나")));
    }

    #[tokio::test]
    async fn service_respects_first_n_sample() {
        let server = MockServer::start().await;
        let parquet_bytes = make_test_parquet(&["row 1", "row 2", "row 3", "row 4", "row 5"]);
        let parquet_url = format!("{}/files/0000.parquet", server.uri());
        let url_list = serde_json::to_string(&vec![parquet_url]).unwrap();

        Mock::given(method("GET"))
            .and(path("/datasets/test/repo/parquet/default/train"))
            .respond_with(ResponseTemplate::new(200).set_body_string(url_list))
            .mount(&server)
            .await;
        mount_parquet_endpoint(&server, "/files/0000.parquet", parquet_bytes).await;

        let chunker =
            DatasetChunker::with_char_fallback(ChunkConfigParams::default_kure_v1()).unwrap();
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let service = DatasetIngestService::with_base_url(
            chunker,
            client,
            format!("{}/datasets", server.uri()),
        );

        let request = IngestRequest {
            repo: "test/repo".into(),
            config: "default".into(),
            split: "train".into(),
            text_columns: vec!["persona".into()],
            sample: SampleStrategy::First { n: 2 },
        };

        let (progress_tx, _progress_rx) = mpsc::channel(64);
        let (chunks_tx, _chunks_rx) = mpsc::channel(64);
        let cancel = Arc::new(AtomicBool::new(false));

        let stats = service
            .run(request, progress_tx, cancel, chunks_tx)
            .await
            .expect("service.run");

        assert_eq!(stats.rows_processed, 2, "First {{n: 2}} → exactly 2 rows");
    }

    #[tokio::test]
    async fn service_cancels_before_run() {
        let server = MockServer::start().await;
        // URL fetch도 실행되기 전 cancel 플래그 → 첫 url 처리 시점에 Cancelled 반환.
        // resolver 응답은 mount 안 함 — Manifest 전 cancel 검증은 url loop 진입 시점에서.
        let parquet_url = format!("{}/files/0000.parquet", server.uri());
        let url_list = serde_json::to_string(&vec![parquet_url]).unwrap();
        Mock::given(method("GET"))
            .and(path("/datasets/test/repo/parquet/default/train"))
            .respond_with(ResponseTemplate::new(200).set_body_string(url_list))
            .mount(&server)
            .await;

        let chunker =
            DatasetChunker::with_char_fallback(ChunkConfigParams::default_kure_v1()).unwrap();
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let service = DatasetIngestService::with_base_url(
            chunker,
            client,
            format!("{}/datasets", server.uri()),
        );

        let request = IngestRequest {
            repo: "test/repo".into(),
            config: "default".into(),
            split: "train".into(),
            text_columns: vec!["persona".into()],
            sample: SampleStrategy::Full,
        };

        let (progress_tx, _progress_rx) = mpsc::channel(64);
        let (chunks_tx, _chunks_rx) = mpsc::channel(64);
        let cancel = Arc::new(AtomicBool::new(true)); // pre-cancel.

        let result = service.run(request, progress_tx, cancel, chunks_tx).await;
        assert!(matches!(result, Err(DatasetImportError::Cancelled)));
    }
}
