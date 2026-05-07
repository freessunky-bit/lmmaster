# ADR-0063 — Dataset Import Pipeline (parquet streaming + chunk + embed + SQLCipher)

* **상태**: Proposed (2026-05-07). Phase 23'.c.2 — 사용자 명시 진입 + 신중 설계.
* **선행**:
  - ADR-0014 (Curated Model Registry) — 큐레이션 thesis.
  - ADR-0024 (Knowledge Stack RAG) — Phase 4.5' RAG 인프라.
  - ADR-0042 (Real Embedder ONNX cascade) — bge-m3 / KURE-v1 / multilingual-e5 임베더.
  - ADR-0035 (SQLCipher Activation) — 암호화 저장.
  - ADR-0061 (Dataset Catalog) — DatasetCategory enum + manifest schema.
  - ADR-0062 (NSFW 데이터셋 정책) — minor_safety_attestation + 라이선스 화이트리스트.
* **결정 노트**: `docs/research/phase-23pc2-dataset-import-decision.md`
* **보강 리서치**: `docs/research/phase-23pc2-dataset-import-reinforcement.md` (Agent 결과 영구화).

## 컨텍스트

사용자 요청 (2026-05-07): 데이터셋 카탈로그 카드 "내 워크스페이스에 추가" 버튼 1번으로 *parquet 다운로드 → chunk → 임베딩 → SQLCipher 저장*까지 자동. 현재는 외부 Python 의존 (`docs/guides/personas-korea-survey-simulation.md` 6-step). 1-click으로 *Python 셋업 0* + *LMmaster 안 완결*.

핵심 충돌 + 해결:
- **외부 통신 0 (ADR-0013)** ↔ HuggingFace parquet 다운로드 필요 → ADR-0026 화이트리스트 (`huggingface.co`) 활용, 사용자 명시 클릭 후만.
- **메모리 효율** ↔ Personas-Korea 1.8GB / rp-opus 2.1GB 통째 로드 X → polars-parquet *streaming* (row group 단위).
- **큐레이션 정체성** ↔ NSFW 데이터셋 import → ADR-0062 minor_safety_attestation 자동 검증 + EULA 재동의 prompt.

## 결정

### 1. 라이브러리 final pick (Agent 보강 리서치 반영)

| 영역 | crate | 이유 |
|---|---|---|
| Parquet streaming | **`parquet` (arrow-rs)** | `ParquetRecordBatchStreamBuilder` + `AsyncFileReader` trait → reqwest range-request 자연 통합. row-group lazy. **projection mask로 5개 컬럼만** 읽기 가능 (Personas-Korea 26 → 5 메모리 ↓). |
| Tokenizer | `tokenizers` (HF Rust, 이미 workspace dep) | KURE-v1 / multilingual-e5와 동일 토크나이저. chunk boundary 정확. |
| Text splitting | `text-splitter` (Benbrandt) | RecursiveCharacter Rust 포팅 + `tokenizers` 기반. 한국어 grapheme/word 인식. |
| Embedder | knowledge-stack `OnnxEmbedder` cascade (production) | Phase 9'.a 그대로. **KURE-v1 default** (한국어 narrative 최적, 118M params 빠름). |
| Store | knowledge-stack `Workspace` SQLCipher + 신규 `datasets` 테이블 | Phase 4.5' 재사용. v1 BLOB brute-force, v1.x sqlite-vec ADR. |

**기각** (Agent 보강 리서치 §1):
- `polars-parquet` — workspace에 polars 미사용, ~30MB 추가 의존, lazy frame 추상화 과잉.
- `parquet2` — async 직접 구성 부담.
- Python sidecar — 페이로드 폭증.

### 2. 신규 crate — `crates/dataset-importer/`

Agent 보강 리서치 §"다음 sub-phase" 권장 — `dataset-catalog`(스키마/큐레이션)와 *별개 crate*. 의존성 분리:
- `dataset-catalog`: schema + manifest validator (현재 crate, 변경 0).
- `dataset-importer` (신규): parquet stream + chunk + indexer pipeline.

