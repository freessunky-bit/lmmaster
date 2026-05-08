# DEFERRED — 후순위 이월 작업 단일 진입점

> **목적**: 다음 세션 / 다음 사용자가 *어떤 후속 작업이 남아 있는지* 한눈에 보고, 진입 조건과 의존성을 빠르게 확인할 수 있게 모아둔 인덱스.
> **갱신 정책**: 새 sub-phase가 항목을 deferred하면 본 문서에 추가. 작업 완료 시 항목 삭제 (또는 `~~취소선~~` 표시).
> **본 문서는 시간순 이력이 아님**. 시간순은 `docs/RESUME.md` + `docs/CHANGELOG.md`.

---

## 🎉 v0.0.1 ship 가능 — GPT Pro 30-issue 검수 종결 (2026-05-04)

R-A/B/C/D/E 5 페이즈 + 분리 #31/#38 + 통합 audit 모두 머지 완료. ADR-0052~0057 신규.

**다음 standby**:
- ⏳ **v0.0.1 release tag push** (사용자 결정): `git tag v0.0.1 && git push origin v0.0.1`

**v2.x 잠재** (POST v0.0.1 release):
- R-E v2.x: KnowledgeStorePool RwLock / WorkspaceCancellationScope chat·bench register / proper LRU / wiremock chunked disconnect 헬퍼 / RuntimeAdapter trait true split
- #31 후속: selected_path_token registry (Tauri dialog plugin 도입 후)
- catalog 외 manifest signature 확장 / proxy 명시 opt-in / typed-i18n-keys

---

## 우선순위 1 — 다음 진입 후보 (세션 즉시 시작 가능)

### ~~Phase 13'.h.2.a — LM Studio chat + vision 어댑터~~ — **2026-05-01 머지 완료**

### ~~Phase 13'.h.2.b/c — llama.cpp server 자동 spawn + mmproj sidecar~~ — **2026-05-03 핵심 머지** (ADR-0051)

* **상태**: 13'.h.2.b (runner-llama-cpp) + 13'.h.2.c.1 (ModelEntry mmproj 스키마 + gemma-3-4b 백필 + validator) ✅ 머지. 잔여 13'.h.2.d (chat IPC LlamaCpp wiring) → 우선순위 1로 승격.
* **머지 산출**: `crates/runner-llama-cpp` 신규 (22 invariant) + `adapter-llama-cpp::chat_stream` OpenAI compat SSE + vision content array (10 invariant) + `MmprojSpec` 백워드 호환 (4 invariant) + build-catalog validator. 합계 +36 신규 테스트.

### Phase 13'.h.2.d/e/f — 사용자 명시 결정(2026-05-03) 따라 순차 진행

세 sub-phase 합계 약 9-13h. 사용자 결정 = "권장안 채택 + 수동 단계는 친절 가이드 + 가이드 자동 갱신 시스템".

### Phase 13'.h.2.d — chat IPC LlamaCpp 분기 wiring (다음 진입 후보, 3-4h)

