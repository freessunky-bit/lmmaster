# ADR-0042 — Real Embedder: ONNX Runtime + bge-m3 / KURE-v1 / multilingual-e5 cascade

- **Status**: Accepted
- **Date**: 2026-04-28
- **Phase**: 9'.a (Real ML wiring — Embedder)
- **Supersedes**: ADR-0024 §3 "v1.x cascade는 placeholder"

## 결정 요약

- ort = "2.0.0-rc.10" + load-dynamic feature로 3-모델 cascade(bge-m3 / KURE-v1 / multilingual-e5-small) 채택.
- 모델 다운로드는 HuggingFace `huggingface.co` 화이트리스트, 사용자 명시 클릭으로만 호출.
- ort/tokenizers/ndarray는 `embed-onnx` feature gate (default off) — baseline build 부담 0.
- 미설치 / 미다운로드 / feature off 모든 분기는 graceful 한국어 안내 + MockEmbedder fallback.
- 사용자 카드 UI(`EmbeddingModelPanel.tsx`)에서 활성 모델 라디오 선택 + 진행률 progressbar.

## Status

Accepted. Phase 9'.a 구현 진입 시점부터 production embedder 후보 중 ort 기반 3-모델 cascade를
표준 채택해요. v1.x 진행 중에 새 모델이 등장하면 manifest table에만 추가하면 돼요.

## Context

`crates/knowledge-stack`의 `MockEmbedder`는 sha256 deterministic vector를 만들어요. RAG 파이프라인은
완성됐지만 의미 ranking이 없어 search 결과가 사실상 무작위에 가까워요. 한국어 특화 검색 정확도를
확보하려면 실제 한국어 임베딩 모델이 필요해요.

세 가지 후보를 검토했어요.

1. **bge-m3** (BAAI/bge-m3) — 1024d, multilingual, Korean+English 혼합 강력. ~580MB.
2. **KURE-v1** (nlpai-lab/KURE-v1) — 768d, 한국어 특화. ~450MB.
3. **multilingual-e5-small** (intfloat/multilingual-e5-small) — 384d, 가벼움. ~120MB.

세 모델 모두 HuggingFace에 ONNX 형태로 호스팅돼 있어 Rust ONNX Runtime (`ort` crate)에서 직접 로드 가능해요.

## Decision

세 모델을 **사용자 선택 기반 cascade** 형태로 모두 지원해요. 활성 모델은 `<app_data_dir>/embed/active.json`에
영속하고, 다운로드된 파일은 `<app_data_dir>/embed/models/<kind>/{model.onnx,tokenizer.json}`에 저장해요.

### 구현 결정

- **`ort = "2.0.0-rc.10"` + `load-dynamic` feature**: Tauri bundle에 ONNX Runtime을 정적 링크하지
  않아요. 사용자 PC에 `onnxruntime` 동적 라이브러리가 없으면 `OnnxEmbedder::from_files`가 graceful
  한국어 에러를 반환해요 (panic X). RAG 자체는 MockEmbedder fallback으로 계속 동작해요.
- **`tokenizers = "0.20"`**: HuggingFace Rust tokenizer. truncation max_len=512 표준 적용.
- **`ndarray = "0.16"`**: ort 텐서 입출력 — 2D 배치 텐서.
- **Feature gate `embed-onnx`**: ort/tokenizers/ndarray는 옵셔널 dep로 묶어 baseline build (feature off)
  부담을 0으로 유지해요. CI smoke build에서도 RAG 파이프라인을 검증할 수 있어요.
- **다운로드 인프라**: `embed_download.rs`는 항상 빌드돼요 (reqwest + futures + bytes만). HuggingFace
  `huggingface.co` 도메인만 화이트리스트 — 외부 통신 0 원칙 예외(사용자 명시 클릭으로만 호출).
- **사용자 UX**: `EmbeddingModelPanel.tsx`가 3카드 + 진행률 progress bar + 활성 라디오. 첫 ingest 전에
  사용자가 모델을 선택·받도록 강제. 안내 문구: "검색 정확도를 위해 모델을 받아주세요. (~600MB)".
- **Mean pooling + L2 normalize**: bge-m3 / KURE-v1 / multilingual-e5 모두 동일 inference 패턴. Pre-pooled
  output (rank=2)도 자동 감지해 그대로 사용.

### 외부 통신 정책

`huggingface.co`는 ADR-0026 §1의 외부 통신 0 정책 예외로 추가해요. **사용자 명시 클릭(받을게요 버튼)으로만**
호출되며, 백그라운드 자동 fetch는 없어요. 모든 다운로드는 sha256 무결성 검증을 거쳐요 (manifest hash가 v1
에선 None이라 검증 skip — UI에 경고 의무. v1.x에서 자체 manifest endpoint로 hash 주입).

## Alternatives Considered

### 거부 1: OpenAI / Cohere embedding API 직접 호출

OpenAI `text-embedding-3-small` 등 외부 API는 한국어 품질이 우수하지만 **외부 통신 0 원칙(ADR-0026)** 위반.
사용자 데이터(검색 query)가 외부 서버로 흘러가요. **거부**.

### 거부 2: Sentence-Transformers Python sidecar

`sentence-transformers` Python 패키지 + 자체 sidecar로 inference 위임. 모델 품질은 우수하지만:

