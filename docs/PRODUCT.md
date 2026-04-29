# LMmaster — 제품 개요

> **한국어 우선, 데스크톱 기반 로컬 AI 컴패니언.**
> LM Studio · Ollama를 *대체하지 않고* 감싸서, "복잡한 로컬 AI 셋업"을 한 번의 마법사로 끝내고, 기존 웹앱이 호출만 해도 즉시 추론·파인튜닝·배포가 되도록 묶는 운영 허브입니다.

본 문서는 **사용자/이해관계자 대상 제품 설명**입니다. 기술 결정 근거는 `docs/adr/`, 페이즈 운영은 `docs/PHASES.md`, 인계 노트는 `docs/RESUME.md`를 참고해 주세요.

---

## 1. 한 줄로

> **"로컬 AI를 한국어로, 그리고 어디로든 옮길 수 있게."**
> 사용자는 첫 실행 후 4단계 마법사만 따라가면 GPU·드라이버·런타임·모델·파인튜닝이 자기 PC에 맞게 자동 셋업되고, 다른 PC로 폴더만 옮겨도 fingerprint repair로 그대로 복구돼요.

---

## 2. 개요 (Executive Summary)

LMmaster는 **로컬 AI 운영 허브**입니다. 단순한 모델 런처도, 단순한 채팅 UI도 아니에요. 다음 4가지를 한 데스크톱 앱 안에서 책임집니다.

1. **셋업 자동화** — Ollama / LM Studio / NVIDIA 드라이버 / VC++ / WebView2 / Vulkan / CUDA 등을 **감지·설치·검증**합니다.
2. **한국어 우선 큐레이션** — EXAONE · HyperCLOVA-X SEED · Polyglot-Ko · A.X 같은 한국어 튜닝 모델이 1순위 카탈로그.
3. **포터블 워크스페이스** — 모델·프리셋·키·지식 스택까지 한 폴더에 묶어 다른 PC로 옮기면 자동 복구.
4. **온디바이스 워크벤치** — 양자화(`llama-quantize`) + LoRA 파인튜닝(LLaMA-Factory CLI / Unsloth / mac MLX-LM) + Ollama 1-click 등록.

**동시에** OpenAI 호환 게이트웨이(`localhost`) 한 개를 노출해, 기존 웹앱은 base URL과 키만 바꿔서 그대로 사용할 수 있어요. 웹앱은 어떤 런타임이 도는지 알 필요가 없습니다.

플랫폼: Windows 11 · macOS 14+ · Ubuntu 22.04+. Tauri 2 + Rust(Axum) + React/TS. 2026년 4월 기준 v1 전 phase 진행 중.

---

## 3. 사용 목적 — 누가 왜 쓰나요

### 3.1 핵심 페르소나

| 페르소나 | 핵심 고민 | LMmaster의 답 |
|---|---|---|
| **개인 개발자/연구자** | 로컬 LLM을 빨리 띄워 자체 도구·웹앱과 붙이고 싶다 | 4단계 마법사 + OpenAI 호환 게이트웨이로 30분 안에 첫 호출 |
| **사내 시범 도입자** | 사내망에서 클라우드 의존 없이 한국어 AI 시범 운영 | 포터블 워크스페이스 + 자체 자가스캔(로그/외부 호출 0) |
| **AI 사이드 프로젝트 운영자** | 여러 PC에 같은 환경을 재현하고 싶다 | fingerprint repair — 다른 PC 폴더 열기만 하면 자동 매칭 |
| **튜닝/공급자** | 도메인 LoRA를 만들어 Ollama에 올리고 싶다 | 5-화면 워크벤치 → GGUF 내보내기 → Modelfile 자동 생성 |
| **기존 웹앱 보유 팀** | 기존 SaaS 웹앱에 로컬 옵션을 1주 안에 붙이고 싶다 | `@lmmaster/sdk` + `LocalCompanionProvider` (provider 1개 추가로 끝) |

### 3.2 사용자 시나리오 6선

1. **첫 실행** — 마법사가 환경 점검 → Ollama 미설치면 silent install, LM Studio는 EULA 안내 → 한국어 추천 모델 3종 → 1-click 다운로드.
2. **채팅** — 모델 카드를 클릭하면 적합한 백엔드(Ollama 또는 LM Studio)에 자동 pull + 통합 채팅 UI.
3. **기존 웹앱 통합** — 웹앱이 `LocalCompanionProvider`만 추가. base URL + scoped key로 OpenAI 호환 응답 그대로.
4. **포터블 이전** — 워크스페이스 폴더를 외장 SSD로 옮긴 뒤 새 PC에서 열면 fingerprint diff → 1-click repair.
5. **워크벤치** — JSONL/CSV/노트 폴더 드롭 → 프리셋 선택 → 학습 → GGUF 내보내기 → Ollama 등록까지 1-click.
6. **자동 갱신** — LM Studio가 새 버전을 내면 토스트 알림 → "다음 실행 때 적용돼요"(JetBrains 패턴) 또는 즉시 적용 선택.

---

