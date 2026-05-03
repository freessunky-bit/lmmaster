# Phase 13'.h.2.b/c — llama.cpp `llama-server` 자동 spawn + mmproj 결정 노트

> **상태**: 작성 중 (2026-05-03). 보강 리서치 결과는 §1에 종합 예정.
> **이전 페이즈**: Phase 13'.h.1 (Ollama vision IPC, 2026-04-30) + 13'.h.2.a (LM Studio chat + vision OpenAI compat 어댑터, 2026-05-01).
> **목적**: llama.cpp 런타임에서도 vision(이미지 입력) 모델을 사용 가능하게 만든다. 일반 사용자는 Ollama/LM Studio로 이미 작동하지만, *llama.cpp 직접 사용자(고급)*도 같은 카탈로그 + 같은 IPC로 vision 흐름을 쓸 수 있게 한다.
> **참조 ADR**: ADR-0050 (3단 사다리 + 비전 IPC) §"부분 채택" 잔여 분 + ADR-0043 (외부 binary 패턴, env override) + ADR-0042 (HF 다운로드 + sha256 + atomic rename).

---

## §0. 메타

- **선행 의존성**: ✅ adapter-ollama (`ChatMessage`/`ChatEvent`/`ChatOutcome` 공유 시그니처) + adapter-lmstudio (`convert_message_to_openai` content array 패턴) + knowledge-stack `embed_download.rs` (HF 다운로드 + 256KB throttle + atomic rename + cancel).
- **신규 crate**: `runner-llama-cpp` (process lifecycle 분리). adapter-llama-cpp 내부에 inline 안 함 — 사유는 §3 기각안 §A.
- **신규 ADR**: ADR-0051 — llama.cpp server spawn + mmproj 정책.
- **예상 작업량**: 8-10시간 (분할 가능 — §6 인계 참조).

---

## §1. 보강 리서치 종합

> Phase 시작 전 dispatch한 sub-agent (general-purpose, 2026-05-03)의 결과 통합. 8개 영역 + 권장 결정 10건.

### 1.1 `llama-server` 최신 사양 (b9010, 2026-05-02 기준)

- 거의 매일 1~2회 릴리스(b9010 = 4월 한 달간 b8607~b8779). LTS 없음 — LMmaster가 *임의 검증 태그*를 핀.
- Windows 매트릭스: x64/arm64 × CPU/CUDA 12/CUDA 13/Vulkan/SYCL/HIP. macOS arm64+Intel x64. Linux Ubuntu × CPU/Vulkan/ROCm/OpenVINO/SYCL.
- 핵심 CLI 플래그 (manpage + tools/server/README):
  - **vision**: `--mmproj FILE` / `--no-mmproj` / `--no-mmproj-offload` / `--mmproj-auto` (`-hf` 자동) / `--image-min-tokens` / `--image-max-tokens`
  - **server**: `--host` / `--port` / `--api-key` / `--api-prefix` / `-to` / `--threads-http` / `--reuse-port`
  - **성능**: `-ngl, --gpu-layers` / `-c, --ctx-size` / `-b/-ub` / `-dev` / `-mg` / `-sm` / `-t, --threads`
  - **chat-template**: `--chat-template JINJA` 또는 프리셋 (llava / qwen2-vision / gemma-3 GGUF 내장 자동 픽업)
  - **로깅**: `-v` / `-lv N` (0~4) / `--log-file` / `--log-disable`
- 거의 모든 인자가 `LLAMA_ARG_<UPPER_SNAKE>` env로 설정 가능. CLI가 env 우선.
- **헬스체크 패턴**: `GET /health` — 로딩 중 503 `{"error":{"code":503,"message":"Loading model"}}`, 준비 완료 200 `{"status":"ok"}`. `GET /props` (build_info `bXXXX-<commit>` + chat_template + total_slots) + `GET /v1/models` (모델 메타).
- **vision 입력은 OpenAI `image_url` content array 단일 표준**. 2025-05 LLaVA 별도 endpoint 통합 — 더 이상 분리 X. adapter-lmstudio 페이로드 빌더 로직 그대로 재사용 가능.
- 시작 시간: 4B vision/CPU 30~90초, GPU 5~20초. 8080 고정은 사용자 환경 충돌 위험 — `bind("127.0.0.1:0")` → `local_addr()` ephemeral port 권장.

### 1.2 mmproj 표준

- mmproj = multimodal projector. 비전 인코더(CLIP/SigLIP) → LLM 임베딩 사상. 별도 GGUF 분리 사유: ① 텍스트 양자화 vs projector 정밀도 분리(보통 F16/BF16/F32 유지) ② 같은 base에 여러 quant + 1~2개 projector ③ 텍스트 전용 사용자 부담 감소.
- **명명 규칙은 배포자별 상이** — 단일 정규식 단정 불가:

| 배포자 | 패턴 | 예 |
|---|---|---|
| ggml-org | `mmproj-model-{precision}.gguf` | `mmproj-model-f16.gguf` |
| bartowski | `mmproj-{model_name}-{precision}.gguf` | `mmproj-google_gemma-3-4b-it-bf16.gguf` |
| unsloth | `mmproj-{PRECISION}.gguf` | `mmproj-BF16.gguf` |
| lmstudio-community | `mmproj-model-f16.gguf` | (ggml-org와 동일) |

공통 가드: 파일명이 `mmproj`로 시작. **projector는 텍스트 모델 양자화와 무관하게 호환** (Q4_K_M 모델 + F16 projector OK). **F16이 표준 권장**, BF16은 GPU 호환성 이슈가 있을 때만, F32는 정밀도 한계 테스트 시.
- **페어링 검증은 llama.cpp 자체가 처리** (n_embd 비교) — stderr `error: mismatch between text model (n_embd = 2816) and mmproj (n_embd = 1536)` + `hint: you may be using wrong mmproj`. **사후 패턴으로 충분** (사전 검증 X).
- mmproj 미지정 시: vision 모델은 텍스트 전용으로만 작동. 이미지 포함 요청은 무시 또는 빌드별 에러.
- 출처: https://github.com/ggml-org/llama.cpp/blob/master/docs/multimodal.md / discussions/22190 / issues/21435.

### 1.3 Tauri 2 sidecar `bundle.externalBin`

- `tauri.conf.json::bundle.externalBin: ["binaries/llama-server"]` + 각 platform별 `-{TARGET_TRIPLE}` 접미사 파일 존재 강제 (Win msvc / macOS apple-darwin / Linux gnu).
- ACL: `shell:allow-spawn` + `shell:allow-kill` + args validator 화이트리스트 (regex로 path/port/숫자만).
- **macOS 노타리 회귀 #11992 미해결** — Tauri 2.1.1 + shell 2.2.0에서 externalBin 사용 시 노타리 실패(status 4000). v1 ship 일정에 영향 큼.
- **권장: bundle 비-사용** — ggml-org Releases 자동 다운로드 + env override가 ROI 우위. 사유: 앱 크기 절감 + GPU 변종 매트릭스 회피 + macOS 노타리 우회 + 보안 패치 즉시 받기 + LMmaster *외부 통신 화이트리스트* 정책의 자연 확장.
- 출처: https://v2.tauri.app/develop/sidecar/ / tauri/issues/11992 / plugins-workspace/issues/2418.

### 1.4 자식 프로세스 lifecycle (Rust + tokio)

- `tokio::process::Command::kill_on_drop(true)` — Unix SIGKILL / Windows TerminateProcess.
- **Windows 함정**: TerminateProcess는 *직접 자식만* — 손자 프로세스 고아. llama-server는 자체 자식 거의 안 만들지만 GPU helper 프로세스 시 위험. **Job Object + JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE** 권장.
- **부모 강제 종료 시 kill_on_drop 미발동** — Tauri `RunEvent::ExitRequested`/`Exit` 훅에서 명시적 cleanup 필요.
- 동적 포트: `TcpListener::bind("127.0.0.1:0")?` → `local_addr()?.port()` → drop → spawn에 전달. localhost-only 단일 사용자 race window 무시 OK.
- 헬스체크 backoff: 초기 100ms × 배수 2.0 + jitter 0.5 + 최대 2~5s 간격 + 최대 60~180s 경과(vision 4B/CPU 90s, 12B/CPU 180s). 503 → 계속, 200 → ready. `backoff` / `backon` crate.
- Windows 콘솔 창 숨김: `CommandExt::creation_flags(0x0800_0000)` (`CREATE_NO_WINDOW`).
- 출처: docs.rs/tokio Child / Command / rust-lang/rust#112423 / tokio-rs/tokio#2685.

### 1.5 stderr → 한국어 매핑 후보 (8 enum variant)

| stderr substring | enum variant | 한국어 메시지 |
|---|---|---|
| `error: mismatch between text model` / `wrong mmproj` | `MmprojMismatch` | "이 모델과 vision 파일이 안 맞아요. 카탈로그에서 모델을 다시 받아 볼래요?" |
| `failed to allocate` / `out of memory` / `CUDA out of memory` | `OutOfMemory` | "GPU 메모리가 부족해요. 더 작은 양자화 또는 컨텍스트 줄여 볼래요?" |
| `bind: address already in use` / `bind() failed` | `PortInUse` | "포트가 다른 프로그램과 겹쳐요. 잠시 뒤에 다시 시도할게요." |
| `failed to load model` / `error loading model` | `ModelLoadFailed` | "모델 파일을 못 읽었어요. 파일이 손상됐는지 확인할래요?" |
| `vk::Queue::submit: ErrorDeviceLost` | `GpuDeviceLost` | "그래픽 드라이버에 문제가 있어요. 드라이버를 업데이트하거나 CPU 모드로 바꿔 볼래요?" |
| connect refused (HTTP 단계) | `RuntimeUnreachable` | "런타임이 응답하지 않아요. 다시 시작해 볼래요?" |
| 프로세스 exit (모든 stderr 라인 + exit code 0이 아닌) | `Crashed` | "런타임이 갑자기 종료됐어요. 로그를 확인해 볼래요?" |
| `--mmproj` 미지정 + 이미지 요청 | `UnsupportedConfig` | "이미지 처리는 vision 파일이 필요해요. 카탈로그에서 받아 올래요?" |