- 콜드 스타트 5~10초 (Python interpreter + torch import).
- Python 환경 부담 (uv/pip 부트스트랩 필수).
- Tauri sidecar 추가 의존성 (이미 Phase 9'.b에서 LLaMA-Factory가 Python sidecar 도입 예정).

**거부**. 콜드 스타트 + 환경 부담이 사용자 경험을 크게 해쳐요. ort native가 우월.

### 거부 3: llama.cpp embedding endpoint

`llama-server`의 `/v1/embeddings` endpoint를 RAG embedder로 활용. 장점은 Phase 1A 부트스트랩이 끝나면
바로 사용 가능. 그러나:

- llama.cpp 임베딩 모델은 한국어 미세 조정 부족 (대부분 영어 중심).
- bge-m3 GGUF 변환본이 있긴 하지만 native ONNX 대비 latency 손실 + tokenizer mismatch 위험.

**거부**. ranking 품질이 직접 ONNX보다 떨어져요.

### 거부 4: 단일 모델만 (bge-m3 또는 KURE-v1) 강제

사용자 카드 UI 없이 "한 모델만 자동 다운로드" 단순화. 장점은 UX 마찰 0. 그러나:

- bge-m3 580MB은 모든 PC에 부담 (특히 약한 SSD / 작은 디스크).
- KURE-v1만 강제하면 영어 비중 높은 자료에서 성능 손실.
- 사용자 선택권은 LMmaster의 핵심 가치(competitive_thesis #5).

**거부**. 3-모델 카드 UI가 사용자 친화 + 선택권 보존.

### 거부 5: 모델 파일 bundle 동봉

설치 패키지에 모델 ONNX를 동봉. 장점은 첫 실행 즉시 검색 가능. 그러나:

- 설치 패키지 크기 +500MB~1.2GB → CDN/배포 비용 상승.
- 사용자가 원하는 모델만 받는 선택권 손실.
- 자동 업데이트 시 모델 hash 체크 부담.

**거부**. v1은 첫 실행 후 사용자가 ingest 버튼 누르기 전 모델 다운로드를 받도록 유도.

## Test invariants

`crates/knowledge-stack/src/embed_download.rs`:
- `kind_kebab_round_trip` — `OnnxModelKind` ↔ kebab-case 직렬화/역직렬화 정합성.
- `manifest_uses_huggingface_only` — 모든 manifest URL이 `huggingface.co/`로 시작.
- `is_downloaded_*` — 두 파일 모두 존재 시에만 true.
- `download_event_kind_kebab_serialization` — IPC channel 메시지 schema (kind=started/progress/...).
- `download_one_writes_file_and_emits_progress` — wiremock으로 1MB 모델 다운로드 + Started/Progress/Verifying 이벤트.
- `download_one_skips_when_final_exists` — idempotent 분기는 외부 호출 없이 통과.
- `download_one_cancel_mid_stream_preserves_partial` — cancel은 final 파일 안 만듦 + KnowledgeError::Cancelled.
- `download_one_bad_status_returns_error` — 404 시 한국어 메시지.
- `download_one_sha256_mismatch_removes_partial` — 무결성 실패 시 .partial 정리.
- `verify_files_sha256_*` — None expected는 통과, mismatch 시 한국어 에러.

`crates/knowledge-stack/src/embed_onnx.rs` (`embed-onnx` feature):
- `from_files_returns_error_when_model_missing` — graceful 한국어 에러 (panic X).
- `from_files_returns_error_when_tokenizer_missing` — 동일.
- `try_load_from_dir_propagates_missing` — directory-level helper도 동일 정책.
- `mean_pool_*` — pure mean pooling 정확도.

`crates/knowledge-stack/src/embed.rs`:
- `default_embedder_*` — preferred_kind + fallback_to_mock 4가지 분기.

`apps/desktop/src-tauri/src/knowledge.rs`:
- `embedding_state_*` — list / set_active 영속 / cancel / 동시 다운로드 거부.
- `knowledge_api_error_*` — 새 에러 variant kebab-case 직렬화.

`apps/desktop/src/components/workspace/EmbeddingModelPanel.test.tsx`:
- 3 카드 렌더 + korean_score 정렬.
- Download 버튼 클릭 → IPC 호출.
- 진행률 progressbar aria-valuenow 반영.
- 실패 이벤트 → alert role.
- Activate 버튼 → setActiveEmbeddingModel 호출.
- Active 카드 → aria-checked=true.
- Cancel 버튼 노출 + handle.cancel() 우선.

## References

- HuggingFace Hub `resolve/main` 패턴: <https://huggingface.co/docs/hub/api>.
- `ort` crate v2 docs: <https://ort.pyke.io/>.
- `tokenizers` Rust crate: <https://github.com/huggingface/tokenizers/tree/main/tokenizers>.
- bge-m3 paper: Chen et al., "BGE M3-Embedding: Multi-Lingual, Multi-Functionality, Multi-Granularity Text Embeddings", 2024.
- KURE-v1 model card: <https://huggingface.co/nlpai-lab/KURE-v1>.
- multilingual-e5: Wang et al., "Multilingual E5 Text Embeddings", 2024.
- ADR-0024 (Knowledge Stack RAG) — embedder trait + per-workspace SQLite.
- ADR-0026 (Auto-Updater Source) — 외부 통신 0 정책 예외 패턴.
- `crates/installer/src/downloader.rs` — sha256 streaming + atomic rename + cancel 패턴 재활용.

## Open follow-ups (v1.x)

- **GPU acceleration**: ort `cuda` / `directml` execution provider feature gate. 현재는 CPU only.
- **Manifest sha256 endpoint**: `huggingface.co/<repo>/raw/main/sha256.txt` (또는 자체 mirror) 운영해
  무결성 검증 활성. v1은 None이라 사용자 경고 의무.
- **Range resume**: `.partial` 파일 기반 Range header resume (현재는 처음부터 새로).
- **추가 모델**: KoSimCSE / EXAONE-Embed 등이 ONNX 호스팅되면 manifest table에 추가만 하면 자동 노출.
- **Cascade 자동 선택**: VRAM/RAM 자가스캔 결과 + 한국어 비중 휴리스틱으로 자동 선택 첫 실행 안내.
