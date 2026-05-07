# Phase 23'.c.2 — Dataset Import Pipeline 결정 노트

> **작성일**: 2026-05-07
> **선행**: ADR-0063 (`docs/adr/0063-dataset-import-pipeline.md`), 보강 리서치 (`phase-23pc2-dataset-import-reinforcement.md`).
> **트리거**: 사용자 명시 — "스켈레톤화 X, 신중 설계 후 진행 + 엘리트 사례 리서치".

---

## 1. 결정 요약

- **A1**: 라이브러리 final pick — `arrow-rs parquet` + `text-splitter` + `tokenizers` + `OnnxEmbedder` cascade + SQLCipher BLOB. polars 거부.
- **A2**: HF endpoint = `huggingface.co/api/datasets/{ds}/parquet/{config}/{split}` (datasets-server 거부, 화이트리스트 적중).
- **A3**: 신규 crate `crates/dataset-importer` (dataset-catalog와 분리, fetch + chunk + indexer pipeline).
- **A4**: chunk_size 512 / overlap 64 (KURE-v1 native), sample default **10K stratified** (`province × occupation`), 전체 import 시 경고 modal.
- **A5**: SQLCipher 스키마 schema_version 2→3 (`datasets` 테이블 + `chunks.dataset_id/row_index/source_metadata_json` 컬럼).
- **A6**: v1 BLOB brute-force + `idx_chunks_dataset` 인덱스. **v1.x sqlite-vec PoC ADR 신설 후 마이그레이션**.
- **A7**: 5단계 progress (Manifest/Downloading/Chunking/Embedding/Writing) + cancel + 부분 commit + 한국어 해요체.
- **A8**: License/EULA — CC-BY footnote 자동, CC-BY-NC EULA modal, NSFW `minor_safety_attested` hard-stop.

상세는 ADR-0063 §1~8 + reinforcement §1~7.

---

## 2. 채택안 cross-reference

| 영역 | 결정 위치 |
|---|---|
| 라이브러리 final pick | ADR-0063 §1, reinforcement §1, §3 |
| HF endpoint + Range request | ADR-0063 §2, reinforcement §2 |
| 권장 sample size 10K stratified | ADR-0063 §3, reinforcement §5 |
| pipeline IndexPhase 5단계 | ADR-0063 §4, §6, reinforcement §4 |
| SQLCipher 스키마 (datasets 테이블 + chunks 컬럼) | ADR-0063 §5, reinforcement §6 |
| IPC + UI Drawer | ADR-0063 §6, §7, reinforcement §4 |
| EULA / minor_safety enforcement | ADR-0063 §8, reinforcement §7 |

---

## 3. 기각안 + 이유 (negative space — 다음 세션 보호)

ADR-0063 "거부된 대안" 12건 + reinforcement 매트릭스 핵심 6건:

| 기각안 | 거부 이유 |
|---|---|
| **`polars-parquet`** | workspace polars 미사용, ~30MB 의존 추가 + lazy frame 추상화 과잉. arrow-rs `parquet`가 workspace fit. |
| **`datasets-server.huggingface.co/parquet`** | 호스트 다름 → ADR-0026 화이트리스트 추가 필요. `huggingface.co/api/.../parquet`이 적중. |
| **별도 vector DB (Qdrant/LanceDB/sqlite-vss)** | 별도 데몬 또는 columnar 파일 = 추가 storage layer. SQLCipher 단일 파일이 백업 단순함 우위. |
| **`sqlite-vec` v1 즉시 도입** | SQLCipher hook 충돌 위험 (둘 다 SQLite core). 정적 링크 + bundled-sqlcipher 검증 ADR 필요 → v1.x 후속. |
| **자동 import (사용자 클릭 0)** | 디스크 사용 강제 + 라이선스 동의 X. 사용자 명시 클릭 필수. |
| **전체 import default** | 100만 row × 200만 chunks = CPU 5시간. *10K stratified default* + "전체" 클릭 시 경고 modal이 정합. |
| **Python `datasets` lib sidecar** | Tauri 페이로드 폭증. Rust 직접 arrow-rs. |
| **자체 chunker (text-splitter 거부)** | tokenizer 인식 boundary가 RAG 표준. 자체 구현 비용 ↑. |
| **chunk size 사용자 설정 (v1)** | 첫 import는 자동 권장만 (512/64 KURE-v1 native). 명시 선택 v2.x. |
| **임베딩 모델 사용자 선택 (v1)** | KURE-v1 default + cascade fallback. 명시 선택은 v2.x. |
| **GPU 강제** | OnnxEmbedder cascade가 CPU+GPU 자동 분기. 강제 X. |
| **CC-BY-NC 자동 차단** | `commercial: false` 라벨 + EULA 재동의로 사용자 자율성 보존. |

