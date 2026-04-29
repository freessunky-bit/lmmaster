# Architecture Decision Records

**ADR 작성 규칙**

- 새 결정마다 `NNNN-제목.md` 파일 1개.
- 번호는 시간순 단조 증가(취소된 ADR도 번호 유지, status만 Superseded/Rejected로 변경).
- 변경 시 새 ADR을 만들고 이전 ADR의 status를 갱신한다.
- Section: Status / Context / Decision / Consequences / Alternatives considered / References.
- 새 ADR 작성 시 본 README 인덱스 표를 갱신한다.

## 인덱스 (36건, 2026-04-28 현재)

### Foundation (Phase α)

| 번호 | 제목 | Status |
|---|---|---|
| 0001 | Companion 데스크톱 + localhost gateway 구조 | Accepted |
| 0002 | Desktop shell로 Tauri 2 채택 | Accepted |
| 0003 | Native core/gateway 언어로 Rust + Axum | Accepted |
| 0004 | 런타임 어댑터 패턴 + RuntimeAdapter trait | Accepted |
| 0005 | Primary portable runtime으로 llama.cpp | **Superseded by 0016** |
| 0006 | Gateway 프로토콜 — OpenAI-compatible REST + SSE | Accepted |
| 0007 | v1 로컬 키 매니저 자체 구현 (LiteLLM 비고정) | Accepted |
| 0008 | 메타데이터 저장소 — SQLite + 옵션 SQLCipher | Accepted |
| 0009 | Portable workspace는 manifest 기반 | Accepted |
| 0010 | UI/문서 한국어 우선 (ko-KR 기본) | Accepted |
| 0011 | 데스크톱 앱과 기존 웹앱이 디자인 시스템 공유 | Accepted |
| 0012 | ML Workbench는 Python sidecar, v1 placeholder | **Modified by 0018** |
| 0013 | Gemini API는 한국어 설명용에만, 판정/추천은 deterministic | Accepted |
| 0014 | 모델 레지스트리는 curated remote manifest + 로컬 cache | Accepted |
| 0015 | 타입 공유 — specta + tauri-specta + 빌드타임 codegen | Accepted |

### Pivot v1 (2026-04-26)

| 번호 | 제목 | Status |
|---|---|---|
| 0016 | Wrap-not-replace — LM Studio + Ollama을 v1 primary backend | Accepted (supersedes 0005) |
| 0017 | Pinokio-style manifest + tauri-plugin-shell 외부 앱 자동 설치 | Accepted |
| 0018 | Workbench는 v1 핵심 산출물 — llama-quantize + LLaMA-Factory CLI | Accepted (modifies 0012) |
| 0019 | Always-latest hybrid bootstrap + 자동 업그레이드 정책 | Accepted |
| 0020 | 자가스캔 + 로컬 LLM augmentation (요약만, 판단은 deterministic) | Accepted |

### Phase 1A — Onboarding stack (2026-04-26)

| 번호 | 제목 | Status |
|---|---|---|
| 0021 | Phase 1A 핵심 스택 (plugin / 다운로드 / 하드웨어 probe / 마법사 UX) | Accepted |

### Phase 3' — Gateway routing (2026-04-27)

| 번호 | 제목 | Status |
|---|---|---|
| 0022 | Gateway routing policy — local-first + per-webapp scope + observability | Accepted |

### Phase 5'~6' — v1 출시 핵심 (2026-04-28)

| 번호 | 제목 | Status |
|---|---|---|
| 0023 | Workbench v1 boundary policy — LoRA / Modelfile / 양자화 wrapper | Accepted |
| 0024 | Knowledge Stack RAG — per-workspace SQLite + NFC chunker | Accepted |
| 0025 | Pipelines — gateway-side filter modules + apply_request/response | Accepted |
| 0026 | Auto-Updater — GitHub Releases 1순위 + 6h poll + 사용자 동의 후 다운로드 | Accepted |

### Phase 7' — v1 Release prep (2026-04-28)

