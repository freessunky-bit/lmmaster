# ADR-0021: Phase 1A 핵심 스택 결정 (plugin 선택, 다운로드, 하드웨어 probe, 마법사 UX)

- Status: Accepted
- Date: 2026-04-26

## Context
Phase 1A는 외부 런타임 detect/install + 한국어 첫실행 마법사를 구현한다. 4영역 보강 리서치(`docs/research/phase-1a-reinforcement.md`)로 다음을 확정했다:
- Tauri 2.10.x 플러그인 생태계 안정 버전 매트릭스
- 다운로드 stack에 backoff crate 사용 금지 (RUSTSEC-2025-0012)
- tauri-plugin-updater 재사용 금지 (Range/resume 미지원)
- 하드웨어 probe에 WMI VRAM 사용 금지 (4GB clamp 버그)
- ash Vulkan probe는 `loaded` feature만 (linked는 loader 부재 시 부팅 실패)
- Channel API가 emit보다 stream progress에 적합

## Decision

### 1. 플러그인 vs 자체 crate
- **`crates/runtime-detector`, `crates/installer`는 plain Rust crate** + `#[tauri::command]` invoke 등록.
  - 데스크톱 전용, 모바일 미지원, 플러그인 lifecycle hook 불필요.
  - 외부 재사용 의도 없음 — workspace member로 충분.
- **사용할 Tauri 플러그인**(crates.io 버전, 2026-04 기준):
  - `tauri-plugin-shell = "2.3"` — 외부 명령 spawn (lms CLI, nvidia-smi)
  - `tauri-plugin-http = "2.5"` — 프런트가 직접 외부 호출 시(거의 안 씀, Rust-side가 1차)
  - `tauri-plugin-dialog = "2.7"` — 파일 선택, 확인 dialog
  - `tauri-plugin-os = "2.3"` — OS arch/version detect 보조
  - `tauri-plugin-fs = "2.5"` — 워크스페이스 디렉터리 read/write
  - `tauri-plugin-store = "2.4"` — first-run flag, 마법사 응답 저장
  - `tauri-plugin-process = "2.3"` — relaunch
  - `tauri-plugin-log = "2.8"` — 프런트 로그 → Rust tracing
  - `tauri-plugin-autostart = "2.5"` — 자가스캔 백그라운드 실행 (v1.x)

### 2. capability JSON 정식 schema
- 필드명은 **`cmd`** (NOT `command`). 일부 v2 docs가 `command`로 잘못 표기되어 있으나 tauri-plugin-shell scope.rs의 `ScopeAllowedCommand` 정의가 정답.
- `args` 배열은 fixed string 또는 `{ "validator": "<regex>" }` 객체 혼재 가능.
- 와일드카드 exec 금지 — 명시적 cmd만.
- 외부 URL 열기는 `shell:allow-open` + `https://*` 제한.
- `OllamaSetup.exe` 같이 동적 경로의 installer는 capability scope로 보호 어려움 → installer crate 내부에서 `std::process::Command`로 직접 spawn (capability 외부). 라이브러리 책임으로 sha256 사전 검증 + 경로 sanitize.

### 3. 다운로드 stack (crates.io 버전)
```toml
reqwest = { version = "0.12", features = ["stream", "rustls-tls", "gzip", "json"] }
reqwest-middleware = "0.4"      # manifest fetch만
reqwest-retry = "0.7"           # manifest fetch만
tokio-util = { version = "0.7", features = ["io"] }
bytes = "1"
sha2 = "0.10"
backon = "1.6"                  # backoff(unmaintained, RUSTSEC-2025-0012) 대체
atomicwrites = "0.4"            # Win ReplaceFileW 보장
tempfile = "3"
```
- 큰 파일 다운로드는 reqwest + Range header 직접. retry middleware 사용 안 함 (Range/resume 모름).
- manifest JSON fetch는 reqwest-middleware + reqwest-retry. ETag/If-Modified-Since 자체 SQLite cache로.
- backon으로 다운로드 attempt 전체를 retry-with-jitter. 단일 chunk 실패는 reqwest stream의 cancel-safe 특성 활용.
- atomicwrites + 5회 retry로 AV-locked rename 우회.
- progress는 256KB 또는 100ms 누적 시 emit.

### 4. tauri-plugin-updater 재사용 금지
- 자체 Range/retry/sha256 미구현. in-memory buffer 방식 — 7GB GGUF 부적합.
- 본체 LMmaster 자체 업데이트에만 사용. 모델/외부 앱 다운로드는 자체 stack.

