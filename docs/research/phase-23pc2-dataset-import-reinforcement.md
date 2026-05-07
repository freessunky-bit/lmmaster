# Phase 23'.c.2 — Dataset Import Pipeline 보강 리서치

> **목적**: 데이터셋 카탈로그 카드 1-click → parquet 다운로드 + chunk + 임베딩 + SQLCipher 저장 자동. 외부 Python 의존 0, 기존 knowledge-stack 인프라 90% 재사용.
> **작성일**: 2026-05-07
> **선행 ADR**: ADR-0014 / 0024 / 0026 / 0035 / 0042 / 0061 / 0062.
> **Agent 보강 리서치 결과** (2026-05-07 Agent dispatch).

---

## 핵심 결론 (1줄)

**`arrow-rs parquet::arrow::async_reader` + HF `/api/.../parquet` 엔드포인트 + `text-splitter` + `tokenizers` + 기존 `OnnxEmbedder` cascade + SQLCipher BLOB brute-force (v1) → sqlite-vec migration (v1.x)** — 외부 통신 화이트리스트 위반 0, Python 의존 0, knowledge-stack 인프라 90% 재사용.

---

## 1. Parquet streaming — `arrow-rs` (`parquet` crate) 채택

| 옵션 | 장점 | 단점 | 채택? |
|---|---|---|---|
| **`parquet` (arrow-rs)** | `ParquetRecordBatchStreamBuilder` + `AsyncFileReader` trait → reqwest range-request 자연 통합. row-group 단위 lazy. apache 표준. async 완전. projection mask로 *5개 컬럼만* (Personas-Korea 26 → 5). | 메모리 최적화는 polars만큼 정교하지 않음 (workspace에 polars 미사용이라 무관). | **✅** |
| `polars-parquet` | row group 병렬 + predicate pushdown 강력. | polars-core (~30MB) 추가 의존. lazy frame 추상화는 *데이터셋 한 번 import*에 과잉. arrow 미사용 codebase에 polars 도입은 ADR 수준 결정. | ❌ |
| `parquet2` | unsafe-free + 가장 가벼움. | API 저수준. async 직접 구성 부담. | ❌ |

**채택 근거**: workspace에 이미 `reqwest` + `tokio` + `futures-util`이 있고, arrow-rs `AsyncFileReader::get_bytes(Range<usize>)`는 한 함수만 구현. row group 100~300MB라 1.8GB도 *6~18 row group* 단위 lazy stream. 메모리 peak ≈ *최대 row group + 임베딩 배치* (~500MB).

**보일러플레이트**:
```rust
// crates/dataset-importer/src/parquet_stream.rs
struct HfParquetReader { url: String, client: reqwest::Client, total_size: u64 }
#[async_trait]
impl AsyncFileReader for HfParquetReader {
    fn get_bytes(&mut self, range: Range<usize>) -> BoxFuture<Result<Bytes>> {
        let url = self.url.clone(); let client = self.client.clone();
        Box::pin(async move {
            let resp = client.get(&url)
                .header("Range", format!("bytes={}-{}", range.start, range.end - 1))
                .send().await?;
            Ok(resp.bytes().await?.into())
        })
    }
}
let mut stream = ParquetRecordBatchStreamBuilder::new(reader).await?
    .with_projection(projection_mask)  // 5 columns only
    .with_batch_size(256).build()?;
while let Some(batch) = stream.try_next().await? { /* embed + write */ }
```

---

## 2. HF endpoint — `huggingface.co/api/datasets/{ds}/parquet/{config}/{split}`

| 옵션 | 장점 | 채택? |
|---|---|---|
| `huggingface.co/api/datasets/{ds}/parquet/{config}/{split}` | 직접 URL 리스트. ETag/redirect 자동. **호스트 = `huggingface.co` (ADR-0026 화이트리스트 적중)**. | **✅** |
| `datasets-server.huggingface.co/parquet?dataset=...` | 메타 풍부. | 호스트 다름 → ADR-0026 화이트리스트 추가 필요 (위험). | ❌ |

**중요 발견**: 모든 parquet 파일이 `huggingface.co/datasets/.../resolve/refs%2Fconvert%2Fparquet/...`로 redirect. **이미 화이트리스트 안**. 추가 ADR 변경 0.

