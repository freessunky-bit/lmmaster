# 1. 아키텍처 요약 — LMmaster

> 산출물 #1. 이 문서는 시스템의 high-level 그림이다. 세부 결정은 ADR과 각 컴포넌트 문서에서 다룬다.

## 1.1 한 줄 정의

LMmaster는 **사용자 PC에서 실행되는 데스크톱 프로그램**으로, 기존 HTML 웹앱과 다른 웹앱들이 **로컬 HTTP API 또는 JS/TS SDK**만 호출해 LLM/STT/TTS 추론을 사용하도록 만드는 **Local AI Companion**이다. 런타임의 설치·업데이트·헬스체크·라우팅·폴백·키 관리·하드웨어 적합도 추천을 LMmaster가 모두 책임진다.

## 1.2 6-레이어 구조

```
┌────────────────────────────────────────────────────────────────┐
│ ① Desktop Control Plane (Tauri 2 + React/TS)                   │
│   - 홈 / 카탈로그 / 설치센터 / 런타임 / 프로젝트 / 키 / 워크벤치 │
│   - 사용자 GUI. localhost gateway에 IPC 또는 HTTP로 접근         │
└──────────────────────────┬─────────────────────────────────────┘
                           │ Tauri IPC (in-process)
┌──────────────────────────┴─────────────────────────────────────┐
│ ② Local Gateway (Rust + Axum, localhost-only)                  │
│   - OpenAI-compatible REST + SSE                                │
│   - API key auth · rate limit · audit log · usage log           │
│   - 모델 라우팅 / 폴백 / 큐                                     │
└──────────────────────────┬─────────────────────────────────────┘
                           │ Adapter trait
┌──────────────────────────┴─────────────────────────────────────┐
│ ③ Runtime Adapters (Rust trait + per-runtime crate)             │
│   - LlamaCppAdapter / KoboldCppAdapter / OllamaAdapter          │
│   - LMStudioAdapter / VllmAdapter                               │
│   - detect / install / start / stop / health / pullModel ...    │
└──────────────────────────┬─────────────────────────────────────┘
                           │ subprocess / HTTP
┌──────────────────────────┴─────────────────────────────────────┐
│ ④ External Runtime Processes (third-party OSS, supervised)      │
│   llama.cpp server · koboldcpp · ollama · lm-studio · vLLM      │
└────────────────────────────────────────────────────────────────┘

⑤ Shared Design System (packages/design-system) — ①과 기존 웹앱이 공유
⑥ JS/TS SDK (packages/js-sdk) — 기존 웹앱과 다른 웹앱이 ②를 호출
   ↓ optional
⑦ ML Workbench Worker (workers/ml, Python sidecar) — v1 placeholder
```

## 1.3 보조 서브시스템

| 서브시스템 | 위치 | 책임 |
|---|---|---|
| Hardware Probe | `crates/hardware-probe` | OS/CPU/RAM/GPU/VRAM/디스크/CUDA·ROCm·Vulkan·Metal·DirectML capability 탐지 |
| Model Registry | `crates/model-registry` | curated remote manifest 동기화, 로컬 cache, 카테고리별 메타데이터 |
| Recommender | `crates/model-registry` 내부 | hardware probe + 모델 메타로 best/balanced/lightweight/fallback 산출 (deterministic) |
| Runtime Manager | `crates/runtime-manager` | adapter 호출, 프로세스 supervisor, health/warmup/standby 상태기계 |
| Portable Workspace | `crates/portable-workspace` | `/app /data /models /cache /runtimes /manifests /logs /projects /sdk /docs /exports`, 상대경로 + manifest |
| Key Manager | `crates/key-manager` | API 키 생성/revoke/scope, SQLite(+ 옵션 SQLCipher) |
| Installer/Orchestrator | `crates/runtime-manager` + `crates/model-registry` | 다운로드(재개) · 체크섬 · 압축해제 · 헬스체크 · 워밍업 · 롤백 |

## 1.4 프로세스 모델

- **단일 Tauri 앱**이 사용자에게 보이는 프로세스. 그 안에 Rust 메인 프로세스가 supervisor 역할.
- supervisor는 **embedded Local Gateway**(Axum)를 동일 프로세스 내 별도 task로 기동. 기본 `127.0.0.1:<auto-pick port>`. 포트는 user-config 가능, 충돌 시 자동 회피.
- supervisor는 **runtime processes**(llama.cpp server 등)를 **자식 프로세스**로 spawn하거나, 외부 설치형(Ollama, LM Studio)은 **이미 떠 있는 프로세스에 attach**.
- **ML Workbench Worker**는 v1에서 placeholder. 활성화 시 `workers/ml`의 Python 서버를 별도 자식 프로세스로 spawn하고 stdio/local socket로 통신.

