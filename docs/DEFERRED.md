# DEFERRED — 후순위 이월 작업 단일 진입점

> **목적**: 다음 세션 / 다음 사용자가 *어떤 후속 작업이 남아 있는지* 한눈에 보고, 진입 조건과 의존성을 빠르게 확인할 수 있게 모아둔 인덱스.
> **갱신 정책**: 새 sub-phase가 항목을 deferred하면 본 문서에 추가. 작업 완료 시 항목 삭제 (또는 `~~취소선~~` 표시).
> **본 문서는 시간순 이력이 아님**. 시간순은 `docs/RESUME.md` + `docs/CHANGELOG.md`.

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

### Tauri Updater 호환 빌드 — 옵션 B (배포 빌드)

* **상태**: pending. 옵션 A (무서명 NSIS installer) 완료 후 정식 배포 흐름.
* **선행 의존성**: 없음. 단 GitHub Releases 자동 업데이트를 원할 때만 필요.
* **예상 작업량**: 1-2h (keypair 생성 + env + 빌드 + Releases 워크플로 step 검증).

작업 스코프:
1. **minisign keypair 생성** — `rsign generate -p ~/.lmmaster.pub -s ~/.lmmaster.key`. password는 강한 패스프레이즈.
2. **`tauri.conf.json::plugins.updater.pubkey` 교체** — 현재 등록된 pubkey가 *짝이 맞는 secret*과 함께 보관 중인지 확인. 없으면 위에서 만든 새 pubkey로 교체.
3. **`TAURI_SIGNING_PRIVATE_KEY` + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` env 설정** — secret key 파일 내용 + password.
4. **`createUpdaterArtifacts` 다시 `true`로 복귀** — 옵션 A 진행 시 임시로 `false`로 둠.
5. **`pnpm tauri build` 재실행** — `.exe` + `.sig` + `latest.json` 산출.
6. **GitHub Releases 자동화** — `.github/workflows/release.yml` 신규. tag push 트리거 → 빌드 → release 생성 → assets 업로드 (.exe, .sig, latest.json). updater endpoints가 GitHub Releases를 가리키므로 (tauri.conf.json::plugins.updater.endpoints) 사용자 PC에서 자동 업데이트 흐름 완성.
7. **Phase 13'.g.2와 통합 결정** — catalog signing (ADR-0047)과 updater signing이 동일 keypair 사용? 분리? Tauri Updater pubkey는 minisign 형식 동일하므로 *같은 키 재사용 가능*하지만 "역할 분리" 관점에서 분리 권장.

진입 조건:
- 옵션 A로 무서명 빌드가 한 번 성공해서 빌드 환경이 검증된 상태.
- minisign keypair secret 보관 정책 결정 (1Password / GitHub Encrypted Secret).

위험 노트:
- pubkey 교체 시 *기존 사용자가 업데이트 못 받음* — 일단 v0.0.1 → v0.1 첫 정식 릴리즈 전에 확정해야 함.
- secret 유출 시 즉시 keypair 회전 + 새 pubkey 임베드한 hotfix 릴리즈 필수.

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

**문서 버전**: v1.0 (2026-04-30 — Phase 13'.c/13'.f/13'.g 완료 후 deferred 인덱스 신설).
