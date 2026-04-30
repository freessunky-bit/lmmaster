# RESUME — LMmaster 세션 인계 노트

> **목적**: 현재 페이즈가 컨텍스트 한계로 끝나면 다음 세션이 즉시 이어받을 수 있게 마지막 상태와 다음 작업을 기록.
>
> **사이즈 정책**: ≤300줄 (Claude attention 최적). 시간순 상세 이력은 `docs/CHANGELOG.md`로 분리.

## 빠른 진입

| 목적 | 파일 |
|---|---|
| 1-page 진행도 / 6 pillar 상태 | `docs/PROGRESS.md` |
| 시간순 상세 이력 (903줄) | `docs/CHANGELOG.md` |
| 페이즈 전략 / 위험 / 컴파스 | `docs/PHASES.md` |
| 행동 규칙 / 자율 정책 | `CLAUDE.md` (project root) |
| 결정 이력 (26건) | `docs/adr/README.md` |
| 보강 리서치 (34건) | `docs/research/` |
| 제품 비전 / 6 pillar | `docs/PRODUCT.md` |

## 누적 검증 (2026-04-29 — 9'.a/9'.b까지 완료. **v1 ship 가능 + v1.x 90%**)

- **cargo (lmmaster-desktop 제외)**: **817 / 0 failed**
- **vitest**: ~400 / 0 failed (38~40 files, 9'.b 일부 vitest는 다음 세션에서 검증)
- **clippy / fmt / tsc / pnpm build**: 모두 clean
- **lmmaster-desktop --lib test exe**: 환경 문제 (docs/troubleshooting.md)
- **Crates**: 22 / **ADR**: **35** (0001~0043) / **결정 노트**: 34+
- **GitHub repo**: https://github.com/freessunky-bit/lmmaster (push + settings 적용)
- **사용자 결정 핵심 2건 완료**: minisign keypair (pubkey 적용) / GitHub repo URL (endpoints 적용)
- **2026-04-29 hotfix**: `Cargo.lock` `tauri-plugin-single-instance 2.0.0` → **2.4.1** (PR #2452 — Windows IPC 핸들 null-check). `cargo check -p lmmaster-desktop` 통과. 결정 노트 `docs/research/phase-resume-single-instance-fix-decision.md` + ADR-0036 References 보강.
- **2026-04-30 install/bench bugfix**: 카탈로그 drawer "이 모델 설치할게요" 무반응 + 30초 측정 즉시 실패 두 ship-blocker 해결. ① `Catalog.tsx` `lmmaster:nav` → `lmmaster:navigate` 이벤트명 통일 (1줄). ② 신규 `apps/desktop/src-tauri/src/model_pull/` 모듈 + `start_model_pull` / `cancel_model_pull` IPC + Ollama `/api/pull` NDJSON 스트리밍 (layer aggregate + EMA speed + cancel-aware send). ③ `bench-harness::runner.rs::harness_loop` `last_error` 보존 + `aggregate` mapping — generic "측정 호출이 모두 실패했어요" 폐기, `RuntimeUnreachable` / `ModelNotLoaded` 등 그대로 매핑. ④ `bench/commands.rs` Ollama preflight (`has_model` 통한 /api/tags 한 방). ⑤ `ModelDetailDrawer` in-place 풀 패널 + `BenchChip` ModelNotLoaded → "이 모델 먼저 받을게요" CTA. ⑥ i18n ko/en 신규 키 9건. 결정 노트 `docs/research/phase-install-bench-bugfix-decision.md`. **검증**: cargo workspace clippy clean / cargo test 47+18 신규 / fmt clean / tsc clean. ms-side `EmbeddingModelPanel.test.tsx` `let onEvent: T | null = null` TS narrow 회피 + `knowledge.ts` 9'.a stale `kind` field 충돌 fix(`model_kind`).
- **2026-04-30 Phase 13'.e (5단계) — 카탈로그 큐레이션 확장 + NEW 탭 + 커뮤니티 인사이트** (ADR-0045): ① **e.1 schema** — `ModelTier` enum (new/verified/experimental/deprecated, default verified) + `CommunityInsights` struct (4-section 한국어 + sources + last_reviewed_at). serde_default로 기존 호환. 결정 노트 `phase-13pe1-schema-decision.md`. ② **e.2 HF metadata cron** — `apps/desktop/src-tauri/src/hf_meta.rs` (HfMetaCache + bulk fetch with `buffer_unordered(5)` + 6h interval task). `get_catalog`이 cache merge로 hf_meta 자동 노출. ③ **e.3 NEW 탭** — Catalog.tsx 사이드바에 🔥 NEW 추가, tier 필터 (deprecated 자동 hide), count badge. i18n ko/en. ④ **e.4 ? 토글 GUI** — ModelDetailDrawer에 collapsible CommunityInsightsPanel (강점/약점/사용분야/큐레이터 코멘트/출처/60일+ stale hint) + HF metadata pill row (downloads/likes/last_modified/NEW badge). ⑤ **e.5 큐레이션 +8** (12→20): Llama-3-Bllossom-8B, EXAONE 3.5 2.4B, SOLAR 10.7B, Phi-4 14B, Mistral Small 24B (NEW), Qwen 2.5 Coder 7B, DeepSeek Coder V2 16B, Llama 3.2 1B. 모두 community_insights 4-section 작성. 잔여 22모델은 `docs/CURATION_GUIDE.md`에 인계. **검증**: model-registry 16/16 / cargo check workspace / tsc clean / build-catalog-bundle.mjs entries=20.
- **2026-04-30 Phase 13'.a — Live model catalog refresh** (ADR-0044): `registry-fetcher`가 app manifest만 fetch하던 한계 해결. 신규 모델(Gemma/Qwen3/DeepSeek-R1)이 앱 재배포 없이 사용자에 노출되도록. ① `manifests/apps/catalog.json` 단일 bundle 생성 (build script `.claude/scripts/build-catalog-bundle.mjs`, 12 entries 자동 머지/정렬/중복검사). ② `default_sources` jsDelivr 1순위↔GitHub 2순위 swap (한국 latency, research §2). repo URL `lmmaster/lmmaster` → `freessunky-bit/lmmaster` fix. ③ `manifest_ids`에 "catalog" 추가, fetcher가 동일 4-tier로 처리. ④ `CatalogState::swap_from_bundle_body` (ModelManifest deserialize + schema_version 검증 + entries 비어있지 않음 검사 + atomic swap). ⑤ `registry_fetcher::refresh_once`가 catalog body 받으면 swap, 아니면 reload_from_bundled 폴백. ⑥ `bundled_dir` path fix (`manifests/snapshot/apps`(없음)→`manifests/apps`). ⑦ Catalog.tsx 카피 갱신 — 모델 카탈로그까지 갱신함을 명시. **검증**: registry-fetcher 9/9 / cargo check workspace clean / tsc clean / ACL drift 0 (75 identifier / 72 명령). 결정 노트 `phase-13pa-live-catalog-decision.md` (기각안 7건). lmmaster-desktop --lib test exe는 환경 문제로 검증 보류 (코드는 cargo check --tests 통과).
- **2026-04-30 ACL hardening (post-bugfix)**: 사용자 실 클릭 테스트 중 발견된 `start_model_pull not allowed: not found` 에러로 Tauri 2 ACL 누락 17건 일괄 audit + fix. `capabilities/main.json`에 `allow-start-model-pull` / `allow-cancel-model-pull` / `allow-list-custom-models` / `allow-workbench-real-status` / `allow-lora-bootstrap-venv` / `allow-cancel-lora-bootstrap` / `allow-list-embedding-models` / `allow-set-active-embedding-model` / `allow-download-embedding-model` / `allow-cancel-embedding-download` / `allow-submit-telemetry-event` / `allow-update-api-key-pipelines` + workspaces 6건 (list/get/create/rename/delete/set_active) = 17 신규 identifier 추가. `permissions/model_pull.toml` + `permissions/workspaces.toml` 신규 파일. `permissions/{workbench,knowledge,telemetry,keys}.toml` 항목 보강. **drift 자동 방지**: `.claude/scripts/check-acl-drift.ps1` 신규 — `invoke_handler!` 명령 vs `capabilities/main.json` 비교 + 누락 시 exit 1. `verify.ps1` 첫 step에 통합 (cargo fmt 보다 먼저 실행). 현재 70/70 명령 모두 ACL 도달 가능 (68 explicit + 2 auto-allowed `ping` / `get_gateway_status`). 한국어 stalled microinteraction (15s/60s threshold copy) `ModelDetailDrawer::useStalledHint` 추가. 리서치 노트: `phase-install-bench-bugfix-decision.md` §6 인계 + 보강 리서치 결과 반영.

### Phase 9'.a 산출물 (Real Embedder — bge-m3 / KURE-v1 / multilingual-e5 cascade) — 2026-04-28

- `crates/knowledge-stack/src/embed_download.rs` (~600 LOC) — HuggingFace 다운로드 + sha256 + atomic rename + cancel + 12 unit tests (wiremock).
- `crates/knowledge-stack/src/embed_onnx.rs` (~330 LOC, `embed-onnx` feature gated) — `OnnxEmbedder` + mean pooling + L2 normalize + 5 tests (graceful 미존재 / mean_pool pure).
- `crates/knowledge-stack/src/embed.rs` — `default_embedder` helper 추가 + 4 신규 tests.
- `apps/desktop/src-tauri/src/knowledge.rs` — `EmbeddingState` + 4 IPC commands (list/set/download/cancel) + 11 신규 tests. `run_ingest`/`search_knowledge`이 active 모델로 embedder 해결.
- `apps/desktop/src/components/workspace/EmbeddingModelPanel.tsx` (~330 LOC) + 9 vitest (a11y radiogroup + 진행률 + cancel + activate).
- `apps/desktop/src/ipc/knowledge.ts` — 신 type/command 6개.
- `apps/desktop/src/pages/Workspace.tsx` — Knowledge tab 위에 EmbeddingModelPanel 추가.
- i18n `screens.workspace.embeddingModels.*` 25 keys × 2 locales.
- `docs/adr/0042-real-embedder-onnx-cascade.md` (5 alternatives rejected: OpenAI API / Python sidecar / llama.cpp embeddings / 단일 모델 / bundle).
- 외부 통신: `huggingface.co` 화이트리스트 (사용자 명시 클릭으로만).
- ort/tokenizers/ndarray는 `embed-onnx` feature off가 default — baseline build 부담 0. 사용자 PC ORT 미설치 시 graceful 한국어 에러 + MockEmbedder fallback.

### Phase 12' 산출물 (in-app guide system)

- `apps/desktop/src/pages/Guide.tsx` (~250 LOC) + `guide.css` — NAV "가이드" + 8 섹션 + 검색 + deep link CTA.
- `apps/desktop/src/i18n/guide-{ko,en}-v1.md` (~400 LOC 합산) — 8 섹션 마크다운 (한국어 해요체).
- `apps/desktop/src/components/_render-markdown.ts` — EulaGate에서 추출한 공유 minimal markdown renderer.
- `apps/desktop/src/components/HelpButton.tsx` (~190 LOC) — ? 도움말 + popover (focus trap + Esc + role=dialog).
- `apps/desktop/src/components/ShortcutsModal.tsx` (~240 LOC) — F1 / Shift+? + Ctrl+1~9 NAV hotkey.
- `apps/desktop/src/components/TourWelcomeToast.tsx` (~150 LOC) — 첫 실행 후 1회 toast (localStorage).
- 5 페이지 헤더에 HelpButton 통합 + App.tsx에 NAV 키 + ShortcutsModal + TourWelcomeToast 마운트.
- ADR-0040 신설 (5 alternatives rejected: Shepherd.js / 외부 docs / tooltip-only / react-markdown / 동영상).
- Tests: Guide(11) + HelpButton(8) + ShortcutsModal(12) + TourWelcomeToast(8) + _render-markdown(20) = 59건.

## 최근 5개 sub-phase (2026-04-28)

상세는 `docs/CHANGELOG.md`. 여기엔 핵심 1줄만.

| Phase | 내용 | 테스트 +N |
|---|---|---|
| 5'.a | `crates/workbench-core` scaffold + ADR-0023 (LoRA/Modelfile/양자화 wrapper) | +81 |
| 4.5'.a | `crates/knowledge-stack` scaffold + ADR-0024 (NFC chunker + per-workspace SQLite) | +56 |
| 6'.a | `crates/auto-updater` + ADR-0025/0026 (semver build-metadata strip 명시) | +52 |
| 5'.b/4.5'.b | Workbench/Knowledge IPC + 5-step UI / Workspace Knowledge tab | +73 |
| 6'.b/6'.c | Auto-updater UI + Settings AutoUpdatePanel / Pipelines UI + 감사 로그 viewer | +89 |
| 5'.c+5'.d | Workbench Validate(bench-harness) + Register(model-registry) 실 연동 | +31 |
| 5'.e | 실 HTTP wiring — Ollama/LM Studio + ollama create shell-out + RuntimeSelector UI | +44 cargo / +4 vitest |
| 6'.d | Gateway audit wiring — `PipelineLayer` → `PipelinesState` mpsc(256) channel + best-effort try_send + chain hot-build from config | +24 cargo / 0 vitest |
| 7'.a | Release scaffold — bundler 매트릭스 + tauri-plugin-updater + EulaGate + opt-in telemetry + ADR-0027 | +8 cargo / +19 vitest |
| Critical 3건 wire-up | #1 LiveRegistryProvider(Ollama+LM Studio routing) + #2 Workspace nav + #3 RegistryFetcher cron 통합 | +25 backend tests / +10 vitest |
| 8'.0 Security/Stability | SQLCipher 활성(feature gate) + single-instance + panic_hook + WAL + artifact retention | +31 cargo / +8 vitest |
| 8'.1 Multi-workspace UX | workspaces.rs (6 IPC) + ActiveWorkspaceContext + WorkspaceSwitcher (사이드바) + Workspace.tsx active 사용 | +12 cargo / +20 vitest |
| 11' Portable export/import | export.rs / import.rs (zip + AES-GCM PBKDF2 + dual zip-slip) + 5 Tauri IPC + ExportPanel/ImportPanel (Settings) | +16 cargo / +21 vitest |
| 8'.a/.b/Env'.a | get_document_path / update.skipped LRU / last_check 일관 / dead key 제거 / Custom Models Catalog / plugin-shell / troubleshooting.md | +6 cargo / +13 vitest |
| 7'.b CI 자동화 | release.yml + tauri-action@v0 + minisign 서명 + SECRETS_SETUP + Issue/PR templates + README.en.md + 베타 채널 + GlitchTip telemetry submit (queue + retry) + ADR-0041 | +0 cargo (lmmaster-desktop test exe 환경 이슈로 23 신규 테스트 CI 실행) / +4 vitest |
| 8'.c Pipelines extension | PromptSanitize Pipeline (NFC + control char strip) + ArcSwap hot-reload + per-key Pipelines matrix(serde default 마이그레이션 free) + SSE chunk transformation (line-aware parser + buffered emit) + ADR-0028/0029/0030 | +62 cargo / +4 vitest |
| 9'.a Real Embedder | embed_download.rs (HuggingFace + sha256 + atomic rename) + embed_onnx.rs (feature-gated ort 2.0.0-rc.10) + EmbeddingModelPanel (3-card UI) + ADR-0042. KnowledgeApiError `kind` 필드 충돌 fix(`model_kind`) | +18 cargo / +10 vitest |
| 9'.b Real Workbench | LlamaQuantizer (llama-quantize binary subprocess + kill_on_drop + 30분 timeout + stderr 한국어 매핑) + LlamaFactoryTrainer (Python venv 자동 부트스트랩 + LLaMA-Factory CLI) + WorkbenchConfig.use_real_* 토글 + 사전 동의 dialog + ADR-0043 | +23 cargo / +5 vitest 추정 |

## 🟡 진행 중

(없음 — 9'.b까지 완료. 다음 standby는 9'.c.)

## 🟢 다음 세션 진입 가이드 (Standby — Phase 9'.c)

**Phase 9'.c — Multi-runtime adapters** (마지막 v1.x ML 페이즈, ~3-4시간)

진입 시:
1. `CLAUDE.md` + `MEMORY.md` 자동 로드.
2. 본 RESUME + `docs/PROGRESS.md` + `docs/research/phase-8p-9p-10p-residual-plan.md` §3.9'.c 참조.
3. Sub-agent 1건 dispatch (자동 chain 패턴):

```
Phase 9'.c — Multi-runtime adapters expansion
├── crates/adapter-llama-cpp/src/lib.rs — llama-server HTTP probe + chat completion
├── crates/adapter-koboldcpp/src/lib.rs — KoboldCpp /api endpoint
├── crates/adapter-vllm/src/lib.rs — OpenAI-compatible vLLM
├── crates/runtime-detector/src/lib.rs — 4종 runtime detect rules 추가
├── apps/desktop/src-tauri/src/registry_provider.rs — LiveRegistryProvider 확장
├── ADR-0044 신설 (Multi-runtime expansion)
└── RuntimeKind enum 확장 (workbench RuntimeSelector UI도)
```

**보강 리서치 1건 + 구현 1 sub-agent / ~3-4시간** 예상.

**진입 신호 예시**:
- "Phase 9'.c 진행"
- "다음 세션 이어서 진행" → 본 standby 자동 진입.

**참고**:
- 9'.b 완료 흔적: `crates/workbench-core/src/{quantize_real,lora_real}.rs` + ADR-0043.
- 9'.b에서 `LMMASTER_LLAMA_QUANTIZE_PATH` env override + venv 부트스트랩 / 사용자 동의 dialog 패턴 정립 — 9'.c도 같은 패턴 따름.

## 🔴 v1 ship 가능 상태 (2026-04-29)

- 사용자 결정 6건 중 핵심 2건 (minisign + repo URL) 완료.
- 나머지 4건 (OV cert / Apple Dev / EULA 법무 / Publisher명) 비상용이라 skip 가능.
- v1 베타 ship 즉시 가능: `run-build.bat` 또는 GitHub Actions release.yml 활용.

## ⏳ v1 ship 전 남은 태스크

### ⛔ 출시 절대 차단 (사용자 결정 6건 + 코드 1건)

1. **Windows OV 인증서** ($150~$300/년) → `tauri.conf.json` `bundle.windows.certificateThumbprint`
2. **Apple Developer Program** ($99/년) → `bundle.macOS.signingIdentity` + `providerShortName`
3. **minisign keypair** (`pnpm exec tauri signer generate`) → `plugins.updater.pubkey` 교체 (현재 placeholder, **자동 업데이트 무결성 검증 불가**)
4. **GitHub repo URL** 확정 → `plugins.updater.endpoints`
5. **EULA 법무 검토** → `eula-{ko,en}-v1.md`
6. **Publisher명** 확정 → `bundle.publisher`

### 🔴 v1 ship 전 권장 (Phase 8'.0 + 8'.1 + 11' + 12', 코드 작업)

7. **SQLCipher 활성** (#23) — API 키 평문 저장 → 암호화 (ADR-0008 보안 약속)
8. **Single-instance + panic hook + WAL** (#26 + #27 + #28) — 안정성
9. **Multi-workspace UX** (#24) — ADR-0024 약속 UI 실현
10. **Workbench artifact retention** (#29) — 디스크 누적 방지
11. **Portable export/import** (#30, **6 pillar 약속**) — `crates/portable-workspace`에 export/import 모듈 + Settings UI
12. **Guide / Help system** (#31) — NAV "가이드" + 8 섹션 + ? tooltip + F1 단축키 + 첫 실행 둘러보기

### 📋 잔재 plan (Standby)

`docs/research/phase-8p-9p-10p-residual-plan.md` (~880 LOC) — 22+7+2=31 미구현 항목 페이즈 단위 작업 계획.
- Phase 7'.a' / 8'.0 / 8'.1 / 11' / 12' = v1 ship 전 권장.
- Phase 8' / 9' / 10' / Env' = v1 출시 후 v1.x.
- 사용자 명시 진행 신호 시 자동 진입.

### Phase 7'.b (사용자 결정 후 실행 가능)

| 작업 | 규모 |
|---|---|
| GitHub Actions `release.yml` (matrix Win/mac/Linux + tauri-action@v0) | 1 sub-agent |
| minisign 서명 자동화 + `latest.json` asset 업로드 | 1 sub-agent |
| GlitchTip self-hosted endpoint 연결 (telemetry.rs `submit_event` + opt-in gating) | 1 sub-agent |
| 베타 채널 토글 Settings UI (`latest-beta.json`) | ~200 LOC |
| README.ko.md / README.md 다국어 매트릭스 | 1 sub-agent |

## 🟢 다음 standby

**Phase 6'.d — Gateway audit wiring** (5'.e 완료 후 자동 dispatch):
- 결정 노트: 새 작성 또는 `phase-6p-updater-pipelines-decision.md` §4 인용.
- `crates/core-gateway/src/pipeline_layer.rs`에 `audit_sender: Option<mpsc::Sender<AuditEntryDto>>` 필드 추가.
- `apps/desktop/src-tauri/src/pipelines.rs::PipelinesState::with_audit_channel()` 헬퍼 + spawn task가 mpsc → record_audit 호출.
- gateway 빌드 시점에 sender 주입.
- 통합 테스트: PipelineLayer 거친 요청이 PipelinesState audit log에 등록되는지.

**Phase 7' — v1 출시 준비** (별도 세션):
- 보강 리서치: `docs/research/phase-7p-release-prep-reinforcement.md` (492 LOC, 24 인용).
- 사용자 결정 필요: Authenticode OV 인증서 구매 ($150~$300) / Apple Developer Program ($99/year).
- bundler 매트릭스 / minisign keypair 생성 / EULA 한국어 작성.
- 다음 세션 시 본 RESUME + PROGRESS + 7' 보강 리서치 노트 참조하여 시작.

## 자율 정책 요약 (전체는 `CLAUDE.md`)

- **자동 진행**: 파일 r/w / 페이즈 chain / 보강 리서치 / sub-agent dispatch / 빌드·테스트 / 의존성 추가.
- **사용자 확인 필요**: `git push` / `rm -rf` / `.env` / 큰 아키텍처 분기 / 동등 설계안 선택 / destructive ops.
- **토큰 한계 시**: sub-phase 분할 → RESUME 갱신 → 자연 종료. 다음 세션이 본 파일로 즉시 이어받음.

## 검증 명령 (sub-phase 종료 시 풀 검증)

```bash
# Bash (Unix)
export PATH="/c/Users/wind.WIND-PC/.cargo/bin:$PATH"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd apps/desktop
pnpm exec tsc -b --clean && pnpm exec tsc -b
pnpm exec vitest run
find src -name '*.js' -not -name '*.config.js' -delete   # 중요: stale .js artifacts 제거
```

```powershell
# PowerShell (Windows native, .claude/scripts/ 헬퍼)
.\.claude\scripts\verify.ps1
```

## ⚠️ 다음 세션 trap 노트 (이번 세션에서 발견)

1. **Stale `.js` artifacts**: `tsc -b` 누락 시 `src/pages/Workbench.js` 같은 컴파일 산출물이 `.tsx`보다 우선 모듈 해상도되어 vitest가 옛 코드 실행. 매 빌드 후 위 `find -delete` 또는 `pnpm exec tsc -b --clean` 수행.
2. **Ark UI Steps a11y 위반**: `Steps.Trigger`가 `aria-controls="steps::r7::content:0"`를 자동 부여하지만 axe `aria-required-children`(div[aria-current] 자식 위반) + `aria-valid-attr-value` 트리거. 5단계 stepper는 semantic `<button>` + 자체 aria-current로 처리.
3. **Rust `semver` crate Ord vs spec 차이**: `semver = "1"`의 `Ord::cmp`가 build metadata를 ordering에 포함(`1.0.0+build1 < 1.0.0+build2`)하는데 semver 2.0 spec은 무시 권장. `auto-updater::version::parse_lenient`에서 `version.build = BuildMetadata::EMPTY` 명시 strip.
4. **Sub-agent cargo 검증 권한**: 일부 sub-agent가 Bash/PowerShell 권한 거부로 cargo 못 돌림. 메인이 항상 후속 검증 책임.
5. **ADR 번호 사전 충돌**: 두 sub-agent가 동시에 동일 ADR 번호 클레임 가능. dispatch 전 명시적 번호 할당 + 메인이 후처리 renumber.
6. **i18n 부모 블록 동시 편집**: 여러 sub-agent가 `screens.*`에 동시 편집 시 last-write-wins. 다른 namespace로 분리하거나 순차 dispatch.
7. **caret + lock의 함정**: `Cargo.toml`의 caret(`"2"`)은 lock이 *없을 때만* 새 patch로 resolve. 한 번 잠긴 lock은 `cargo update` 없이 자동 갱신되지 않음. `tauri-plugin-single-instance 2.0.0` Windows null pointer 패닉(PR #2452, 2.2.2에서 fix)이 본 함정의 대표 사례 — Cargo.toml은 `"2"` 그대로인데 lock은 옛 patch에 stuck. v1.x 진입 / 새 sub-phase 진입 시 `cargo update --workspace --dry-run` 1회로 lock-stale 의존성 표면화 권장. 결정 노트: `docs/research/phase-resume-single-instance-fix-decision.md`.

## 참고

- `docs/CHANGELOG.md` — 본 세션 이전 모든 sub-phase 상세 (Phase α / 0 / 1' / 1A / 2'~6').
- `docs/PROGRESS.md` — 6 pillar 상태 + 페이즈 진행도 막대그래프.
- `docs/PHASES.md` — 페이즈별 위험 Top-3 + 컴파스 ("Ollama가 다음 분기에 같은 기능 출시하면 USP 살아남는가").
- `~/.claude/projects/.../memory/MEMORY.md` — auto memory 14건 인덱스.
- 본 세션 5'.e 완료 후 `RESUME` "최근 5개" 표 + "진행 중" 섹션 갱신 + 메모리 v13 발행.
