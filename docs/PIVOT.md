# PIVOT — v1 포지셔닝 확정 및 전환 계획

> 2026-04-26 확정. GPT 외부 검토 결과와 4영역 보강 리서치(Pinokio·Open WebUI/Jan/Msty/Cherry/Foundry·LM Studio+Ollama orchestrability·온디바이스 파인튜닝 UX)를 바탕으로 v1 포지셔닝을 명확히 했다.
> 이 문서는 변경 이전 산출물의 부정 범위와 새 페이즈 구조를 한 곳에 정리한다.

## 0. 결정 요약 (Executive)

LMmaster v1 포지셔닝은 **"LM Studio/Ollama를 대체"가 아니라 "LM Studio/Ollama를 포함해 묶는 한국어 기반 로컬 AI 운영 허브"**다. 자체 런타임(llama.cpp 자식 프로세스)은 v1 default가 아니라 v1.x의 zero-config 옵션으로 격하한다.

대신 v1 리소스가 초반 집중되는 6 pillar:

1. **자동 설치 + 환경 셋업** — LM Studio/Ollama 감지·설치·업데이트, GPU/드라이버 자동 매칭, 한국어 zero-touch 마법사.
2. **한국어 인터페이스** — onboarding/추천/오류/도움말이 한국어 기본.
3. **포터블 워크스페이스** — 셋업 후 다른 환경으로 이동, 자동 fingerprint repair.
4. **카테고리 큐레이션** — 에이전트/캐릭터/코딩/사운드/온디바이스 한 화면, hardware-aware 추천.
5. **워크벤치 (v1 핵심으로 격상)** — 양자화 + 도메인 LoRA 파인튜닝.
6. **자동 갱신** — 본체·LM Studio·Ollama·모델 카탈로그 1-click 업데이트.

이로써 ADR-0005(llama.cpp primary)는 **Superseded**, ADR-0012(workbench placeholder)는 **Modified**된다. 새 ADR-0016·0017·0018이 추가된다.

## 1. 무엇이 그대로 유지되는가 (Continuity, ~80%)

| 산출물 | 변경 |
|---|---|
| ADR-0001 (Companion 데스크톱 + localhost gateway) | **유지**. 오히려 강화 — 이제 gateway는 LM Studio/Ollama로의 thin proxy. |
| ADR-0002 (Tauri 2) | **유지**. |
| ADR-0003 (Rust + Axum) | **유지**. |
| ADR-0004 (Adapter pattern + RuntimeAdapter trait) | **유지**. 트레이트는 동일, 어댑터 우선순위만 재배치. |
| ADR-0006 (OpenAI-compatible REST) | **유지**. v1 gateway는 이 위에서 LM Studio/Ollama로 라우팅. |
| ADR-0007 (자체 키 매니저) | **유지**. |
| ADR-0008 (SQLite + SQLCipher) | **유지**. |
| ADR-0009 (Portable workspace manifest) | **유지**. 사용자가 명시적으로 원하는 핵심. |
| ADR-0010 (한국어 우선) | **유지**. 더 강조됨. |
| ADR-0011 (디자인 시스템 공유) | **유지**. |
| ADR-0013 (Gemini 경계) | **유지**. |
| ADR-0014 (Curated model registry) | **유지**. 카테고리 큐레이션이 더 중요해짐. Pinokio 패턴(Verified + Community 2-tier)과 Foundry 패턴(hardware-aware badge) 도입 권장. |
| ADR-0015 (specta 타입 공유) | **유지**. |
| 디자인 토큰 / SDK 인터페이스 / 9개 화면 IA | **유지**. |
| Phase 0 산출물 (Tauri+Axum boot, /health, 디자인 토큰) | **유지**. 손대지 않음. |

## 2. 무엇이 바뀌는가 (Delta, ~20%)

### 2.1 ADR-0005 (llama.cpp primary) → **Superseded by ADR-0016**

- v1 default backend는 LM Studio + Ollama (HTTP attach + 자동 설치).
- llama.cpp 자식 프로세스는 v1.x의 "둘 다 설치하기 어려운 환경" zero-config 옵션으로 격하.
- KoboldCpp / vLLM 어댑터는 Phase 2 이후로 미룸 (당장 우선순위 낮음).

