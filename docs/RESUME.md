# RESUME — LMmaster 세션 인계 노트

> **목적**: 현재 페이즈가 컨텍스트 한계로 끝나면 다음 세션이 즉시 이어받을 수 있게 마지막 상태와 다음 작업을 기록.
>
> **사이즈 정책**: ≤300줄 (Claude attention 최적). 시간순 상세 이력은 `docs/CHANGELOG.md`로 분리.

## 빠른 진입

| 목적 | 파일 |
|---|---|
| 1-page 진행도 / 6 pillar 상태 | `docs/PROGRESS.md` |
| **후순위 이월 작업 단일 진입점** | **`docs/DEFERRED.md`** |
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
- **Crates**: 22 / **ADR**: **50** (0001~0054) / **결정 노트**: 41+
- **GitHub repo**: https://github.com/freessunky-bit/lmmaster (push + settings 적용)
- **사용자 결정 핵심 2건 완료**: minisign keypair (pubkey 적용) / GitHub repo URL (endpoints 적용)
- **2026-04-29 hotfix**: `Cargo.lock` `tauri-plugin-single-instance 2.0.0` → **2.4.1** (PR #2452 — Windows IPC 핸들 null-check). `cargo check -p lmmaster-desktop` 통과. 결정 노트 `docs/research/phase-resume-single-instance-fix-decision.md` + ADR-0036 References 보강.
- **2026-04-30 install/bench bugfix**: 카탈로그 drawer "이 모델 설치할게요" 무반응 + 30초 측정 즉시 실패 두 ship-blocker 해결. ① `Catalog.tsx` `lmmaster:nav` → `lmmaster:navigate` 이벤트명 통일 (1줄). ② 신규 `apps/desktop/src-tauri/src/model_pull/` 모듈 + `start_model_pull` / `cancel_model_pull` IPC + Ollama `/api/pull` NDJSON 스트리밍 (layer aggregate + EMA speed + cancel-aware send). ③ `bench-harness::runner.rs::harness_loop` `last_error` 보존 + `aggregate` mapping — generic "측정 호출이 모두 실패했어요" 폐기, `RuntimeUnreachable` / `ModelNotLoaded` 등 그대로 매핑. ④ `bench/commands.rs` Ollama preflight (`has_model` 통한 /api/tags 한 방). ⑤ `ModelDetailDrawer` in-place 풀 패널 + `BenchChip` ModelNotLoaded → "이 모델 먼저 받을게요" CTA. ⑥ i18n ko/en 신규 키 9건. 결정 노트 `docs/research/phase-install-bench-bugfix-decision.md`. **검증**: cargo workspace clippy clean / cargo test 47+18 신규 / fmt clean / tsc clean. ms-side `EmbeddingModelPanel.test.tsx` `let onEvent: T | null = null` TS narrow 회피 + `knowledge.ts` 9'.a stale `kind` field 충돌 fix(`model_kind`).
- **2026-05-03 Phase R-B — Catalog Trust Pipeline 머지** (ADR-0053 + ADR-0054 채택). GPT Pro 30-issue 중 v0.0.1 ship-blocker 보안+신뢰 카테고리(S3+S4+S5+R4+T2) 5건 일괄 해소. ① **R-B.1 (T2 minisign round-trip)** — minisign-verify 0.2.5 자체 fixture(`RWQf6LRC...` + body `b"test"` + 1556193335 prehashed sig)로 4 invariant 활성화 (정상/변조/dual-key fallback/no-match). `from_minisign_strings` parser가 bare base64 + multi-line 두 형식 모두 수용 (`parse_pubkey` private helper). ② **R-B.2 (S3 SQLCipher feature gate)** — `crates/knowledge-stack/Cargo.toml`에 `sqlcipher = ["rusqlite/bundled-sqlcipher-vendored-openssl"]` feature 추가 (key-manager와 동일 패턴, ADR-0035 차용). `KnowledgeStore::open_with_passphrase()` 신규 메서드 + `apply_passphrase()` private helper (PRAGMA key + sqlite_master 검증). 평문 모드 `open()` 백워드 호환 유지. caller wiring(knowledge.rs가 keyring secret 적용 + 마이그레이션 + per-workspace secret)은 sub-phase #38로 분리. ③ **R-B.3 (S4 cache poisoning fix)** — `manifest_cache` 스키마 v2 `signature_verified INTEGER DEFAULT 0` 컬럼 추가. 신규 설치 v2 직접 진입(row count==0 검사) + 기존 v1 → v2 ALTER idempotent. `CacheRow`/`CachePutInput`/`FetchedManifest`에 `signature_verified: bool` 필드. `Cache::mark_verified()` 메서드 신규. `try_source` put: false / `try_bundled` put: true. ④ **R-B.4 (S5 signed fetch wiring)** — `RegistryFetcher::mark_signature_verified` 외부 노출. `fetch_one_with_signature` cache 적중 시 row marker 검사 → false면 invalidate + 재페치 1회(Box::pin 재귀, 무한 방지). desktop `verify_catalog_signature`에 `signature_verified_marker: bool` 인자 추가 — cache+marker=true → 즉시 Verified, marker=false → network verify 강제. verify 성공 → `mark_signature_verified` 호출. `refresh_once`가 catalog row marker 보존. ⑤ **R-B.5 (R4 release workflow 검수)** — `.github/workflows/release.yml` + `sign-catalog.yml` 모두 sound. SECRETS_SETUP.md §1+§1.b 가이드 완비. 코드 변경 0. ⑥ **ADR-0053** (knowledge-stack SQLCipher feature gate) + **ADR-0054** (manifest_cache v2 + verified marker) 신설. 결정 노트 `docs/research/phase-r-b-catalog-trust-pipeline-decision.md` (6 섹션, 16 기각안). **검증**: cargo clippy registry-fetcher + lmmaster-desktop -D warnings clean / cargo fmt clean / registry-fetcher 33 unit + 9 integration (5 신규 invariant) / knowledge-stack 70/70 (1 신규) / 회귀 0건. 다음 standby = **Phase R-C.1 (S7 reqwest no_proxy + allowlist)**.
- **2026-05-03 Phase R-A — Security Boundary 머지** (ADR-0052 채택). GPT Pro 정적 검수 30건 중 v0.0.1 ship-blocker 보안 카테고리(S1+R1+S2+T4) 4건 일괄 해소. ① **R-A.1 CSP 명시** — `tauri.conf.json#app.security.csp` 9 directive 명시 (default-src 'self' / script-src + 'wasm-unsafe-eval' / style-src + 'unsafe-inline' / img-src + asset.localhost / font-src data: / connect-src + 127.0.0.1:* + ws://127.0.0.1:* + ipc.localhost / frame-src 'none' / object-src 'none' / base-uri 'self' / form-action 'none'). webview default → explicit allowlist. ② **R-A.1 shell scope 화이트리스트** — `capabilities/main.json::shell:allow-open` scope을 `https://**` + `http://**`(전체 인터넷)에서 4개 도메인(GitHub / HF / jsdelivr / lmstudio.ai)으로 좁힘. ③ **R-A.2 portable import 경로 경계** — `apps/desktop/src-tauri/src/workspace/portable.rs::resolve_import_target(workspace_base, requested) -> Result<PathBuf, PortableApiError>` pure function 신규 (canonicalize + parent canonicalize fallback + starts_with(&base_canon) 검증 + 제어 문자 거부). `start_workspace_import`이 raw `target_workspace_root` 직접 사용 X — 항상 resolve를 거침. workspace 외부 임의 디렉터리 삭제 가능성 차단. ④ **R-A.2 ConflictPolicy default = Rename** — 기존 Overwrite(자동 wipe)에서 Rename(자동 suffix)으로. 사용자 명시 시에만 Overwrite. `default_conflict_policy()` 함수에 ADR-0052 주석. ⑤ **R-A.2 PortableApiError::PathDenied { reason }** thiserror variant 추가, kebab `kind: "path-denied"`, 한국어 "workspace 밖 경로에는 가져올 수 없어요". ⑥ **R-A.4 회귀 invariant 7건** — `cfg(test)` 영역에 `default_conflict_policy_is_rename` / `resolve_import_target_none_returns_workspace_base` / `_empty_string_returns_workspace_base` / `_accepts_subdir` / `_rejects_parent_traversal` / `_rejects_absolute_outside` / `_rejects_control_chars` / `portable_api_error_path_denied_kebab_serialization`. ⑦ **R-A.5 ADR-0052 신설** — `docs/adr/0052-tauri-ipc-path-boundary-and-csp.md` (4 결정 묶음 + 8 기각안). 결정 노트 `docs/research/phase-r-a-security-boundary-decision.md` (6 섹션). ⑧ **분리: S6 (Knowledge IPC tokenized path)** — sub-phase #31로 별도 분리. 8+ IPC + frontend 영향 큼. R-A 후속에서 진행. **검증**: cargo clippy lmmaster-desktop -D warnings clean / portable-workspace 38/38 격리 통과 / lmmaster-desktop --lib test exe Windows DLL 한계는 pre-existing (resolve_import_target은 pure function, 컴파일 + 통합 테스트로 회귀 보호). 다음 standby = **Phase R-B.1 (S3 SQLCipher feature gate)**.
- **2026-05-03~04 Phase 14' v1~v6 — UX/디자인 시스템 라운드** (commit 17건, daa83b4..fe27081). ① **Phase 14' v1**: 컬러 이모지(🤗🔥🔞⚠️) → lucide-react 픽토그램 일괄 교체. HuggingFaceMark 자체 SVG. CLAUDE.md §4.3에 픽토그램 정책 추가. HelpButton React Portal + collision 자동 정렬(트럼 sidebar 잘림 fix). Brand v1 둥근 사각+M zigzag. ② **Phase 14' v2 chunky**: Brand chunky filled + NEW 탭 minimalist outline chip (노랑 → sage). ③ **CI fail 4건 fix**: pnpm/action-setup version 충돌(ERR_PNPM_BAD_PM_VERSION) / RegistryFetcherService E0063 missing field / sign-catalog secret 미설정 graceful skip / hardware-probe macOS unused_unsafe (objc2-metal v0.2 safe wrapper) / @tauri-apps minor sync (api 2.10.1 → 2.11). ④ **Window 사이즈** 1280x800 → 1440x900 (16:10 유지, Linear/Cursor 대비 +120px), minWidth 960→1180. ⑤ **자동화**: `bump-version.mjs` 4 파일 동시 갱신 + `release.yml::generate-notes` 자동 changelog + README "자주 묻는 질문" 보강. ⑥ **MOJITO Lab publisher** 확정 (tauri.conf publisher/copyright + Settings AdvancedPanel + i18n ko/en). ⑦ **자동 업데이트 활성**: `createUpdaterArtifacts: true` + GitHub Secret 등록(`TAURI_SIGNING_PRIVATE_KEY` + password) + `v0.0.1` tag 재push. ⑧ **Phase 14' v3 splash**: SplashScreen 신규 — 8 node + connection arcs + concentric pulse + orbital ring + 5단계 진행 텍스트 cycling + dot indicator. dev 4.5s / prod 3.0s 자동 감지. ⑨ **Phase 14' v4**: Anthropic Imagine 시연 차용 — 좌측 typography "지능을 모으고 [있어요]" + 우측 SVG sphere illusion globe + perspective fake + bezier arc. ⑩ **Phase 14' v5**: Three.js Globe3D 도입 (`pretendard@1.3.9` + `three@0.x` named imports tree-shake). 진정한 3D sphere + WebGL 회전 + 12 노드(Ollama/LM Studio/llama.cpp/Gemma 3/Qwen 2.5/EXAONE 4.0/KURE-v1/bge-m3/Codestral/Whisper/Mistral/Phi-4) HTML overlay 라벨 + 200개 dust particles parallax. dispose chain 17 calls 완전 cleanup. ⑪ **Phase 14' v6 권장안**: Pretendard 폰트 도입(`pretendard` npm) + 컬러 톤다운(`#38ff7e` → `#5eddae` HSL 154/64/62, 채도 -36%) + Brand v4 chunky M letterform + status dot + 글로우 50% 축소 + 노드 0.018/arc opacity 0.32 + atmosphere fog `#3fb887`. 타이포 hierarchy: h1 700 Bold + sub 400 Regular + stat-num 600 + letter-spacing -0.025em + word-break keep-all + max-width 18ch/30ch + h1↔stats 48px gap. ⑫ **사용자 카피 Set 3 채택**: splash "지금 [여기], 나만의 AI" + 홈 "[함께여서] 가능한 것들" (둘 다 sage gradient accent). MOJITO Lab 카피라이트 splash 절대 하단. ⑬ **ModelCard footer**: chip↔CTA hierarchy 분리(space-between + gap 12px) + start CTA ghost border(transparent + 1px primary + bold + arrow translateX(1px) hover, Linear/Stripe). ⑭ **run-dev.bat fix**: LF→CRLF line ending + 한국어 주석 영어 변환 (cmd.exe 토큰화 fail 해결) + cleanup step (port 1420 occupied 시 자동 taskkill). ⑮ **index.html pre-mount splash**: cargo 직후 흰색 frame flicker 제거 (검은 bg + sage radial pulse). 메모리 leak 점검 — 7 영역 모두 cleanup 정상(Tauri unlisten + DOM removeEventListener + setTimeout/setInterval clear + Three.js dispose chain + AnimatePresence GC).
- **2026-05-03 Phase 13'.h.2.b/c.1 머지 — llama.cpp `llama-server` runner + adapter + mmproj 스키마** (ADR-0051 Proposed). ADR-0050 잔여분 v2 마이그레이션이었던 것을 v1.x로 끌어올림. ① **`crates/runner-llama-cpp` 신규 crate** (process lifecycle 분리) — `LlamaServerHandle::start(spec, cancel)` + ServerSpec(model_path/mmproj_path/gpu_layers/ctx_size/chat_template) + ServerEndpoint(base_url/port). 모듈 4종: `port.rs`(TcpListener bind 0 ephemeral), `spawn.rs`(LMMASTER_LLAMA_SERVER_PATH env override + kill_on_drop + Windows CREATE_NO_WINDOW 0x0800_0000 + stderr piped), `health.rs`(/health 200ms × 60초 backoff polling, cancel-aware), `stderr_map.rs`(8 한국어 enum: MmprojMismatch/OutOfMemory/PortInUse/ModelLoadFailed/GpuDeviceLost/RuntimeUnreachable/Crashed/UnsupportedConfig). RunnerError tagged enum kebab-case + 한국어 해요체. **22/22 invariant** (port allocation distinct + env resolve + 6 stderr 패턴 + 한국어 매핑 + health round-trip 503→200 + cancellation). ② **`adapter-llama-cpp::chat_stream`** OpenAI compat `/v1/chat/completions` SSE 스트리밍 + vision content array (`[{type: text}, {type: image_url}]`) — adapter-lmstudio 패턴 재사용. RuntimeAdapter 8 메서드 unimplemented! 모두 제거 (install/update/pull_model/remove_model = bail with 한국어 안내, start/stop/restart = attach 모드, health/list_models/warmup = HTTP). capability_matrix.vision=true (mmproj 적용 시). **10/10 invariant** (detect 200 + unreachable + plain text 변환 + vision content array + empty text 생략 + chat_stream deltas + Cancelled + ModelNotLoaded). ③ **`ModelEntry::mmproj: Option<MmprojSpec>`** 백워드 호환 필드 (보강 리서치 §1.2 표준). MmprojSpec(url/sha256:Option/size_mb/precision:Option/source:Option). gemma-3-4b 백필 — F16 mmproj-model-f16.gguf 851MB ggml-org 출처. **+4 invariant** (legacy without mmproj / round-trip 전 필드 / minimal 필드 / vision entry round-trip). ④ **build-catalog-bundle.mjs validator 보강** — mmproj.url https + huggingface.co/github.com 화이트리스트 + size_mb 양수 + sha256 64-hex 또는 null + precision f16/bf16/f32 + source 큐레이터 known 5종 warning. catalog 40 entries 통과. ⑤ **Workspace Cargo.toml** members += "crates/runner-llama-cpp" + workspace dep 등록. adapter-llama-cpp Cargo.toml: tokio-util/futures/thiserror/runner-llama-cpp/adapter-ollama dep 추가. ⑥ **결정 노트 6-section** (`docs/research/phase-13ph2bc-llama-server-mmproj-decision.md`) — 보강 리서치 §1 (8 영역 + 권장 결정 10건) + §2 채택안 5건 + §3 기각안 8건(Tauri sidecar / 자동 다운로드 / 단일 inline / sha256 강제 등) + §4 미정 11건 (자동 다운로드 13'.h.4 / chat_template_hint 13'.h.3 / known_issues 13'.h.5 / Job Object / RAM 사전 검증) + §5 invariant 매트릭스 + §6 Phase 분할 인계. ADR-0051 신설 (Proposed → 본 sub-phase 머지로 Accepted 승급 가능). ⑦ **격리 검증** (lmmaster-desktop test exe 환경 문제로 풀 verify 보류 — RESUME 기존 표시 일관): cargo fmt clean / clippy clean / runner-llama-cpp 22 + adapter-llama-cpp 10 + model-registry 62 (4 신규 mmproj) = **94/94 격리 통과**. **다음 sub-phase 인계 = Phase 13'.h.2.d (chat IPC LlamaCpp 분기 wiring + Tauri State LlamaCppRunnerState 보관 + 단일 instance 재사용 정책)**. v1.x 후속: 13'.h.2.c.2 (mmproj 자동 다운로드 IPC) / 13'.h.3 (chat_template_hint) / 13'.h.4 (binary 자동 다운로드 + GPU detect) / 13'.h.5 (known_issues 마커).
- **2026-05-01 Phase 13'.h.2.a — LM Studio chat + vision 어댑터 머지** (ADR-0050 부분 채택 확장). 기존 `start_chat`이 LM Studio runtime_kind에 "v1.x 후속" unsupported error 반환하던 것을 *실제 OpenAI compat 호출*로 wire. ① **adapter-lmstudio Cargo.toml** — adapter-ollama dep 추가 (ChatMessage/ChatEvent/ChatOutcome 공유로 frontend 단일 시그니처 유지). ② **`LmStudioAdapter::chat_stream(model_id, messages, on_event, cancel)`** 신규 — `/v1/chat/completions` SSE 스트리밍 + `[DONE]` 마커 감지 + cancel→stream drop. ③ **vision content array 변환** — `convert_message_to_openai(m)` 헬퍼: `images` 비어있으면 plain text content, 있으면 `[{type: "text"}, {type: "image_url", image_url: {url: data:image/jpeg;base64,...}}]` OpenAI 표준. ④ **OpenAI compat DTO** 6 struct (Request/Turn/Content untagged enum/ContentPart tagged enum/ImageUrl/Chunk/Choice/Delta) — 기존 warmup용 `ChatRequest`/`ChatMessage`와 이름 충돌 회피(별개 모듈 영역). ⑤ **`chat/mod.rs::start_chat` LmStudio 분기** — unsupported error → `LmStudioAdapter::new().chat_stream` 호출. Ollama와 동일 ChatEvent 흐름. **검증**: cargo fmt clean / clippy clean (redundant_closure fix) / adapter-lmstudio 12 + adapter-ollama 21 = **33/33** / tsc clean / ACL drift 0 (변경 없음). 결과: 카탈로그 `vision_support: true` 모델(gemma-3-4b)을 Ollama + LM Studio 두 runtime 모두에서 사용 가능. **남은 13'.h.2.b/c (llama.cpp server 자동 spawn + mmproj sidecar)는 v2 마이그레이션** — Ollama/LM Studio 두 어댑터로 vision 사용 시 일반 사용자 가시 거의 100% 커버.
- **2026-05-01 Phase 13'.g.2.c + 13'.g.2.d 병렬 머지 — v1.x 보안 라인업 완성** (ADR-0047 완전 채택). **Track A1**: `FetcherError::SignatureFailed` + `SignatureMissing` variant + `FetcherCore::fetch_one_with_signature(id, verifier)` 메서드 — body fetch + .minisig fetch + verify. cache hit / Bundled tier verify skip. **Track A2**: `RegistryFetcher`에 `signature_url_for` / `fetch_signature_text` / `source_timeout` 3 helper 노출. desktop `registry_fetcher.rs::verify_catalog_signature` async helper + `CatalogSignatureStatus` enum 5 variant (Disabled / Verified / Failed / MissingSignature / BundledFallback). `refresh_once`가 catalog body 받으면 verify 시도, 실패 시 catalog_body=None으로 강등(bundled fallback). `last_signature_status` field 보존. **Track A3**: `get_catalog_signature_status` IPC + ACL identifier `allow-get-catalog-signature-status` (drift 83→86). `Diagnostics.tsx`에 `SignatureSection` 신규 — 5 variant tone(ok/warn/error/neutral) + 한국어 카피 + role=alert 빨간 카드(failed). i18n `diagnostics.signature.{title/empty/checkedAt/verified/failed/missing/bundled/disabled}` × ko/en. `diagnostics.css`에 tone별 border-left 토큰. **Track B**: `.github/workflows/sign-catalog.yml` 신규 — main push 시 `manifests/apps/catalog.json` 변경 감지 후 `rsign sign` + .minisig 자동 commit/push. self-check verify 단계 + `[skip-sign]` opt-out. `.github/SECRETS_SETUP.md`에 §1.b 신설 — keypair 등록 + 빌드 시점 env + CI_PUSH_TOKEN. **검증**: cargo fmt clean / clippy clean / model-registry 80 + registry-fetcher 30+4=**34/34** / tsc clean / ACL drift 0 (86 ids / 83 commands) / build-catalog 40 entries.
- **2026-05-01 Phase 13'.f.2.4 + 13'.g.2.b 부분 병렬 머지**. **Track A (큐레이션 종료)**: 잔여 5 모델 manifest — `roleplay/mythomax-l2-13b.json` (영어 RP 클래식, Llama 2 Community), `roleplay/synatra-7b-v0.3-rp.json` (한국어 RP, **CC-BY-NC commercial:false**, ko-conversation 72 + roleplay 80), `agents/aya-expanse-32b.json` (다국어 32B, **CC-BY-NC commercial:false**, translation-multi 88 + ko-conversation 76), `slm/yi-1.5-6b-chat.json` (Apache-2 small), `agents/mixtral-8x7b-instruct.json` (Apache-2 SMoE, VRAM 28GB+ 워닝). 카탈로그 35 → **40 entries**. **거부 유지** — CodeLlama 7B/13B/34B (Codestral 22B + Qwen 2.5 Coder 7B 우위), StarCoder2 3B/7B/15B (instruct 약함, base completion만). v1.x 큐레이션 페이즈 **종료** (목표 +30 모델 중 +24 완료, 6 거부 유지 결정). **Track B (보안 부분)**: `SourceConfig::resolve_signature_url(manifest_id)` 헬퍼 신설 — body URL + ".minisig" 패턴, Bundled tier는 graceful Err. **+4 invariant** (jsdelivr/github 패턴 / Bundled reject / bad-id reject). FetcherCore::fetch_one_with_signature 완전 wiring + caller 통합은 13'.g.2.c (Diagnostics 빨간 카드)와 묶음. **검증**: cargo fmt clean / clippy clean / model-registry 80 + registry-fetcher 30+4=**34/34** (signature 5/5 + source 11/11 +4 신규) / build-catalog 40 entries / tsc clean / ACL drift 0.
- **2026-05-01 Phase 13'.f.2.3.1 — NSFW 게이팅 UI + 비상업 chip + 시드 4 모델**. ① **`useAdultContentAllowed` hook 신규** — `lmmaster.adult_content_allowed` localStorage 영속, storage event sync. 외부 통신 0 정책 준수. ② **Catalog 헤더 토글** "🔞 성인 모델 보임/숨김" — role=switch + aria-checked. 기본 OFF. ③ **`visible` useMemo content_warning 필터** — adultAllowed=false면 `content_warning === "rp-explicit"` 모델 hidden (전체/카테고리/검색/필터 모두 적용). ④ **ModelCard chip 2종** — `🔞 성인` (content_warning rp-explicit) + `⚠ 비상업` (commercial=false). i18n + tooltip. ⑤ **TS 미러** ipc/catalog.ts — `ModelPurpose`/`ContentWarning` union + ModelEntry에 `purpose?`/`commercial?`/`content_warning?` 필드. ⑥ **시드 4 모델** — `roleplay/stheno-l3-8b.json` (rp-explicit, Llama 3 Community, RP 시드), `agents/aya-expanse-8b.json` (CC-BY-NC, **commercial: false**, 23 언어 — translation-multi 82 + ko-en 75 + ko-conversation 70), `agents/llama-3.1-70b-instruct.json` (Llama 3.1 Community, 워크스테이션, 700M+ 워닝), `agents/yi-1.5-34b-chat.json` (Apache-2 워크스테이션 대안). 카탈로그 31 → **35 entries**. ⑦ **i18n** — `catalog.adultContent.{on/off/toggleTitle/chip/chipTitle}` + `catalog.commercial.{chip/chipTitle}` × ko/en. ⑧ **catalog.css** — `.catalog-adult-toggle` (노란 액티브 상태) + `.catalog-card-chip-adult/-noncommercial` 디자인 토큰만. **검증**: cargo fmt clean / model-registry 80 + registry-fetcher 30 / build-catalog 35 entries / tsc clean / catalog vitest 57/57 / ACL drift 0. **NSFW 정책 = 식별 + 사용자 토글 게이팅** (차단 X) — 큐레이션 thesis와 사용자 자율성 동시 충족.
- **2026-04-30 Phase 13'.f.2.2.1 + 13'.g.2.a 병렬 머지**. **Track A (큐레이션)**: ① `commercial: bool`(default true) + `content_warning: Option<ContentWarning>` 필드 + `ContentWarning::RpExplicit` enum. `serde(default)`/`default_commercial()` 백워드 호환. ② **시드 3 모델** — `agents/llama-3.1-8b-instruct.json` (Llama 3.1 Community, agent-tool-use 78 + translation-multi 70 + 700M+ 워닝), `agents/kullm3.json` (CC-BY-NC-4.0, **commercial: false**, ko-conversation 86 + ko-rag 75), `roleplay/nous-hermes-2-mistral-7b-dpo.json` (Apache-2, roleplay-narrative 80). 카탈로그 28 → **31 entries**. **Track B (보안)**: ① `signature.rs::SignatureVerifier::from_embedded()` — `option_env!("LMMASTER_CATALOG_PUBKEY"/_SECONDARY)`로 빌드 시점 임베드. env 미설정 시 `Ok(None)` (개발 빌드 graceful). ② 신규 invariant `from_embedded_graceful_when_env_unset`. **부분 분할**: Track B의 13'.g.2.b (FetcherCore .minisig wiring + body URL 패턴 + cache verify mode) + g.2.c (Diagnostics 빨간 카드) + g.2.d (CI 서명 파이프라인)는 별개 sub-phase로 분할 deferred — body URL 패턴이 source 4-tier별로 달라 큰 작업. **검증**: cargo fmt clean / clippy clean / model-registry 80/80 + registry-fetcher 30/30 (signature 5 + 1 신규 from_embedded) / build-catalog 31 entries / tsc clean / ACL drift 0.
- **2026-04-30 Phase 13'.f.2.1 — ModelPurpose 분기 + 시드 4 모델 머지**. ① **`ModelPurpose` enum 신규** (`general-chat` 기본 / `fine-tune-base` / `retrieval` / `reranker`) — `serde(default)` 백워드 호환 + 4 invariant (default / kebab-case round-trip / legacy fallback). ② **Recommender purpose 분기** — `purpose != GeneralChat` 모델은 chat target에서 자동 `ExclusionReason::PurposeMismatch`로 제외 + 2 신규 invariant (Retrieval excluded / FineTuneBase excluded). ③ **시드 4 모델 manifest** — `embeddings/bge-m3.json` (RAG 표준, MIT, ko-rag 75) + `embeddings/kure-v1.json` (한국어 SOTA, MIT, ko-rag 88) + `agents/yi-ko-6b.json` (fine-tune 베이스, Apache-2) + `slm/yi-1.5-9b.json` (다국어 일반, Apache-2). 모두 community_insights 4-section 작성. ④ **Catalog SIDEBAR_TABS에 `embeddings` 추가** + i18n `catalog.category.embeddings/rerank` ko/en 동시. ⑤ **`build-catalog-bundle.mjs`** — 24 → **28 entries** validator 통과. ExclusionReason match 4곳에 `PurposeMismatch` arm 추가 (recommender_test.rs). **검증**: cargo fmt clean / clippy clean / model-registry 58 + 22 = **80/80** (ModelPurpose 3 + recommender 2 + 기존 75 0건 깨짐) / build-catalog 28 entries / vitest 74/74 (catalog 컴포넌트 + pages) / tsc clean / ACL drift 0. 결정 노트 §3 임베딩 카테고리 신설 + §5 fine-tune 베이스 카테고리 핵심 흐름 ship 가능.
- **2026-04-30 Phase 13'.h.1 — Ollama vision IPC + Chat 이미지 첨부 머지** (ADR-0050 부분 채택 확장 — 12'.a + 12'.b + 13'.h.1). 결정 노트 §2.6 + §5.4 + §6 (위험 매트릭스 — base64 페이로드) 구현. ① **`adapter_ollama::ChatMessage`에 `images: Option<Vec<String>>` 필드 추가** — `serde(default)` + `skip_serializing_if = Option::is_none`로 백워드 호환 100%. ② **TS `ipc/chat.ts` ChatMessage 미러** — 같은 시그니처. ③ **신규 IPC 0** — 기존 `start_chat`이 자동으로 vision 메시지 통과 (Ollama가 `messages[i].images` 자동 인식). ACL 변경 0건. ④ **`apps/desktop/src/lib/image.ts`** 신규 — `processImageForVision(blob)`: max 4096px 리사이즈 + JPEG 90% 압축 → base64. `scaleToMax`/`stripDataUrlPrefix` pure helper + 9 unit invariant (jsdom canvas 미구현으로 e2e 부분만 부분). ⑤ **`Chat.tsx` paperclip + 첨부**: `selectedEntry.vision_support` true일 때 paperclip 활성, file input + 미리보기 썸네일 + 제거 버튼 + 에러 alert. 전송 시 `attached.map((a) => a.base64)`를 `userTurn.images`에 채움. ⑥ **`urlencoding` crate** — Phase 11'.c에서 이미 추가됨, 13'.h.1은 추가 의존성 0. ⑦ **chat.css** — 첨부 row + thumb + remove + bubble-images (디자인 토큰만, 인라인 색 0). ⑧ **외부 통신 0 정책 준수** — base64 인코딩 + 압축 모두 클라이언트 브라우저 내. **검증**: cargo fmt clean / clippy adapter-ollama+model-registry+shared-types clean / adapter-ollama 17+4=**21/21** (ChatMessage round-trip 4 신규 + 백워드) / image.ts 9/9 / Catalog 9/9 (ModelCard listitem.button → listitem.textContent로 narrowing) / 전체 vitest 119+ / tsc clean / ACL drift 0.
- **2026-04-30 Phase 12'.b — RAG 시드 진입점 (Stage 1) 머지** (ADR-0050 부분 채택 확장 — 12'.a + 12'.b). 결정 노트 §2.5 Stage 1 + §5.4 구현. ① **`RagSeedStep`** 신규 — 의도별 권장 메시지(`ko-rag` → KURE-v1 권장, `vision-*` → graceful "v2에서 지원" 안내), path text input + ingest 시작(폴더 picker는 Tauri dialog plugin 미설치로 v2+ deferred), 진행 상태 라이브 표시(reading/chunking/embedding/writing → done), Workspace deep link. ② **knowledge-stack 재활용** — `startIngest`/`cancelIngest`/`isTerminalIngestEvent` IPC + `ActiveWorkspaceContext::useActiveWorkspace`로 workspace_id 자동 주입. ③ **Workbench.tsx 통합** — `PromptTemplateStep` 다음에 `RagSeedStep` 마운트 (URL hash 있을 때만). reducer/STEP_KEYS 변경 0 → **기존 17 invariant 0건 깨짐** 보장. ④ **i18n ko/en** — `screens.workbench.ragSeed.{title, subtitle, koRagTip, visionDeferred, pathLabel/Placeholder/Hint, start, running, workspaceLink, noWorkspace, modelHintLabel, startFailed, done, stage.{reading,chunking,embedding,writing}}` 18 키 × 2 locales. ⑤ **workbench.css** — Stage 1 카드 + 권장 메시지 + deferred + status (디자인 토큰만, prefers-reduced-motion 자동 비활성). ⑥ **외부 통신 0 정책 준수** — knowledge-stack은 로컬 SQLite + ONNX 임베더, 외부 호출 0. **검증**: cargo fmt clean / tsc clean / vitest 12 file **110/110** (RagSeedStep 10 신규 + 기존 100 0건 깨짐). 결정 노트 §5.4 모든 invariant 충족.
- **2026-04-30 Phase 12'.a — Workbench 컨텍스트 바 + 프롬프트 템플릿 layer 머지** (ADR-0050 부분 채택). 결정 노트 §2.5 + §5.4 + §7.3 구현 (3단 사다리 중 Stage 0). **비파괴 layer 패턴** — URL hash 없으면 기존 5단계 default 진입(기존 사용자 0 영향), hash 있으면 컨텍스트 바 + Stage 0가 stepper 위에 노출. ① **`hash.ts` + 11 unit 테스트** — `parseWorkbenchHash`/`buildWorkbenchHash` (`#/workbench?model=X&intent=Y`). ② **`WorkbenchContextBar`** — 의도 + 모델 칩 + "변경" 버튼(Catalog 라우팅) + 7 vitest a11y. ③ **`PromptTemplateStep` (Stage 0)** — `use_case_examples` 카드 그리드 + 클립보드 복사 + "내 패턴 저장" localStorage(`lmmaster.prompts.<intent>`) + "더 깊게 (LoRA)" CTA(stepper로 scroll) + 9 vitest. ④ **Workbench.tsx 통합** — URL hash 파싱 useEffect + hashchange listener + getCatalog로 ModelEntry lookup + stepperRef로 advance scroll. STEP_KEYS/reducer 변경 0 → **기존 17 invariant 0건 깨짐**. ⑤ **Catalog ModelCard "이 모델로 시작 →" CTA** — 의도 + 모델로 hash 라우팅. ⑥ **i18n ko/en** — `screens.workbench.{context, promptTemplate}.*` + `catalog.card.startWorkbench` 19 키 × 2 locales. ⑦ **catalog.css + workbench.css** — 컨텍스트 바 / Stage 0 카드 그리드 / advance CTA / toast. 디자인 토큰만(인라인 색 0). prefers-reduced-motion 자동 비활성. **검증**: cargo fmt clean / clippy clean / tsc clean / vitest 11 file 100/100 (workbench 27 신규 + catalog 48 + 기존 25). 외부 통신 0 정책 준수(localStorage 사용, 파일 IPC는 v2+ deferred). RAG 시드(12'.b) + 비전 IPC(13'.h)는 후속 sub-phase.
- **2026-04-30 Phase 11'.c — HuggingFace 하이브리드 검색·바인딩 머지** (ADR-0049 채택). 결정 노트 §2.4 + §5.3 + §7.2 구현. ① **`apps/desktop/src-tauri/src/hf_search.rs`** 신규 — HF Hub Search API(`GET /api/models?search=...&limit=20&sort=likes`) + 한국어 graceful 에러(`HfSearchError` 4 variant kebab-case) + `curation_request_url(repo)` 빌더 + 5 unit invariant. 외부 통신 ADR-0026 §1 화이트리스트(`huggingface.co`) 기존. `urlencoding` crate 추가. ② **2 IPC** — `search_hf_models(query)` + `register_hf_model(repo, file?)` (CustomModel 자동 매핑, modelfile에 한국어 워닝 prepend). ACL 2 신규 identifier + `permissions/hf_search.toml`. drift 0 (82 → 85). ③ **TS IPC** `apps/desktop/src/ipc/hf_search.ts` — Rust 미러 + frontend `curationRequestUrl(repo)` (URLSearchParams 기반 GitHub Issue prefilled URL). ④ **HfSearchModal** 신규 — role=dialog + aria-modal + Esc/배경 클릭 닫기 + 노란 ⚠ 배너 + "큐레이션 외" 배지 + downloads/likes/lastModified 미리보기 + 두 CTA(지금 시도 / 큐레이션 추가 요청). 등록 성공 → onRegistered 콜백 → ModelDetailDrawer 자동 진입. ⑤ **Catalog.tsx 통합** — 검색바 옆 `🤗 HuggingFace` 트리거 버튼 + HfSearchModal 마운트 + customModelToEntry 매핑으로 등록 후 즉시 drawer 노출. ⑥ **GitHub Issue 폼** `.github/ISSUE_TEMPLATE/curation-request.yml` — repo / 사용 의도 / 라이선스 검토 dropdown / 추가 메모 4 필드. ⑦ **외부 통신 0 정책 준수** — 자동 POST 거부, `tauri-plugin-shell::open`으로 시스템 브라우저만. ⑧ **i18n ko/en** — `catalog.hfSearch.{title, banner, tryNow, requestCuration, ...}` 14 키 × 2 locales. ⑨ **catalog.css** — modal/banner/hits/actions 디자인 토큰만 (인라인 색 0). **검증**: cargo fmt clean / clippy clean / shared-types 7 + model-registry 75 + 환경 가능 시 hf_search 5 invariant / build-catalog 24 entries / tsc clean / vitest catalog 48/48 (HfSearchModal 10 신규) / ACL drift 0. 결정 노트 §5.3 모든 invariant 충족.
- **2026-04-30 Phase 11'.b — Catalog 의도 보드 + Recommender 의도 가중 머지** (ADR-0048 후속). 결정 노트 §2.2 + §2.3 + §5.2 + §7.1 구현. ① **Recommender 시그니처 wrapper 분리** — `compute()`/`Catalog::recommend()` keep + `compute_with_intent()`/`recommend_with_intent()` 신규 (intent: Option<&IntentId>). 기존 16+1 caller 0 변경 = backward compat 100% 보장. evaluate()에 `domain_scores[intent]*0.4 + intents.contains(intent)*5` 가중 적용. ② **신규 4 invariant** (intent_none_matches_legacy / high_score_wins / unknown_id_graceful_no_op / determinism 100회) — 기존 16 invariant 0건 깨짐. ③ **TS 미러** — `apps/desktop/src/ipc/catalog.ts`에 `IntentId` type + `intents`/`domain_scores` 필드 + `getRecommendation(category, intent?)` 시그니처 확장. ④ **IntentBoard 컴포넌트** 신규 (`apps/desktop/src/components/catalog/IntentBoard.tsx`) — INTENT_VOCABULARY 11종 inline + `radiogroup` 12 radio + a11y 6/6 vitest 통과. ⑤ **Catalog.tsx 통합** — intent state + IntentBoard 마운트 + recommendation useEffect deps에 intent 추가. ⑥ **ModelCard 도메인 점수 바** — intent 선택 + domain_scores[intent] 존재 시 mono-numeric 점수 바 (--primary accent + tabular-nums). ⑦ **catalog.css** — IntentBoard chips + 점수 바 토큰만 (인라인 색 0). prefers-reduced-motion 자동 비활성. ⑧ **i18n ko/en** — `catalog.intent.{heading,all,scoreAria,11종}` × 2 locales. **검증**: cargo fmt clean / clippy clean / shared-types 7 + model-registry 55+20=75 / build-catalog 24 entries 통과 / tsc clean / vitest catalog 38/38. 결정 노트 §5.2 모든 invariant 충족.
- **2026-04-30 Phase 11'.a — Intent 축 + domain_scores 스키마 머지** (ADR-0048 채택). 결정 노트 §2.1 + §5.1 따라 ① `crates/shared-types/src/intents.rs` 신규 — `INTENT_VOCABULARY` 11종 시드 + `is_registered_intent`/`intent_label_ko` 헬퍼 + 7 신규 invariant (kebab-case, 중복, Hangul 라벨, registered/unknown, label round-trip). ② `crates/model-registry/src/manifest.rs` — `ModelEntry`에 `intents: Vec<IntentId>` + `domain_scores: BTreeMap<IntentId, f32>` 필드 추가 (`#[serde(default)]` 호환), `ManifestValidationError` enum + `validate_entry` 함수 + 9 신규 invariant (legacy 호환 + UnknownIntent/ScoreOutOfRange/DuplicateIntent reject + boundary 0/100 + round-trip + 한국어 메시지). ③ `build-catalog-bundle.mjs` — `INTENT_VOCABULARY`를 `intents.rs` 정규식 파싱 (SSOT 자동 동기화) + entry validator 통합 (vocab/range/duplicate). ④ 시드 백필 8개 모델 (gemma-3-4b vision/agent, codestral-22b coding-fim, qwen-2.5-coder-7b coding, whisper-large-v3-korean voice-stt, exaone-3.5-7.8b ko-conversation/agent/translation, hcx-seed-1.5b ko-conversation/rag, llama-3.3-70b agent-tool-use/translation-multi/ko-conversation, polyglot-ko-12.8b roleplay/ko-conversation). 11종 intent 모두 시드 사용 검증. **검증**: cargo fmt clean / clippy clean / shared-types 7/7 / model-registry 55+16=71/71 (기존 16 invariant 0건 깨짐) / build-catalog-bundle 24 entries 통과 / tsc clean (frontend 영향 0). ADR-0048 Proposed → Accepted.
- **2026-04-30 v1.x 도메인 축 thesis 채택** (사용자 명시 승인): "의도(intent) → 하드웨어 적합 → 도메인 벤치마크 → 모델 셀렉트 → 도메인 특화 물꼬"가 v1.x로 채택됨. 카테고리 enum 확장 거부, intent 자유 태그 N:N + `domain_scores: BTreeMap<IntentId, f32>`로 풀이. HF 검색은 하이브리드(C — 큐레이션 1급 + "지원 외" 별도 진입점). Workbench는 3단 사다리(프롬프트→RAG→LoRA)로 재배치. 영상 분석 / 자세 분석 / 자유 텍스트 의도 매핑은 v2+ 분리. 종합 결정 노트 `docs/research/phase-11p-12p-v1x-domain-axis-decision.md` (8 섹션 + UI/UX 와이어 + 페이즈 분할 + 의존성 그래프 + 위험 매트릭스 + 12건 기각안). ADR-0048 (Intent 축), ADR-0049 (HF 하이브리드), ADR-0050 (Workbench 사다리 + 비전 IPC) Proposed. DEFERRED 우선순위 1에 Phase 11'.a 등록. 메모리 `v1x_roadmap_2026_04_30.md` 신규.
- **2026-04-30 Phase 13'.g (ADR-0047) — minisign 카탈로그 서명 검증 (infrastructure only)**: Phase 13'.a로 jsDelivr → GitHub raw 카탈로그가 무서명이라 변조 위험. 본 sub-phase는 *검증 코드만* 머지 — 실 keypair 임베드 + CI 서명 파이프라인 + Diagnostics 빨간 카드는 v1.x. ① **`minisign-verify` v0.2** crate 추가 (zero-deps Rust verify-only). ② **`registry_fetcher::signature::SignatureVerifier`** — primary + (optional) secondary pubkey, dual key 90일 overlap 패턴 (Tauri Updater + reachy-mini KEY_ROTATION 차용). ③ **`SignatureError`** 4 variant 한국어 (`NoPublicKey`, `InvalidPublicKey`, `InvalidSignature`, `VerifyFailed`). ④ **단위 테스트** 4 신규 + 1 ignored — 형식 거부 / 한국어 메시지 / round-trip placeholder. **검증**: registry-fetcher 20+9 통과 (4 신규 + 1 ignored), cargo workspace clippy clean, fmt clean, ACL drift 0. ADR-0047 + 결정 노트 `phase-13pg-catalog-signature-decision.md` (기각안 12건 — rsign2/rust-minisign 풀/ed25519-dalek 직접/Sigstore/TUF/age/Vault/단일 키/silent fallback 등).
- **2026-04-30 Phase 13'.f — 큐레이션 +4 (20→24)**: 사용자 명시 +22 큐레이션을 토큰 예산 분할. 영향이 가장 큰 4개만 본 sub-phase에 작성하고 나머지 18개는 Phase 13'.f.2로 인계. ① **HCX-Seed 1.5B** (네이버, slm/, NEW tier) — 한국어 SLM 신상, NEW 탭 첫 입주자. ② **Codestral 22B** (Mistral, coding/, verified) — 시스템 언어 코드 + FIM 강자, *비상업 라이선스 경고* 명시. ③ **Qwen 2.5 1.5B** (slm/, verified, Apache-2) — 멀티링구얼 + 상업 자유. ④ **Llama 3.3 70B** (agents/, verified) — 워크스테이션급 플래그십. **검증**: model-registry 16/16 통과 (24 entries로 늘어나도 모든 host bucket 시나리오 유지), build-catalog-bundle.mjs 중복 id 0, schema invariant (maturity / verification.tier enum) 모두 준수. 결정 노트 `phase-13pf-curation-plus4-decision.md` (기각안 17건 — KULLM3/Synatra/Yi-Ko/Phi-3.5/CodeLlama/StarCoder2/Mixtral/Llama 3.1/임베딩 3종/RP 3종 등 deferred 사유 포함).
- **2026-04-30 Phase 13'.c — API 키 scope 편집 + Crash 뷰어**: Phase 13'.b 후속. 부분 노출이던 키 관리에서 사용자가 *평문 재발급 없이* scope을 갱신할 수 없던 문제 + Diagnostics에서 panic_hook이 적재한 crash 파일을 *볼 길이 없던* 빈 슬롯 둘을 동시에 메움. ① **`KeyManager::update_scope`** + `KeyStore::update_scope` 추가 (5 신규 invariant 테스트). key_prefix / key_hash / created_at 보존. revoked 키 편집 허용. ② **IPC `update_api_key_scope`** + `KeyApiError::EmptyScope` (models + endpoints 둘 다 빈 scope 거부, 무용 키 차단). ③ **`ApiKeyEditModal`** 신규 컴포넌트 — alias / key_prefix read-only + origins / models / endpoints / expires_at / pipelines override 편집. i18n `keys.editModal.*` + `keys.actions.edit` + `keys.errors.{emptyScope, updateFailed}` ko/en 동시. ④ **`crash` 모듈 신규** — `panic_hook::crash_dir()` read-only 노출 후 `list_crash_reports(limit)` (mtime DESC, 50 default) + `read_crash_log(filename)` (1 MB cap + path traversal 방어 5건 단위 테스트). ⑤ **CrashSection (Diagnostics 5번째 row, full-width)** — toggle expand로 `<pre>` 본문 조회 + 4분기 (빈/미초기화/TooLarge/일반) 한국어. ⑥ ACL 3 신규 (`allow-update-api-key-scope`, `allow-list-crash-reports`, `allow-read-crash-log`). **검증**: key-manager 74/74 (5 신규), cargo workspace clippy clean, fmt re-applied (8 파일), tsc clean, ACL drift 0 (80/83 mapped). 결정 노트 `phase-13pc-key-edit-crash-viewer-decision.md` (기각안 10건).
- **2026-04-30 Phase 13'.b — Gateway metrics middleware + Diagnostics 실 데이터** (ADR-0046): Diagnostics 페이지 4개 MOCK (latency / 최근 요청 / bench batch / repair history)이 사용자 신뢰도 직격 → 모두 실 IPC로 wire. ① **`crates/core-gateway::usage_log::GatewayMetrics`** 신규 — 메모리 ring buffer (60s latency / 50 recent). `record_metrics` Tower middleware로 path-only 저장 (PII 차단). `build_router` ServiceBuilder 외측 mount. ② **3 IPC 신규** — `get_gateway_latency_sparkline` (30 bucket 평균) / `get_gateway_recent_requests` / `get_gateway_percentiles` (p50/p95). ③ **`bench::cache_store::list_recent`** + IPC `list_recent_bench_reports` (file mtime scan, sort 후 N개 deserialize). ④ **`workspace::get_repair_history`** + JSONL append-only at `app_data_dir/workspace/repair-log.jsonl` — `check_workspace_repair`가 tier != green 시 자동 append. ⑤ **Frontend Diagnostics** — MOCK 4건 모두 제거, 5초 polling (gateway 3건 동시 `Promise.all`) + mount 시 1회 bench/repair fetch. ⑥ ACL 5 신규 identifier. **검증**: core-gateway 13/13 (5 신규 invariant), cargo workspace clean, tsc clean, ACL drift 0 (77/80 mapped). 결정 노트 `phase-13pb-gateway-metrics-decision.md` (기각안 8건).
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

## 🟢 다음 세션 진입 가이드 (Standby — 3종 병렬 가능)

**v1.x 도메인 축 + 보안 라인업 종료 — v1 ship 가능 상태 (2026-05-01)**

| 페이즈 | 상태 |
|---|---|
| 11'.a/b/c (Intent + Recommender + HF 하이브리드) | ✅ |
| 12'.a/b (Workbench 3단 사다리) | ✅ |
| 13'.h.1 (Ollama vision IPC + Chat 첨부) | ✅ |
| 13'.f.2.* (큐레이션 +24, 40 entries) | ✅ |
| **13'.g.2.* (minisign infrastructure + wiring + Diagnostics + CI)** | ✅ |

**다음 standby — 사용자 명시 결정(2026-05-03) 따라 13'.h.2.d → e → f 순차 진행** (총 9-13h):
- **13'.h.2.d** (3-4h) — chat IPC LlamaCpp 분기 wiring + Tauri State 단일 instance + ExitRequested 훅 + 모델 로드 진행률 emit.
- **13'.h.2.e** (4-6h) — `LlamaCppSetupWizard` 7단계 stepper(GPU 자동 감지 → 권장 빌드 → 다운로드 페이지 → 압축 → 환경변수 → 자동 검증 → mmproj). Phase 12' guide system 인프라 차용.
- **13'.h.2.f** (2-3h) — `manifests/guides/llama-cpp-setup.json` + registry-fetcher manifest_id 등록 + minisign 검증(ADR-0047 차용) + CI 자동 서명 + 큐레이터 SOP. 큐레이터가 빌드 회귀 발견 시 6h 내 모든 사용자에 가이드 갱신.
- 진입 조건 ✅ — runner-llama-cpp + adapter-llama-cpp + MmprojSpec + Phase 12' guide system + registry-fetcher + ADR-0047 minisign 모두 머지.
- DEFERRED.md §13'.h.2.d/e/f 참조.

**v1.x 후속 (D/E/F 머지 후)**:
- 13'.h.2.c.2 (mmproj 자동 다운로드 IPC, 3-4h)
- 13'.h.3 (chat_template_hint 카탈로그 필드, 2-3h)
- 13'.h.4 (binary 자동 다운로드 + GPU detect, 6-8h) — 가이드 채택 후 ROI 재평가, 가이드만으로 일반 흐름 충족 시 v2로 deferred 가능
- 13'.h.5 (known_issues 마커, 2-3h) — 가이드 manifest known_issues와 일관 통합
- 13'.h.6 (Windows Job Object + ExitRequested 훅, 1-2h)
- **사용자 keypair + 빌드 시점 env 적용** — `.github/SECRETS_SETUP.md` §1.b 따라 등록 후 첫 release 빌드. CI 파이프라인이 자동 .minisig 생성.

**(Phase 13'.f.2 시리즈 종료 2026-05-01 — 큐레이션 페이즈 완성)**
- v1.x 카탈로그 **40 entries**. 카테고리 — agents 16 / coding 5 / roleplay 5 / slm 7 / sound-stt 1 / embeddings 2 / fine-tune-base 1 (Yi-Ko 6B). 라이선스 — Apache-2 / MIT 압도적 다수, CC-BY-NC 4건(KULLM3 / Aya 8B/32B / Synatra), Llama Community 4건(Llama 3.1/3.3 + Stheno + Mythomax), 기타 2건. NSFW 라벨 1건(Stheno) — 사용자 토글 게이팅. CodeLlama / StarCoder2는 거부 유지(Codestral 22B + Qwen 2.5 Coder 7B 우위 명확).
- 결정 노트: `docs/research/phase-13pg-catalog-signature-decision.md` §4
- 작업: a) `build.rs` env pubkey 임베드 b) FetcherCore 서명 검증 wiring c) Diagnostics 빨간 카드 d) CI 서명 파이프라인.
- 진입 조건: 0 (즉시).

