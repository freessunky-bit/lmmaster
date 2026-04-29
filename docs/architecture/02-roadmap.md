# 4. 단계별 구현 로드맵

> 산출물 #4. M0 → M6의 마일스톤별 산출물, exit criteria, 의존 관계.

각 마일스톤은 **end-to-end 동작**을 목표로 한다(언제든 멈춰도 데모 가능). 후행 마일스톤이 선행 마일스톤을 깨면 안 된다.

## M0 — Skeleton & Decision Lock (2주 추정)

목표: repo가 빌드 가능하고, 모든 핵심 결정이 ADR로 잠겨 있다.

산출물:
- 모노레포 워크스페이스(Cargo workspace + npm workspace).
- `apps/desktop` Tauri 2 + Vite + React 빈 셸이 실행됨.
- `crates/core-gateway`가 `127.0.0.1:auto-port`에서 `GET /health` 응답.
- `packages/design-system` 토큰 + `packages/js-sdk` 빈 패키지 publish 준비.
- `docs/adr` 0001~0014 모두 Accepted.
- 한국어 README v0 + 한국어 시작 가이드 placeholder.

Exit criteria:
- `cargo build --workspace`, `pnpm build` 모두 통과.
- 데스크톱 앱이 dark + 네온 그린 토큰만 적용된 빈 사이드바 + 빈 콘텐츠로 뜬다.
- gateway health endpoint가 SDK ping 단위 테스트 통과.

## M1 — Hardware Probe & Recommender (2주)

목표: PC를 점검하고 deterministic하게 추천을 산출한다.

산출물:
- `crates/hardware-probe`: OS / CPU / RAM / GPU(NVIDIA/AMD/Intel/Apple) / VRAM / 디스크 / CUDA·ROCm·Vulkan·Metal·DirectML capability 탐지.
  - Win: WMI + nvml(있으면) + 그래픽 드라이버 쿼리.
  - mac: system_profiler + Metal capability.
  - Linux: `/proc`, `/sys`, `lspci`, nvml/rocm-smi.
- `crates/model-registry`: manifest 스키마 + 로컬 cache + Recommender(deterministic 점수 함수) + seed manifest(데모용 4~6개 모델).
- 홈 화면: 사양 요약 카드 + 권장 모델 3종(best/balanced/lightweight) + fallback 카드.

Exit criteria:
- 새 PC에서 첫 실행 시 hardware probe → 한국어 사양 요약 + 추천 3종이 5초 이내 표시.
- 동일 PC + 동일 manifest = 같은 추천(스냅샷 테스트).

의존: M0.

## M2 — Runtime Adapter & Local Gateway (3주)

목표: llama.cpp를 어댑터로 실행해 OpenAI 호환 채팅이 동작한다.

산출물:
- `crates/runtime-manager` + `crates/adapter-llama-cpp`.
- 자식 프로세스 supervisor: spawn / health / restart / graceful stop.
- `POST /v1/chat/completions` (stream + non-stream). `GET /v1/models`.
- API key 인증 미들웨어 + 사용 로그.
- 모델 다운로드(재개 가능, sha256 검증), 워밍업, standby 상태기계.
- 설치 센터 UI: 다운로드 큐 + 진행률 + 오류 복구.

Exit criteria:
- curl 또는 OpenAI SDK로 `chat/completions` 스트리밍 응답 수신.
- 모델 다운로드 도중 네트워크 끊김 → 재시도로 재개.
- 앱 재시작 후 standby 상태 복구.

의존: M0, M1.

## M3 — JS/TS SDK & 기존 웹앱 통합 예제 (2주)

목표: 기존 웹앱이 PR 1개로 LMmaster 사용 가능.

산출물:
- `packages/js-sdk` v0.1: 핵심 메서드 — pingHealth, getInstalledRuntimes, getInstalledModels, getRecommendedModels, installRuntime, installModel, getInstallProgress, issueApiKey, listApiKeys, chatCompletions, streamChat, embeddings(스키마만), getGatewayStatus, bindProject, getProjectBindings.
- `examples/webapp-local-provider`: 기존 웹앱 형태의 미니 데모. provider 1개 추가해 채팅이 LMmaster를 거쳐 응답.
- 미설치/미실행 감지 + 한국어 모달 + custom URL scheme(`lmmaster://`).
- 키 발급 GUI + clipboard 복사 + scope 선택.