## 1.5 데이터 흐름 (핵심 시나리오)

### A. 기존 웹앱이 채팅 요청
```
HTML 웹앱
  → @lmmaster/sdk.streamChat({model, messages})
    → POST http://127.0.0.1:<port>/v1/chat/completions
       Authorization: Bearer <local-api-key>
      → Local Gateway: 키 검증 → 모델 라우팅 결정
        → RuntimeAdapter.chat(model, messages, stream=true)
          → llama.cpp server /completion (streaming)
        ← SSE chunks
      ← SSE chunks (OpenAI-compatible delta format)
    ← AsyncIterable<Delta>
  ← UI에 토큰 단위 출력
```

### B. 사용자가 Control Plane에서 모델 설치
```
React UI: "설치" 버튼 클릭
  → Tauri IPC: install_model(model_id)
    → ModelRegistry.resolve(model_id) → manifest
    → RuntimeAdapter.pullModel(manifest) (재개 가능, checksum 검증)
    → 진행률은 IPC event로 push, gateway에도 동일 진행률 endpoint 노출
    → 완료 후 RuntimeAdapter.warmup(model)
    → standby 상태로 전환, 카탈로그 카드에 "사용 가능" 뱃지
```

### C. 다른 웹앱 개발자가 키 발급 후 연동
```
Control Plane: "프로젝트 연결" → "키 발급" → scope/모델 선택
  → KeyManager.issue(scope) → SQLite 저장
  → UI에 1회 표시 + clipboard 복사
  → 개발자는 자기 웹앱에서 SDK 초기화 시 사용
  → 이후 호출은 Local Gateway 인증 통과
```

## 1.6 외부 인터페이스(계약)

| 인터페이스 | 형태 | 사용자 |
|---|---|---|
| **Local HTTP Gateway** | OpenAI-compatible REST + SSE, 향후 Anthropic shim | 모든 클라이언트 (웹앱·CLI·외부 IDE) |
| **JS/TS SDK** | npm package `@lmmaster/sdk`, ESM + types | 웹앱 / 노드 백엔드 |
| **Tauri IPC** | invoke + event 채널 | 데스크톱 GUI 내부 전용 |
| **Discovery probe** | `GET /health` + 표준 디스커버리 포트 후보 | 기존 웹앱이 LMmaster 설치/실행 여부 감지 |

raw runtime port (예: llama.cpp의 `:8080`)는 **외부에 절대 노출하지 않는다**. 모든 트래픽은 Local Gateway를 통과한다.

## 1.7 상태 모델 (모델/런타임)

```
[Not Installed] → [Downloading] → [Verifying] → [Extracting]
                                                    ↓
                  [Failed] ←── error ──────  [Installed/Cold]
                     ↓ retry                       ↓ start
                 [Recover]                    [Warming Up]
                                                    ↓
                                                [Standby]
                                                    ↓ request
                                                [Active]
                                                    ↓ idle timeout
                                                [Standby]
```

각 상태는 SQLite에 persist. 앱 재시작 후 복구. UI/SDK 모두 동일 상태 enum을 본다.

## 1.8 Portable Workspace 원칙

- 모든 경로는 **상대경로 + 워크스페이스 매니페스트(`manifest.json`)** 로 표현.
- 첫 실행 또는 다른 PC로 폴더를 옮긴 경우 hardware re-probe 후 manifest의 `host_fingerprint`와 비교, 다르면 **repair flow** 진입.
- 같은 OS/아키텍처 계열 안에서의 이동은 지원. cross-OS 이동은 재설치 가이드 제시(허용은 하되 경고).

## 1.9 비핵심(v1 placeholder)

- ML Workbench (파인튜닝/양자화/export/온디바이스 패키징)는 UI 메뉴와 인터페이스 자리만 만들고 **disabled** 상태로 둔다.
- Anthropic-compatible shim, embeddings, rerank은 **스키마만** 먼저 잡고 구현은 후속.
- 사운드 도메인은 STT / TTS / realtime voice 세 슬롯을 registry/adapter에 만들어두고 v1은 STT 1개 + TTS 1개 정도만 실제 동작.

## 1.10 의도적으로 하지 않는 것 (non-goals)

- 자체 추론 엔진을 새로 만들지 않는다. 성숙한 OSS(llama.cpp 등)를 adapter로 얹는다.
- 브라우저-only 웹앱이 아니다. 데스크톱 프로그램이다.
- 기존 웹앱을 뜯어고치지 않는다. companion provider 추가만 한다.
- training을 v1 핵심으로 만들지 않는다.
- Ollama 단일 종속으로 만들지 않는다 — 그건 어댑터 중 하나일 뿐이다.
- 라이트 테마를 만들지 않는다.
