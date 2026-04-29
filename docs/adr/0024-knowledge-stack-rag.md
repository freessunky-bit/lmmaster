# ADR-0024: Knowledge Stack (RAG) per-workspace policy

- Status: Accepted
- Date: 2026-04-28
- Related: ADR-0009 (portable workspace), ADR-0010 (Korean-first), ADR-0014 (curated model registry), ADR-0017 (manifest installer), ADR-0020 (self-scan local LLM), ADR-0022 (gateway routing)
- 결정 노트: `docs/research/phase-4p5-rag-decision.md`

## Context

LMmaster는 G1 — Knowledge Stack zero-config RAG — 갭을 다음 차별화 축으로 둔다 (`competitive_thesis` 메모리). 사용자 PC에 흩어진 한국어 기술 문서·노트·스크립트를 워크스페이스별로 격리해 보관하고, 로컬 LLM이 검색·요약하도록 만들 RAG 인프라가 필요하다. 다음 제약을 동시에 만족해야 한다.

1. **외부 통신 0** — Pinecone / OpenAI Embeddings API 등 외부 API 호출 금지 (ADR-0013, ADR-0022 §외부 통신 0).
2. **per-workspace 격리** — workspace A의 chunk가 workspace B 검색에 절대 노출되지 않아야 한다 (ADR-0009 + ADR-0022 per-app data segregation 일관).
3. **한국어 우선** — NFC 정규화 + 한국어 종결 어미(다/까)와 문장 부호(. ! ?)를 동시에 boundary로 인식.
4. **deterministic ground-truth** — 임베딩이 아직 실제 모델로 연결되기 전이라도, MockEmbedder가 sha256 기반 deterministic 벡터를 반환해 unit test/통합 test가 안정적으로 통과해야 한다.
5. **scaffold가 production-grade** — `unimplemented!()` / `panic!()` outside-of-test 금지. 실제 ingest 파이프라인 (Reading → Chunking → Embedding → Writing → Done) 동작.

## Decision

### 1. per-workspace SQLite DB (rusqlite bundled)

각 워크스페이스는 자체 SQLite 파일을 갖는다. `KnowledgeStore::open(path)`로 열고, 스키마는 `workspaces / documents / chunks` 3-테이블이 모두 `workspace_id` 컬럼을 PRIMARY/FK로 강제한다. 모든 query에 `WHERE workspace_id = ?`를 명시해 cross-workspace leak을 차단한다.

### 2. Korean-aware chunker — NFC 후 단락 → 문장 → 글자 윈도

`chunker::chunk_text(input, target_size, overlap)` 단계:

1. **NFC 정규화** (`unicode-normalization`) — 자모 분리(NFD)·완성형(NFC)이 섞여도 동일 chunk로 매칭.
2. **단락 분할** — `\n\n`로 1차 split. 단락이 target_size를 초과하면 다음 단계.
3. **문장 분할** — `. ! ?` + 한국어 종결 어미 `다` `까` 뒤 공백을 boundary로. 여전히 초과 시 다음 단계.
4. **글자 윈도** — char_indices walk로 multi-byte 한글 중간 절단 방지 + overlap만큼 직전 chunk 끝과 겹침.
5. **id = sha256(content prefix)[..16]** — deterministic, 동일 chunk content이면 동일 id.

### 3. Embedder trait + MockEmbedder

```rust
#[async_trait]
pub trait Embedder: Send + Sync {
    fn dim(&self) -> usize;
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, KnowledgeError>;
}
```

- `MockEmbedder { dim: 384 }`은 sha256(text)을 byte 단위로 fold해 [-1, 1] 범위 deterministic f32 벡터로 변환.
- 384 dim은 multilingual-e5-small / bge-small-multilingual과 일치 — 실 모델 swap 시 차원 호환.
- 실 모델 cascade(bge-m3 → KURE-v1 → EXAONE-Embed)는 v1.1 ADR addendum.

### 4. KnowledgeStore — schema + cosine top-k

스키마:

```sql
CREATE TABLE workspaces  (id TEXT PRIMARY KEY, name TEXT NOT NULL, created_at TEXT);
CREATE TABLE documents   (id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, path TEXT NOT NULL,
                          sha256 TEXT NOT NULL, ingested_at TEXT NOT NULL,
                          UNIQUE(workspace_id, sha256),
                          FOREIGN KEY(workspace_id) REFERENCES workspaces(id));
CREATE TABLE chunks      (id TEXT PRIMARY KEY, document_id TEXT NOT NULL, workspace_id TEXT NOT NULL,
                          content TEXT NOT NULL, embedding BLOB NOT NULL,
                          start INTEGER NOT NULL, end INTEGER NOT NULL,
                          FOREIGN KEY(document_id) REFERENCES documents(id),
                          FOREIGN KEY(workspace_id) REFERENCES workspaces(id));
CREATE INDEX idx_chunks_workspace ON chunks(workspace_id);
CREATE INDEX idx_chunks_doc ON chunks(document_id);
CREATE INDEX idx_documents_workspace ON documents(workspace_id);
```