### 1.6 보안 / 라이선스

- llama.cpp **MIT** — bundle 또는 다운로드 후 실행 모두 OK.
- **백엔드 변종**: CUDA DLL은 NVIDIA EULA, Vulkan은 LGPL 컴포넌트 일부 — ggml-org Releases에서 *유저가 받게* 하면 모두 우회.
- **mmproj 가중치 라이선스**: base 모델 라이선스 그대로 (gemma-3는 Gemma Terms, llava는 Llama 2 Community 700M MAU 제한, Qwen2-VL은 Apache 2.0 또는 Tongyi Qianwen).

### 1.7 알려진 회귀 / 함정 (사전 인지 필수)

1. **Gemma 4 mmproj CUDA SIGABRT** (#21402, b8639/b8650) — `clip_model_loader::load_tensors` abort. 워크어라운드: `--no-mmproj`(텍스트만).
2. **mmproj OOM with auto-fit** (#19980) — `--n-gpu-layers` auto가 mmproj 크기 미반영. 워크어라운드: `--fit-margin 2048` 또는 명시 `-ngl N`.
3. **Vulkan + mmproj heap corruption** (#22128, b8840 AMD) — 300~500 요청 후 SIGSEGV. 워크어라운드: `--cache-ram 0`.
4. **chat-template 차이** — gemma-3는 GGUF 내장(자동), llava는 `--chat-template llava` 명시 필요, Qwen2.5-VL은 `qwen2-vision`. 카탈로그 entry에 `chat_template_hint`로 자동 주입 권장.
5. **Windows 콘솔 창 노출** — `CREATE_NO_WINDOW` 누락 시 검은 창 깜빡.
6. **모델 > 물리 RAM** (#18563, b6000+) — CRASH 0xC0000005. 다운로드 직전 deterministic 검증.
7. **macOS 노타리 #11992** — externalBin 사용 시 fail. v1 ship에 macOS sidecar bundle 사용 X.

### 1.8 권장 결정 vs §2 채택안 매핑

보강 리서치 권장 결정 10건 vs 본 결정 노트 §2 채택안 5건:

| 보강 리서치 권장 | §2 채택안 | 결과 |
|---|---|---|
| #1 runner crate 분리 | A1 ✅ | 일치 |
| #2 ggml-org Releases 다운로드 + env override 양립 | A2 (env override만) | **부분 일치** — 자동 다운로드는 §4 v1.x 후속 (Phase 13'.h.4) |
| #3 vision payload OpenAI image_url | A4 ✅ | 일치 |
| #4 MmprojSpec + Option 백워드 호환 + F16 표준 | A3 ✅ | 일치 (precision/source 필드 추가 보강) |
| #5 port 0 + exponential backoff | A5 ✅ | 일치 |
| #6 Windows CREATE_NO_WINDOW + Job Object + ExitRequested 훅 | A5 (CREATE_NO_WINDOW) | **부분 일치** — Job Object/ExitRequested 훅은 §4 v1.x 후속 |
| #7 stderr enum 7~8종 + 해요체 | §1.5 그대로 채택 | 일치 |
| #8 known_issues 카탈로그 마커 | (신규) | §4 v1.x 후속에 추가 |
| #9 카탈로그 검증 mmproj 필수 (vision_support=true) | A3 (validator 보강) | 일치 |
| #10 chat_template_hint 자동 주입 | (신규) | §4 v1.x 후속(Phase 13'.h.3)에 이미 deferred |

**미해결 단정**: ① mmproj 미지정 시 정확한 vision 요청 에러 메시지 ② 빌드별 정확한 stderr substring ③ `--log-disable` 최신 빌드 존재 여부 — Phase 13'.h.2.b 첫 검증 라운드에서 실 stderr 캡처로 확정.

---

---

## §2. 채택안 (5건)

### A1. **`runner-llama-cpp` 신규 crate 분리** — adapter는 HTTP client만, runner는 process lifecycle.

이유:
- adapter-ollama / adapter-lmstudio는 **외부 데몬에 attach**(사용자가 별도 실행) — process lifecycle 책임 0.
- llama.cpp는 **자체 spawn 필요** — 그 책임이 어댑터에 섞이면 SRP 위반 + 어댑터 단위 테스트가 process 의존성 발생.
- `runtime-manager::supervisor` 모듈이 이미 placeholder로 존재 (`crates/runtime-manager/src/lib.rs:88-89` — "Phase 5'+ llama.cpp 자식 프로세스 모드"). 본 sub-phase가 그 자리를 채움.
- 향후 `runner-koboldcpp` / `runner-vllm`을 같은 패턴으로 추가 가능 (현재 두 어댑터도 `unimplemented!("M2")` 스켈레톤).

신규 워크스페이스 멤버:
```
crates/runner-llama-cpp/
├── src/
│   ├── lib.rs          — LlamaServerHandle (process supervisor)
│   ├── spawn.rs        — Command + kill_on_drop + Windows CREATE_NO_WINDOW
│   ├── port.rs         — TcpListener bind 0 → OS 할당 추출
│   ├── health.rs       — /health polling backoff
│   └── stderr_map.rs   — line → 한국어 RuntimeError 매핑
└── Cargo.toml
```

### A2. **`llama-server` binary 발견 = env override + 안내** (옵션 B 채택)

3가지 옵션 비교:
- 옵션 A (Tauri sidecar `bundle.externalBin`): Win/Mac/Linux × x86_64/aarch64 × CUDA/Vulkan/Metal/ROCm/CPU = **20+ binary 매트릭스**. 각 100~500MB. 빌드 + 배포 비용 폭증.
- **옵션 B (env override + 안내)**: 사용자가 직접 build 또는 download → `LMMASTER_LLAMA_SERVER_PATH` 환경변수로 path 지정. 미설정 시 한국어 안내 + Settings에 link.
- 옵션 C (첫 실행 자동 다운로드 + GPU detect): Phase 1A.1 Ollama 패턴. 매력적이지만 대상 사용자(고급)는 직접 관리 선호 + GPU detect 룰 + cuBLAS/Vulkan 분기 + 새 ADR 필요.

채택 이유:
- llama.cpp 직접 사용자 = **고급 사용자**. binary 직접 관리에 익숙.
- Phase 9'.b LlamaQuantizer가 이미 `LMMASTER_LLAMA_QUANTIZE_PATH` env override 패턴 정착 (ADR-0043). 같은 패턴 재사용 = 사용자 학습 비용 0.
- 옵션 C는 v2 마이그레이션 — DEFERRED.md에 등록.

### A3. **mmproj 페어링 = ModelEntry 스키마 확장** (백워드 호환)

`crates/model-registry/src/manifest.rs::ModelEntry`에 신규 필드 추가:
```rust
/// 비전 모델의 mmproj projector 파일 정보 (Phase 13'.h.2.c, ADR-0051).
///
/// llama.cpp는 vision 모델이 GGUF 본체 + mmproj-*.gguf 두 파일로 구성됨.
/// `vision_support: true` + 본 필드 누락 시 → llama.cpp 어댑터에서 한국어 안내 노출.
/// Ollama / LM Studio는 내부 처리하므로 본 필드 무시.
#[serde(default)]
pub mmproj: Option<MmprojSpec>,

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MmprojSpec {
    pub url: String,
    /// 32-byte hex sha256. None이면 사용자 경고 노출(ADR-0042 정책).
    #[serde(default)]
    pub sha256: Option<String>,
    pub size_mb: u64,
    /// "f16" / "bf16" / "f32" — F16이 표준. UI hint chip에 표시.
    #[serde(default)]
    pub precision: Option<String>,
    /// "bartowski" / "ggml-org" / "unsloth" / "lmstudio-community" — 출처 큐레이션 기록.
    #[serde(default)]
    pub source: Option<String>,
}
```

`#[serde(default)]`로 기존 39 entries 영향 0. gemma-3-4b 1건만 본 sub-phase에서 백필 — F16 권장 (보강 리서치 §1.2 표준 권장).

**페어링 검증은 사전 X — llama.cpp 자체가 사후 처리**(stderr `mismatch between text model and mmproj`). LMmaster는 stderr 패턴 매칭으로 한국어 매핑(`§1.5 MmprojMismatch`).

### A4. **Vision payload = OpenAI compat content array** (LM Studio 패턴 재사용)

llama.cpp `llama-server`는 OpenAI compat `/v1/chat/completions` 지원 (`tools/server`). 본체에서 LLaVA-style endpoint는 deprecated path.

따라서:
- `adapter-llama-cpp::chat_stream(model_id, messages, on_event, cancel)` 신설 — `LmStudioAdapter::chat_stream`과 거의 동일 시그니처.
- `convert_message_to_openai` 헬퍼는 **adapter-lmstudio에서 노출**하거나 *별개로 구현*. 본 sub-phase에선 별개 구현 + adapter-lmstudio의 OpenAI compat DTO struct들은 타입 충돌 회피 위해 별개 모듈에서 재선언 (이름은 같지만 namespace 분리).
- 이미지 base64는 `data:image/jpeg;base64,...` data URL로 인라인.

거부: native LLaVA endpoint — llama.cpp 본체에서 deprecated. 더 이상 권장 안 함.

### A6. **수동 셋업 단계는 in-app 친절 가이드로 안내** (사용자 명시 결정 2026-05-03)

env override 채택(§A2)의 비용 = "사용자가 직접 binary 받고 환경변수 설정". 이 친화도 격차를 in-app **단계별 wizard**로 메움 — Phase 12' guide system(ADR-0040) 인프라 차용.

신규 sub-phase **13'.h.2.e** — `LlamaCppSetupWizard` 단계 stepper:
1. **GPU 자동 감지** — 기존 `hardware-probe` crate 결과 그대로 표시 (NVIDIA / AMD / Intel / Apple Silicon / CPU-only).
2. **권장 빌드 안내** — 감지된 GPU에 맞는 ggml-org Releases asset 추천(NVIDIA → CUDA 13, AMD → Vulkan, Mac → arm64 또는 KleidiAI, 기타 → CPU). 한국어 해요체 카드.
3. **다운로드 페이지 열기** — `tauri-plugin-shell::open` 화이트리스트(github.com 추가) 사용해 시스템 브라우저로 ggml-org Releases 페이지 직링.
4. **압축 풀기 + 경로 안내** — Win/Mac/Linux 별 권장 위치 + clipboard 복사 버튼.
5. **환경변수 설정 가이드** — OS별 명령어 카드 (Win: `setx LMMASTER_LLAMA_SERVER_PATH "..."`, Mac/Linux: `export ...` + `~/.zshrc` append). clipboard 복사.
6. **자동 검증** — `LMMASTER_LLAMA_SERVER_PATH` 잡혔는지 확인 + `--version` spawn 1회 + 한국어 결과 표시(✅ 잡혔어요 / ❌ 아직 안 보여요).
7. **mmproj 받기** — 별개 단계. 카탈로그에서 `vision_support: true` 모델 선택 시 "이미지 분석에 필요한 vision 파일이에요" 한국어 안내 + sha256 검증.

a11y: `<dialog role="dialog" aria-modal>` + `Esc` + 첫 input auto-focus + 진행 percentage. i18n ko/en 동시.

진입점: ① Settings → "고급 런타임" 카드 ② 처음 vision 모델 채팅 시도 시 LlamaCpp 미감지면 자동 modal ③ Diagnostics → "외부 런타임" 카드.

### A7. **셋업 가이드 자동 갱신 시스템** (사용자 명시 결정 2026-05-03)

llama.cpp는 거의 매일 빌드, mmproj 모델별 다르고, GPU별 권장 빌드도 변동. 가이드가 stale되면 사용자가 잘못된 안내로 시간 낭비 → **registry-fetcher 인프라(ADR-0026 + Phase 13'.a 패턴) 그대로 차용**해 가이드 자체를 카탈로그처럼 갱신.

신규 sub-phase **13'.h.2.f** — 가이드 manifest 자동 갱신:
- `manifests/guides/llama-cpp-setup.json` 단일 bundle — 마크다운 본문 + 권장 빌드 버전(예: `b9010+`) + GPU별 asset URL 룰 + `known_issues: Vec<KnownIssue>` 마커.
- registry-fetcher `manifest_ids`에 `"llama-cpp-setup"` 추가 — 6시간 polling (기존 catalog와 동일 cron + 화이트리스트 jsDelivr 1순위/GitHub raw 2순위).
- minisign 서명 검증 (ADR-0047 catalog 패턴 그대로 — `from_embedded` pubkey 재사용 또는 별개 키).
- 새 버전 도착 시 Diagnostics 카드 + 다음 wizard 진입 시 자동 적용. 사용자 동의 dialog 없음(읽기 전용 안내).
- 큐레이터 운영 — 새 llama.cpp 빌드 회귀 발견 시(예: §1.7 #1 Gemma4 CUDA SIGABRT) 마커 + 워크어라운드 추가 → CI가 자동 서명 → 6시간 내 모든 사용자에 도착.
- frontend `LlamaCppSetupWizard`는 manifest의 빌드 버전/asset URL을 동적 표시 — 마크다운 본문은 기존 `_render-markdown.ts` 차용.

### A5. **자식 프로세스 lifecycle 정책**

```rust
tokio::process::Command::new(server_path)
    .kill_on_drop(true)
    .stdout(Stdio::null())   // bench / chat에서 쓰지 않음, /v1/* HTTP만 사용
    .stderr(Stdio::piped())  // 한국어 매핑용 라인 capture
    // Windows: CREATE_NO_WINDOW (0x08000000) 플래그 — 콘솔 창 숨김
    .args(["--model", &model_path, "--mmproj", &mmproj_path, "--port", &port.to_string(),
           "--host", "127.0.0.1", "--log-disable"])
    .spawn()?
```

- 포트 자동 할당: `TcpListener::bind("127.0.0.1:0")` → 즉시 drop 후 동일 포트로 spawn (race window 작음, 단일 사용자 데스크톱 OK).
- 헬스체크: `/health` 200ms × 60초 backoff polling. 첫 모델 로드는 GGUF 크기에 따라 10~50초.
- stderr 라인별 capture + `port already in use` / `out of memory` / `model file not found` → 한국어 `LlamaServerError` enum.
- graceful shutdown: drop 시 SIGKILL (Unix) / TerminateProcess (Windows). 정상 stop은 `--api-key` 등이 있을 시 POST `/shutdown` (선택사항, Phase 13'.h.2.b 본 sub-phase는 drop 의존).

---

## §3. 기각안 + 이유 (negative space)

### A. **adapter-llama-cpp 내부 spawn 모듈 inline** (vs A1 신규 crate 분리)
거부 이유: SRP 위반. adapter는 HTTP client wrapping이 책임 — process lifecycle은 별개. supervisor 모듈이 runtime-manager에 이미 placeholder 존재. 또한 adapter 단위 테스트가 wiremock + process spawn 두 의존성 동시 짊어지면 flaky 위험.

### B. **Tauri sidecar `bundle.externalBin`** (vs A2 env override)
거부 이유: 빌드 매트릭스 20+ binary, 각 100~500MB. macOS notarization + Windows code signing이 Tauri 단일 빌드와 별개. 사용자 PC에 GPU별 다른 binary 필요. 일반 사용자는 Ollama/LM Studio로 이미 vision 작동 — ROI 매우 낮음.

### C. **자동 다운로드 + GPU detect** (옵션 C)
거부 이유: Ollama 같은 단일 vendor가 아니라 ggml-org/llama.cpp는 release asset이 백엔드별 매트릭스. cuBLAS / Vulkan / Metal / ROCm / CPU 분기 룰 + 사용자 GPU detect 정확도 + 빌드별 호환성 검증 비용 → 별개 ADR 필요. v2 마이그레이션.

### D. **mmproj sha256 옵션 X — 항상 강제** (vs A3 Option<>)
거부 이유: HF 모델 중 mmproj sha256을 manifest에 명시하지 않은 경우 다수. ADR-0042가 이미 "sha256 None이면 검증 skip + 사용자 경고" 정책 정착. 본 sub-phase도 일관 유지.

### E. **별개 IPC `chat_with_image(model_id, prompt, image_base64)`** (ADR-0050 §"결정" 5번)
ADR-0050 §"결정" 5번은 신규 IPC를 제시했지만, 실제 13'.h.1 (Ollama)과 13'.h.2.a (LM Studio) 머지 시점에 *기존 `start_chat`이 `messages[].images` 자동 처리*하는 방향으로 합쳐졌음. 본 13'.h.2.b/c도 동일 — 신규 IPC 0개. ACL 변경 0건.

### F. **mmproj 자동 다운로드 본 sub-phase 포함** (vs A3 v2 deferred)
거부 이유: model.gguf + mmproj 페어링 다운로드는 별도 IPC + 진행률 + cancel + sha256 + retry — 자체로 큰 sub-phase. 본 sub-phase는 *경로를 받으면 작동*하게 만들고, 다운로드 자동화는 v2. 사용자는 HF에서 mmproj 직접 받아 `LMMASTER_LLAMA_MMPROJ_PATH` 또는 카탈로그 manifest의 `mmproj.url` 보고 수동 download.

### G. **stderr → tracing log only** (vs A5 한국어 RuntimeError 매핑)
거부 이유: bench/chat에서 stderr를 사용자에게 노출 안 하면 "측정 호출이 모두 실패했어요" 같은 generic error로 회귀(2026-04-30 install/bench bugfix와 동일 trap). 한국어 매핑 = 사용자 신뢰도.

### H. **단일 instance pool** (다중 model 동시 spawn 거부)
거부 이유: llama-server는 자체적으로 한 모델만 로드. 다중 모델은 다중 server instance + 다중 포트. 본 sub-phase는 "현재 채팅 모델 하나" 단일 instance만. 다중 instance는 v2 (Workbench 다중 모델 비교 시).

### I. **사용자 자율 셋업 — 가이드 X** (vs §A6 친절 wizard)
거부 이유: env override(§A2) 채택의 비용 = "사용자가 알아서 받고 설정". 가이드 없이 README 링크만 노출하면 일반 사용자가 "어떤 빌드? 어디서? 어떻게 설정?" 막힘. *고급 사용자라도* OS별 환경변수 설정은 trial-and-error. Phase 12' guide system 인프라 이미 있으니 wizard 추가 비용 작음. 사용자 명시 결정.

### J. **가이드 정적 — 갱신 X** (vs §A7 자동 갱신)
거부 이유: llama.cpp 빌드는 거의 매일 변하고(보강 리서치 §1.1 b9010), 회귀 이슈도 빌드별로 다름(§1.7 #1~#3). 정적 가이드는 1주만 지나도 stale → 사용자가 잘못된 권장 빌드 받아 시간 낭비. registry-fetcher 인프라(ADR-0026 + 13'.a) 이미 있으니 가이드 manifest 추가 비용 작음. 사용자 명시 결정.

---

## §4. 미정 / 후순위 이월 (v1.x ~ v2)

| 항목 | 사유 | 진입 후보 |
|---|---|---|
| mmproj 자동 다운로드 + 페어링 다운로드 IPC | §3.F. 큰 sub-phase | v1.x (Phase 13'.h.2.c.2) |
| llama-server binary 자동 다운로드 + GPU detect (보강 리서치 #2) | §3.C. 별개 ADR + 빌드 매트릭스 룰 + ggml-org Releases 화이트리스트 추가 | v1.x (Phase 13'.h.4) |
| 다중 instance pool (다중 모델 동시 채팅) | §3.H. 단일 사용자 데스크톱 + Workbench 비교 시 필요 | v2 |
| `--api-key` + POST `/shutdown` graceful stop | drop 기반 SIGKILL로 일반 OK. graceful은 server v0.5+ 보장 안 됨 | v1.x |
| Tauri sidecar 패키징 | §3.B. ROI 낮음. macOS 노타리 #11992 미해결 | v2+ |
| `chat_template_hint: Option<String>` 카탈로그 필드 (보강 리서치 #10) | gemma-3는 GGUF 내장 자동, llava류는 `--chat-template llava` 명시 필요 | v1.x (Phase 13'.h.3) |
| `known_issues: Vec<String>` 카탈로그 마커 (보강 리서치 #8) | gemma4_cuda_mmproj_abort / vulkan_amd_mmproj_heap 등. 사용자 GPU+모델+빌드 조합이 마커에 걸리면 사전 경고 | v1.x (Phase 13'.h.5) |
| Windows Job Object + JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE (보강 리서치 #6) | TerminateProcess는 직접 자식만 — 손자 프로세스 고아. llama-server 자체 자식 거의 없지만 GPU helper 시 위험 | v1.x |
| Tauri `RunEvent::ExitRequested` 훅 명시 cleanup (보강 리서치 #6) | 부모 강제 종료 시 kill_on_drop 미발동 | v1.x |
| 다운로드 직전 RAM/디스크 deterministic 검증 (보강 리서치 §1.7 #6) | `model_size + mmproj_size + 컨텍스트 추정` > 사용자 RAM 시 사전 경고 | v1.x |
| OOM / port collision graceful retry | §A5 stderr 매핑 + 사용자 안내 후 v1.x | v1.x |

---

## §5. 테스트 invariant

본 sub-phase가 깨면 안 되는 invariant:

### runner-llama-cpp
- [ ] **port allocation**: `TcpListener::bind("127.0.0.1:0")`로 받은 포트가 즉시 spawn에 재사용되어 충돌 안 남 (10회 round-trip).
- [ ] **kill_on_drop**: handle drop 시 자식 프로세스 종료 (`Child::wait` non-blocking 확인).
- [ ] **health backoff**: 60초 timeout 내 `/health` 200 응답 없으면 `LlamaServerError::HealthcheckTimeout` (한국어).
- [ ] **stderr 매핑**: `port already in use` 라인 → `LlamaServerError::PortInUse` (한국어 메시지).
- [ ] **stderr 매핑**: `out of memory` → `LlamaServerError::OutOfMemory`.
- [ ] **stderr 매핑**: `model file not found` → `LlamaServerError::ModelNotFound`.
- [ ] **graceful drop**: drop 후 `try_wait()` 완료 (좀비 프로세스 0).

### adapter-llama-cpp
- [ ] **chat_stream OpenAI compat round-trip**: wiremock으로 SSE 응답 mock → ChatEvent::Delta 누적 + Completed.
- [ ] **chat_stream `[DONE]` 마커** 감지 → ChatEvent::Completed.
- [ ] **chat_stream cancel** → 스트림 drop + ChatOutcome::Cancelled.
- [ ] **vision content array 변환**: `images` Vec 비었으면 plain content, 비어있지 않으면 `[{type: "text"}, {type: "image_url", image_url: {url: data:image/jpeg;base64,...}}]`.
- [ ] **모델 not loaded**: HTTP 404 + `not found` 텍스트 → `ChatEvent::Failed { message: "이 모델이 llama-server에 로드돼 있지 않아요" }`.
- [ ] **detect()**: server 미실행 시 `installed: false` + 한국어 hint.
- [ ] **list_models()**: `/v1/models` 응답 파싱.

### model-registry
- [ ] **MmprojSpec 백워드 호환**: 기존 39 entries (mmproj 필드 없음) deserialize 통과.
- [ ] **MmprojSpec round-trip**: gemma-3-4b 백필 후 entry serde JSON round-trip.
- [ ] **build-catalog-bundle.mjs**: validator가 `mmproj.url`이 https://huggingface.co 또는 https://github.com 도메인인지 확인 (외부 통신 화이트리스트).

### chat IPC
- [ ] **start_chat LlamaCpp 분기**: 기존 Ollama / LmStudio 분기 0건 회귀. Unsupported error 자리에 LlamaCppAdapter 호출 wire.
- [ ] **ACL drift 0**: capabilities/main.json 변경 없음 (신규 IPC 0건).

### vitest (frontend, 본 sub-phase는 0 필수)
- [ ] **Chat.tsx**: `vision_support: true` + RuntimeKind.LlamaCpp 모델 선택 시 paperclip 활성 (기존 Ollama/LmStudio와 동일 경로). 회귀 0건.

---

## §6. 다음 페이즈 인계

### 본 sub-phase가 ship한 직후 가능한 작업

**Phase 13'.h.2.c.2 — mmproj 자동 다운로드 IPC** (3-4h, v1.x):
- `start_mmproj_pull(model_id) → cancel_mmproj_pull` 신규 IPC.
- knowledge-stack `embed_download::download_with_progress` 차용 (256KB throttle + atomic rename + sha256).
- ModelDetailDrawer에서 vision 모델은 model.gguf + mmproj 둘 다 받기 표시.
- ACL `allow-start-mmproj-pull` / `allow-cancel-mmproj-pull` 2건 추가.

**Phase 13'.h.3 — chat_template 모델별 분기** (2-3h, v1.x):
- ModelEntry에 `chat_template: Option<String>` 추가 (gemma3 / llava / chatml / mistral 등).
- llama-server `--chat-template` 인자로 전달.
- gemma-3-4b는 "gemma3" 백필.

### v2 마이그레이션 후보

- Phase 13'.h.4 — llama-server 자동 다운로드 + GPU detect (옵션 C, §3.C).
- Phase 13'.h.5 — 다중 instance pool (다중 모델 동시 채팅, §3.H).
- Tauri sidecar 패키징 검토 (§3.B).

### 위험 노트 (다음 세션 trap)

1. **wiremock SSE 응답** — `adapter-lmstudio::tests::run_prompt_*`처럼 `text/event-stream` content-type + `data: {...}\n\n` 라인 단위 응답 빌더 헬퍼 차용.
2. **Windows CREATE_NO_WINDOW** — `std::os::windows::process::CommandExt::creation_flags(0x08000000)`. cfg(windows) gate.
3. **포트 race** — `TcpListener::bind("127.0.0.1:0")` 후 listener를 drop하기 전에 port를 추출(`local_addr()`). drop과 spawn 사이 race window는 단일 사용자 데스크톱에선 무시 OK.
4. **adapter-llama-cpp Cargo.toml** — `tokio-util` (CancellationToken) + `futures-util` (StreamExt) + 신규 `runner-llama-cpp` workspace dep 추가 필요.
5. **PathBuf serialization** — manifest의 mmproj path는 사용자 PC 경로 (env 또는 manifest URL → 다운로드 경로). serde에는 url + sha256만, 다운로드 경로는 runtime 결정.

### Phase 분할 권장 (전체 약 17-22시간, 분할 진행)

- ✅ **13'.h.2.b** (머지 2026-05-03): runner-llama-cpp crate 신설 + spawn/health/stderr_map/port + 단위 invariant 22 + adapter-llama-cpp chat_stream + 10 invariant.
- ✅ **13'.h.2.c.1** (머지 2026-05-03): ModelEntry MmprojSpec 스키마 확장 + gemma-3-4b 백필 + build-catalog validator + 4 invariant.
- ⏳ **13'.h.2.d** (3-4h, 다음 진입 후보): chat IPC LlamaCpp 분기 wiring + Tauri State `Arc<Mutex<Option<LlamaServerHandle>>>` 단일 instance + ExitRequested 훅 cleanup + 30~90초 모델 로드 진행률 emit.
- ⏳ **13'.h.2.e** (4-6h): `LlamaCppSetupWizard` — 7단계 stepper(§A6) + Settings/Diagnostics/첫 시도 진입점 3종 + i18n ko/en + Phase 12' guide system 차용.
- ⏳ **13'.h.2.f** (2-3h): 가이드 자동 갱신 — `manifests/guides/llama-cpp-setup.json` + registry-fetcher `manifest_ids` 등록 + minisign 검증(ADR-0047 차용) + 큐레이터 SOP 문서.
- ⏳ **13'.h.2.c.2** (3-4h, v1.x): mmproj 자동 다운로드 IPC.
- ⏳ **13'.h.3** (2-3h, v1.x): chat_template_hint 카탈로그 필드.
- ⏳ **13'.h.4** (6-8h, v1.x): binary 자동 다운로드 + GPU detect (가이드 시스템 채택 후 ROI 재평가 — 가이드만으로 충분하면 v2로 deferred).
- ⏳ **13'.h.5** (2-3h, v1.x): known_issues 마커 — 가이드 manifest의 `known_issues` 필드(§A7)와 일관 통합.
- ⏳ **13'.h.6** (1-2h, v1.x): Windows Job Object + ExitRequested 훅 cleanup.

---

**문서 버전**: v1 (2026-05-03 — 초안). §1 보강 리서치 종합은 sub-agent 결과 도착 후 추가.