**Rate limit**: 5분 fixed window + 429 시 `RateLimit` 헤더 reset 초 backoff. anonymous 1.8GB 단일 다운로드 + range request이라 통상 한도 내. backoff = `backon` crate (workspace dep) 재사용.

---

## 3. Tokenizer + chunking — `text-splitter` + `tokenizers` 채택

**채택**: `text-splitter` 0.x + `tokenizers` (이미 workspace dep). 한국어 호환 ✅ (UTF-8 char + grapheme cluster + Markdown/Text).

**현 LMmaster chunker (`knowledge-stack::chunker`)는 char 기반 fallback** — v1 RAG document(.md/.txt)에 충분. *대량 row 데이터셋 narrative*는 token-aware splitter가 정확 (KURE-v1/multilingual-e5는 *512 token* native, char ≠ token).

**보일러플레이트**:
```rust
// crates/dataset-importer/src/chunker.rs
use text_splitter::{ChunkConfig, TextSplitter};
use tokenizers::Tokenizer;

pub struct DatasetChunker { splitter: TextSplitter<Tokenizer> }
impl DatasetChunker {
    pub fn new(tokenizer_path: &Path, max_tokens: usize, overlap: usize) -> Result<Self> {
        let tokenizer = Tokenizer::from_file(tokenizer_path)?;
        let config = ChunkConfig::new(max_tokens).with_sizer(tokenizer).with_overlap(overlap)?;
        Ok(Self { splitter: TextSplitter::new(config) })
    }
    pub fn chunks<'a>(&'a self, narrative: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.splitter.chunks(narrative)
    }
}
```

**한국어 RAG 권장**:
- **chunk_size = 512 tokens** (KURE-v1 / bge-m3 / multilingual-e5 모두 max=512 native).
- **overlap = 64 tokens (12.5%)** — 2026 RAG 표준 (10~25% 범위 중간).
- KSS-style 한국어 sentence splitter 추가는 v1.x — `text-splitter` grapheme/word level이 한국어 종결 어미(`다`/`까`) 자연 인식.

---

## 4. 1-click UX — Anthropic Claude Projects + Notion Drive 패턴

**모범**:
- **Anthropic Claude Projects > Knowledge**: 파일 업로드 → 4단계 progress (`Reading`→`Indexing`→`Embedding`→`Ready`) inline pill + cancel + 실패 retry.
- **Cohere RAG Connectors / OpenAI Custom GPT Knowledge**: drag-drop + 백그라운드 + 완료 toast.
- **LangChain HuggingFaceDatasetLoader**: code-only — UX 0.

**LMmaster — 5단계 progress**:
```rust
#[serde(rename_all = "kebab-case")]
pub enum DatasetIngestStage {
    Manifest,      // /api/.../parquet 호출
    Downloading,   // parquet row group lazy stream
    Chunking,      // text-splitter
    Embedding,     // OnnxEmbedder cascade
    Writing,       // SQLCipher INSERT
    Done,
}
```

**UX 디테일**:
- Drawer → "추가할게요" 1버튼 → background task spawn (Tauri channel + `runtime-manager` 패턴 reuse).
- 진행률: `processed / total rows` + ETA (`elapsed / processed × remaining`).
- `prefers-reduced-motion` 자동 적용.
- Cancel → `Arc<AtomicBool>` flag (`knowledge-stack::CancelToken` 결).
- 부분 commit — cancel 시점까지 chunks `partial = true` 플래그로 store.
- 한국어 카피 (해요체):
  - 시작: "내 워크스페이스에 추가할게요" / 메타: "약 1.8GB · 한국어 페르소나 100만 개"
  - 진행: "받고 있어요 (3.2/100만)" → "조각으로 나누고 있어요" → "임베딩 만들고 있어요" → "저장하고 있어요"
  - 완료: "100만 개 페르소나를 추가했어요. RAG 검색에 자동 사용해요."
  - 실패: "다운로드를 못 마쳤어요. 인터넷이 잠시 끊겼나 봐요. 다시 시도할래요?"

---

## 5. Embedder throughput — 기존 `OnnxEmbedder` cascade

**현 상태** (`knowledge-stack::embed_onnx`): `ort` 2.0-rc.10 + `tokenizers` 0.20 + `ndarray` + `load-dynamic`.

**Throughput 추정**:
- bge-m3 (568M, dim 1024): CPU AVX2 ~30 chunks/s (batch=8). CUDA RTX 4060 ~600 chunks/s (batch=64).
- multilingual-e5-large (560M, dim 1024): 비슷.
- **KURE-v1 (118M, dim 768)**: 한국어 특화. 3~4× 빠름. CPU ~120 chunks/s.