---

## 4. 미정 / 후순위 이월

- **sqlite-vec PoC** — v1.x ADR 신설. SQLCipher hook 충돌 bench 필수. 100만+ chunks 도달 시 자동 마이그레이션 prompt.
- **KSS-style 한국어 sentence splitter** — `text-splitter` grapheme/word fallback이 v1 충분. 종결 어미(`다`/`까`) 정밀 인식은 v1.x.
- **chunk size / overlap 사용자 설정 UI** — v1 advanced settings 미노출. v2.x.
- **임베딩 모델 명시 선택** — KURE-v1 default. 사용자 선택 토글은 v2.x.
- **재개 가능 다운로드** — Range header로 자연 resume 지원. 부분 row group commit 후 재시작은 자동. v1.x에 *명시 "이어 받기"* 버튼.
- **다중 dataset 동시 import** — v1은 큐 직렬화. 동시 N개는 v2.x.
- **stratified 샘플링 advanced 필터** — `province × occupation` 기본. 사용자 정의 SQL WHERE는 v2.x.

---

## 5. 테스트 invariant (sub-phase 6단계별)

ADR-0063 §"테스트 invariant"의 6 sub-phase 정확히:

### 23'.c.2.a — workspace dep + 신규 crate 골격
- `cargo build --workspace` ✅
- `parquet` (arrow-rs) + `text-splitter` 정상 link.
- `crates/dataset-importer` workspace 멤버.
- `cargo clippy --workspace --all-targets -D warnings` ✅.

### 23'.c.2.b — `parquet_stream.rs` (HfParquetReader)
- parquet 1MB sample → row count 일치 (100x deterministic).
- Range header로 row group 단위 lazy fetch.
- timeout / cancel 정상.
- HF 429 응답 시 `RateLimit` 헤더 retry-after honor (`backon` crate).
- projection mask로 5컬럼만 (메모리 절약 검증).

### 23'.c.2.c — `chunker.rs` + `pipeline.rs`
- chunk boundary 100x deterministic (동일 텍스트 동일 chunk 수).
- chunk_size 512 / overlap 64 정확.
- 임베딩 dim 일치 (KURE-v1 = 768).
- 한국어 narrative 정상 (UTF-8 + grapheme).

### 23'.c.2.d — IPC `dataset_import` + 진행 채널
- 5단계 stage 매번 emit (Manifest → Downloading → Chunking → Embedding → Writing → Done).
- cancel 신호 시 graceful + 부분 commit (`partial = true`).
- EULA 재동의 거부 시 import 차단.
- Tauri capabilities ACL 등록 (drift check).

### 23'.c.2.e — UI DatasetImportDrawer
- a11y (radiogroup / focus-trap / Esc / role=dialog).
- 샘플 슬라이더 boundary (10 / 100 / 1K / 10K / 100K / 전체).
- 진행 다이얼로그 prefers-reduced-motion 토큰.
- 한국어 해요체 카피 검증.

### 23'.c.2.f — License + minor_safety enforcement
- minor_safety_attested 누락 시 import 차단.
- license 화이트리스트 외 → 경고 + 사용자 명시 동의.
- CC-BY footnote 자동 (5종 출처 — Personas-Korea).
- CC-BY-NC EULA modal 동의 후만 import.

총 신규 invariant 30+ 예상.

---