`search(workspace_id, query_embedding, k)`는 `WHERE workspace_id = ?`로 필터된 chunks의 embedding BLOB을 row-by-row decode, cosine 유사도 in-memory 계산, top-k heap. extension 없이 deterministic. ~10K chunks까지 < 100ms.

### 5. IngestService — 진행률 + 협력 cancel

```rust
pub struct IngestService { store: Arc<Mutex<KnowledgeStore>>, embedder: Arc<dyn Embedder> }

ingest_path(workspace_id, path, target_chunk_size, overlap, progress_tx, cancel_token)
    -> Result<IngestSummary, KnowledgeError>
```

- 단계: `Reading → Chunking → Embedding → Writing → Done`. 각 단계 직전 cancel_token 검사.
- `path`가 디렉터리이면 `.md` / `.txt` 재귀 스캔. PDF는 v1.1 (plain text fallback 없음 — `extract_text` 호출자가 처리).
- `progress_tx` (tokio mpsc) — 단계 + processed/total 방출.
- `cancel_token` (Arc<AtomicBool>) — 매 loop iteration / 단계 진입 시 검사.

## Consequences

**긍정**:
- per-workspace DB로 cross-leak이 schema-level로 차단된다 (테스트로 invariant 강제).
- MockEmbedder deterministic으로 unit/integration test가 안정. 실 임베딩 모델 swap이 trait 교체만으로 가능.
- rusqlite bundled로 별도 데몬 / 외부 서비스 없이 portable 배포.
- Korean-aware chunker가 NFC + 종결 어미 boundary로 한국어 RAG retrieval 품질 향상.
- 진행률 mpsc + 협력 cancel로 큰 디렉터리 ingest 시 UX 안정.

**부정**:
- brute-force cosine은 ~10K chunks 이상에서 latency 우려. v1.1에서 sqlite-vec 또는 hnswlib 검토.
- v1은 .md / .txt만 fully 지원. PDF/DOCX는 호출자 책임.
- MockEmbedder는 시뮬레이션 — 실 retrieval 품질은 실 모델 연결 후에야 검증 가능.
- embedding BLOB을 chunks row에 직접 저장 (denormalize) — 다중 모델 비교는 v1.x.

## Alternatives rejected

### a. Vector DB external service (Pinecone / Weaviate / ChromaDB)

거부. (1) Pinecone = 외부 API 호출 (외부 통신 0 위반). (2) ChromaDB persistent = 별도 데몬 + 디스크 점유 (포터블 정책 위반). (3) v1 chunk 규모(~10K)에서 sqlite brute-force cosine으로 충분. **재검토 트리거**: chunk count > 100K + sqlite-vec 실패.

### b. Single global store (모든 워크스페이스 공유 DB)

거부. workspace 격리는 ADR-0022 per-app data segregation 정책의 backbone. 단일 DB + `WHERE workspace_id = ?`로도 격리 가능하지만 backup/migrate/delete가 워크스페이스 단위 atomic으로 안 떨어진다. per-workspace SQLite 파일이 backup·삭제·이주를 원자화한다. **재검토 트리거**: 사용자가 명시적으로 cross-workspace 검색을 요구할 때 (현재 명시 거부).

### c. Embedding API call (OpenAI Embeddings / Cohere)

거부. (1) 외부 통신 0 위반. (2) 사용자 데이터 외부 유출 위험. (3) 한국어 retrieval F1이 multilingual local 모델보다 일정 부분에서 떨어진다 (한국어 SOTA bge-m3 / KURE 우위). MockEmbedder + 실 local 임베딩 모델 cascade로 충분. **재검토 트리거**: 사용자가 명시 opt-in으로 OpenAI Embeddings를 요구할 때 (이때도 외부 통신 0 정책 ADR addendum 별도 필요).

## References

- 결정 노트: `docs/research/phase-4p5-rag-decision.md`
- ADR-0009 (portable workspace) — workspace_id 격리 패턴.
- ADR-0010 (Korean-first) — NFC + 종결 어미 boundary 채택 근거.
- ADR-0013 (Gemini boundary) — 외부 통신 0.
- ADR-0014 (curated model registry) — 임베딩 모델 카탈로그 통합 지점.
- ADR-0017 (manifest installer) — 임베딩 모델 다운로드 패턴.
- ADR-0022 (gateway routing) — per-app data segregation.
- BAAI/bge-m3 — multilingual SOTA.
- LangChain document loaders — kind-based 파서 분기 패턴.