### 5. progress 전송: Channel API
- `tauri::ipc::Channel<T>` 사용. emit 대비 typed/ordered/per-invocation/auto-cleanup.
- 마법사 unmount 시 Channel ref drop 필수 (issue #13133).
- 256KB 또는 100ms 누적 후 send.

### 6. First-run 플래그 / 마법사 응답
- `tauri-plugin-store`로 `app_data_dir()/settings.json`에 저장.
- SQLite는 사용 로그/모델 카탈로그/사용자 데이터 전용 (ADR-0008).

### 7. 하드웨어 probe stack
```toml
sysinfo = "0.34"                # bump from 0.31; OS/CPU/RAM/disk
nvml-wrapper = "0.11"           # NVIDIA, libloading graceful fail
wgpu = "29"                     # vendor-agnostic GPU enum
ash = { version = "0.38", features = ["loaded"] }  # Vulkan, NOT linked
winreg = "0.55"                 # Windows registry
windows = { version = "0.58", features = ["Win32_Graphics_Direct3D12"] }
glibc_version = "0.1"           # Linux only
```
mac 전용:
```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.6"
objc2-metal = "0.3"
```
정책:
- 모든 probe는 `tokio::task::spawn_blocking` 또는 rayon으로 병렬 → cold < 1s, warm < 400ms.
- WMI VRAM 사용 금지 (4GB clamp).
- nvml init 실패는 `Result::Err` 처리 후 "no NVIDIA"로 보고 (panic 금지).
- ash는 `loaded` feature — `Entry::load()`가 loader 부재 시 `Err` 반환.
- 외부 CLI spawn은 최후 수단 (250~400ms 지연).

### 8. 마법사 React stack
- **상태 머신**: `xstate@5` + `@xstate/react`. 4 stage × idle/running/done/error 가드.
- **mutation**: `@tanstack/react-query@5`로 각 stage Tauri command 호출.
- **stepper**: `@ark-ui/react` Steps (linear mode, full ARIA).
- **에러 경계**: `react-error-boundary` per-stage.
- **layout**: dark canvas + 560px 중앙 카드 + ≥1024px 좌측 stepper rail 200px.
- **A11y**: `role="dialog" aria-modal=true`, polite + assertive live regions, Esc→cancel-confirm, ARIA-current step.
- **한국어 톤**: 해요체 일관, 닫기/취소 분리(닫기 in dialog, 취소 in async ops), Toss writing 가이드 준수.

### 9. Last Responsible Moment 적용
첫실행 마법사가 **묻는** 항목 = 단 2개:
- 언어 (UI를 의미 있게 렌더링하기 위함).
- 첫 모델 프리셋(small / medium / skip).

기본값으로 두는 항목 = 모든 것:
- 모델 저장 경로(default `<workspace>/models/`)
- GPU vs CPU 모드(자동 감지)
- telemetry(default off, 배너로 나중 안내)
- 단축키, 토크나이저 옵션 등.

### 10. Phase 1A sub-phase 분할
1A는 1A.1 / 1A.2 / 1A.3 / 1A.4 4개 sub-phase로 운영. 각 sub-phase 끝에 cargo test + dev 실행 검증 + RESUME 갱신. 자세한 작업 명세는 PIVOT.md / RESUME.md.

## Consequences
- 데스크톱 본체 디스크 + 컴파일 타임 증가 (wgpu/ash/objc2-metal). 측정 후 wgpu가 과한 경우 vendor-direct로 대체 검토.
- backon은 backoff 대비 API가 약간 다름 — 마이그레이션 비용 한 번.
- atomicwrites + tempfile 조합으로 Win AV 친화 다운로드 보장.
- WMI 미사용으로 Win에서 일부 GPU 정보(예: Intel iGPU 모델명)가 약할 수 있음 → wgpu adapter info로 보강.

## Alternatives considered
- **runtime-detector를 Tauri 플러그인으로**: 거부 — 모바일/외부 재사용 의도 없음. plain crate가 단순.
- **tauri-plugin-updater 재사용**: 거부 — 모델 다운로드에 부적합.
- **WMI VRAM**: 거부 — 4GB clamp 버그.
- **emit으로 progress**: 거부 — JSON 직렬화/eval 부하 + ordering 보장 없음.
- **Zustand only for wizard state**: 거부 — 4 stage × 4 sub-state 가드를 Zustand로 표현하면 복잡도 폭증. XState 채택.
- **Radix Tabs as stepper**: 거부 — wizard 의미론과 맞지 않음. ArkUI Steps 채택.

## References
- `docs/research/phase-1a-reinforcement.md`
- `docs/PIVOT.md`
- ADR-0017 (manifest+installer 패턴)
- ADR-0019 (always-latest hybrid bootstrap)
- ADR-0001 / 0002 / 0003 / 0004 (companion / Tauri 2 / Rust+Axum / adapter pattern)
- crates.io: tauri-plugin-{shell, http, dialog, os, fs, store, process, log, autostart}
- v2.tauri.app/develop/calling-frontend (Channel API)
- v2.tauri.app/security/{capabilities, scope, permissions}
- RUSTSEC-2025-0012 (backoff unmaintained)
- nvml-wrapper, wgpu, ash, winreg, atomicwrites
- ark-ui.com/docs/components/steps, xstate v5
