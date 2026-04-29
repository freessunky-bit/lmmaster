# Phase 1A 보강 리서치 — 종합

> 2026-04-26. 4영역 병렬 리서치(Tauri 2.10.x 플러그인 / resumable HTTP / GPU detect / wizard UX) 결과 종합.
> Phase 1' 일반 보강(`docs/research/phase-1-reinforcement.md`)에서 정한 큰 방향을 **구현 결정 수준**으로 좁힌다.

## 0. 적용 결정 표

| 영역 | 결정 | 비고 |
|---|---|---|
| 플러그인 vs 자체 crate | runtime-detector / installer는 **plain Rust crate + `#[tauri::command]`** | 데스크톱 전용, 모바일 미고려, plugin overhead 불필요. Pot/Spacedrive/Lapce 패턴. |
| Tauri 플러그인 버전 (2026-04 기준) | shell 2.3.5, http 2.5.8, dialog 2.7.0, os 2.3.2, fs 2.5.0, store 2.4.2, process 2.3.1, log 2.8.0, autostart 2.5.1 | 모두 Tauri 2.10.3 호환. 정확한 minor pin. |
| capability JSON 필드명 | **`cmd`** (not `command` — Phase 1' 리서치 텍스트에 잠시 혼선 있었음. 2.x 정식은 `cmd`) | tauri-plugin-shell scope.rs source 확정. v2 docs 일부 문서가 `command`로 잘못 표기. |
| 다운로드 progress 전송 | **Tauri 2 Channel API** (`tauri::ipc::Channel<T>`) | 1KB/30fps에서 emit JSON 직렬화/eval 부하 회피. ordered, typed, per-invocation. |
| First-run 완료 플래그 저장 | **tauri-plugin-store** (`settings.json` in app_data_dir) | SQLite는 1 boolean에 과함. |
| 앱 데이터 vs 워크스페이스 manifest | tauri-plugin-store는 **앱 설정**(언어 선택, 마법사 완료 여부). 워크스페이스 manifest는 **포터블 데이터**(설치된 모델·런타임). | ADR-0009와 일관. |
| 다운로드 stack | reqwest 0.12 + tokio-util 0.7 + bytes + sha2 0.10 + **backon 1.6** + atomicwrites 0.4 + tempfile 3 | backoff crate는 unmaintained (RUSTSEC-2025-0012). |
| reqwest 미들웨어 | **manifest 페치만** reqwest-middleware 0.4 + reqwest-retry 0.7. 큰 파일 다운로드는 직접 backon으로. | retry 미들웨어는 Range/resume 모름. |
| Streaming sha256 | hasher state가 단일 진실원 — 디스크 재읽기 금지. resume 시 .partial 바이트 먼저 hashing → range 요청. | |
| Atomic 파일 쓰기 (Win) | **atomicwrites 0.4** (ReplaceFileW 사용, ACL 보존) + AV-locked 대비 backon 5회 retry | tempfile::persist는 같은 볼륨에서만 atomic. |
| Tauri-plugin-updater 재사용? | **NO** — 자체 Range/retry/sha256 없음. 30MB 패치엔 OK, GGUF 7GB엔 부적합. | |
| Progress 전송 cadence | **256KB 또는 100ms 중 먼저 도달 시** (둘 다 만족 시까지 누적). hashing은 매 청크. | indicatif 50ms vs cargo/rustup 80~250ms 평균. |
| ETag/If-Modified-Since | **자체 SQLite cache** (`manifest_cache` 테이블) | http-cache-reqwest는 RFC 7234 fully하느라 무거움. |
| 하드웨어 probe stack | **sysinfo + nvml-wrapper + wgpu + ash(loaded) + objc2-metal + winreg** | wgpu가 vendor-agnostic primary enumerator (DX12/Metal/Vulkan/GL). |
| sysinfo 버전 | 0.31 → **0.34** (2025-04 안정). VRAM 미커버 — wgpu/nvml 보강 필요. | 워크스페이스 deps bump. |
| Vulkan probe | ash 0.38 + `loaded` feature (NOT `linked`) | linked는 loader 부재 시 부팅 실패. |
| WMI VRAM | **사용 금지** | `Win32_VideoController.AdapterRAM`이 4GB clamp 버그 — 신뢰 불가. |
| Apple Silicon | objc2-metal `MTLCreateSystemDefaultDevice` + `recommendedMaxWorkingSetSize` | system_profiler spawn은 250~400ms 지연 — IOKit/Metal 직접 사용. |
| HW probe 성능 예산 | **cold < 1s, warm 200~400ms** | 모든 probe를 `tokio::task::spawn_blocking`으로 병렬화. |
| React 마법사 state | **XState v5 + TanStack Query** | 4 stage × 4 sub-state(idle/running/done/error)는 useReducer로 표현 가능하지만 가드/cancel/inspector 측면에서 XState 유리. |
| Stepper 컴포넌트 | **@ark-ui/react Steps** (linear mode + WAI-ARIA 완비) | Radix Tabs는 wizard 의미론과 다름. ArkUI는 headless+토큰 친화. |
| 마법사 layout | dark canvas + 560px 중앙 카드 + ≥1024px에서 좌측 stepper rail 200px | Toss/Linear/Raycast 표준. |
| 한국어 톤 | 해요체 일관 (`닫기` not `취소` in dialogs) | Toss UX writing 가이드 준수. ADR-0010 보강. |
| Last Responsible Moment 적용 | 첫실행 마법사가 묻는 것 = **언어 + 첫 모델 프리셋** 단 2개 | telemetry/저장 경로/GPU 모드/단축키는 default + 설정에서 변경. |
| 첫 모델 큐 (선택) | EXAONE-3.5-2.4B Q4 (~1.5GB) — small. medium은 Qwen2.5-7B Q4 (~4GB) | 명시 동의 화면 + "건너뛰기". |

## 1. Tauri 2 플러그인 의사결정

### runtime-detector / installer는 plain crate
- 모바일 미지원, ACL은 `#[tauri::command]` 단위로 충분.
- workspace member crate (`crates/runtime-detector`, `crates/installer`)로 두고 invoke handler에 등록.
- 플러그인 패턴은 **재사용 가치가 클 때**(awesome-tauri 등재 가능 수준) 사용.

### tauri-plugin-shell capability ACL 정확 schema
필드는 `cmd` (NOT `command`). `args` 배열에 fixed string 또는 `{ "validator": "<regex>" }` 객체 혼재.
```json
{
  "permissions": [
    "core:default",
    {
      "identifier": "shell:allow-execute",
      "allow": [
        { "name": "ollama-installer", "cmd": "OllamaSetup.exe", "args": ["/SILENT"], "sidecar": false },
        { "name": "lms-server", "cmd": "lms", "args": ["server", { "validator": "^(start|stop|status)$" }], "sidecar": false }
      ]
    },
    { "identifier": "shell:allow-open", "allow": [{ "url": "https://*" }] }
  ]
}
```

⚠️ **OllamaSetup.exe는 동적 경로** (다운로드 위치)이지만 capability scope는 cmd 문자열 매칭 — `Command::new("OllamaSetup.exe").current_dir(<dir>)` 형태로 호출하거나, **scope 불필요한 `std::process::Command`를 installer crate 내부에서 사용**(권장). tauri-plugin-shell ACL은 lms 같은 PATH 기반 명령에만 적용.

### Channel API
```rust
use tauri::ipc::Channel;
#[tauri::command]
async fn install_app(id: String, on_progress: Channel<InstallEvent>) -> Result<(), String> {
    on_progress.send(InstallEvent::Started { total }).ok();
    while let Some(chunk) = stream.next().await {
        // ... write + hash ...
        on_progress.send(InstallEvent::Progress { downloaded }).ok();
    }
    Ok(())
}
```
프런트:
```ts
const ch = new Channel<InstallEvent>();
ch.onmessage = (m) => store.setProgress(m);
await invoke('install_app', { id: 'ollama', onProgress: ch });
```

⚠️ 메모리 누수 issue #13133 — 마법사 unmount 시 Channel reference drop 필수.

## 2. 다운로드 stack (구현 수준)

### Cargo.toml additions (workspace)
```toml
backon = "1.6"
atomicwrites = "0.4"
tempfile = "3"
reqwest-middleware = "0.4"
reqwest-retry = "0.7"
wiremock = "0.6"        # dev-only
```
sysinfo: 0.31 → 0.34 bump (warning이 일부 확장 메서드 deprecate. 무관 시 OK).

### Cancellation
```rust
loop {
    tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            file.flush().await.ok();
            // .partial 보존 — 다음 실행 시 resume.
            return Err(DownloadError::Cancelled);
        }
        next = stream.next() => match next {
            Some(Ok(chunk)) => { hasher.update(&chunk); file.write_all(&chunk).await?; }
            Some(Err(e))    => return Err(e.into()),
            None            => break,
        }
    }
}
```

### Atomic rename Win
```rust
use atomicwrites::{AtomicFile, AllowOverwrite};
backon::Retryable::retry(|| async {
    AtomicFile::new(&final_path, AllowOverwrite)
        .write(|f| std::io::copy(&mut partial_reader, f))
}, ExponentialBuilder::default().with_max_times(5))
```

## 3. 하드웨어 probe stack

### Crate 매트릭스
```toml
sysinfo = "0.34"               # OS / CPU / RAM / disk
nvml-wrapper = "0.11"          # NVIDIA (graceful fail)
wgpu = "29"                    # vendor-agnostic GPU enum
ash = { version = "0.38", features = ["loaded"] }   # Vulkan probe (non-linked)
winreg = "0.55"                # Windows registry
windows = { version = "0.58", features = ["Win32_Graphics_Direct3D12"] }  # Win DirectML
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.6"
objc2-metal = "0.3"
[target.'cfg(target_os = "linux")'.dependencies]
glibc_version = "0.1"
```

### probe 함수 (병렬)
```rust
pub async fn probe_all() -> HardwareReport {
    let (os, cpu, mem, disk, gpus, runtimes) = tokio::join!(
        spawn_blocking(probe_os),
        spawn_blocking(probe_cpu),
        spawn_blocking(probe_mem),
        spawn_blocking(probe_disk),
        spawn_blocking(probe_gpus),
        spawn_blocking(probe_runtimes),
    );
    HardwareReport { os: os??, cpu: cpu??, mem: mem??, disk: disk??, gpus: gpus??, runtimes: runtimes??, probed_at: SystemTime::now(), probe_ms }
}
```
- 목표: cold 첫 실행 < 1s, warm < 400ms.
- WMI는 사용 금지 (4GB clamp + 300~800ms cold).

### unified report
```rust
pub struct HardwareReport {
    pub os: OsInfo, pub cpu: CpuInfo, pub mem: MemInfo, pub disk: Vec<DiskInfo>,
    pub gpus: Vec<GpuInfo>, pub runtimes: RuntimeInfo,
    pub probed_at: SystemTime, pub probe_ms: u32,
}
pub struct GpuInfo {
    pub vendor: GpuVendor, pub model: String,
    pub vram_bytes: Option<u64>,
    pub backends: BackendCaps,    // cuda/vulkan/metal/directml/rocm
    pub driver_version: Option<String>, pub pci_id: Option<(u16,u16)>,
}
pub struct RuntimeInfo {
    pub cuda_toolkit: Option<Version>, pub cuda_runtime: Option<Version>,
    pub vulkan: Option<Version>, pub metal: Option<MetalTier>,
    pub directml: bool, pub rocm: Option<Version>,
    pub webview2: Option<String>, pub vcredist_2022: Option<String>,
    pub glibc: Option<Version>,
}
```
recommender(Phase 2)는 vram_bytes + backends를, installer(Phase 1A.3)는 runtimes + os.arch를 소비.

### 함정
- nvml-wrapper의 `Nvml::init()`는 nvml.dll/libnvidia-ml.so 부재 시 `LibloadingError` — wrap in Result + report "no NVIDIA". OK.
- `Win32_VideoController.AdapterRAM`은 4GB clamp 버그 — VRAM 신뢰 불가. nvml/wgpu/IOKit 사용.
- ash linked feature는 loader 부재 시 부팅 실패 — `loaded`만 사용.

## 4. React 마법사

### 컴포넌트 stack
- `@ark-ui/react` Steps (linear) — stepper.
- `xstate` 5.x + `@xstate/react` — 마법사 state machine.
- `@tanstack/react-query` 5 — 각 stage의 invoke 호출 mutation 래핑.
- `react-error-boundary` — stage별 에러 경계.
- 헤드리스 + 디자인 토큰 사용 — base.css/components.css에 stepper 추가.

### 4 stage state machine 구조
```
machine: wizard
states:
  language: { entry: persist locale; on NEXT → env_check }
  env_check:
    invoke: detect_environment (Tauri command)
    states: { running, done, error_fixable, error_blocking }
    on RETRY: env_check.running
    on NEXT (after done): first_model
  first_model:
    states:
      consent: choose preset (small/medium/skip)
      installing:
        invoke: install_model with Channel<ProgressUpdate>
        states: { downloading, verifying, finished, error }
        on CANCEL: consent
        on RETRY: installing.downloading
      skipped
    on DONE: completed
  completed: final
```

### 한국어 카피 표
| 단계 | idle | running | done | error |
|---|---|---|---|---|
| 1. 언어 | "한국어로 시작할게요" / "다른 언어" | — | — | — |
| 2. 환경 점검 | "환경을 확인할게요" | "준비하고 있어요…" | "준비됐어요" | "GPU를 못 찾았어요. CPU로도 진행할 수 있어요." + [자동으로 고치기] |
| 3. 첫 모델 | "약 1.5 GB · 약 2분 · [다운로드] [건너뛰기]" | "모델을 받고 있어요" | "모델이 준비됐어요" | [다시 시도] / [다른 모델 고르기] |
| 4. 완료 | — | — | "다 됐어요!" [시작하기] | — |

### A11y 필수
- `role="dialog" aria-modal="true" aria-labelledby="wizard-title"`.
- Stepper: `<ol aria-label="설치 단계">` (Ark가 제공).
- Step item: `aria-current="step"` on active.
- Progress: `role="progressbar" aria-valuenow/min/max/text`.
- Live region 2개: polite (stage 전환), assertive (에러).
- Esc → cancel-confirm.

## 5. Phase 1A 분할 결정

이번 세션 컨텍스트 budget상 1A 전체를 한 번에 구현하면 품질 저하 위험. **4 sub-phase로 분할**:

- **1A.1 (이번 세션)**: 보강 리서치 + ADR-0021 + manifest 2건 + `crates/runtime-detector` HTTP probe + 통합 테스트.
- **1A.2**: `crates/runtime-detector` 확장 — 하드웨어 probe (sysinfo+nvml+wgpu+winreg) + 환경 prereq detect + cross-platform tests.
- **1A.3**: `crates/installer` — Pinokio-style manifest executor + Channel API progress + atomic .partial→final + tauri-plugin-shell capability JSON.
- **1A.4**: React 마법사 — 4 stage Steps + XState + 한국어 카피 + 디자인 토큰 적용 + e2e 검증.

각 sub-phase 끝에 cargo test + dev 실행 검증 + RESUME 갱신.

## 6. 신설 ADR

**ADR-0021** — Phase 1A 핵심 스택 결정 (이 보강 리포트의 §0 결정 표 + crate selection rationale 정리).

## 7. 출처 (검증)

- Tauri plugins: lib.rs/crates/tauri-plugin-{shell,http,dialog,os,fs,store,process,log,autostart}, v2.tauri.app/plugin/{shell,store,updater}, plugins-workspace GitHub.
- Tauri Channel API: v2.tauri.app/develop/calling-frontend, IPC issue #13133 (memory leak).
- Capability schema: tauri-plugin-shell scope.rs, v2.tauri.app/security/{capabilities,scope,permissions}.
- Download: reqwest 0.12 / tokio-util 0.7 / sha2 / backon 1.6 / atomicwrites 0.4 / RUSTSEC-2025-0012 (backoff unmaintained).
- HW probe: sysinfo crates.io / nvml-wrapper / wgpu releases / ash 0.38 / objc2-metal / winreg 0.55+ / wmi-rs (4GB clamp 알려진 이슈).
- React: ark-ui.com/docs/components/steps / xstate 5 / TanStack Query 5 / react-error-boundary / Toss UX writing 가이드.
- 한국어 톤: developers-apps-in-toss.toss.im/design/ux-writing.html