Exit criteria:
- 데모 웹앱이 LMmaster 종료 상태에서 "설치/실행해주세요" 모달, 실행 후 자동 연결.
- 발급한 키로 외부 앱이 채팅 가능.
- SDK 타입 d.ts 완전.

의존: M2.

## M4 — UX 완성: 카탈로그·런타임·프로젝트·키·로그·설정 (3주)

목표: 9개 화면을 모두 한국어로 완성하고 디자인 시스템 100% 적용.

산출물:
- 모델 카탈로그(필터/검색/비교/태그/적합도).
- 런타임/엔진 화면(설치·상태·포트·헬스).
- 프로젝트 연결 화면(웹앱 바인딩, endpoint 복사, SDK 예제 코드 출력).
- 로컬 API/키 관리(권한, 사용 로그, revoke, scope).
- 진단/로그 화면(설치/runtime/gateway 로그 + troubleshooting 카드).
- 설정(경로, 캐시 정책, 자동 업데이트, 포트, 언어, 실험 기능, Gemini 토글).
- Command palette + Toast + Confirm dialog + Loading overlay 패턴 완성.

Exit criteria:
- 키보드 접근성 통과(모든 인터랙션 키보드만으로 가능).
- 다크/네온 외 컬러 사용 0건.
- 한국어 카피 voice & tone 가이드 준수(코드 리뷰).

의존: M3.

## M5 — 추가 어댑터: KoboldCpp / Ollama / LM Studio / vLLM (3주)

목표: 다중 런타임 카탈로그 완성.

산출물:
- `crates/adapter-koboldcpp` (자식 프로세스 supervised).
- `crates/adapter-ollama` (외부 설치 attach + pull/run 위임).
- `crates/adapter-lmstudio` (외부 설치 attach).
- `crates/adapter-vllm` (Linux+CUDA 우선, 비활성/고급 옵션).
- capability matrix가 gateway routing에 반영(예: 비전 요청 → vision 가능 어댑터로).

Exit criteria:
- 4개 어댑터 모두 회귀 테스트 통과.
- 사용자가 같은 모델을 두 어댑터로 비교 실행 가능.

의존: M2.

## M6 — Gemini 한국어 도우미 + 사운드(STT/TTS) 슬롯 + Workbench placeholder (2주)

목표: v1 출시 가능 상태.

산출물:
- Gemini 한국어 설명 통합(opt-in, 토글, 오프라인 fallback 한국어 템플릿 항상 보유).
- STT 1개 + TTS 1개 어댑터/모델(예: faster-whisper / piper) 데모 동작.
- 워크벤치 화면 placeholder + 인터페이스 정의(파인튜닝/양자화/export 버튼은 disabled).
- 한국어 문서 일체 완성: 시작 / 설치 / 웹앱 연동 / 키 발급 / 문제 해결 / 개발자 SDK / 워크벤치 확장 가이드.

Exit criteria:
- v1 출시 체크리스트 100%.
- 코드 사인/공증(Win/mac), Linux AppImage 검증.

의존: M4, M5.

## Post-v1 (참고)

- v1.1: Anthropic-compatible shim, embeddings 실제 구현, rerank.
- v1.2: 사용자 추가 manifest, 워크벤치 SFT/LoRA 베타.
- v1.3: 양자화/GGUF export, 온디바이스 패키징.
- v1.4: 영어 locale, 다중 사용자 OS 지원 강화.
- v2: LiteLLM remote/team mode 옵션, dynamic plugin 로딩.

## 마일스톤 의존 그래프

```
M0 ── M1 ── M2 ── M3 ── M4 ── M6
                  └── M5 ──────┘
```

M4와 M5는 일부 병렬 가능(다른 팀원이 어댑터 추가, UX 팀이 화면 완성).

## 측정 지표

- **첫 실행 → 추천까지** 시간 목표: < 10초.
- **모델 다운로드 → 첫 응답까지** 목표: 5GB 모델 기준 다운로드 후 < 30초 워밍업.
- **gateway 응답 latency overhead**: raw runtime 대비 < 5% (측정 단위: 첫 토큰 시간).
- **번들 크기**: 데스크톱 본체 설치본 < 200MB(런타임/모델 제외).