## 4. USP — Unique Selling Proposition

### 4.1 6 pillar (PIVOT 2026-04-26 확정)

1. **자동 설치 + 환경 셋업** — 매니페스트 기반 declarative installer + sha256 검증 + atomic rename + Inno/MSI exit-code 정확 인식 + Channel<InstallEvent> 실시간 진행률.
2. **한국어 인터페이스** — 마법사·오류·도움말·자가스캔 요약 모두 해요체. ko-KR 기본 locale.
3. **포터블 워크스페이스** — fingerprint manifest로 PC↔PC 이동 가능. 외장 SSD 운영 가능.
4. **카테고리 큐레이션** — 에이전트/캐릭터/코딩/사운드/온디바이스 5×5~10 모델, 한국어 튜닝 모델 1순위 + Foundry-style hardware-aware "✅ 이 PC에 맞아요" 배지 + Pinokio-style 2-tier(Verified/Community).
5. **온디바이스 워크벤치 (v1 핵심)** — 양자화 + LoRA + Korean 데이터 정합성 검증 + GGUF→Ollama 등록 1-click.
6. **자동 갱신** — 본체·LM Studio·Ollama·모델 카탈로그를 6~24시간 폴러로 추적. 토스트 + JetBrains-style "다음 실행 때 적용".

### 4.2 7가지 초격차 강화 thesis (경쟁 리서치 기반, 2026-04-27)

> "한국어 locale-only" 한 가지로는 부족합니다. **여러 축의 합집합**이 1년 안에 추격 불가능한 wedge를 만듭니다.

1. **Korean-substrate companion (Korean-localized launcher 아님).**
   ko-locale은 기본기. 추가로 (a) **한국어 튜닝 모델 1순위 카탈로그** (EXAONE 4.0 32B/1.2B, HyperCLOVA-X SEED 8B Omni, Polyglot-Ko, K-EXAONE 236B-A23B), (b) **한글/한자 mixed-script RAG 파이프라인** + 한글 정규화 임베딩, (c) **Korean QA evals** (pytest-shaped fixtures — AI Toolkit "evals-as-tests" 패턴 차용).
   *Cherry/Jan/Msty/GPT4All 누구도 (a)·(b)·(c) 셋 다 갖고 있지 않습니다.*

2. **Wrap-not-replace 게이트웨이 + key-per-webapp scope.**
   localhost 게이트웨이가 웹앱별 scoped 키를 발급, 정책에 따라 local-Ollama / local-LM-Studio / cloud-fallback을 라우팅. 각 런타임의 자체 lifecycle/업데이트는 그대로 존중.
   *AnythingLLM/Open WebUI는 provider를 흡수하면서 lifecycle 통제권을 잃습니다 — 우리는 ADR-0016에서 명시적으로 회피.*

3. **포터블 워크스페이스 fingerprint repair.**
   {runtime versions, models, presets, knowledge stacks, scoped keys}를 manifest로 묶어 폴더 이동만으로 새 PC에서 자동 복구.
   *LM Studio Hub는 preset만, Jan/Msty/Cherry는 portable 개념 자체 없음, AnythingLLM workspace는 hardware migration 미지원.*

4. **매니페스트 installer (resumable + sha256 + atomic-rename) + 2-tier 카탈로그 + 하드웨어 인지 추천.**
   Phase 1A.3에서 절반 구축. Pinokio의 Verified/Community 거버넌스 + Foundry의 hardware-aware 추천 로직을 우리 manifest 스키마에 결합.

5. **워크벤치 = 양자화 + LoRA + Korean 데이터 정합성 + GGUF→Ollama 1-click.**
   LLaMA-Factory가 학습을 담당하고, 우리는 (a) **5-화면 한국어 플로우**, (b) **Korean tokenizer 정합성 검증** (HCX-Seed tokenizer alignment, 한자 mixed-script normalization, 한글-only 필터), (c) **GGUF→Ollama Modelfile 자동 생성**을 책임.
   *HF AutoTrain은 cloud-tilted, AI Toolkit은 Azure-tilted. 한국어 데이터셋 검증은 누구도 안 함.*

6. **자가스캔 = deterministic 판정 + opt-in 로컬 LLM 요약.**
   주기 6h cron + 부팅 grace + 사용자 트리거. **판정은 deterministic**, 사용자 향 **요약만** Ollama 모델로 한국어 자연어. 외부 통신·로그 0.
   *Cherry agents는 phone-home, Jan/GPT4All은 스캔 자체 없음. 정부/기업 보안 요건 충족.*

7. **Pipelines 패턴을 게이트웨이에 적용.**
   Open WebUI는 *UI*에 plugin 적용 — 우리는 *gateway*에 적용. 웹앱별로 PII redact, retry policy, observability를 wire-level filter로 끼움. 웹앱 코드 무수정.
   *어떤 경쟁자도 이 primitive를 게이트웨이 layer에 갖고 있지 않습니다.*

### 4.3 "단순 런처가 아닌 컴패니언"의 4가지 입증