```rust
// crates/dataset-importer/src/parquet_stream.rs

struct HfParquetReader {
    url: String,
    client: reqwest::Client,
    total_size: u64,
}

#[async_trait]
impl AsyncFileReader for HfParquetReader {
    fn get_bytes(&mut self, range: Range<usize>) -> BoxFuture<Result<Bytes>> {
        let url = self.url.clone();
        let client = self.client.clone();
        Box::pin(async move {
            let resp = client.get(&url)
                .header("Range", format!("bytes={}-{}", range.start, range.end - 1))
                .send().await?;
            Ok(resp.bytes().await?.into())
        })
    }
    fn get_metadata(&mut self) -> BoxFuture<Result<Arc<ParquetMetaData>>> { /* HEAD + footer */ }
}
```

**HF endpoint**: `huggingface.co/api/datasets/{ds}/parquet/{config}/{split}` — ADR-0026 화이트리스트 적중 (datasets-server 거부, 별도 호스트).

**Rate limit**: 5분 fixed window + 429 시 `RateLimit` 헤더 reset 초 backoff (`backon` workspace dep 재사용).

### 3. 권장 sample size — 10K stratified default

Agent 보강 리서치 §5 throughput 추정:
- KURE-v1 (118M dim 768): CPU ~120 chunks/s, GPU ~600 chunks/s.
- Personas-Korea 100만 row × 평균 800자 ≈ **200만 chunks**.
- KURE-v1 CPU 단독 전체: **4.6시간** (비현실적).
- KURE-v1 GPU 전체: 30~50분.
- *권장 sample 1만 row stratified*: CPU 3분, GPU 1분.

**결정**:
- 기본 sample = **10K rows stratified** (`province × occupation` 균등 분포).
- "전체" 클릭 시 *경고 modal* — "전체는 GPU 30분, CPU 5시간이에요. 정말 진행할래요?".
- 배치 크기 32 (메모리 안전 + GPU 활용 균형). CPU fallback 시 8.

### 4. 신규 모듈 — `crates/dataset-importer/src/pipeline.rs`

```rust
pub struct DatasetIndexer<'a> {
    workspace: &'a Workspace,
    embedder: &'a RealEmbedder,
    chunk_size: usize,    // 기본 512 토큰
    chunk_overlap: usize, // 기본 64 토큰
}

impl DatasetIndexer<'_> {
    /// 1 row text → chunk → embed → SQLCipher store.
    pub async fn index_row(&self, dataset_id: &str, row_index: u64, text: &str) -> Result<usize>;

    /// 진행 상태 콜백 (Tauri event 브릿지).
    pub fn with_progress<F>(self, on_progress: F) -> Self
    where F: Fn(IndexProgress);
}

pub struct IndexProgress {
    pub phase: IndexPhase,  // Download / Parse / Chunk / Embed / Store
    pub current: u64,
    pub total: u64,
    pub eta_secs: Option<u64>,
}
```

### 5. SQLCipher 스키마 확장 (schema_version 2 → 3)

Agent 보강 리서치 §6 권장:

```sql
-- 신규 datasets 테이블 (워크스페이스 종속).
CREATE TABLE IF NOT EXISTS datasets (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    catalog_id TEXT NOT NULL,           -- ADR-0061 카탈로그 entry id
    name TEXT NOT NULL,
    config TEXT,
    split TEXT NOT NULL,
    license TEXT NOT NULL,
    rows_imported INTEGER NOT NULL,
    sample_strategy TEXT NOT NULL,      -- 'full' | 'stratified-{n}' | 'first-{n}'
    minor_safety_attested INTEGER NOT NULL DEFAULT 0,
    eula_accepted_at TEXT,
    imported_at TEXT NOT NULL,
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
);
-- chunks 테이블에 dataset_id + row_index + source_metadata 추가 (NULL = 일반 document).
ALTER TABLE chunks ADD COLUMN dataset_id TEXT REFERENCES datasets(id);
ALTER TABLE chunks ADD COLUMN row_index INTEGER;        -- parquet 원본 row pointer (citation).
ALTER TABLE chunks ADD COLUMN source_metadata_json TEXT; -- {province, occupation, ...} citation 메타.
CREATE INDEX IF NOT EXISTS idx_chunks_dataset ON chunks(workspace_id, dataset_id);
```