**Personas-Korea 100만 row × 평균 800자 narrative ≈ 200만 chunks**:
- KURE-v1 CPU 단독: ~4.6시간 — *전체 import 비현실적*.
- KURE-v1 GPU: ~30~50분.
- *권장 sample: 1만 row stratified* → ~2만 chunks → CPU 3분, GPU 1분.

**결정**: 기본 sample = **10K rows** (stratified `province × occupation`). "전체" 클릭 시 *경고 modal* — "전체는 GPU 30분, CPU 5시간이에요. 정말 진행할래요?".

배치 크기 = 32 (메모리 안전 + GPU 활용 균형). CPU fallback 시 8.

---

## 6. SQLCipher 스키마 vs `sqlite-vec` — v1 BLOB, v1.x sqlite-vec

**현 store** (`knowledge-stack::store`): BLOB f32 little-endian + brute-force cosine top-k heap (~10K chunks 안정).

**확장 매트릭스**:

| 옵션 | v1 채택? | 이유 |
|---|---|---|
| **현행 BLOB + brute-force + dataset_id index** | **✅** | 단순함. 10K~100K chunks까지 < 100ms. SQLCipher 호환 ✅. 코드 변경 최소. |
| `sqlite-vec` (`vec0` virtual table) | v1.x 후보 | SQLCipher와 *런타임 충돌 위험* — 둘 다 SQLite 핵심 hook. 정적 링크 + bundled-sqlcipher 환경에서 검증 ADR 필요. 100만+ chunks에서 비로소 의미. |
| LanceDB / Qdrant | ❌ | 별도 데몬 또는 Lance columnar 파일 = 외부 통신 0 정체성과 *추가 storage layer*. SQLCipher 단일 파일 = 백업 단순함의 가치 큼. |

**스키마 확장**:
```sql
-- 신규: datasets 테이블 (워크스페이스 종속).
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
-- chunks 테이블 컬럼 추가 (NULL = 일반 document).
ALTER TABLE chunks ADD COLUMN dataset_id TEXT REFERENCES datasets(id);
ALTER TABLE chunks ADD COLUMN row_index INTEGER;        -- parquet 원본 row pointer (citation).
ALTER TABLE chunks ADD COLUMN source_metadata_json TEXT; -- {province, occupation, ...} citation.
CREATE INDEX IF NOT EXISTS idx_chunks_dataset ON chunks(workspace_id, dataset_id);
```

**brute-force 한계**: 100만 chunks × 1024 dim × 4B = 4GB BLOB + 1초+ 검색. v1 sample 10K 권장. v1.x = sqlite-vec PoC + SQLCipher 호환 검증 ADR.

---

## 7. License / EULA enforcement (ADR-0061/0062 통합)

- **CC-BY-4.0 (Personas-Korea, KREW)**: import 시 `datasets.license` 컬럼 + 검색 결과 footnote (`row_index` → `source_metadata_json.attribution`). UI 카드 푸터에 출처 5개 자동.
- **CC-BY-NC (rp-opus)**: import 직전 EULA modal — "비상업적 사용에만 동의해요" 체크 + `datasets.eula_accepted_at` 기록. 미동의 차단.
- **NSFW (LimaRP)**: `minor_safety_attested = 1` 검증 (ADR-0062 §1) + parquet 다운로드 *전* hard-stop 검사. 미달 시 dataset-catalog 자체 제외.

---

## 결정 포인트 (10건)

| # | 결정 | 채택 |
|---|---|---|
| **D1** | Parquet streaming 라이브러리 | `arrow-rs parquet::arrow::async_reader` + `AsyncFileReader` impl |
| **D2** | HF endpoint | `huggingface.co/api/datasets/{ds}/parquet/{config}/{split}` (화이트리스트 적중) |
| **D3** | Tokenizer-aware chunker | `text-splitter` + `tokenizers` (workspace dep 재사용) |
| **D4** | Chunk size / overlap | 512 tokens / 64 overlap (12.5%) — KURE-v1 native |
| **D5** | 임베딩 cascade | 기존 `OnnxEmbedder` 재사용. KURE-v1 default (한국어 narrative 최적) |
| **D6** | 권장 sample size | 10K stratified (`province × occupation`). full 시 경고 modal |
| **D7** | 벡터 검색 | v1 BLOB brute-force + dataset_id index. v1.x sqlite-vec PoC ADR |
| **D8** | Cancellation | `Arc<AtomicBool>` token + 부분 commit (`partial = true`) |
| **D9** | Progress UX | 5단계 (Manifest/Downloading/Chunking/Embedding/Writing) + ETA + 한국어 해요체 |
| **D10** | License/EULA | CC-BY footnote 자동, CC-BY-NC EULA modal, NSFW minor_safety_attested 검증 |