* **상태**: pending. 진입 조건 ✅ — runner-llama-cpp + adapter-llama-cpp 머지 완료.
* **결정 노트**: `docs/research/phase-13ph2bc-llama-server-mmproj-decision.md` §6 인계.
* **작업 스코프**:
  1. `apps/desktop/src-tauri/src/chat/mod.rs::start_chat`에 `RuntimeKind::LlamaCpp` 분기 추가 — 현재 `UnsupportedRuntime` 자리.
  2. Tauri State `Arc<Mutex<Option<LlamaServerHandle>>>` 보관 — 채팅 시 동일 model_path면 재사용, 다르면 기존 shutdown + 새 spawn (단일 instance 정책).
  3. ModelEntry → ServerSpec 변환 헬퍼 (model_path/mmproj_path는 catalog에서 가져온 후 사용자 cache dir 경로로 매핑).
  4. 사용자가 `LMMASTER_LLAMA_SERVER_PATH` env 미설정 시 한국어 안내 카피 + Settings link.
  5. 30~90초 모델 로드 진행률 ChatEvent로 emit (UI Chat.tsx에 stalled microinteraction). 또는 단순 spinner.
  6. Tauri `RunEvent::ExitRequested` 훅 + LlamaServerHandle::shutdown 명시 cleanup (보강 리서치 #6 권장).

### Phase 13'.h.2.e — `LlamaCppSetupWizard` 단계별 셋업 가이드 (4-6h)

* **상태**: pending. 사용자 명시 결정 2026-05-03 (ADR-0051 §결정 7).
* **선행 의존성**: ✅ runner-llama-cpp 머지. Phase 12' guide system 인프라 (ADR-0040 — `Guide.tsx` / `_render-markdown.ts` / `HelpButton.tsx`).
* **결정 노트**: `phase-13ph2bc-llama-server-mmproj-decision.md` §A6.

작업 스코프 (7단계 stepper):
1. **GPU 자동 감지** — `hardware-probe::probe_gpu()` 결과 그대로 카드 표시 (NVIDIA / AMD / Intel / Apple Silicon / CPU).
2. **권장 빌드 카드** — GPU별 ggml-org Releases asset 추천. 본문은 가이드 manifest(13'.h.2.f)에서 동적 fetch.
3. **ggml-org Releases 페이지 열기** — `tauri-plugin-shell::open` 화이트리스트 `github.com` 추가 + 한국어 안내 카피.
4. **압축 풀기 위치 안내** — Win `C:\Tools\llama-cpp\`, Mac `/usr/local/llama-cpp/`, Linux `~/.local/bin/llama-cpp/` 권장. clipboard 복사 버튼.
5. **환경변수 설정 가이드** — OS 분기:
   - Win: `setx LMMASTER_LLAMA_SERVER_PATH "C:\Tools\llama-cpp\llama-server.exe"` (PowerShell)
   - Mac/Linux: `export LMMASTER_LLAMA_SERVER_PATH=/usr/local/llama-cpp/llama-server` + `~/.zshrc` append 명령
   - clipboard 복사 + "환경변수 적용 후 LMmaster 다시 시작" 안내.
6. **자동 검증** — `runner-llama-cpp::spawn::resolve_binary_path()` 호출 + `--version` spawn 1회 + 한국어 결과 (✅ 잡혔어요 / ❌ 아직 안 보여요 + retry).
7. **mmproj 별개 wizard** — 카탈로그 `vision_support: true` 모델 선택 시 "이미지 분석에 필요한 vision 파일이에요" + `mmproj.url` direct download 안내(13'.h.2.c.2 자동 다운로드 머지 후 in-place pull).

진입점 3종:
- Settings → "고급 런타임" 카드 → "셋업 마법사 열기" 버튼.
- Diagnostics → "외부 런타임" 카드 → llama.cpp 미감지 시 빨간 카드 + 마법사 link.
- Chat → 처음 vision 모델 시도 시 LlamaCpp 미감지면 자동 modal (한 번만, 다시 안 보임 토글).

a11y: `role="dialog" aria-modal aria-labelledby` + Esc 닫기 + 첫 input auto-focus + 진행 percentage. i18n ko/en 동시.

### Phase 13'.h.2.f — 셋업 가이드 자동 갱신 시스템 (2-3h)

* **상태**: pending. 사용자 명시 결정 2026-05-03 (ADR-0051 §결정 8).
* **선행 의존성**: ✅ registry-fetcher (ADR-0026 + Phase 13'.a 패턴), ✅ minisign infrastructure (ADR-0047 + 13'.g.2.c).
* **결정 노트**: `phase-13ph2bc-llama-server-mmproj-decision.md` §A7.

작업 스코프:
1. **`manifests/guides/llama-cpp-setup.json`** 신규 — 마크다운 본문 + 권장 빌드 버전(예: `b9010+`) + GPU별 asset URL 룰 + `known_issues: Vec<KnownIssue>` 마커 + `last_reviewed_at`.
2. **`GuideManifest` Rust struct** — `crates/registry-fetcher` 또는 별개 crate. schema_version + 한국어 마크다운 + asset rules. serde 표준.
3. **registry-fetcher `manifest_ids`** — `"llama-cpp-setup"` 추가. 6h polling (catalog와 동일 cron). jsDelivr 1순위 / GitHub raw 2순위 / Bundled 폴백.
4. **minisign 서명 검증** — ADR-0047 `SignatureVerifier::from_embedded()` 재사용. `.minisig` fetch + verify. 실패 시 bundled fallback.
5. **GuideState IPC** — `get_setup_guide() -> GuideManifest` + 갱신 시 `guide-updated` event emit. ACL `allow-get-setup-guide`.
6. **Diagnostics 카드** — "셋업 가이드 v{n} 도착" 알림 + 다음 wizard 진입 시 자동 적용. 사용자 동의 dialog 없음 (읽기 전용 안내).
7. **CI 자동 서명** — `.github/workflows/sign-guides.yml` (catalog signing 패턴 차용 — `sign-catalog.yml`). main push 시 가이드 변경 감지 → minisign sign → commit.
8. **큐레이터 SOP** — `docs/runbooks/guide-update.md` 신설. 새 llama.cpp 빌드 회귀 / 새 GPU 변종 / 새 mmproj 모델 발견 시 manifest 갱신 절차.

frontend `LlamaCppSetupWizard`(13'.h.2.e)는 manifest의 빌드 버전/asset URL을 동적 표시 — 마크다운 본문은 기존 `_render-markdown.ts` 차용.

### Phase 13'.h.2.c.2 — mmproj 자동 다운로드 IPC (3-4h, v1.x)

* **상태**: pending. 진입 조건 ✅ — MmprojSpec 스키마 머지.
* **작업 스코프**: `start_mmproj_pull(model_id) → cancel_mmproj_pull` IPC. knowledge-stack `embed_download::download_with_progress` 차용 (256KB throttle + atomic rename + sha256). ModelDetailDrawer에서 vision 모델은 model.gguf + mmproj 둘 다 받기 표시. ACL 2건 추가.

### Phase 13'.h.3 — chat_template_hint 카탈로그 필드 (2-3h, v1.x)

* **상태**: pending. 보강 리서치 §1.7 #4.
* **작업 스코프**: ModelEntry에 `chat_template_hint: Option<String>` (gemma3 / llava / qwen2-vision / chatml). `ServerSpec::chat_template`에 자동 주입.

### Phase 13'.h.4 — llama-server binary 자동 다운로드 + GPU detect (옵션 C, 6-8h, v1.x)

* **상태**: deferred. 보강 리서치 §1.1 + §1.3 #2 권장.
* **작업 스코프**: ggml-org/llama.cpp Releases asset 자동 발견 + GPU detect (NVIDIA / Vulkan / Metal / ROCm / CPU 분기) + 화이트리스트 도메인 추가 + 사용자 동의 dialog. Phase 1A.1A 패턴 차용. ADR 신설 후보.

### Phase 13'.h.5 — known_issues 카탈로그 마커 (2-3h, v1.x)

* **상태**: deferred. 보강 리서치 §1.7 + §8.
* **작업 스코프**: `known_issues: Vec<String>` 카탈로그 필드 — gemma4_cuda_mmproj_abort / vulkan_amd_mmproj_heap 마커. 사용자 GPU+모델+빌드 조합이 마커에 걸리면 사전 경고. v2에서 자동 우회(`--cache-ram 0`, `--no-mmproj-offload`).

### Phase 13'.h.6 — Windows Job Object + ExitRequested 훅 cleanup (1-2h, v1.x)

* **상태**: deferred. 보강 리서치 §1.4 + #6.
* **작업 스코프**: `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`로 손자 프로세스 트리 종료 보장. Tauri `RunEvent::ExitRequested` 훅에서 명시 cleanup.

---

### ~~Phase 13'.g.2.c — FetcherCore wiring + Diagnostics 빨간 카드~~ — **2026-05-01 머지 완료**

### ~~Phase 13'.g.2.d — CI 서명 파이프라인~~ — **2026-05-01 머지 완료** (`.github/workflows/sign-catalog.yml`)

---

### Phase 13'.g.2.d — CI 서명 파이프라인

* **선행 의존성**: ✅ 13'.g.2.b 머지 후 진입.
* **예상 작업량**: 2-3h.

작업 스코프: `.github/workflows/sign-catalog.yml` 신규 — main branch push 시 트리거, secret minisign keypair로 `rsign sign`, `.minisig` 자동 업로드.

---

### Phase 13'.f.2.2 — 큐레이션 잔여 14 모델 + NSFW/NC 라벨 (legacy 항목 — 13'.f.2.2.1 + 13'.f.2.3로 분할 완료)

---

### Phase 13'.f.2 — 큐레이션 잔여 18 모델 + RP/임베딩 카테고리 schema (legacy 항목 — 13'.f.2.1/2.2로 분할 완료)

* **상태**: pending
* **선행 의존성**: 없음 — 즉시 진입 가능. catalog schema 변경은 model-registry crate.
* **예상 작업량**: 6-8h.
* **결정 노트 (이미 작성)**: `docs/research/phase-13pf-curation-plus4-decision.md` §4 "미정 / 후순위 이월".

작업 스코프:
1. **임베딩 카테고리 신설** — `category: "embedding"` enum 추가 (`crates/model-registry/src/`). RAG UI(워크벤치)에서 별도 셀렉터로 노출. `purpose: "retrieval"` 분류로 chat 추천 알고리즘에서 자동 제외.
   - bge-m3 (BAAI MIT, 100+ 언어 RAG 표준)
   - KURE-v1 (고려대, 한국어 RAG SOTA, MIT)
   - multilingual-e5-large (intfloat MIT, 94 언어)
2. **RP 카테고리 입주** (roleplay/ 디렉터리는 비어 있음).
   - Nous-Hermes-2 Mistral 7B DPO (Apache-2)
   - MythoMax L2 13B (Llama-2)
3. **NSFW 라벨 정책**: `content_warning: "rp-explicit"` 필드 추가 + 첫 화면 추천 제외 + "성인 콘텐츠 허용" 토글 시에만 노출. Stheno L3 8B는 이 정책 적용 후 입주.
4. **NC-라이선스 라벨**: `commercial: false` 라벨 + UI에서 비상업 chip 표시.
   - KULLM3 (CC-BY-NC, 한국어 강세)
   - Synatra-7B-v0.3-RP (CC-BY-NC)
   - Aya Expanse 8B/32B (CC-BY-NC, 23 언어)
5. **fine-tune 베이스 카테고리**: `purpose: "fine-tune-base"` 분류.
   - Yi-Ko-6B (Apache-2, 한국어 추가 학습 베이스)
   - Llama 3.1 8B/70B (베이스 모델 fine-tune 시드)
6. **나머지 9개**:
   - Yi-1.5 6B/9B/34B (Apache-2)
   - Mixtral 8x7B Instruct (Apache-2) — VRAM 26GB+ 경고
   - Phi-3.5 mini 3.8B (MIT, MoE는 llama.cpp 미지원이라 제외)
   - Codestral 22B는 13'.f에서 이미 추가 (제외)
   - CodeLlama 7B/13B/34B — *Codestral / Qwen Coder 우위*라 거부했지만 사용자 결정에 따라 재검토 가능
   - StarCoder2 3B/7B/15B — *base completion 강점*은 있지만 instruct 모드 약함, 후순위
   - TinyLlama 1.1B / Qwen 2.5 0.5B / SmolLM2 — Llama 3.2 1B / Qwen 2.5 1.5B와 niche 중복
7. **라이선스 거부 케이스 재검토**: HCX-Seed 1.5B는 라이선스 약관 확인 후 `community` tier로 입주 완료 (13'.f). 그 외 모델 재검토 필요 시 본 문서에 추가.

진입 조건:
- model-registry crate 스키마 변경 필요 → `category` enum + `purpose` 필드 + `content_warning` 옵션.
- 기존 `tier` enum (`new` / `verified` / `experimental` / `deprecated`) 호환 유지.
- 신규 카테고리 4개 (`embedding`, `roleplay`, `fine-tune-base`, NC-flag) 모두 추천 알고리즘에서 *기본 추천 제외* (UI에서 명시 활성화 시에만 노출).

리서치 노트 1차 보고서 — 본 세션의 보강 리서치 (Agent general-purpose 출력) 22 모델 메타데이터: HF repo / Ollama tag / 권장 quant / VRAM / 한국어 강도 / tier / 라이선스 / community insight 모두 수집 완료. 다음 세션은 이 자료로 manifest 자동 작성 가능.

---

### Phase 13'.g.2 — minisign 카탈로그 서명 wiring

* **상태**: pending
* **선행 의존성**: ADR-0047 (이미 머지) — `SignatureVerifier` 인프라.
* **예상 작업량**: 4-6h.
* **결정 노트 (이미 작성)**: `docs/research/phase-13pg-catalog-signature-decision.md` §4 "미정 / 후순위 이월".

작업 스코프 (3단계 분할 가능):

#### Phase 13'.g.2.a — 빌드 시점 pubkey 임베드
- `crates/registry-fetcher/build.rs` 신규 — env `LMMASTER_CATALOG_PUBKEY` + `LMMASTER_CATALOG_PUBKEY_SECONDARY` 읽어 컴파일 시점 상수로 임베드.
- 미설정 시 빌드 fail (안전 우선) 또는 `option_env!` + runtime 경고.
- ENV 값은 minisign 표준 base64-block 형식.

#### Phase 13'.g.2.b — FetcherCore 통합
- `FetcherCore::fetch_one_with_signature(id) -> Result<FetchedManifest, FetcherError>` 신규 변종.
- body fetch 후 `<id>.json.minisig` 추가 fetch (실패 시 NoSignature error).
- `SignatureVerifier::verify(body, sig_text)` 호출 — 실패 시 `SignatureVerifyFailed` 변종.
- `registry_fetcher::refresh_catalog_now`가 catalog manifest_id에 한해 verify 모드 활성.

#### Phase 13'.g.2.c — Diagnostics 빨간 카드 + bundled fallback 정책
- Phase 13'.b의 metrics 인프라 패턴 차용 — `get_catalog_signature_status` IPC 신설.
- `Diagnostics.tsx`에 GatewaySection 옆 또는 CrashSection 위에 SignatureSection 추가.
- 검증 실패 시: 1) bundled fallback로 자동 강등 + Diagnostics 빨간 카드 + 다음 refresh 시도 시까지 fresh fetch 차단.
- 한국어 카피: "카탈로그 서명을 확인하지 못했어요. 안전을 위해 기본 목록을 사용하고 있어요. 잠시 후 다시 시도할게요."

#### Phase 13'.g.2.d — CI 서명 파이프라인 (별도 sub-phase)
- `.github/workflows/sign-catalog.yml` 신규 — main branch push 시 트리거.
- step:
  1. `node .claude/scripts/build-catalog-bundle.mjs` — catalog.json 생성.
  2. `rsign sign manifests/apps/catalog.json -s "$MINISIGN_SECRET" -W` — secret은 GitHub Encrypted Secret.
  3. `manifests/apps/catalog.json.minisig` 산출.
  4. (자동 PR 또는) 동일 커밋에 .minisig 포함하여 푸시.
- secret 등록 SOP: `docs/runbooks/catalog-signing-rotation.md` (신규 작성 필요).

#### Phase 13'.g.2.e — 키 회전 SOP 문서
- `docs/runbooks/catalog-signing-rotation.md` 신규.
- 12개월 회전 + 90일 overlap 절차:
  1. 새 secondary 키 생성 (`rsign generate -p secondary.pub -s secondary.key`).
  2. `LMMASTER_CATALOG_PUBKEY_SECONDARY` env로 다음 앱 릴리즈에 임베드.
  3. CI는 새 secondary key로 서명 시작.
  4. 90일간 primary로 검증 가능 → 구버전 사용자 graceful upgrade.
  5. 90일 후 secondary→primary 승격, 새 secondary 후보 추가, primary deprecate.
- 사고 대응 (키 유출 시 즉시 회전): primary 폐기 + secondary로 즉시 강제 앱 업데이트.

위험 노트:
- minisign-verify v0.2 `PublicKey::decode` 입력 형식이 정확히 single base64 line인지 multi-line인지 fixture 기반 검증 미완료. 본 sub-phase에서 실 keypair로 확정.
- GitHub Encrypted Secrets는 fork된 PR에서 노출 안 됨 — workflow trigger를 main branch만으로 제한해야 함.

---

## 우선순위 2 — Phase 14'.x 후보 (Phase 13' 종료 후)

### v1.x 폴리싱 항목

* **scope 편집 audit log** — "누가 언제 어떤 scope을 어떻게 바꿨다" 영속 기록. v1.x.
* **crash 파일 자동 청소** — 30일 이상 된 crash auto-delete. v1.x.
* **crash search/filter** — 키워드 / 날짜 범위. 50개 cap이라 v1 충분.
* **alias rename IPC** — 위험성 vs UX trade-off. v1.x.
* **expires_at datepicker** — i18n + timezone 처리 부담. RFC3339 raw 입력은 1인 데스크톱 앱 규모에 적정.
* **SQLite access log + 7d retention** (Phase 13'.b deferred) — 영속 access log. 메모리 ring buffer는 재시작 시 비워짐.
* **`time_to_first_byte_ms` for SSE** (Phase 13'.b deferred) — 스트리밍 첫 토큰 latency 별도 메트릭.
* **`key_id` fingerprint masking** in access log (Phase 13'.b deferred) — Authorization → key_id fingerprint, middleware chain 의존성 정리 후.
* **repair-log.jsonl 5MB rotation** (ADR-0046 deferred) — 무한 증가 위험. v1.x.

### Phase 13'.c.2 후보

* **API 키 사용량 통계** — 키별 호출 횟수 + 마지막 호출 path/model. Diagnostics에 카드 신설.
* **Crash 자동 업로드 (사용자 동의 시)** — telemetry opt-in과 분리해 panic 시 자동 submit. 외부 통신 0 정책 우회 — 사용자 명시 동의 필수.

### Phase 13'.b.2 후보 (Diagnostics 폴리싱)

* P2 polish: bench:started listener, list_workbench_runs UI, list_ingests UI, workbench_serialize_examples, get_pipelines_config debug, submit_telemetry_event manual trigger.

---

## 우선순위 3 — 운영 / 인프라

### Phase R-K (v1.x 진입) — Tauri Updater 활성 (옵션 A 복귀)

* **상태**: pending. 옵션 B-3 hybrid (active=false + ManualUpdatePanel)가 v0.0.x 운영 중 (ADR-0064 §4).
* **선행 의존성**: 무서명 NSIS 빌드 1회 검증 + minisign keypair 보관 정책 확정.
* **예상 작업량**: 2-3h (keypair 생성 + config 토글 + UI 분기 + e2e 1회).

작업 스코프:
1. **minisign keypair 발급** — `rsign generate -p ./tauri-pubkey -s ./tauri-priv.key`. 강한 패스프레이즈 필수.
2. **`tauri.conf.json::plugins.updater.pubkey` 교체** — 기존 임베드 키(`BF5C36D65E99C44F` 시작)는 짝 secret 미확인이므로 *반드시 새 keypair*. 옛 사용자 PC가 새 키로 서명된 빌드를 받지 못하므로 v1.0.0 release notes에 "이번 한 번만 수동 받아주세요" 필수 안내.
3. **`tauri.conf.json::plugins.updater.active = true`** 토글 (현재 `false`).
4. **`tauri.conf.json::bundle.createUpdaterArtifacts = true`** 토글 (현재 `false`).
5. **GitHub Secret 등록** — `TAURI_SIGNING_PRIVATE_KEY`(secret 파일 본문) + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
6. **`.github/workflows/release.yml` env 추가** — 위 두 secret을 빌드 step env로 주입.
7. **capability 복구** — `apps/desktop/src-tauri/capabilities/main.json`에 다음 3개 다시 추가:
   - `updater:default`
   - `allow-start-auto-update-poller`
   - `allow-stop-auto-update-poller`
8. **`Settings.tsx` 분기 토글** — `const AUTO_UPDATE_ENABLED = true;`. `<AutoUpdatePanel />` 자동 복귀 + `<ManualUpdatePanel />` 자동 차단.
9. **i18n 키 deprecation** — `screens.settings.autoUpdate.manualMode.*` 9키는 보존 (다음 v1.x ship 시 deletion sweep으로 정리).
10. **첫 release tag로 e2e** — `latest.json` + `*.sig` 산출 + 사용자 PC가 기존 v0.0.x에서 자동 알림을 받을 수 있는지 검증 (단 첫 pubkey-bump 1회는 수동).

진입 조건:
- 옵션 B-3로 무서명 빌드가 충분히 안정 (release notes / Settings 카피 검증).
- minisign keypair secret 보관 정책 결정 (1Password / GitHub Encrypted Secret).

위험 노트:
- pubkey 교체 시 *기존 사용자가 업데이트 못 받음* — Tauri trust-on-first-use라 옛 pubkey가 박힌 PC는 새 pubkey를 거부.
- secret 유출 시 즉시 keypair 회전 + 새 pubkey 임베드한 hotfix 릴리즈 필수.

---

### Phase R-L (v1.x 진입) — Ollama Linux 자동화 (옵션 A: download_and_extract)

* **상태**: deferred. 옵션 B (open_url + 공식 가이드)가 v0.3.x 운영 중 (ADR-0064 §1). supply-chain RCE 표면 제거 완료.
* **옵션 A 의도**: `manifests/apps/ollama.json`의 linux 분기를 `download_and_extract`로 전환:
  ```json
  "linux": {
    "method": "download_and_extract",
    "url_template": "https://github.com/ollama/ollama/releases/latest/download/ollama-linux-amd64.tar.zst",
    "version_url": "https://api.github.com/repos/ollama/ollama/releases/latest",
    "extract_to": "/usr",
    "sha256": "auto-fetched-from-sha256sum-txt"
  }
  ```
* **선행 의존성**:
  1. `extract.rs::detect_format`이 `.tar.zst` 지원 — 현재 zip / tar.gz / dmg만. zstd crate 의존성 추가 필요.
  2. **sha256 자동 갱신 cron** — Ollama Releases의 `sha256sum.txt` 1시간마다 fetch → manifest 자동 갱신 → minisign 재서명. catalog signing 인프라 (ADR-0047) 재사용 가능.
  3. **CPU/GPU 변종 분기** — `hardware-probe`로 GPU detect 후 amd64/amd64-rocm/arm64/jetpack5/jetpack6 분기. manifest schema 확장 (`linux_variants`).
  4. **sudo 우회** — `/usr` 쓰기는 root 필요. 사용자 홈에 풀고 `~/.local/bin`에 symlink하는 sudo-free path 권장.
  5. **systemd unit 자동 작성** — Tauri가 `/etc/systemd/system/`에 쓸 권한 0. user-mode systemd (`~/.config/systemd/user/ollama.service` + `systemctl --user enable`)로 회피.
  6. **AMD ROCm 추가 번들** — 사용자 GPU detect 후 자동 추가 다운로드. UX 동의 필요 (~990MB).
* **언제 채택?**: Linux 사용자 점유율 > 15% + manual install 실패율 > 30% 텔레메트리 신호 충족 시. v1.0 ship 후 6주 모니터링.
* **체크리스트**:
  - [ ] zstd crate 추가 + extract.rs `.tar.zst` 지원 + 4개 invariant 테스트
  - [ ] sha256sum.txt 자동 갱신 cron + minisign 재서명 GitHub Action
  - [ ] hardware-probe GPU detect 결과 → variant 매핑 deterministic 함수
  - [ ] user-mode systemd 통합 — 5개 배포판 매트릭스 (Ubuntu 22/24, Fedora 40/41, Arch)
  - [ ] sudo-free 설치 path UX 결정 노트
  - [ ] ADR 신설 (가칭 "ADR-0065 Ollama Linux 자동화 — sudo-free user-local install")

---

### Code signing (Windows / macOS)

* **Windows**: `tauri.conf.json::bundle.windows.certificateThumbprint`이 현재 `null`. 코드 서명 인증서 (Authenticode) 미적용 → SmartScreen "확인되지 않은 게시자" 경고 노출. 정식 배포 시 EV 또는 OV 인증서 필요 (~$300/년).
* **macOS**: `signingIdentity` `null`. notarization 미적용. 사용자가 *우클릭 → 열기*로 우회 가능하지만 정식 배포는 Developer Program ($99/년) 필요.

### GitHub Releases CI

* `.github/workflows/release.yml` 신규 — tag push 시 자동 빌드 + bundle 산출 + Release 생성.
* Tauri Updater endpoints가 GitHub Releases를 가리키므로 (tauri.conf.json::plugins.updater.endpoints) 자동 업데이트 흐름과 통합.
* minisign 서명 (Phase 13'.g.2.d)과 통합 시 catalog 서명 + updater 서명을 동일 keypair 또는 분리 운영 결정 필요.

---

## 작업 항목 추가 시 체크리스트

새 deferred 항목을 본 문서에 추가할 때:
- [ ] 우선순위 그룹 결정 (1: 즉시 진입 가능 / 2: v1.x / 3: 운영).
- [ ] 선행 의존성 명시.
- [ ] 예상 작업량 (시간 단위).
- [ ] 기존 결정 노트 / ADR 참조.
- [ ] 작업 스코프를 가능하면 sub-phase로 분할 (3-6h 단위).
- [ ] 위험 노트 / 미해결 질문 명시.
- [ ] 한국어 카피 톤 (CLAUDE.md §4.1) 필요 항목은 별도 표시.

---

## 2026-05-08 세션 종료 — 차후 후보 (선택, 비필수)

> 본 섹션은 사용자 요청 (2026-05-08, v0.5.1 직후) 으로 정리한 *차후 진행 후보*. **모두 비필수** — 사용자 측 e2e 검증 후 *진짜 막히는 부분*만 진입.
> 기존 우선순위 1~3 섹션과 별도로, 본 세션에서 *내가 sub-phase 잘게 쪼개면서 만들어낸 미세 개선*을 정직하게 표시.

### 핵심 종결 (2026-05-08)
- ✅ GPT Pro 검수 19 finding 모두 종결 (v0.3.1~v0.3.4, ADR-0064)
- ✅ Phase 13'.h.2.d Round 1~4 (v0.4.0) — chat IPC LlamaCpp 분기 wiring
- ✅ Phase 13'.h.2.c.2 (v0.4.2) — LlamaCpp 모델 자동 다운로드 (catalog → cache_dir)
- ✅ Phase 13'.h.2.e.1 (v0.5.0) — Settings UI + settings.json + startup env 주입
- ✅ Phase 13'.h.2.e.2/e.3 (v0.5.1) — Catalog/Chat LlamaCpp 분기 + 한국어 banner

**비전 chat = 클릭만으로 시작 가능 흐름 완결**.

### 차후 후보 (선택, 진입 우선순위 낮음)

| # | 항목 | Effort | 진입 권장도 | 진입 조건 |
|---|---|---|---|---|
| 1 | **Phase 13'.h.2.e.4** — cache_dir GGUF 존재 검사 + dropdown filter | 2-3h | 낮음 (현재도 한국어 안내) | 사용자가 "받지 않은 모델이 dropdown에 보여서 헷갈려요"라고 명시 보고 |
| 2 | **Phase 13'.h.2.e.5** — quant 선택 UI (현재 default first quant) | 2-3h | 낮음 | 사용자가 Q4_K_M 외 다른 quant 명시 요청 |
| 3 | **Phase 13'.h.5** — known_issues 카탈로그 마커 (mmproj 누락 등 사전 경고) | 2-3h | 낮음 | 큐레이터가 manifest 데이터 품질 점검 후 |
| 4 | **Phase R-M** — vitest 환경 호환 fix (Workspace 12 it skipped) | 2-4h | 낮음 (CI는 통과) | 회귀 가드 보강이 *진짜 필요한* 시점 |
| 5 | **Phase R-K** — Updater 옵션 A 활성 + 새 keypair | 4-6h | **중간 (v1.0 진입 시 필요)** | v0.x 동안은 수동 업데이트 안내로 OK |
| 6 | **Phase R-L** — Ollama Linux 자동 설치 | 3-4h | 낮음 (Windows-first) | Linux 사용자 명시 요청 |

### v1.x 후속 reinforce (검수 19 finding 종결 후 추가 강화)

| # | 항목 | Effort | 진입 조건 |
|---|---|---|---|
| 7 | Knowledge chunk-level cancel + spawn_blocking refactor | 6-8h | 사용자가 *대용량 ingest 도중 cancel 응답 늦음*을 보고 |
| 8 | Workbench DNS rebinding hardening (`reqwest::resolve()` 정적 매핑) | 3-4h | 보안 audit 시점 |
| 9 | KeyStore passphrase rotation (`KeyStore::rekey()`) | 4-6h | 사용자가 비밀번호 회전 요구 (현재 0건) |
| 10 | selected_path_token Mobile photopicker | — | Tauri Mobile v2.x 진입 |

### 운영 위험 (조치 필요 시점에 진입)

- **Updater pubkey 짝 secret 미확인**: `tauri.conf.json:122` 임베드 키 `BF5C36D65E99C44F` — Phase R-K (#5) 진입 시 *반드시 새 keypair 발급* (옛 사용자 PC가 새 키로 서명된 빌드를 못 받음 — Tauri trust-on-first-use).
- **EmbeddingModelPanel.test.tsx Windows local flaky**: jsdom waitFor timing. Linux CI는 통과. 본 sub-phase 변경 무관.

### 정직한 판정 요약

오늘 세션의 *진짜 가치*: 검수 종결 + 비전 chat 활성. **이 외 모두 polish / 미래 시나리오 / 환경 이슈**.

**다음 세션 진입 결정은 사용자 측 실 사용 피드백 기준** — 본 차후 후보 리스트는 *제안*일 뿐, *진짜 막히는 부분*이 보고되기 전까지 자율 진입 X.

---

**문서 버전**: v1.1 (2026-05-08 — 검수 19 finding 종결 + 비전 chat 흐름 완결 + 차후 후보 정리).