**brute-force 한계**: 100만 chunks × 1024 dim × 4B = 4GB BLOB + 1초+ 검색. v1 sample 10K 권장. **v1.x = sqlite-vec PoC + SQLCipher 호환 검증 ADR**.

### 6. IPC — `apps/desktop/src-tauri/src/datasets.rs::dataset_import`

```rust
#[tauri::command]
pub async fn import_dataset_to_rag(
    handle: AppHandle,
    dataset_id: String,
    workspace_id: String,
    sample_size: Option<u64>,    // None = 전체, Some(N) = N row stratified 샘플
    on_progress_event: String,    // Tauri event 이름
    cancel_token: CancellationToken,
) -> Result<DatasetImportSummary, DatasetsApiError>;
```

진행 이벤트 — Tauri `emit_to(window, on_progress_event, IndexProgress)` 매 row group마다.

### 7. UI — Trends.tsx 데이터셋 카드 + DatasetImportDrawer

각 카드에 *"내 워크스페이스에 추가"* 버튼 추가:

1. 클릭 → 모달:
   - **샘플 크기** 슬라이더 (10 / 100 / 1K / 10K / 100K / 전체)
   - **관심사 필터** (Personas-Korea: `province`, `occupation`, `age_band` — 26 컬럼 stratified)
   - **임베딩 모델** 자동 권장 (KURE-v1 / multilingual-e5)
   - **NSFW 데이터셋이면 EULA v2 재동의 prompt** + minor_safety_attestation 표시
2. 진행 다이얼로그 — 5 phase 진행 바 + ETA + cancel.
3. 완료 → toast "X chunks 추가됨" + Workspace > 지식 자료에 자동 카드 등장.

### 8. License / EULA enforcement

- **CC-BY-4.0 (Personas-Korea)** — RAG 검색 결과에 *footnote 자동 표시* (5개 출처: KOSIS / 대법원 / 건강보험공단 / 한국농촌경제연구원 / NAVER Cloud).
- **CC-BY-NC (rp-opus)** — *비상업 EULA 재동의 prompt* (ADR-0062 §5). 사용자 거부 시 import 차단.
- **NSFW (rp-explicit)** — minor_safety_attestation 자동 검증 + 사용자에 표시 ("미성년 콘텐츠 0건 검증 완료").
- 라이선스 메타는 SQLCipher 저장 시 함께 (검색 결과 footnote 출력용).

## 근거

- **polars-parquet streaming**: 2GB+ 안전 + 메모리 1GB 이하 유지. row group 단위 lazy.
- **tokenizers + 임베더 일관**: KURE-v1 토크나이저로 chunk → 같은 임베더로 embed = boundary 정확 + 검색 품질 ↑.
- **knowledge-stack 재사용**: Phase 4.5' production 인프라 + Phase 9'.a Real Embedder cascade. 신규 코드 ↓.
- **NSFW EULA 재동의**: ADR-0062 §5 정합. 사용자 자율성 + 법적 책임 명시.
- **샘플 크기 슬라이더**: 사용자 PC 성능 + 디스크 + 임베딩 시간 trade-off 사용자가 선택.

## 거부된 대안