## 6. 다음 페이즈 인계 — sub-phase 6단계

### 진입 조건 (모두 ✅ — 사용자 명시 승인 받으면 즉시 진입)
- ADR-0024 / 0026 / 0042 / 0061 / 0062 production.
- knowledge-stack `OnnxEmbedder` cascade — Phase 9'.a.
- `dataset-catalog` crate + 4 시드 entry.
- 본 ADR-0063 + reinforcement + 결정 노트 (이번 세션 완료).

### 6 sub-phase (Phase 23'.c.2.a~f)

| Phase | 제목 | 의존성 | 작업량 | DoD |
|---|---|---|---|---|
| **23'.c.2.a** | workspace dep + `dataset-importer` crate 골격 | 본 ADR | 0.5일 | Cargo.toml + lib.rs + 단위 테스트 1건 + clippy 0 warnings |
| **23'.c.2.b** | `parquet_stream.rs` (HfParquetReader + AsyncFileReader impl) | 23'.c.2.a | 1~2일 | HF endpoint resolve + Range request + `backon` rate limit + 5 invariant |
| **23'.c.2.c** | `chunker.rs` + `pipeline.rs` (text-splitter + OnnxEmbedder + DatasetIngestService) | 23'.c.2.b | 1~2일 | chunk boundary deterministic + 임베딩 batch 32 + 4 invariant |
| **23'.c.2.d** | IPC `dataset_import` + 진행 채널 + cancel token + SQLCipher 마이그레이션 | 23'.c.2.c | 1~2일 | schema 2→3 + Tauri command + capabilities + 5 invariant |
| **23'.c.2.e** | UI `DatasetImportDrawer.tsx` + Trends 카드 버튼 + i18n ko/en | 23'.c.2.d | 1일 | a11y + 한국어 해요체 + 5 invariant |
| **23'.c.2.f** | License + minor_safety + EULA modal 통합 + 운영 가이드 | 23'.c.2.e | 1일 | hard-stop + footnote + CURATION_GUIDE 갱신 + 4 invariant |

**총 작업량**: 6~10일 (1~2주). 한 세션 불가 — 다음 세션부터 분할 진입.

### 위험 매트릭스

| 위험 | 영향 | 완화 |
|---|---|---|
| HF rate limit 429 | 큰 다운로드 중단 | `RateLimit-Retry-After` honor + `backon` exponential backoff |
| 1.8GB 다운로드 중 슬립 | 부분 진행 손실 | Range header 자연 resume + 부분 commit 후 재시작 |
| sqlite-vec future migration | v1.x 호환성 | v1.x ADR 신설 시 SQLCipher hook 충돌 bench 필수 |
| `text-splitter` 한국어 boundary | RAG 검색 품질 | v1 grapheme/word level 충분, KSS-style v1.x 보완 |
| 사용자 디스크 부족 | 4GB+ 다운로드 실패 | import 시작 전 디스크 free space 검사 + 한국어 안내 |
| 임베딩 GPU OOM | 배치 크기 조정 | OnnxEmbedder cascade 자동 fallback (32 → 16 → 8 → CPU) |
| 부분 commit 시 inconsistency | 검색 결과 부정확 | `partial = true` 플래그 검색 시 *명시 표시* + 사용자에 "이어 받기" 권유 |

### 다음 standby (다음 세션 진입 시점)
- Phase 23'.c.2.a 진입 — workspace dep `parquet` + `text-splitter` 추가 + `crates/dataset-importer` 신설.

### 검증 명령 (각 sub-phase 종료 시)
```powershell
.\.claude\scripts\verify.ps1
# 추가:
# - cargo test --package dataset-importer
# - cargo test --package knowledge-stack
# - pnpm exec vitest run apps/desktop/src/components/datasets
```

---

## 출처 (보강 리서치 §References 인용)

`docs/research/phase-23pc2-dataset-import-reinforcement.md` §References — 14개 출처 (arrow-rs / HF rate limits / text-splitter / RAG 2026 / sqlite-vec / Tauri async / KSS / ort / Claude Projects / SQLite RAG).