| 항목 | 런처라면 | 컴패니언인 이유 |
|---|---|---|
| **API 표면** | 모델만 띄움 | OpenAI 호환 + scoped key + per-webapp 라우팅 정책 (게이트웨이 proxy) |
| **상태 책임** | 실행 후 잊음 | 워크스페이스 fingerprint + 자가스캔 + 자동 업데이트 |
| **데이터 표면** | 입출력만 통과 | 한국어 RAG (zero-config Knowledge Stack 패턴) + 워크벤치 데이터 검증 |
| **lifecycle** | 없음 | 본체 + 외부 런타임 + 모델 카탈로그 + 사용자 워크벤치 자산까지 통합 lifecycle |

---

## 5. 사용자가 얻는 기대 효과

### 5.1 개인 사용자
- 첫 호출까지 **30분 이내** (마법사 자동 셋업 + 추천 모델 1-click).
- 사양 정확 매칭 — VRAM 4GB 부정확 표시 / 드라이버 누락 / Vulkan loader 부재 같은 흔한 실수를 자동 안내.
- 외장 SSD 한 개로 회사 PC↔집 PC 동일 환경.

### 5.2 사이드 프로젝트 / 1인 운영자
- 기존 웹앱을 **1주 안에** 로컬 옵션으로 확장 — `@lmmaster/sdk` + provider 1개.
- 사용자가 LMmaster 미설치/미실행 상태일 때 자동 감지 + 한국어 안내 모달 + custom URL scheme(`lmmaster://`) 자동 실행.
- API 키별 scope로 여러 웹앱 동시 운영.