**3순위 — Phase 13'.h.2 — llama.cpp 멀티모달 서버 분기** (ADR-0050 잔여, 6-8h)
- llama.cpp server 모드 (`llama-server --image` 또는 LLaVA endpoint) + 어댑터 분기 + 모델 검증.
- 진입 조건: ✅ 13'.h.1 머지. v1.x 후순위 — Ollama vision만으로도 카탈로그 vision 모델(gemma-3-4b) 사용 가능.

**(Phase 13'.h.1 완료 2026-04-30 — 본 standby에서 graduated)**
- ADR-0050 부분 채택 확장(12'.a + 12'.b + 13'.h.1). v1.x 도메인 축 thesis 핵심 흐름(의도 → 모델 → 도메인 점수 → 셀렉트 → 도메인 특화 + 비전 사용) ship 가능.

**2순위 — Phase 13'.g.2 — minisign wiring (a/b/c/d 4단계)** (사용자 명시 "갱신해" 승인 2026-04-30, 4-6h)
- 결정 노트: `docs/research/phase-13pg-catalog-signature-decision.md` §4
- 인프라 머지 완료 (ADR-0047) — keypair 빌드 임베드 + FetcherCore wiring + Diagnostics 빨간 카드 + CI 서명 파이프라인

**3순위 — Phase 13'.f.2 — 큐레이션 잔여 18 모델** (6-8h)
- DEFERRED.md 우선순위 1 참조

**(deferred to ADR-0048 머지 후)** Phase 9'.c — Multi-runtime adapters (이전 standby, ~3-4시간)

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
6. ~~**Publisher명** 확정 → `bundle.publisher`~~ — **2026-05-03 확정**: `MOJITO Lab` (tauri.conf.json publisher/copyright + Settings AdvancedPanel publisher/copyright dl + i18n ko/en `screens.settings.advanced.{publisher,copyright,copyrightValue}` 6 키 동시 갱신).

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