---

## 다음 sub-phase (Phase 23'.c.2.a~f) 진입 조건

**전제 (모두 ✅)**:
- ADR-0024 (knowledge-stack), ADR-0026 (외부 통신), ADR-0061 (데이터셋 카탈로그), ADR-0062 (NSFW 정책) — production.
- `OnnxEmbedder` cascade — Phase 9'.a production.
- `dataset-catalog` crate + 4 시드 entry — 등록.

**신규 작업**:
1. **신규 crate `crates/dataset-importer`** — `parquet_stream.rs` + `chunker.rs` + `pipeline.rs`.
2. **`knowledge-stack::store` 마이그레이션** — `datasets` 테이블 + chunks 컬럼 (schema_version 2 → 3).
3. **Tauri command `dataset_import`** — cancel token + progress channel (`runtime-manager` 패턴 reuse).
4. **UI Drawer `DatasetImportDrawer.tsx`** + 5단계 progress + i18n ko/en + a11y (focus-trap, Esc, role=dialog).
5. **결정 노트 6-section** (negative space: polars 거부 / sqlite-vec v1 거부 / datasets-server 거부 / 전체 import default 거부).
6. **테스트 invariant**: deterministic chunker 100x, BLOB round-trip, minor_safety hard-stop, cancel partial commit, EULA gate, ETag cache idempotent, projection mask 컬럼 절약.

**위험**:
- HF rate limit → backoff (`backon`) + 429 reset header parse.
- 1.8GB 다운로드 중 사용자 PC 슬립 → reqwest stream resume Range header로 자연 resume + 부분 row group commit 후 재시작.
- sqlite-vec future migration → v1.x ADR 신설 시 SQLCipher hook 충돌 bench 필요.
- `text-splitter` 한국어 boundary → KSS-style sentence splitter 보완 v1.x 후보.

---

## 출처 (Agent 보강 리서치 §References)

- [parquet::arrow::async_reader — arrow-rs](https://arrow.apache.org/rust/parquet/arrow/async_reader/index.html)
- [HuggingFace Hub Rate limits](https://huggingface.co/docs/hub/rate-limits)
- [HuggingFace dataset-viewer / List Parquet files](https://huggingface.co/docs/dataset-viewer/parquet)
- [text-splitter — Benbrandt (GitHub)](https://github.com/benbrandt/text-splitter)
- [Best Chunking Strategies for RAG (2026) — Firecrawl](https://www.firecrawl.dev/blog/best-chunking-strategies-rag)
- [intfloat/multilingual-e5-large — Maximum Chunk Size for RAG](https://huggingface.co/intfloat/multilingual-e5-large/discussions/27)
- [sqlite-vec — asg017 (GitHub)](https://github.com/asg017/sqlite-vec)
- [Using sqlite-vec in Rust — Alex Garcia](https://alexgarcia.xyz/sqlite-vec/rust.html)
- [Tauri 2 long-running async tasks — sneakycrow](https://sneakycrow.dev/blog/2024-05-12-running-async-tasks-in-tauri-v2)
- [ort — pykeio (GitHub)](https://github.com/pykeio/ort)
- [KSS Korean sentence segmentation — hyunwoongko](https://github.com/hyunwoongko/kss)
- [Working with CJK text in Generative AI — tonybaloney](https://tonybaloney.github.io/posts/cjk-chinese-japanese-korean-llm-ai-best-practices.html)
- [Claude Projects guide](https://support.claude.com/en/articles/9519177-how-can-i-create-and-manage-projects)
- [SQLite Retrieval Augmented Generation — Turso](https://turso.tech/blog/sqlite-retrieval-augmented-generation-and-vector-search)
- [Vector Database Benchmarks 2026 — CallSphere](https://callsphere.ai/blog/vector-database-benchmarks-2026-pgvector-qdrant-weaviate-milvus-lancedb)
