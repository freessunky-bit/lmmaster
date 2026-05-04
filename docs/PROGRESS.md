# PROGRESS — LMmaster v1 진행 대시보드

> 1-page 진행도. 자세한 시간순 이력은 `docs/CHANGELOG.md`, 페이즈 전략은 `docs/PHASES.md`, 인계 노트는 `docs/RESUME.md`.

**마지막 갱신**: 2026-05-04 (GPT Pro 30-issue 검수 + 분리 sub-phase #31/#38 + 통합 audit 모두 종결. R-A/B/C/D/E 5 페이즈 + 분리 2건 + audit 1건 = **27 commits 머지**. ADR-0052~0057 신규. **v0.0.1 ship 가능**.)

## 6 Pillar (제품 약속) 상태

| Pillar | 상태 | 핵심 산출물 |
|---|---|---|
| 자동 설치 | ✅ | Phase 1A.1~1A.3 — Ollama silent + LM Studio open_url + manifest installer + dual zip-slip 방어 |
| 한국어 | ✅ | Phase 4 + en audit — 해요체 일관 + Linear/Stripe en 톤 + i18n ko/en 동기 |
| 포터블 | ✅ | `crates/portable-workspace` + manifest 기반 |
| 큐레이션 | ✅ | Korean preset 109건(7 카테고리) + curated model registry |
| 워크벤치 | ✅ | Phase 5'.a/b/c/d/e — 5단계 state machine + 실 HTTP (Ollama + LM Studio) + ollama create shell-out |
| 자동 갱신 | ✅ | Phase 6'.a/b 완료 — auto-updater + Settings 토글 + JetBrains-style toast |

## 누적 검증 (2026-05-04 현재)

- **cargo build --workspace**: clean (link 단계까지)
- **cargo clippy --workspace --all-targets -- -D warnings**: 0 warning
- **cargo clippy -W dead_code -W unused_imports**: 0 warning
- **cargo fmt**: 0 diff
- **pnpm exec tsc -b**: 0 errors
- **ACL drift**: 83 명령 / 86 identifier (drift 0)
- **lmmaster-desktop --lib test exe**: 환경 문제 (Windows DLL, pre-existing — `docs/troubleshooting.md`)
- **신규 invariant**: R-E 29건 + #31 7건 + #38 2건 = 38건 신규

## 산출 자산

| 카테고리 | 갯수 | 비고 |
|---|---|---|
| Crates | 24 | R-E.2 `openai-compat-dto` + R-E.3 `chat-protocol` 추가 |
| ADR | 53 (0001~0057) | R-A 0052 / R-B 0053+0054 / R-C 0055 / R-D 0056 / R-E 0057 |
| 결정 노트 | 44+ | `docs/research/` (R-A/B/C/D/E 결정 노트 + 진입점 노트) |
| Korean preset | 109 | 7 카테고리, 의료/법률 disclaimer 강제 |
| React 페이지 | 10 | Home/Catalog/ApiKeys/Workspace/Install/Runtimes/Projects/Workbench/Diagnostics/Settings |

## v1 진행도 (페이즈 단위)

```
Phase α   Foundation docs        ████████████████████ 100%
Phase 0   Tauri+Axum boot        ████████████████████ 100%
Phase 1'  Bootstrap+Self-scan    ████████████████████ 100%
Phase 1A  Onboarding wizard      ████████████████████ 100%
Phase 2'  Catalog+Recommender    ████████████████████ 100%
Phase 3'  Gateway routing        ████████████████████ 100%
Phase 4   10 화면 + presets      ████████████████████ 100%
Phase 4.5' RAG (knowledge)       ████████████████████ 100%
Phase 5'  Workbench              ████████████████████ 100%
Phase 6'  Pipelines + Updater    ████████████████████ 100%
Phase 7'  v1 Release prep        ████████████████████ 100%
Phase 8'~14' v1 보강 + 디자인     ████████████████████ 100%
Phase R-A Security Boundary      ████████████████████ 100%  (S1+R1+S2+T4)
Phase R-B Catalog Trust          ████████████████████ 100%  (T2+S3+S4+S5+R4)
Phase R-C Network + Correctness  ████████████████████ 100%  (S7+C1+R3+C3)
Phase R-D Frontend Polish        ████████████████████ 100%  (K1+K2+K3+K4)
Phase R-E Architecture Cleanup   ████████████████████ 100%  (T3+C2+A1+A2+P1+P4+R2)
분리 #31  Knowledge IPC boundary  ████████████████████ 100%
분리 #38  knowledge SQLCipher     ████████████████████ 100%
통합 audit  wiring + dead-code   ████████████████████ 100%
```

## v0.0.1 ship 가능 — 모든 ship-blocker + cleanup 종결

| 항목 | 상태 |
|---|---|
| GPT Pro 30-issue 검수 (17 ship-blocker + 7 cleanup + 6 deferred) | ✅ 완료 |
| ADR-0052~0057 6건 신규 + 결정 노트 | ✅ |
| 통합 wiring audit (R-E.7 cancel_scope register 누락 + model_pull cancel cascade 누락) | ✅ 수정 |
| GitHub repo push (`freessunky-bit/lmmaster`) | ✅ |
| release.yml + sign-catalog.yml | ✅ |
| minisign keypair + Tauri secret 등록 | ✅ |
| **v0.0.1 release tag push** | ⏳ 사용자 결정 (`git tag v0.0.1 && git push origin v0.0.1`) |

## v1.x 후속 (출시 후, v1 범위 밖)

R-E의 v2.x 잠재 + 기존 v1.x 후속:

- **R-E v2.x 잠재**: KnowledgeStorePool RwLock 전환 / WorkspaceCancellationScope chat·bench register wiring 점진 적용 / proper LRU(move-to-front) / wiremock chunked disconnect 헬퍼 추출 / RuntimeAdapter trait true split (새 어댑터 추가 시) / KnowledgeStorePool 통계 IPC (Diagnostics)
- **#31 후속**: selected_path_token registry — 사용자가 dialog로 선택한 ingest 소스 path tokenization (Tauri dialog plugin 도입 후)
- **catalog 외 manifest signature** — ollama.json / lm-studio.json 등도 verify (현재 catalog만)
- **proxy 명시 opt-in** — Settings 토글로 corporate proxy 사용 (현재 .no_proxy 강제)
- **typed-i18n-keys crate** — t() 컴파일 타임 키 검증
- ApiKeys per-key Pipelines matrix UI (ADR-0025 §3)
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