### 5.3 사내 시범
- **외부 통신 0** — localhost-only 바인딩, no-proxy, 자가스캔 결과는 디바이스 안에만.
- 부서 PC 표준 셋업 — 워크스페이스 폴더를 NAS에 두고 fingerprint repair로 모든 PC 동일 환경.
- 한국어 감사 로그 (Phase 6'에서 `/diagnostics export` JSON).

### 5.4 튜닝/공급자
- 도메인 데이터(법률/의료/마케팅) JSONL 드롭 → 30분~6시간 학습 → GGUF → Ollama 등록 → 사내 게이트웨이에서 즉시 호출.
- LLaMA-Factory + Unsloth(가속) + MLX-LM(mac) 분기 자동.

---

## 6. 차별화 — 경쟁 매트릭스

> 자세한 분석은 본 문서 부록 §A 참조. 아래는 핵심 축 요약.

| 축 | LMmaster | LM Studio | Ollama | Jan | Msty | Cherry | AnythingLLM | Open WebUI | Pinokio | Foundry Local |
|---|---|---|---|---|---|---|---|---|---|---|
| 한국어 1st locale + 모델 큐레이션 | ✅ | ✗ | ✗ | ✗ | ✗ | △ (CN) | ✗ | ✗ | ✗ | ✗ |
| Wrap-not-replace + LM Studio/Ollama lifecycle 존중 | ✅ | n/a | n/a | ✗ | ✗ | ✗ | ✗ | ✗ | △ (run scripts) | ✗ |
| OpenAI 호환 게이트웨이 + per-webapp 키 | ✅ | △ (단일 endpoint) | △ | △ (:1337) | ✗ | ✗ | ✗ | ✅ (multi-user) | ✗ | △ |
| 포터블 워크스페이스 fingerprint repair | ✅ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| 매니페스트 기반 resumable installer | ✅ | ✗ | ✗ | △ | ✗ | ✗ | ✗ | ✗ | ✅ | △ |
| 양자화 + LoRA 워크벤치 (in-app) | 🟡 (Phase 5') | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| GGUF→Ollama 1-click 등록 | 🟡 | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| 자가스캔 + 로컬 LLM 한국어 요약 | 🟡 (Phase 1' 잔여) | ✗ | ✗ | ✗ | ✗ | △ (cloud agent) | ✗ | ✗ | ✗ | ✗ |
| RAG (Knowledge Stack 패턴) | 🟡 (G1) | ✗ | ✗ | ✗ | ✅ | △ | ✅ | ✅ | ✗ | ✗ |
| Pipelines/Tool 확장 surface | 🟡 (G3 game) | ✗ | △ (Agent Store) | △ | ✗ | △ | ✅ (Agent Skill Store) | ✅ (Pipelines) | ✗ | ✗ |

✅ 정식 / 🟡 로드맵 진행 중 / △ 부분 / ✗ 없음.

> **단일 축에서 1등인 경쟁자는 있지만, 7-점 thesis의 합집합을 가진 경쟁자는 없습니다.** 이게 우리의 wedge.

---

## 7. 기능 일람 — 구현 / 진행 / 예정

> ✅ = 구현 완료, 🟢 = 이번 phase 진행 중, 🟡 = 다음 sub-phase, ⏳ = 후속 phase.

### 7.1 인프라 / 게이트웨이 (Phase 0~1A.1)
- ✅ **Tauri 2.10 + Axum 0.8 supervisor** — `127.0.0.1:0` bind → 자동 포트, `gateway://ready` emit, `RunEvent::ExitRequested` graceful shutdown.
- ✅ **디자인 시스템** — Pretendard Variable + dark + 네온 그린 + 4px grid + 2-layer focus + reduced-motion.
- ✅ **runtime-detector** — Ollama `/api/version` + LM Studio `/v1/models` HTTP probe. 1.5s timeout.
- ✅ **manifest evaluator** — 4 detect rule (http.get / shell.which / registry.read / fs.exists) + platform 필터 + Pinokio-style aggregation.
- 🟡 **Gateway proxy → LM Studio/Ollama 라우팅 + scoped key** — Phase 3' (G8 — multi-provider routing policy).
- 🟡 **Pipelines 패턴 (gateway-side filter)** — Phase 6' (Thesis #7).

### 7.2 하드웨어 / 환경 점검 (Phase 1A.2)
- ✅ **OS / CPU / RAM / Disk** (sysinfo).
- ✅ **GPU** — NVML(NVIDIA) + DXGI(Win 비-NVIDIA) + Metal(mac) + AMD sysfs(Linux). 4GB clamp 버그 회피.
- ✅ **Vulkan probe** — ash 0.38 loaded feature, validation layer 미사용.
- ✅ **Win 레지스트리** — WebView2 / VC++ 2022 / NVIDIA driver / CUDA toolkit.
- ✅ **DLL probe** — d3d12 / DirectML / nvcuda / vulkan-1.
- ✅ **mac sysctl + Metal family + Rosetta**, **Linux glibc + libstdc++**.
- 🟡 **하드웨어 벤치마크** (G4) — Phase 2'에서 모델별 token/sec 30초 측정 + 카탈로그 노출.

### 7.3 설치기 / Installer (Phase 1A.3)
- ✅ **resumable downloader** — reqwest stream + Range header resume + streaming sha256 + backon retry-with-jitter + atomic rename.
- ✅ **`download_and_run`** — Inno Setup / NSIS / MSI exit code 정확 인식 (3010/1641/8 reboot).
- ✅ **`download_and_extract`** — zip 8.x / tar.gz / dmg(macOS, hdiutil + ditto + Drop guard auto-detach). Dual zip-slip 방어.
- ✅ **`shell.curl_pipe_sh`** — Linux/macOS 한정 + shell injection 방어.
- ✅ **`open_url`** — webbrowser crate (Win/mac/Linux).
- ✅ **`post_install_check`** — HTTP polling + cancel-respect + deadline.
- ✅ **`Channel<InstallEvent>`** — Tauri 2 IPC, kebab-case tagged enum (started/download/extract/post-check/finished/failed/cancelled), per-window window-close detect.
- ✅ **`InstallRegistry`** — id↔CancellationToken Mutex map. 중복 install 거부, app exit 시 cancel-all.
- ✅ **Capability TOML** — `allow-install-app` / `allow-cancel-install` / `allow-detect-environment` strict ACL.

### 7.4 첫 실행 마법사 (Phase 1A.4)
- ✅ **xstate v5 머신** — language → scan → install → done. localStorage 동기 hydrate + sanitize for persist.
- ✅ **Step 1 — 언어 선택** — ko/en radiogroup + i18n.changeLanguage 즉시.
- ✅ **Step 2 — 환경 점검** — 자동 invoke detect_environment, 4 카드(OS/메모리/GPU/런타임) + status pill (ok/warn/muted) + 한국어 hint (RAM<8GB / 디스크<20GB) + RETRY.
- 🟢 **Step 3 — 첫 모델 설치** — InstallProgress (다운로드/추출/post-check/취소/reboot 안내) + 큐레이션 카드.
- ✅ **Step 4 — 완료** — ✓ 마크 + "시작할게요" CTA.
- 🟡 **vitest + axe-core** — Phase 1A.4.d.

### 7.5 자가스캔 / 자동 갱신 (Phase 1' 잔여 + Phase 6')
- 🟡 **`crates/registry-fetcher`** — manifest 4-tier fallback (vendor API ‖ GitHub releases → jsdelivr → bundled) + ETag/If-Modified-Since + SQLite cache TTL 1h.
- 🟡 **`crates/scanner`** — `tokio-cron-scheduler` 6h cron + on-launch grace + UI 트리거. `summarize_via_local_llm()` (Ollama `/api/generate` keep_alive 30s, 모델 cascade EXAONE → HCX-SEED → Qwen2.5-3B), 한국어 deterministic fallback.
- 🟡 **`tauri-plugin-updater` 통합** + JetBrains-style "다음 실행 때 적용돼요" 토스트.

### 7.6 큐레이션 카탈로그 (Phase 2')
- 🟡 **5 카테고리 × 5~10 모델** — 에이전트/캐릭터/코딩/사운드(STT·TTS)/온디바이스(SLM).
- 🟡 **한국어 튜닝 모델 1순위** — EXAONE 4.0 32B/1.2B, HyperCLOVA-X SEED 8B Omni, Polyglot-Ko 1.3/5.8/12.8B, K-EXAONE 236B-A23B (위 등급 잠금 배지).
- 🟡 **2-tier 거버넌스** (Pinokio 패턴) — Verified(LMmaster 검증) / Community(외부 기여).
- 🟡 **하드웨어-인지 배지** (Foundry 패턴) — "✅ 이 PC에 맞아요" / "위 등급은 잠금" / "토큰/초 ~28".
- 🟡 **벤치마크 통합** (G4) — 30초 짧은 측정으로 token/sec 산출.

### 7.7 게이트웨이 / SDK (Phase 3')
- 🟡 **Gateway proxy** — Axum SSE proxy + GPU contention 직렬화 + per-webapp scoped key.
- 🟡 **multi-provider routing policy** (G8) — local-first / cloud-fallback / 정책 driven.
- 🟡 **`@lmmaster/sdk`** — `LocalCompanionProvider` + 미설치/미실행 감지 + custom URL scheme(`lmmaster://`).
- 🟡 **portable workspace fingerprint repair** (Thesis #3) — manifest diff + 1-click repair plan.
- 🟡 **per-app data segregation** (G6) — workspace 단위 isolation.

### 7.8 한국어 UX 9 화면 (Phase 4)
- 🟡 홈 / 카탈로그 / 설치 센터 / 런타임 / 프로젝트 연결 / 로컬 API / 워크벤치 / 진단 / 설정.
- 🟡 command palette (Raycast 패턴) + 키보드 접근성 + virtual list.
- 🟡 **300+ Korean preset bundle** (G2 — Cherry Studio 패턴) — 코딩/번역/법률/마케팅/의료/교육/리서치 등.

### 7.9 워크벤치 (Phase 5')
- 🟡 **5-화면 플로우** — 모델 선택 → 데이터 드롭(JSONL/CSV/노트 폴더) → 프리셋 → 학습 → 내보내기.
- 🟡 **양자화** — `llama-quantize` 자식 프로세스 driver, 진행률 파싱.
- 🟡 **LoRA** — LLaMA-Factory CLI 1순위 + Unsloth 가속 + mac MLX-LM 분기.
- 🟡 **Korean 데이터 정합성 검증** (Thesis #5) — HCX-Seed tokenizer alignment + 한자 mixed-script + 한글-only 필터.
- 🟡 **GGUF → Ollama Modelfile 자동 생성** + 1-click 등록.
- 🟡 **Korean QA evals** (pytest-shaped fixtures, AI Toolkit 패턴 차용).

### 7.10 RAG / Knowledge Stack (Phase 4.5' — 신설 권장)
- 🟡 **G1 — Msty 패턴 zero-config RAG** — PDF/CSV/MD/DOCX/YouTube 드롭 → 즉시 채팅. 한글-정규화 임베딩.
- 🟡 **per-workspace document isolation**.

### 7.11 Agent / MCP (Phase 6')
- 🟡 **G3 — Agent / Skill manifest** — 우리 app/model manifest와 동형 JSON. Web search / Chart / Code interpreter / MCP client.
- 🟡 **G7 — MCP host** — 데스크톱 MCP host로 동작, 웹앱이 게이트웨이 통해 talk.
- 🟡 **STT / TTS** — faster-whisper + piper.

### 7.12 자가스캔 / 진단 / Gemini 도우미 (Phase 6')
- 🟡 **자가스캔 결과 한국어 자연어 요약** — 옵트인, 로컬 LLM only.
- 🟡 **Gemini 한국어 설치 도우미** — 옵트인. 판정/추천은 deterministic, 설명만 Gemini (ADR-0013).
- 🟡 **`/diagnostics export`** — JSON 산출, 사용자 동의 후만.

---

## 8. 인터페이스 가이드

### 8.1 첫 실행 — 4단계 마법사

```
┌──────────────────────────────────────────────────────────────┐
│ ●─────○─────○─────○                                         │
│ 언어   환경   첫모델  완료                                    │
├──────────────────────────────────────────────────────────────┤
│  언어를 선택해 주세요                                         │
│  언제든 설정에서 바꿀 수 있어요                               │
│                                                               │
│  ┌─────────────┐ ┌─────────────┐                              │
│  │ ● 한국어    │ │ ○ English   │                              │
│  └─────────────┘ └─────────────┘                              │
│                                                               │
│                              [   계속할게요   ]                │
└──────────────────────────────────────────────────────────────┘
```

- 키보드 `Tab` / `Space`로 라디오 선택, `Enter`로 진행.
- 화면 전환은 200ms slide+fade, `prefers-reduced-motion: reduce`면 fade only.
- 에러는 per-step ErrorBoundary가 잡아 한국어 fallback("문제가 생겼어요. 다시 시도해 볼까요?") 표시.

### 8.2 환경 점검 카드 (Step 2)

```
┌──────────────────────────────────────────────────────────────┐
│ 환경을 살펴봤어요                                              │
│ 확인을 마쳤어요. 아래에서 살펴봐 주세요.                       │
├──────────────────────────────────────────────────────────────┤
│ 운영체제                                          [ 괜찮아요 ] │
│ Windows 11 (10.0.26200) · x86_64                              │
├──────────────────────────────────────────────────────────────┤
│ 메모리                                            [ 괜찮아요 ] │
│ 16.0GB 사용 가능 / 32.0GB 전체                                 │
├──────────────────────────────────────────────────────────────┤
│ GPU 가속                                          [ NVIDIA  ] │
│ NVIDIA GeForce RTX 4080 · 16.0GB VRAM                          │
├──────────────────────────────────────────────────────────────┤
│ 런타임                                            [ 사용 중 ] │
│ • Ollama       사용 중       v0.3.x                            │
│ • LM Studio    설치 안 됨                                       │
└──────────────────────────────────────────────────────────────┘
                                         [이전으로] [계속할게요]
```

- `aria-busy="true"` 동안 4 카드 skeleton shimmer.
- 임계 미달(RAM<8GB / 디스크 가용<20GB) 시 카드 테두리 노란색 + hint 텍스트.
- 점검 실패 시 RETRY 버튼 → 캐시 클리어 후 재실행.

### 8.3 첫 모델 설치 (Step 3, Phase 1A.4.c 진행 중)

```
[ 추천 모델 ]
┌─────────────────────────┐ ┌─────────────────────────┐
│ EXAONE 4.0 1.2B         │ │ HyperCLOVA-X SEED 8B    │
│ 한국어 · 1.2B · 800MB   │ │ 한국어+멀티모달 · 5GB   │
│ ✅ 이 PC에 맞아요       │ │ ✅ 이 PC에 맞아요       │
│                         │ │                         │
│ [   받을게요   ]        │ │ [   받을게요   ]        │
└─────────────────────────┘ └─────────────────────────┘
                                    [나중에 할게요]

[ 설치 진행률 — 받을게요 클릭 후 ]
┌──────────────────────────────────────────────────────────────┐
│ EXAONE 4.0 1.2B 받고 있어요                                    │
│ ▓▓▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░  61% · 28.4MB/s · 약 12초 남았어요    │
│                                                                │
│ ▸ 자세히 보기                                                  │
│                                                  [ 그만두기 ]  │
└──────────────────────────────────────────────────────────────┘
```

- 진행률은 Channel<InstallEvent>의 `download.progress` 256KB / 100ms 누적.
- "그만두기"는 즉시 `cancelInstall(id)` invoke → 다운로드 중단, `.partial` 보존(다음 시도에 resume).
- reboot 필요 시 (MSI 3010 등) "다시 시작이 필요해요. 지금 할까요? / 다음에 할게요" 분기.

### 8.4 홈 / 카탈로그 / 설치 센터 (Phase 2'~4)

- 사이드바 9 메뉴 + 하단 Tailscale-style **게이트웨이 status pill** ("사용 가능 :43821").
- 홈 = 추천 카드 + 자가스캔 결과 + 최근 호출 통계.
- 카탈로그 = 5 카테고리 탭 + Verified/Community 토글 + 하드웨어-인지 필터.
- 설치 센터 = 진행 중 작업 + 최근 설치 + 큐.

### 8.5 워크벤치 5단계 (Phase 5')

```
모델 선택 → 데이터 드롭 → 프리셋 → 학습 → 내보내기
                                     │
                                     ├─ Korean 검증
                                     │   ├─ HCX-Seed tokenizer alignment
                                     │   ├─ 한자 mixed-script normalize
                                     │   └─ 한글-only 필터
                                     │
                                     └─ GGUF + Modelfile + Ollama 등록
```

- 각 단계는 xstate 머신으로 BACK/RETRY 가능.
- 데이터 드롭은 폴더/JSONL/CSV/노트 자동 변환.
- 내보내기 시 GGUF + 자동 생성된 Ollama Modelfile + 1-click `ollama create` 호출.

### 8.6 진단 / 설정

- **자가스캔 트리거** — 사이드바 진단 → "지금 점검할게요" 버튼.
- **결과 표시** — deterministic 카드(빨강/노랑/녹색) + 옵션 토글 시 한국어 자연어 요약.
- **`/diagnostics export`** — 사용자 동의 후 JSON 다운로드 (지원 요청용).

### 8.7 기존 웹앱 통합

```ts
import { LocalCompanionProvider } from "@lmmaster/sdk";

const provider = new LocalCompanionProvider({
  scopedKey: "<api-key-from-LMmaster>",
});

if (!(await provider.isAvailable())) {
  // "LMmaster를 설치해주세요" 모달 + lmmaster://launch URL scheme
}

const completion = await provider.chat({
  model: "exaone-4-1.2b",
  messages: [...],
});
```

- 웹앱 부팅 시 `GET /health`로 미설치/미실행 감지 → 자동 안내.
- API 키는 LMmaster 사이드바 "로컬 API"에서 발급, scope per webapp.

---

## 9. 한국어 카피 / 톤 (해요체)

> **Toss 8원칙 + Karrot/Kakao Pay 톤 참고. 사용자-주체로 작성.**

| 상황 | ✅ Use | ✗ Avoid |
|---|---|---|
| 다음 단계 | "계속할게요" | "다음" / "Next" |
| 뒤로 | "이전으로" | "취소" |
| 건너뛰기 | "나중에 할게요" | "건너뛰기" |
| 닫기 | "닫기" | "취소" |
| 에러 | "문제가 생겼어요. 다시 시도해 볼까요?" | "오류 발생" |
| 대기 | "잠시만 기다려 주세요" / "최대 3초 정도 걸려요" | "Loading..." |
| 성공 | "준비됐어요" / "받았어요" / "끝냈어요" | "Success" / "Done" |
| 위험 안내 | "지금 정리하면 12.4 GB 회수할 수 있어요" | "권장" 같은 명령형 |

로안워드는 그대로 (런타임 / 모델 / GPU 가속 / 토큰 / API 키). 첫 등장 시 한 번 풀어서: "로컬 모델(인공지능 두뇌)".

---

## 10. 로드맵 — 타임라인 / 마일스톤

```
Phase α  ████████████████████  ✅ Foundation docs
Phase 0  ████████████████████  ✅ Tauri+Axum boot
Phase 1A.1~1A.3 ██████████████  ✅ runtime-detector / hardware-probe / installer / IPC
Phase 1A.4.a~b  ██████████████  ✅ 마법사 골격 + Step 1/2/4
Phase 1A.4.c    ░░░░░░░░░░░░░  🟢 Step 3 첫 모델 설치 (다음)
Phase 1A.4.d    ░░░░░░░░░░░░░  🟡 vitest + axe-core
Phase 1' 잔여   ░░░░░░░░░░░░░  ⏳ registry-fetcher / scanner / runtime-manager 보강
Phase 2'        ░░░░░░░░░░░░░  ⏳ 카탈로그 + 한국어 1순위 + 하드웨어 인지 + 2-tier
Phase 3'        ░░░░░░░░░░░░░  ⏳ Gateway proxy + SDK + 포터블 + multi-provider 라우팅
Phase 4         ░░░░░░░░░░░░░  ⏳ 9 한국어 화면 + Korean presets 100+ + command palette
Phase 4.5'      ░░░░░░░░░░░░░  ⏳ Knowledge Stack RAG (Msty 패턴, G1)
Phase 5'        ░░░░░░░░░░░░░  ⏳ 워크벤치 v1 (양자화 + LoRA + Korean 검증 + GGUF→Ollama)
Phase 6'        ░░░░░░░░░░░░░  ⏳ 자동 갱신 + Gemini + STT/TTS + MCP host + Pipelines + 출시
```

각 페이즈는 **보강 리서치 → 설계 조정 → 프로덕션 구현 → 검증** 4단계로 진행. 토큰 한계 시 RESUME → 새 세션.

---

## 11. 운영 원칙 (사용자 약속)

- **외부 통신 0**: localhost-only 바인딩, no-proxy, 자가스캔 결과는 디바이스 로컬.
- **EULA / 라이선스 준수**: LM Studio는 silent install 안 함(EULA), Ollama만 silent install(MIT).
- **deterministic 우선**: 판정/추천은 항상 deterministic 로직, 사용자 향 자연어 요약만 LLM.
- **opt-in 데이터 수집**: 텔레메트리 기본 OFF. `/diagnostics export`도 사용자 명시 동의 후만.
- **포터블 우선**: 모든 상태가 워크스페이스 폴더에. registry/AppData에는 캐시·사용자 환경 외 저장 안 함.
- **한국어 우선**: 모든 사용자-노출 메시지·로그·도움말이 한국어 1차. 영어는 fallback.

---

## 부록 A — 경쟁 사례 핵심 학습 (요약)

> 전체 분석은 본 세션 리서치 산출물 참고. 아래는 v1 직접 적용 항목.

| 출처 | 적용 패턴 | 매핑 |
|---|---|---|
| **LM Studio** | Hub 패턴 (presets publish + revisions), Hardware tab 깊이 | Phase 2' (큐레이션) + Phase 5' (워크벤치 hardware 비교) |
| **Ollama new app** | 드래그-드롭 파일 + 멀티-세션 채팅 | Phase 4 — 모방하지 않고 워크벤치+포터블+Korean으로 차별화 |
| **Jan.ai** | Assistants + Threads + MCP host | Phase 4 (presets) + Phase 6' (MCP host, G7) |
| **Msty** | Knowledge Stack zero-config RAG, split chat | Phase 4.5' (G1 — RAG) + Phase 4 (split chat 후순위) |
| **AnythingLLM** | Workspace + Agent Skill Store | Phase 3' (workspace) + Phase 6' (G3 — agent manifest) |
| **Cherry Studio** | 300+ assistants 프리셋, 멀티 메신저 sandbox | Phase 4 (G2 — 100+ Korean presets) |
| **Open WebUI** | Pipelines (UI plugin) | Phase 6' (Thesis #7 — pipelines를 게이트웨이 layer에 적용) |
| **GPT4All** | LocalDocs (folder-only RAG) | Phase 4.5' (Msty + GPT4All 합집합) |
| **Page Assist** | 브라우저 사이드바 컴패니언 | Phase 6' (sdk 브라우저 bridge) |
| **Pinokio** | Verified/Community 2-tier + reproducible install | Phase 2' (G5 — 2-tier 거버넌스) + Phase 1A.3 (이미 매니페스트) |
| **Foundry Local** | hardware-detection → 카탈로그 필터 | Phase 2' (G4 — 하드웨어 인지 배지) |
| **AI Toolkit / Foundry Toolkit** | Evaluation-as-tests (pytest-style) | Phase 5' (Thesis #5 — Korean QA evals) |
| **JetBrains Toolbox 3.3** | "다음 실행 때 적용" + 오프라인 미러 | Phase 6' (자동 갱신 토스트) |
| **Tailscale windowed UI** | 상태 pill + dock 빨간 배지 (blocking 시) | Phase 4 (게이트웨이 pill) |

## 부록 B — 한국어 튜닝 모델 1순위 카탈로그 (Phase 2' 시드)

| 모델 | HF slug | 양자화 | 카테고리 | 비고 |
|---|---|---|---|---|
| EXAONE 4.0 32B | `LGAI-EXAONE/EXAONE-4.0-32B-GGUF` | Q4_K_M ~19 GB | 추론 / 에이전트 | LG AI Research, ko/en 이중 |
| EXAONE 4.0 1.2B | `LGAI-EXAONE/EXAONE-4.0-1.2B-GGUF` | ~800 MB | 온디바이스 / SLM | 저-VRAM 첫 모델 |
| EXAONE 3.5 7.8B Instruct | `LGAI-EXAONE/EXAONE-3.5-7.8B-Instruct-GGUF` | ~5 GB | 일반 에이전트 | 중간 사양 추천 |
| K-EXAONE 236B-A23B | `LGAI-EXAONE/K-EXAONE-236B-A23B-GGUF` | Q4 ~120 GB | 위 등급 (잠금 배지) | 데이터센터 시연 |
| HyperCLOVA-X SEED 8B Omni | `naver-hyperclovax/HyperCLOVAX-SEED-Vision-Instruct-7B` 류 | ~5 GB | 멀티모달 / 코딩 | 한국어 토크나이저 ≈2× |
| Polyglot-Ko 1.3B / 5.8B / 12.8B | `EleutherAI/polyglot-ko-*` | varies | 오픈소스 헤리티지 | 라이선스 명확 |
| A.X 시리즈 (확정 시) | TBD | TBD | 캐릭터 / 롤플레이 | Phase 2' 큐레이션 시점에 검증 |

---

## 부록 C — 로드맵 갭(G1~G8) 요약

| # | 갭 | 노출 경쟁자 | 반영 페이즈 |
|---|---|---|---|
| G1 | Knowledge-Stack-style 제로 컨피그 RAG | Msty / GPT4All / Page Assist / AnythingLLM | Phase 4.5' (신설) |
| G2 | Korean 프리셋 100+ 번들 | Cherry / Jan / LM Studio Hub | Phase 4 |
| G3 | Agent / Skill manifest (app/model과 동형) | AnythingLLM / Open WebUI / Ollama Agent Store | Phase 6' (ADR 0017 addendum) |
| G4 | 하드웨어 벤치마크 (probe만 아님) | LM Studio Hardware tab | Phase 2' |
| G5 | 2-tier (Verified/Community) 카탈로그 거버넌스 | Pinokio / LM Studio Hub | Phase 2' (ADR 0014 확장) |
| G6 | 워크스페이스별 데이터 격리 | AnythingLLM / Msty | Phase 3' |
| G7 | MCP host capability | Jan / Page Assist / AI Toolkit | Phase 6' |
| G8 | multi-provider routing 정책 (per-webapp scope) | Cherry / Jan / Open WebUI 부분 | Phase 3' (ADR 0006 확장) |

---

## 부록 D — 참고 링크

- **PIVOT 결정**: `docs/PIVOT.md`
- **ADR 인덱스**: `docs/adr/README.md` (0001~0021 + 향후 0022~0024 예정 — 0022: 게이트웨이 라우팅 정책 / 0023: Knowledge Stack RAG / 0024: Pipelines surface)
- **페이즈 운영**: `docs/PHASES.md`
- **인계 노트**: `docs/RESUME.md`
- **사용자 가이드**: `docs/guides-ko/`
- **개발자 가이드**: `docs/guides-dev/`
- **연구 노트**: `docs/research/phase-*-decision.md`
- **OSS 라이선스 매트릭스**: `docs/oss-dependencies.md`
- **위험 분석**: `docs/risks.md`

---

**문서 버전**: v1.0 (2026-04-27 초안). PRODUCT.md는 PIVOT 이후 비전·USP·차별화·로드맵을 종합한 단일 진입점입니다. 페이즈 진행으로 기능이 추가될 때마다 §7 / §8 / §10을 업데이트해 주세요.
