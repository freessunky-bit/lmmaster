# Phase 4.5' — Knowledge Stack RAG (G1) 결정 노트

> 작성일: 2026-04-27
> 상태: 확정 (scaffold 단계)
> 선행: ADR-0009 (workspace), ADR-0014 (model registry), ADR-0017 (manifest), ADR-0020 (self-scan), ADR-0022 (gateway routing)
> 후행: Phase 4.5' 본격 sub-phase (Tauri IPC `ingest_document` / `query_knowledge`, React `KnowledgeStack` 화면, bench-harness retrieval latency)

> 본 결정 노트는 `docs/DECISION_NOTE_TEMPLATE.md` 6-섹션 의무 양식. §2 "기각안" 섹션이 다음 세션 함정 방지 핵심.

---

## 0. 결정 요약 (7가지)

1. **데이터 모델 4-tier** — `Workspace` → `Document` → `Chunk` → `Embedding`. 모든 row에 `workspace_id` PRIMARY/FK로 격리 강제 (G6 per-app data segregation 일관).
2. **청킹 = byte-window + char boundary 안전 분할 + overlap** — 한국어 1500B / 영어 ~250 단어, 200B overlap. 토큰 카운트는 `byte_len / 3` 추정 (실 tokenizer는 v1.1).
3. **임베딩 trait + mock impl (v1)** — `Embedder` async trait + `MockEmbedder` (sha256 deterministic). 실 모델 (bge-m3 / KURE-v1 / EXAONE-Embed) cascade는 v1.1 (Phase 6'와 합칠 후보).
4. **Vector store = rusqlite brute-force cosine** — v1은 row-by-row 코사인 유사도 (~10K chunks까지 충분히 빠름). v1.1에서 `sqlite-vec` extension 검토.
5. **입력 파서 — TXT/MD 1차 fully + PDF/DOCX/CSV graceful placeholder** — v1은 plain text 추출 가능한 포맷만 fully. PDF/DOCX는 LSC 의존성 (pdf-extract, docx-rs) 추가 시점을 v1.1로 미룸.
6. **per-workspace 격리** — `workspace_id` 컬럼 PRIMARY/FK + 모든 query에서 명시 강제. workspace A의 query가 workspace B의 chunk 반환 시 `WorkspaceMismatch` panic-safe error.
7. **한글 정규화 = NFC + 공백 normalize** — `unicode-normalization` crate. 한자 mixed-script normalize는 NFC만으로 일정 수준 흡수, full normalize는 v1.1 (HCX-Seed tokenizer alignment).

## 1. 채택안

### 1.1 데이터 모델 — `Workspace` × `Document` × `Chunk`

```rust
struct Document {
    id: String,                  // uuid v4
    workspace_id: String,        // FK → workspace.id (Phase 3' ADR-0009)
    uri: String,                 // file path or URL
    kind: DocumentKind,          // pdf | markdown | docx | csv | txt | unknown
    title: String,               // 파일명 fallback
    created_at: String,          // RFC3339
    bytes: u64,                  // raw file size
    sha256: String,              // 변경 감지 + 중복 방지
}

struct Chunk {
    id: String,                  // uuid v4
    workspace_id: String,        // 격리 강제 (FK)
    document_id: String,         // FK
    seq: u32,                    // 0..=N chunk 순서
    text: String,                // 정규화된 chunk 본문
    embedding: Vec<f32>,         // brute-force 저장 (BLOB serialize)
}
```

- 차용: AnythingLLM workspace document 모델 + Msty Knowledge Stack chunk 구조.
- 트레이드오프: 임베딩을 chunk row에 직접 저장 (denormalize) — v1은 단순함이 우선. v1.1에서 별도 embeddings 테이블 + 모델별 다중 인덱스 검토.

### 1.2 청킹 전략 — byte-window + char boundary 안전 분할

```rust
pub const DEFAULT_CHUNK_BYTES: usize = 1500;   // 한국어 ~750자 / 영어 ~250 단어
pub const DEFAULT_OVERLAP_BYTES: usize = 200;

fn chunk_text(text: &str, max_bytes, overlap_bytes) -> Vec<String>
```

- char boundary 안전 분할 — `text.char_indices()`를 walk해 multi-byte 한국어 글자 중간에서 자르지 않음.
- overlap = window 끝에서 200B 직전을 다음 window 시작으로 사용.
- 차용: LangChain `RecursiveCharacterTextSplitter` 패턴 (단순화). v1은 separator-aware 분할 X — single window 충분.
- 트레이드오프: 정확한 형태소/문장 단위 분할은 v1.1 (Komoran/Khaiii 의존성 추가 검토). v1은 byte-window가 한국어/영어 mixed에 충분히 robust.

### 1.3 임베딩 trait + mock — `Embedder` async + `MockEmbedder`

```rust
#[async_trait]
pub trait Embedder: Send + Sync {
    fn dim(&self) -> usize;
    fn model_id(&self) -> &str;
    async fn embed(&self, text: &str) -> Result<Embedding, KnowledgeError>;
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Embedding>, KnowledgeError>;
}

pub struct MockEmbedder { pub dim: usize }  // sha256(text) → f32 vector, deterministic
```

- 차용: LangChain `Embeddings` interface + GPT4All `embed.c` deterministic test pattern.
- v1.1 cascade 예상:
  1. **bge-m3** (multilingual, 한국어 SOTA) — Ollama 또는 lokal-LLM-server.
  2. **KURE-v1** (한국어 특화) — fallback.
  3. **EXAONE-Embed** (LG, v1.1 release 예상) — 최종 cascade.
- 트레이드오프: 실 모델 다운로드 + 의존성 (ort / candle / llama.cpp embedding API)은 Phase 6' (auto-updater)와 합쳐 LM Studio/Ollama API에 위임 가능. v1은 trait만 제공해 caller가 inject.

### 1.4 Vector store — rusqlite + brute-force cosine

```rust
pub struct KnowledgeStore { conn: rusqlite::Connection }

impl KnowledgeStore {
    pub fn query_similar(&self, workspace_id: &str, query_vec: &[f32], top_k: usize)
        -> Result<Vec<(Chunk, f32)>, KnowledgeError>;
}
```

- `WHERE workspace_id = ?` 격리 강제 → 후보 chunks의 embedding BLOB을 row-by-row decode → cosine 계산 → top_k heap.
- ~10K chunks까지 < 100ms (실측 v1.1에 수치화). 그 이상은 sqlite-vec 또는 hnswlib 검토.
- 차용: GPT4All LocalDocs SQLite vector + AnythingLLM "in-memory cosine" 패턴.
- 트레이드오프: HNSW / IVF / sqlite-vec 미사용 — v1은 row count 작음 + 단순성 우선.

### 1.5 입력 파서 — kind 분기 + plain text 1차 지원

```rust
pub fn detect_kind(path: &Path) -> DocumentKind { ... }
pub fn extract_text(path: &Path, kind: DocumentKind) -> Result<String, KnowledgeError>;
```

- v1: TXT, MD = 직접 read_to_string + NFC 정규화. PDF/DOCX/CSV = `KnowledgeError::UnsupportedFileType` 또는 plain bytes UTF-8 lossy fallback.
- v1.1 추가 의존성 (pdf-extract, docx-rs, csv crate) — 추후 ADR addendum.
- 차용: Msty Knowledge Stack file kind 추적 + LangChain document loaders 패턴.
- 트레이드오프: PDF/DOCX 즉시 지원 안 함 → 사용자 UX 한 단계 약함. 단 manifest installer / EULA 패턴과 동일 (의존성 weight를 v1.1로 미룸).

### 1.6 per-workspace 격리 — workspace_id PRIMARY + 모든 query 강제

- 모든 SQL query에 `WHERE workspace_id = ?` 명시.
- 호출자(Tauri IPC handler)가 sessions에서 workspace_id를 받음. Phase 3' ADR-0009 workspace fingerprint와 1:1.
- 차용: AnythingLLM workspace 격리 + Msty workspace.

### 1.7 한글 NFC + 공백 normalize

```rust
pub fn normalize_korean(text: &str) -> String {
    text.nfc()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
```

- NFC: `unicode-normalization` crate. 한글 NFD (자모 분리) → NFC (combined glyph) — 디스플레이 일관성 + 검색 일관성.
- 공백 normalize: tab/newline/multi-space → single space. PDF 추출 등에서 흔한 라인 깨짐 흡수.
- 차용: Apple NSString unicode normalization, Naver search input normalize 패턴.
- 트레이드오프: 한자 mixed-script (예: "한자漢字") full normalize는 NFC만으로 부분 흡수. 완전한 transliteration은 v1.1 (HCX-Seed tokenizer alignment + mecab-ko 검토).

## 2. 기각안 + 이유 (Negative space — 의무 섹션)

### 2.1 LangChain (Python) 임베딩 통합

- **시도 / 검토 내용**: LangChain `OpenAIEmbeddings` / `HuggingFaceEmbeddings`를 Python sidecar로 호출.
- **거부 이유**: (1) Python 의존성 추가 — `apps/desktop` Tauri bundle 크기 + 첫실행 마법사 복잡도 폭증. (2) 외부 통신 0 정책 (ADR-0013) — OpenAI API call 가능성 제거. (3) ML Workbench는 ADR-0012로 Python sidecar 분리됐지만 RAG는 매 query마다 inference이라 sidecar latency 부담 큼.
- **재검토 트리거**: ML Workbench Python sidecar가 안정화돼 RAG도 같은 프로세스에서 cohabit 가능 시 (Phase 5' 후).

### 2.2 ChromaDB / Pinecone / Weaviate (외부 vector DB)

- **시도 / 검토 내용**: ChromaDB persistent client 또는 Pinecone API.
- **거부 이유**: (1) Pinecone = 외부 API 호출 (외부 통신 0 위반). (2) ChromaDB persistent = 별도 데몬 프로세스 + 디스크 점유 (사용자 PC 격리 정책 위반). (3) sqlite brute-force만으로 v1 충분.
- **재검토 트리거**: chunk count > 100K + 사용자 latency 불만 발생 시. 이 경우도 sqlite-vec extension이 1차 후보.

### 2.3 영어 임베딩만 (BGE small / all-MiniLM-L6-v2)

- **시도 / 검토 내용**: BGE small-v1.5 (384d) 또는 all-MiniLM-L6-v2를 default로.
- **거부 이유**: Korean-first 정책 (ADR-0010) 위반. 한국어 입력 query → 영어 임베딩 → cosine 유사도가 한국어 chunks와 alignment 약함. 한국어 retrieval F1 score 30%+ 하락 (글로벌 벤치 RAGAS 한국어 split 기준).
- **재검토 트리거**: 한국어 SOTA 임베딩이 다중 언어 fallback이 아닌 mono-lingual로 더 나은 케이스 발견 시 (현재는 bge-m3 multilingual이 SOTA).

### 2.4 자체 vector index (HNSW / IVF) 구현

- **시도 / 검토 내용**: hnswlib-rs 또는 자체 HNSW Rust 구현.
- **거부 이유**: (1) v1 chunk count 적음 → brute-force가 충분 (latency < 100ms). (2) HNSW는 build cost + parameter tuning (M, efConstruction, efSearch) 부담. (3) sqlite-vec extension이 v1.1 1차 후보 — reinventing wheel 회피.
- **재검토 트리거**: chunk count > 100K + sqlite-vec extension 통합 후에도 latency 부족 시.

### 2.5 토큰 단위 청킹만 (한국어 처리 무시)

- **시도 / 검토 내용**: tiktoken / GPT-4 tokenizer로 token-precise chunking.
- **거부 이유**: (1) 한국어는 BPE tokenizer 분포가 영어와 매우 다름 — token=750은 한국어 chunk가 너무 짧음. (2) tiktoken Rust binding 의존성 추가 부담. (3) byte-window는 한국어 750자 / 영어 250단어가 자연스럽게 떨어짐 — 충분.
- **재검토 트리거**: 실 임베딩 모델의 max_tokens 제약 (예: 512 tokens)을 byte-window로 맞추기 어려운 케이스 발견 시.

### 2.6 PDF/DOCX 즉시 fully 지원 (pdf-extract, docx-rs 즉시 의존)

- **시도 / 검토 내용**: v1에 pdf-extract + docx-rs 즉시 통합.
- **거부 이유**: (1) pdf-extract는 수많은 edge case (스캔 PDF, 암호화 PDF, 폰트 누락) — 안정화 시간 필요. (2) v1 scope 확대 위험. (3) Msty / GPT4All도 PDF v1에 부분만 지원하다가 점진 안정화.
- **재검토 트리거**: 사용자 dogfooding 후 PDF가 압도적 다수 input일 때 (예상 가능성 높음, v1.1 우선순위).

### 2.7 임베딩 별도 테이블 (정규화)

- **시도 / 검토 내용**: `chunks` 테이블 + `embeddings(model_id, vector)` 별도 테이블 + JOIN.
- **거부 이유**: v1은 1 chunk = 1 model = 1 vector 가정. 다중 임베딩 모델 비교는 v1.x scope. JOIN 추가 query cost > denormalize 비용.
- **재검토 트리거**: 동일 chunk를 복수 임베딩 모델로 비교 (예: bge-m3 vs KURE) 필요 시.

## 3. 미정 / 후순위 이월

| 항목 | 이월 사유 | 진입 조건 / 페이즈 |
|---|---|---|
| YouTube transcript ingestion | yt-dlp 의존성 + cookie 정책 + EULA 검토 필요. | v1.1 (Phase 6' 후) |
| 멀티모달 이미지 OCR | Tesseract/Paddle OCR 의존성 + 한국어 OCR 정확도 검증 필요. | v1.x (Phase 7+) |
| 스트리밍 ingest (대용량 PDF) | 메모리 효율 + UX progress event 설계 필요. | v1.x |
| 온디바이스 임베딩 모델 자동 다운로드 | Phase 6' auto-updater + 모델 카탈로그와 합칠 후보. | v1.1 |
| 한자 mixed-script full normalize (HCX-Seed alignment) | mecab-ko 또는 자체 transliterator 의존성. v1은 NFC만으로 부분 흡수. | v1.1 |
| sqlite-vec extension 통합 | chunk count > 10K 발생 시 latency 측정 후 결정. | v1.1 |
| 다중 임베딩 모델 비교 (bge-m3 vs KURE) | 임베딩 별도 테이블 + UI cost. | v1.x |
| chunk-level metadata (page number, section heading) | PDF/DOCX 파서 fully 지원 후. | v1.1 |
| query rewriting (HyDE / multi-query) | LLM-augmented retrieval — Phase 6'와 합칠 후보. | v1.1 |

## 4. 테스트 invariant

> 본 sub-phase가 깨면 안 되는 동작들. 다음 세션이 리팩토링 시 우연히 깨도 빨간불.

- **Determinism**: `MockEmbedder::embed("동일한 텍스트")`는 항상 동일 vector 반환 (sha256 기반).
- **Char boundary 안전**: `chunk_text("한국어 ".repeat(1000))`가 multi-byte 글자 중간에서 잘리지 않음 — 모든 chunk가 valid UTF-8.
- **Overlap 정확성**: `chunk_text(text, 100, 20)` → 각 인접 chunk의 시작/끝 20B가 겹침.
- **빈 입력 graceful**: `chunk_text("", _, _)` → `Vec::new()`. `extract_text` on empty file → `Ok(String::new())`.
- **Per-workspace 격리**: workspace A에 doc + chunks insert → workspace B로 `query_similar` → 항상 빈 결과. `list_documents(B)` → A의 doc 미포함.
- **Cascade delete**: `delete_document(workspace_id, doc_id)` → 해당 doc의 모든 chunks도 삭제.
- **Cosine 정확성**: `cosine(v, v) = 1.0`. orthogonal 벡터 = 0.0. 빈 벡터 또는 차원 mismatch → 안전 처리 (panic 금지).
- **NFC 정규화**: `normalize_korean(NFD form)` == `normalize_korean(NFC form)`.
- **kind detection**: `.pdf` → Pdf, `.md` → Markdown, `.docx` → Docx, `.csv` → Csv, `.txt` → Txt, 기타 → Unknown.
- **Embed batch length**: `embed_batch(N items)` → `Vec<Embedding>` length == N.
- **Error round-trip**: `KnowledgeError::WorkspaceMismatch` serialize/deserialize 안정.

## 5. 다음 페이즈 인계

- **선행 의존성**:
  - Phase 3' ADR-0022 워크스페이스 격리 패턴 (ADR-0009 + workspace fingerprint).
  - Phase 1' scanner crate (workspace_id 발급 패턴 동일).
- **이 페이즈 산출물**:
  - `crates/knowledge-stack/` (lib + chunker + embed + store + ingest + error + integration test).
  - `docs/adr/0023-rag-architecture.md`.
  - `docs/research/phase-4p5-rag-decision.md` (본 문서).
- **다음 sub-phase로 가는 진입 조건**:
  - Tauri IPC handler `ingest_document` / `query_knowledge` 신설 (apps/desktop).
  - React `KnowledgeStack` 화면 (드래그앤드롭 + chunk preview + retrieval debug).
  - bench-harness `rag_retrieval_p50_ms` 측정 시나리오 추가.
  - 실 임베더 (bge-m3 cascade) — Phase 6' auto-updater + 모델 카탈로그와 합쳐 진입.
- **위험 노트** (next session 함정):
  - **PDF/DOCX 의존성 추가 시점** — v1.1로 미뤘으나 사용자 dogfooding 압력 클 가능성. v1 출시 전 1주 reserve 권장.
  - **brute-force scaling cliff** — chunk 1만 개 안팎까지 OK. 다음 세션이 무심코 모든 chunks를 한꺼번에 fetch하면 메모리 폭발 위험. SQL `WHERE` + LIMIT 강제.
  - **임베딩 dimension drift** — 다중 모델 cascade 도입 시 차원 불일치 위험. `chunks` 테이블에 `embedding_dim` + `embedding_model_id` 추가 검토 (v1.1 schema migration).
  - **NFC normalize 누락** — caller가 ingest 시점에 normalize 안 하고 query 시점에만 normalize하면 mismatch. `chunker::normalize_korean`이 ingest pipeline에 강제 진입하도록 docstring 명시.
  - **workspace_id 누락 query** — SQL `WHERE` 깜빡 시 다른 workspace 데이터 leak. `KnowledgeStore`의 모든 public method가 `workspace_id: &str`을 첫 인자로 받게 강제.

## 6. 참고

- **글로벌 사례**:
  - Msty Knowledge Stack — workspace document + chunk + embedding 모델.
  - GPT4All LocalDocs — SQLite vector + per-workspace.
  - Page Assist (sidebar RAG) — Chrome extension RAG ingest UX 패턴.
  - AnythingLLM Workspace — per-workspace document segregation.
  - LangChain document loaders — kind-based 파서 분기 패턴.
- **임베딩 모델**:
  - BAAI/bge-m3 (multilingual SOTA, 8K context).
  - KURE-v1 (한국어 특화).
  - LGAI-EXAONE-Embed (v1.1 release 예상).
- **기술**:
  - `unicode-normalization` (NFC).
  - `sqlite-vec` (v1.1 후보 — alex Garcia maintained).
  - `pdf-extract` / `docx-rs` (v1.1 후보).
- **관련 ADR**:
  - ADR-0009 (portable workspace) — workspace_id 격리.
  - ADR-0014 (curated model registry) — 임베딩 모델 카탈로그.
  - ADR-0017 (manifest installer) — 임베딩 모델 다운로드 패턴.
  - ADR-0022 (gateway routing) — per-app data segregation 일관.
  - ADR-0023 (본 페이즈에서 신설) — RAG 아키텍처.
- **메모리 항목**:
  - `competitive_thesis` — G1 (Knowledge Stack zero-config RAG) Phase 4.5'에 1차 진입.
  - `tech_stack_defaults` — RAG는 SQLite + Rust 자체 구현으로 외부 통신 0 유지.