### 2.2 ADR-0012 (Workbench placeholder) → **Modified by ADR-0018**

- 워크벤치는 v1 placeholder가 아니라 **v1 핵심 산출물**.
- v1 MVP: 양자화(llama-quantize) + LoRA 파인튜닝(LLaMA-Factory CLI) 통합.
- 5-화면 플로우: 모델 선택 → 데이터 드롭 → 프리셋 → 학습 → 내보내기.

### 2.3 어댑터 우선순위 재배치 (ADR-0004는 유지, 우선순위만 재배치)

| 어댑터 | v1 위치 | 통합 방식 |
|---|---|---|
| **OllamaAdapter** | **1순위** | HTTP attach (`:11434`). 미설치 시 우리가 silent install 가능 (MIT). |
| **LMStudioAdapter** | **1순위** | HTTP attach (`:1234`). + `lms` CLI 보조. 미설치 시 사용자에게 공식 설치 페이지 안내(EULA 상 재배포 금지). |
| **LlamaCppAdapter** | v1.x | 자식 프로세스. 둘 다 설치 어려운 환경의 zero-config 옵션. |
| KoboldCppAdapter | v2 | 보류 (수요 적고 AGPL 부담). |
| VllmAdapter | v2 | 고사양 서빙 옵션. |

### 2.4 새 컴포넌트 추가 (ADR-0017)

- `crates/runtime-detector` — Ollama `/api/version` HTTP probe + LM Studio `/v1/models` probe + 레지스트리/plist fallback.
- `crates/installer` — 설치 manifest 기반 다운로드(SHA256 검증) + tauri-plugin-shell로 silent installer 실행.
- `crates/updater` — 6~24h 폴러: GitHub releases (Ollama), LM Studio changelog JSON, 우리 모델 manifest. 비차단 토스트.
- 기존 `crates/runtime-manager`는 라이프사이클 supervisor로 역할 축소(자식 프로세스 spawn은 llama.cpp 모드일 때만).

### 2.5 새 화면 (디자인 시스템·9 IA는 유지)

- **첫 실행 마법사** — "환경을 점검하는 중", "Ollama를 설치할까요?", "LM Studio는 공식 사이트로 안내합니다", "추천 모델을 받겠습니다" 4단계 한국어 stepper. 기존 9개 화면 IA에 영향 없이 추가.
- **워크벤치 화면 5단계** — 기존 placeholder 자리에 실제 동작.

## 3. 새 페이즈 구조

기존 M0~M6에서 Phase 0(완료)는 그대로 두고, Phase 1 이후 재배치:

| Phase | 코드 | 상태 | Goal |
|---|---|---|---|
| α | docs | ✅ | Foundation docs (ADR + arch + guides + scaffold) |
| **0** | M0 | ✅ | Tauri+Axum boot + dev shell + 디자인 토큰 |
| **1 (new)** | M1 | 다음 | **외부 런타임 감지·설치·헬스체크 + 한국어 첫실행 마법사** (Ollama silent install + LM Studio 설치 안내 + 버전 detect + GPU/CUDA 자동 판단) |
| **2 (new)** | M2 | | hardware probe(경량) + **curated 카탈로그 by 카테고리** + Foundry-style hardware-aware 추천 (5-10 모델/카테고리, 점진 공개) |
| **3 (new)** | M3 | | **Gateway proxy → LM Studio/Ollama 라우팅** + SDK + portable workspace + 기존 웹앱 통합 데모 |
| **4** | M4 | | 9개 한국어 UX 화면 완성 + 첫실행 마법사 + command palette |
| **5 (new)** | M5 | | **워크벤치 v1**: 양자화(llama-quantize) + LoRA(LLaMA-Factory CLI) + JSONL 데이터 인입 + 5단계 플로우 |
| **6 (new)** | M6 | | **자동 갱신 폴러** + Gemini 한국어 도우미 + STT/TTS 브릿지 + v1 출시 체크리스트 |

각 페이즈는 여전히 4단계(보강 리서치 → 설계 조정 → 프로덕션 구현 → 검증) 운영. PHASES.md를 동시 업데이트한다.

