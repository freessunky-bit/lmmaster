# PROGRESS — LMmaster v1 진행 대시보드

> 1-page 진행도. 자세한 시간순 이력은 `docs/CHANGELOG.md`, 페이즈 전략은 `docs/PHASES.md`, 인계 노트는 `docs/RESUME.md`.

**마지막 갱신**: 2026-04-29 (Phase 8'.0 + 8'.1 + 11' + 12' 완료 — **v1 ship 전 권장 코드 작업 100%**. 사용자 결정 6건 + Phase 7'.b 자동화만 남음.)

## 6 Pillar (제품 약속) 상태

| Pillar | 상태 | 핵심 산출물 |
|---|---|---|
| 자동 설치 | ✅ | Phase 1A.1~1A.3 — Ollama silent + LM Studio open_url + manifest installer + dual zip-slip 방어 |
| 한국어 | ✅ | Phase 4 + en audit — 해요체 일관 + Linear/Stripe en 톤 + i18n ko/en 동기 |
| 포터블 | ✅ | `crates/portable-workspace` + manifest 기반 |
| 큐레이션 | ✅ | Korean preset 109건(7 카테고리) + curated model registry |
| 워크벤치 | ✅ | Phase 5'.a/b/c/d/e — 5단계 state machine + 실 HTTP (Ollama + LM Studio) + ollama create shell-out |
| 자동 갱신 | ✅ | Phase 6'.a/b 완료 — auto-updater + Settings 토글 + JetBrains-style toast |

## 누적 검증 (2026-04-28 현재)

- **cargo**: 845 / 0 failed / 1 ignored (macOS-only dmg)
- **vitest**: 251 / 0 failed (26 files)
- **합계**: **1096 tests / 0 failed**
- **clippy**: 0 warnings (workspace --all-targets)
- **fmt**: 0 diff
- **tsc**: 0 errors

## 산출 자산

| 카테고리 | 갯수 | 비고 |
|---|---|---|
| Crates | 22 | 4 신규(workbench-core / knowledge-stack / auto-updater / pipelines) |
| ADR | 26 (0001~0026) | 0005 superseded by 0016, 0012 modified by 0018 |
| 결정 노트 | 34 | `docs/research/` |
| Korean preset | 109 | 7 카테고리, 의료/법률 disclaimer 강제 |
| React 페이지 | 10 | Home/Catalog/ApiKeys/Workspace/Install/Runtimes/Projects/Workbench/Diagnostics/Settings |

## v1 진행도 (페이즈 단위)

```
Phase α  Foundation docs        ████████████████████ 100%
Phase 0  Tauri+Axum boot        ████████████████████ 100%
Phase 1' Bootstrap+Self-scan    ████████████████████ 100%
Phase 1A Onboarding wizard      ████████████████████ 100%  (a~e + d.1~d.3)
Phase 2' Catalog+Recommender    ████████████████████ 100%  (a/b/c)
Phase 3' Gateway routing        ████████████████████ 100%
Phase 4  10 화면 + presets      ████████████████████ 100%  (a~h + cleanup + en audit)
Phase 4.5' RAG (knowledge)      ████████████████████ 100%  (.a + .b)
Phase 5' Workbench              ████████████████████ 100%  (a/b/c/d/e ✅)
Phase 6' Pipelines + Updater    ████████████████████ 100%  (a/b/c/d ✅)
Phase 7' v1 Release prep        ████████████░░░░░░░░  60%  (.a scaffold ✅, 사용자 결정 + .b 자동화 대기)
```

## v1 ship 전 남은 태스크 (3건)

| 페이즈 | 내용 | 규모 | 상태 |
|---|---|---|---|
| **사용자 결정 6건** | OV cert / Apple Dev / minisign keypair / repo URL / EULA 법무 / publisher 명 | (구매·결정만) | ⏳ 사용자 |
| **Phase 8'.0** | ~~SQLCipher / single-instance / panic hook / WAL / artifact retention~~ | ✅ 완료 (2026-04-29) | ✅ |
| **Phase 8'.1** | ~~Multi-workspace UX (ADR-0024 약속 실현)~~ | ✅ 완료 (2026-04-29) | ✅ |
| **Phase 11'** | ~~Portable workspace export/import (6 pillar "Portable" 약속 실현)~~ | ✅ 완료 (2026-04-29) | ✅ |
| **Phase 12'** | ~~Guide / Help system~~ | ✅ 완료 (2026-04-29) | ✅ |
| **Phase 7'.b** | release.yml CI matrix + minisign 자동 서명 + GlitchTip endpoint + 베타 토글 + README 다국어 | 4-5 sub-agent | ⏳ 사용자 결정 후 |

**v1 코드 100% 완료** — Phase α / 0 / 1' / 1A / 2' / 3' / 4 / 4.5' / 5' / 6' / 7'.a 전부 ✅. 잔재 audit 추가 발견(2026-04-29) — Phase 8'.0/8'.1 v1 ship 전 권장.

## v1.x 후속 (출시 후, v1 범위 밖)

- 4.5'.c — 실 Embedder (bge-m3 / KURE-v1 cascade), `MockEmbedder` 교체
- ApiKeys per-key Pipelines matrix UI (ADR-0025 §3)
- Streaming chunk transformation (현재 SSE는 byte-perfect pass-through만)
- Catalog가 `listCustomModels`로 사용자 정의 모델 노출
- `KnowledgeStore::get_document_path` 헬퍼 — `SearchHit.document_path` 실 경로 표시
- `tauri-plugin-shell` 정식 도입 (현재 `window.open`)
- `lmmaster.update.skipped.{version}` LRU 청소
- Pipeline 사용자 정의 + per-route activation matrix
- PromptSanitize Pipeline (NFC + control-char strip)

## 다음 진입 가이드

새 세션 시작 시 자동 로드:
1. `CLAUDE.md` — 행동 규칙 (300줄 미만 유지)
2. `~/.claude/projects/.../memory/MEMORY.md` — auto memory index

수동 참조:
- 현재 인계: `docs/RESUME.md` (250줄 미만 유지)
- 본 대시보드: `docs/PROGRESS.md` (150줄 미만 유지)
- 결정 이력: `docs/adr/` + `docs/adr/README.md`
- 결정 노트: `docs/research/<phase>-decision.md` 또는 `<phase>-reinforcement.md`
- 시간순 이력: `docs/CHANGELOG.md`

## 페이즈별 산출물 빠른 매핑

| Phase | 주요 crate / 모듈 | 결정 노트 | ADR |
|---|---|---|---|
| 1' | runtime-manager / scanner / registry-fetcher | phase-1p-* (3건) | 0019 / 0020 |
| 1A | runtime-detector / hardware-probe / installer | phase-1a* (8건) | 0017 / 0021 |
| 2' | preset-registry / model-registry / bench-harness | phase-2p* (3건) | 0014 / 0022 |
| 3' | core-gateway | phase-3p-gateway | 0022 |
| 4 | 10 React 화면 | phase-4* (8건) | 0011 |
| 4.5' | knowledge-stack | phase-4p5-rag | 0024 |
| 5' | workbench-core | phase-5p-workbench / 5pe | 0018 / 0023 |
| 6' | auto-updater / pipelines | phase-6p / 7p | 0025 / 0026 |