1. **별도 vector DB (Qdrant / LanceDB / sqlite-vss)** — 기존 SQLCipher BLOB + cosine 충분. 의존성 ↑ 거부. v2.x 검색 성능 측정 후 검토.
2. **Python `datasets` lib sidecar** — Tauri 페이로드 폭증. Rust 직접 polars-parquet.
3. **HF API 직접 fetch (datasets-server X)** — model file resolve가 더 까다로움. datasets-server `/parquet` endpoint 표준.
4. **import 후 *별도* LLM 요약 단계** — chunk 자체가 narrative 텍스트라 추가 요약 불필요. 검색 시점 LLM이 컨텍스트로 활용.
5. **자동 import (사용자 클릭 0)** — 디스크 사용 + 라이선스 동의 강제 X. 1-click이지만 *명시 클릭 필수*.
6. **`text-splitter` 거부 + 자체 chunker** — `text-splitter`가 표준 + `tokenizers`와 호환. 자체 구현 비용 ↑.
7. **chunk size hardcode 512** — 사용자 설정 X. v1은 권장 default + advanced 옵션 v2.x.
8. **임베딩 모델 사용자 선택** — 첫 import는 *자동 권장*만 (KURE-v1 한국어 / multilingual-e5 다국어). 명시 선택은 v2.x.
9. **CC-BY-NC 데이터셋 자동 차단** — `commercial: false` 라벨 + EULA 재동의로 사용자 자율성 보존이 정공.
10. **stratified 샘플링 자체 구현** — Personas-Korea 26 컬럼 기반. 사용자 필터 → SQL WHERE → polars filter chain.
11. **GPU 임베딩 강제** — Real Embedder cascade가 CPU + GPU 자동 분기. 강제 X.
12. **import 진행 multi-window** — 단일 모달 + 진행 바. 다중 import는 *큐* 처리.

## 결과 / 영향

### 신규 산출물
- `crates/dataset-catalog/src/loader.rs` — DatasetLoader (parquet streaming).
- `crates/knowledge-stack/src/dataset_indexer.rs` — DatasetIndexer (chunk + embed + store).
- `apps/desktop/src-tauri/src/datasets.rs` 확장 — `import_dataset_to_rag` IPC + 진행 이벤트.
- `apps/desktop/src/pages/Trends.tsx` — 카드 "내 워크스페이스에 추가" 버튼 + 모달 + 진행 다이얼로그.
- `apps/desktop/src/components/datasets/DatasetImportModal.tsx` — 신규 컴포넌트.
- 단위 테스트 — parquet streaming 100x deterministic, chunk boundary, embed dim, SQLCipher round-trip, EULA 재동의 mock.

### Workspace Cargo.toml
- `polars-parquet = { version = "0.45", default-features = false, features = ["streaming"] }`
- `text-splitter = "0.10"`

### EULA / 라이선스
- EULA v2 갱신 (또는 v1 §7 NSFW 데이터셋 정책 §5에 *비상업 재동의* 포함됨 — 충분).

### 백워드 호환
- 기존 knowledge-stack 흐름 (문서 import) 영향 0.
- SQLCipher 스키마는 *ALTER TABLE ADD COLUMN* (NULL 허용) — 마이그레이션 안전.

## 테스트 invariant (sub-phase별)

### 23'.c.2.a (workspace dep + 골격)
- `cargo build --workspace` ✅
- `polars-parquet` 정상 link.

### 23'.c.2.b (DatasetLoader)
- parquet 1MB sample → row count 일치 (deterministic).
- 100x stream 동일 결과.
- timeout / cancel 정상 동작.

### 23'.c.2.c (DatasetIndexer)
- chunk boundary deterministic (동일 텍스트 100x 동일 chunk 수).
- chunk_size = 512 / overlap = 64 정확.
- 임베딩 dim 일치 (KURE-v1 = 1024).

### 23'.c.2.d (IPC)
- 진행 이벤트 매 row group마다 emit.
- cancel 신호 시 graceful 종료 + 부분 chunks 정리.
- EULA 재동의 거부 시 import 차단.

### 23'.c.2.e (UI)
- 모달 a11y (radiogroup / focus-visible / Esc).
- 샘플 슬라이더 boundary (10 ~ 1M).
- 진행 다이얼로그 prefers-reduced-motion 토큰.

### 23'.c.2.f (License + minor_safety)
- minor_safety_attestation 누락 시 import 차단.
- license 화이트리스트 외 → 경고 + 사용자 명시 동의.
- 검색 결과 footnote 출처 5종 (Personas-Korea) 자동 표시.

## 다음 단계

1. Agent 보강 리서치 결과 영구화 (`phase-23pc2-dataset-import-reinforcement.md`).
2. 결정 노트 6-section 작성.
3. sub-phase 6단계 진입 — 23'.c.2.a → b → c → d → e → f.
4. v0.3.0 ship — 모든 sub-phase + EULA 검증 후.