## 4. 마이그레이션 영향 (구체적 코드/문서 작업)

### 4.1 즉시 (이번 세션 / 다음 세션 진입 전)
- [x] `docs/PIVOT.md` (이 문서)
- [x] `docs/research/pivot-reinforcement.md` — 4영역 리서치 종합
- [x] ADR-0005 status → Superseded by 0016 헤더 추가
- [x] ADR-0012 status → Modified by 0018 헤더 추가
- [x] ADR-0016 (Wrap-not-replace), ADR-0017 (Manifest+Installer), ADR-0018 (Workbench v1 core) 신설
- [x] ADR README 인덱스 갱신
- [x] `docs/PHASES.md` 페이즈 표 재배치
- [x] 메모리 `product_pivot_v1` 추가

### 4.2 Phase 1 시작 시 (다음 세션)
- [ ] `crates/runtime-detector` 신설 (Ollama/LM Studio HTTP probe + OS 별 파일 시스템 fallback)
- [ ] `crates/installer` 신설 (download + sha256 + tauri-plugin-shell silent spawn)
- [ ] `manifests/apps/{ollama,lm-studio}.json` — Pinokio-style 설치 매니페스트
- [ ] 첫실행 마법사 화면(react) — 한국어 4단계 stepper
- [ ] 어댑터 우선순위 재배치 (OllamaAdapter / LMStudioAdapter 실제 attach 구현, llama-cpp는 stub 유지)
- [ ] `docs/oss-dependencies.md` 갱신 — LM Studio/Ollama가 v1 핵심, llama.cpp는 v1.x 옵션, LLaMA-Factory/Unsloth 추가
- [ ] tauri-plugin-shell, tauri-plugin-http, tauri-plugin-updater 추가 (capability JSON 권한 명시)

### 4.3 Phase 5 시작 시 (워크벤치)
- [ ] `workers/ml/` placeholder를 LLaMA-Factory CLI driver로 교체
- [ ] `crates/quantizer` (llama-quantize 자식 프로세스 driver)
- [ ] 5-화면 플로우 한국어 UX
- [ ] mac MLX-LM 분기

## 5. 사용자에게 약속하는 v1 핵심 시나리오

1. **(첫 실행)** 사용자가 LMmaster를 실행한다. 한국어 마법사가 환경을 점검 → Ollama가 없으면 silent install, LM Studio가 없으면 공식 페이지 1-click 안내 → "추천 모델 3종"을 카테고리별로 표시.
2. **(채팅)** 사용자가 모델 카드를 클릭하면 자동으로 적합한 백엔드(Ollama 또는 LM Studio)에 모델이 pull되고 LMmaster의 통합 채팅 UI에서 즉시 사용 가능.
3. **(기존 웹앱)** 기존 웹앱은 `@lmmaster/sdk`만 추가하고 `LocalCompanionProvider`를 부르면 LMmaster가 어떤 백엔드를 쓰는지 모른 채 OpenAI 호환 응답을 받는다.
4. **(포터블)** 워크스페이스 폴더를 다른 PC로 옮기면 fingerprint 비교 후 자동 repair (Ollama 재설치 + 모델 재다운로드 안내).
5. **(워크벤치)** "내 데이터로 파인튜닝" 메뉴 → JSONL/CSV/노트 폴더 드롭 → 프리셋 선택 → 학습 → GGUF 내보내기 → Ollama에 등록까지 1-click.
6. **(업데이트)** 1주일 뒤 LM Studio가 새 버전 릴리스 → LMmaster가 토스트로 알림 → 사용자가 "설치" 클릭하면 LM Studio 자체 업데이터를 우리가 호출.

## 6. 다음 단계

이 PIVOT 문서·ADR 3건이 사용자 승인 후 **Phase 1 보강 리서치**로 진입한다.
Phase 1 보강 영역(예상): Pinokio detector/installer 코드 패턴 deep-read · tauri-plugin-shell capability ACL · LM Studio EULA 정확한 인용 · Ollama silent install 옵션 검증 · GPU detect 라이브러리(`wgpu-hal`/nvml).