| 번호 | 제목 | Status |
|---|---|---|
| 0027 | Release bundler / sign / EULA / telemetry policy | Accepted |

### Phase 8'.0 — Security & Stability hardening (2026-04-28)

| 번호 | 제목 | Status |
|---|---|---|
| 0035 | KeyManager SQLCipher activation + OS keyring secret | Accepted |
| 0036 | Stability — single-instance + panic hook + SQLite WAL | Accepted |
| 0037 | Workbench artifact retention (TTL + size LRU) | Accepted |

### Phase 8'.1 — Multi-workspace UX (2026-04-28)

| 번호 | 제목 | Status |
|---|---|---|
| 0038 | Multi-workspace UX + ActiveWorkspaceContext (ADR-0024 약속 UI 실현) | Accepted |

### Phase 11' — Portable Workspace export/import (2026-04-29)

| 번호 | 제목 | Status |
|---|---|---|
| 0039 | Portable workspace export/import — single zip + AES-GCM key wrap (ADR-0009 6 pillar 약속 완성) | Accepted |

### Phase 12' — In-app guide system (2026-04-29)

| 번호 | 제목 | Status |
|---|---|---|
| 0040 | In-app guide system — NAV 가이드 + 8 섹션 + ? 도움말 + F1 단축키 + first-run toast | Accepted |

### Phase 7'.b — CI 자동화 (2026-04-29)

| 번호 | 제목 | Status |
|---|---|---|
| 0041 | GlitchTip self-hosted telemetry endpoint | Accepted |

### Phase 8'.c — Pipelines extension (2026-04-29)

| 번호 | 제목 | Status |
|---|---|---|
| 0028 | Pipelines hot-reload via ArcSwap | Accepted |
| 0029 | Per-key Pipelines override matrix | Accepted |
| 0030 | SSE chunk transformation policy (supersedes ADR-0025 §감내한 트레이드오프) | Accepted |

### Phase 9' — Real ML wiring (2026-04-29)

| 번호 | 제목 | Status |
|---|---|---|
| 0042 | Real Embedder ONNX cascade (bge-m3 / KURE-v1 / multilingual-e5-small) | Accepted |
| 0043 | Real Workbench — llama-quantize binary + LLaMA-Factory CLI | Accepted |

### Phase 8'.c — Pipelines 확장 (2026-04-28)

| 번호 | 제목 | Status |
|---|---|---|
| 0028 | Pipelines hot-reload via ArcSwap (사용자 토글 즉시 반영) | Accepted |
| 0029 | Per-key Pipelines override (`Scope.enabled_pipelines` 화이트리스트) | Accepted |
| 0030 | SSE chunk transformation (streaming 응답에 PII redact 적용; supersedes ADR-0025 §감내한 트레이드오프) | Accepted (supersedes ADR-0025 §"감내한 트레이드오프" 일부) |

### Phase 9' — Real ML wiring (2026-04-28)

| 번호 | 제목 | Status |
|---|---|---|
| 0042 | Real Embedder cascade — ort + bge-m3 / KURE-v1 / multilingual-e5-small + HuggingFace 다운로드 | Accepted |

## Supersede / Modify 그래프

```
0005 (llama.cpp primary) ──superseded──> 0016 (wrap-not-replace)
0012 (Python sidecar) ──modified──> 0018 (Workbench v1 core)
0025 §"감내한 트레이드오프"(SSE byte-perfect) ──partial-supersede──> 0030 (SSE chunk transformation)
```

## 다음 후보 (Phase 7'.b 이후)

- **ADR-0028** — Gateway audit channel (PipelineLayer ↔ PipelinesState 연동)
- **ADR-0029** — GlitchTip self-hosted endpoint 연결 (텔레메트리 v1.x)

## 참고

- Phase별 ADR 매핑: `docs/PHASES.md`.
- 결정 노트(보강 리서치): `docs/research/`.
- 시간순 변경 이력: `docs/CHANGELOG.md`.
