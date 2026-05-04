# CHANGELOG — LMmaster v1 시간순 변경 이력

> 본 파일은 모든 sub-phase 완료 항목의 **시간순 상세 로그**. Claude는 자동 로드하지 않으며, 필요 시 `grep` / 특정 페이즈 검색용.
>
> - 1-page 대시보드: `docs/PROGRESS.md` (≤150줄, 자주 갱신)
> - 현재 인계: `docs/RESUME.md` (≤300줄, 매 sub-phase 갱신)
> - 페이즈 전략: `docs/PHASES.md`
> - 결정 이력: `docs/adr/README.md` + `docs/adr/`
> - 보강 리서치 / 결정 노트: `docs/research/`

## v0.0.1 ship 가능 — GPT Pro 30-issue 검수 종결 (2026-05-04)

### Phase R-A — Security Boundary (2026-05-03, ADR-0052)
- S1 CSP 9 directive + R1 shell:allow-open scope 4 도메인 + S2 portable import path boundary + Rename default + T4 7 path 회귀 invariant.

### Phase R-B — Catalog Trust Pipeline (2026-05-03, ADR-0053+0054)
- T2 minisign round-trip 4 invariant + S3 knowledge-stack SQLCipher feature gate + S4 manifest_cache schema v2 + signature_verified marker + S5 signed fetch wiring + cache poisoning 방어 + R4 release workflow 검수.

### Phase R-C — Network + Correctness (2026-05-03, ADR-0055)
- S7 reqwest .no_proxy() 7 사이트 + R3 9 사이트 폴백 제거 + C1 chat_stream graceful early disconnect (3 어댑터 delta_emitted) + C3 derive_filename 5 path traversal 검증 + 6 invariant.

### Phase R-D — Frontend Polish (2026-05-03, ADR-0056)
- K1 i18n 이모지 11건 제거 + Diagnostics SignatureSection lucide wiring + K2 errors namespace 3 키 + K3 Catalog hardcoded 5건 정리 + K4 PortableApiError path-denied variant + kind switch i18n.

### Phase R-E — Architecture Cleanup (2026-05-04, ADR-0057, 7 sub-phase 통합)
- T3 wiremock chat_stream graceful 회귀 가드 (raw TcpListener × 3 어댑터 × 2 case = 6 invariant)
- C2 신규 `crates/openai-compat-dto` (8 DTO 추출 + 6 invariant)
- A1 신규 `crates/chat-protocol` (ChatMessage/Event/Outcome 추출 + 7 invariant + adapter-ollama re-export)
- A2 (재스코프) adapter-ollama 역의존 완전 제거 (RuntimeAdapter trait split은 ROI 낮아 보류)
- P1 KnowledgeStorePool (Arc 캐시 + FIFO eviction max=4 + 5 invariant)
- P4 Channel send 실패 → cancel cascade (chat + portable + model_pull)
- R2 WorkspaceCancellationScope (opt-in register + 5 invariant)

### 분리 sub-phase
- **#31** (R-A.3 분리) — Knowledge IPC store_path boundary 검증. validate_store_path + 7 invariant.
- **#38** (R-B.2 후속) — knowledge-stack SQLCipher caller wiring. KnowledgeStorePool::with_passphrase + provision_knowledge_passphrase + keyring `knowledge-secret` entry.

### 통합 audit (2026-05-04)
- R-E.7 cancel_scope 죽은 인프라 발견 + 수정: ingest_path register + ExitRequested cancel_all
- model_pull Channel cancel cascade 누락 수정 (R-E.6 패턴 적용)

**누적**: R-A/B/C/D/E 5 페이즈 + 분리 2건 + audit 1건 = 27 commits / ADR-0052~0057 6건 신규 / 회귀 0건.

**다음 standby**: v0.0.1 release tag push 사용자 결정 (`git tag v0.0.1 && git push origin v0.0.1`).

---

## 시간순 완료 이력 (Phase α → Phase 6'.c, 2026-04-26 ~ 2026-04-28)

### ✅ 완료된 sub-phase 모음

#### Phase α — Foundation docs
- **ADR 0001~0020** (20건). 0005·0012는 pivot으로 superseded/modified.
- 아키텍처 4종, 한국어 가이드 + 개발자 가이드, 스캐폴딩 142+ files.
- 디자인 토큰, voice & tone 가이드.

#### 메타 — Pivot 확정 (2026-04-26)
- v1 포지셔닝: **"LM Studio/Ollama를 포함해 묶는 한국어 기반 로컬 AI 운영 허브"**.
- `docs/PIVOT.md` 작성, 6 pillar 정의 (자동 설치 / 한국어 / 포터블 / 큐레이션 / 워크벤치 / 자동 갱신).
- ADR-0016 (Wrap-not-replace) / 0017 (Manifest+Installer) / 0018 (Workbench v1 core) 신설.
- ADR-0005 → Superseded, ADR-0012 → Modified.
- `docs/PHASES.md` 페이즈 표 재배치 — Phase 1' / 2' / 3' / 5' / 6' 의미 변화.

#### Phase 0 — Tauri+Axum boot
- workspace Cargo.toml: axum 0.8, tower-http 0.6, tokio-util 0.7.
- `crates/core-gateway`: build_router + serve_with_shutdown + shutdown signal helper + auth stub + 통합 테스트 3건.
- Tauri 2 supervisor 패턴 (P1~P5 보강 적용): tauri::async_runtime::spawn, RunEvent::ExitRequested, app.manage(GatewayHandle), Emitter::emit("gateway://ready", port).
- `apps/desktop/src-tauri/capabilities/main.json`.
- `apps/desktop/src/{App.tsx, ipc/gateway.ts, i18n/ko.json}` — gateway://ready listen + 한국어 표시.
- 디자인 토큰 보강 (Pretendard, Radix alpha, 2-layer focus, 게이트웨이 pill).
- ADR-0015 (specta 타입 공유).
- **사용자 환경 자동 검증 완료** (2026-04-26 second run):
  - pnpm 9.15.9 + Rust 1.95.0 (msvc) + VS 2022 Build Tools 17.14.31 (Win11 SDK 10.0.26100) 설치.
  - `cargo fmt --check` ✅ / `cargo build --workspace` ✅ (~40s incremental) / `cargo clippy --workspace --all-targets -- -D warnings` ✅ / `cargo test -p core-gateway` ✅ 3/3 passed / `pnpm -r --filter '!@lmmaster/desktop' build` ✅.
  - 빌드 중 발견·수정한 사항 (소스에 영구 반영):
    1. tower-http 0.6의 `RequestBodyLimitLayer`가 `TraceLayer`와 response body 타입 호환 안 됨 (`ResponseBody<Body>: Default` 미구현). → axum의 `DefaultBodyLimit::max(2MB)` Router-level로 교체.
    2. `TimeoutLayer::new(Duration)` deprecated. → `TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(30))`. 인자 순서는 `(status, timeout)`.
    3. `apps/desktop/src-tauri/icons/icon.ico` 누락 시 tauri-build가 Windows에서 실패 → PowerShell로 placeholder `source.png` 생성 후 `pnpm tauri icon ./src-tauri/icons/source.png`로 모든 플랫폼 아이콘 자동 생성. icons/ 디렉터리 git 추적.
- **시각 확인 대기 항목**: `pnpm --filter @lmmaster/desktop tauri dev` 실행 후 한국어 사이드바 9 메뉴 + 하단 네온 그린 pill + "로컬 게이트웨이가 포트 NNNNN에서 실행 중입니다" 표시 확인.

#### Phase 1' — 설계 완료 (2026-04-26)
- 사용자 추가 요구 반영: **(a) zero-knowledge 부팅** (b) **자가스캔 + 자동 업그레이드 (가능 시 로컬 LLM)**.
- 4영역 elite 리서치(`docs/research/phase-1-reinforcement.md`).
- ADR-0019 / ADR-0020 신설. 메모리 `zero_knowledge_and_self_scan` 추가.

#### Phase 1A.1 — manifest + runtime-detector 구현 완료 (2026-04-26)
- 4영역 보강 리서치 종합(`docs/research/phase-1a-reinforcement.md`) — Tauri 2.10.x 플러그인 매트릭스 / 다운로드 stack(reqwest+backon+atomicwrites, **backoff crate는 RUSTSEC-2025-0012로 사용 금지**) / 하드웨어 probe stack / React 마법사 UX(XState+ArkUI Steps+해요체).
- ADR-0021 신설 (Phase 1A 핵심 스택 결정).
- `manifests/apps/ollama.json` (MIT, silent install 가능) + `manifests/apps/lm-studio.json` (EULA 재배포 금지, `open_url`만).
- **`crates/runtime-detector`** (신설): Detector + Ollama/LM Studio HTTP probe + 9건 wiremock 테스트.

#### Phase 1A.2.a — hardware-probe 구현 완료 (2026-04-26)
- 3영역 보강 리서치(`docs/research/phase-1a2a-reinforcement.md`) — NVIDIA+Apple GPU detect / Windows 레지스트리+DLL probe / mac sysctl+Metal+Linux glibc.
- **wgpu 의존성 드롭** — wgpu-hal-29.0.1의 windows-core 0.61 ↔ Tauri의 0.62 trait 충돌. 대신 Windows DXGI를 windows-rs 0.62로 직접 사용 (4GB clamp 버그 없는 정확한 VRAM).
- **`crates/hardware-probe` 전체 재작성**:
  - `types.rs` — HardwareReport / OsInfo / CpuInfo / MemInfo / DiskInfo / GpuInfo / GpuVendor (PCI ID 매핑) / GpuBackend / GpuDeviceType / RuntimeInfo. kebab-case serde + optional skip.
  - `sys.rs` — sysinfo 0.31 wrapper (OS / CPU / RAM / Disk).
  - `gpu.rs` — NVML(NVIDIA) + DXGI(Win 비-NVIDIA, cfg) + Metal(mac, cfg) + sysfs(Linux AMD, cfg). 모든 path graceful fail.
  - `win.rs` (cfg=windows) — winreg 0.55: WebView2 (`pv`), VC++ 2022 (`Installed`+`Version`), NVIDIA driver (Class GUID + `MatchingDeviceId` PCI\VEN_10DE 필터, raw "32.0.15.5186" → user-visible "551.86" 변환), CUDA toolkit list. libloading 0.8: d3d12.dll / DirectML.dll / nvcuda.dll / vulkan-1.dll.
  - `mac.rs` (cfg=macos) — `libc::sysctlbyname` 직접 FFI (sysctl crate 회피, ~30µs); objc2-metal 0.3 `MTLCreateSystemDefaultDevice` + `recommendedMaxWorkingSetSize` + `supportsFamily` (Apple7/8/9/Mac2 tier).
  - `linux.rs` (cfg=linux) — `gnu_get_libc_version` 직접 FFI; `/etc/os-release` inline parse; libcuda.so.1 / libvulkan.so.1 dlopen probe; `/sys/class/drm/card*/device` AMD enumeration.
  - **10건 통합 테스트 통과**: gpu_vendor PCI 매핑 / serde round-trip / NVIDIA driver 형식 변환 / VCRedist 2022 (BuildTools 설치 검증) / WebView2 / D3D12 / 전체 probe 검증 / JSON 직렬화 / 시간 budget < 2s.
- 워크스페이스 검증: `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace` 모두 ✅.
- 통합 테스트 합계: core-gateway 3건 + runtime-detector 9건 + hardware-probe 10건 = **22건 / 0 failed**.

#### Phase 1A.2.b — Vulkan probe + manifest evaluator 구현 완료 (2026-04-26)
- 2영역 보강 리서치(`docs/research/phase-1a2b-reinforcement.md` 미작성 — 에이전트 결과 직접 코드 반영) — Pinokio detect evaluator 패턴 + ash 0.38 Vulkan probe production 패턴.
- **`crates/runtime-detector/src/manifest.rs` 신설**:
  - `AppManifest` (Pinokio-style declarative 파싱)
  - `DetectRule` 4-variant tagged enum: `http.get` / `shell.which` / `registry.read` / `fs.exists`
  - `ManifestEvaluator::from_json` / `from_path` / `evaluate`
  - Platform 필터 (manifest는 portable, runtime이 cfg-aware skip)
  - Aggregation: running > installed > not-installed (Pinokio 정확 해석)
  - **11건 테스트** (wiremock + tempdir + 실제 manifests/apps/ollama.json 파싱).
- **`crates/hardware-probe/src/vulkan.rs` 신설**:
  - `ash 0.38` `loaded` feature — `Entry::load()` graceful fail
  - validation layer / surface extension 사용 안 함 (overhead 회피)
  - `enumerate_physical_devices` + `get_physical_device_properties` + `get_physical_device_memory_properties`
  - VRAM = DEVICE_LOCAL heap 합산 (Intel iGPU/AMD/일부 NVIDIA cross-check)
  - **5건 테스트** (loader 유무 무관 panic 안 함 + version format + cstr handling + serde).
- **`hardware-probe/src/gpu.rs` 보강** — Windows에서 NVML 부재/driver 없을 때 `crate::win::nvidia_driver_version()` 레지스트리 fallback.
- **`hardware-probe/src/linux.rs` 보강** — `libstdcpp_version()` 추가 (`/usr/lib/<arch>/libstdc++.so.6` 심볼릭 링크 target에서 추출).
- **`hardware-probe/src/lib.rs` 보강** — `probe()`에 vulkan probe 병렬 호출 + GpuInfo VRAM cross-check + `RuntimeInfo.vulkan_devices` 채움.
- **`RuntimeInfo` 확장** — `libstdcpp: Option<String>` + `vulkan_devices: Option<VulkanProbe>`.
- 워크스페이스 검증: fmt + clippy + test 모두 ✅. 통합 테스트 합계: core-gateway 3 + runtime-detector 20(11+9) + hardware-probe 15(10+5) = **38건 / 0 failed**.

#### Phase 1A.3.a — production resumable downloader 구현 완료 (2026-04-26)
- 결정 노트(`docs/research/phase-1a3a-decision.md`) — Phase 1A 보강 §2의 다운로드 stack 결정을 코드 레벨로 옮김.
- **`crates/installer` 신설**:
  - `Cargo.toml` — reqwest(stream) + tokio + tokio-util(rt+io) + futures-util + bytes + sha2 + hex + backon 1.6 + atomicwrites 0.4 + tempfile 3.
  - `error.rs` — `DownloadError` (thiserror): Http/BadStatus/Io/HashMismatch/Cancelled/RetriesExhausted/InvalidRequest. `is_retryable()` 정확 분기 (5xx/connect/timeout만 retry, hash mismatch/cancel은 fatal).
  - `progress.rs` — `DownloadEvent` (kebab-case serde): Started/Progress/Verified/Finished/Retrying. `ProgressSink` trait + `Fn(DownloadEvent)` blanket impl + `NoopSink`. Tauri Channel/mpsc/closure 어느 sink든 호환.
  - `downloader.rs` — `Downloader::download()`:
    - `.partial` 임시 파일 + Range header(`bytes=N-`) resume
    - 서버 응답 206=partial OK, 200=Range 무시 → hasher+`.partial` 자동 리셋
    - **streaming sha256** (`Sha256::update` per chunk, hasher state가 단일 진실원, 디스크 재읽기 없음)
    - **취소 안전** (`tokio::select!` `cancel.cancelled()` vs `stream.next()`, biased 우선순위)
    - 취소 시 `.partial` 보존 (resume 가능), hash mismatch 시만 `.partial` 삭제
    - **backon retry-with-jitter** (5xx/connect/timeout만; ExponentialBuilder + max_times 5 + min/max delay)
    - **progress throttle** — 256KB 또는 100ms 누적 후 emit (rustup/cargo 표준)
    - **atomic rename** — `.partial` → final, 5회 retry로 AV-locked 우회
- **7건 통합 테스트 (wiremock + tempdir)**: fresh download / sha256 mismatch / cancellation / invalid url / invalid parent dir / 404 fail-fast (no retry) / progress throttle → **7/7 passed**.
- 워크스페이스 검증: fmt + clippy --all-targets -D warnings + test 모두 ✅. 통합 테스트 합계: core-gateway 3 + runtime-detector 20 + hardware-probe 15 + **installer 7** = **45건 / 0 failed**.

#### Phase 1A.3.b.1 — manifest install/update schema + ActionExecutor (download_and_run / open_url) (2026-04-26)
- 1영역 보강 리서치 (subprocess installer invocation in Tauri 2 + Rust) — `tokio::process::Command` + `kill_on_drop` + `biased select!` + Inno Setup/MSI 정확한 exit code + `webbrowser` crate. tauri-plugin-shell capability scope는 동적 EXE 경로에 부적합 — Tauri command boundary가 보안 perimeter.
- **`runtime-detector::manifest` AppManifest 확장**:
  - `install: Option<InstallSpec>` — platform별(`windows`/`macos`/`linux`) 분기. `for_current_platform()` 헬퍼.
  - `update: Option<UpdateSpec>` — `source: GithubRelease/VendorEndpoint` + `trigger: RerunInstall/OpenUrl`.
  - `PlatformInstall` enum 4-variant tagged: `download_and_run` / `download_and_extract` / `shell.curl_pipe_sh` / `open_url`.
  - 각 method spec — args/min_disk_mb/min_ram_mb/sha256/post_install_check/timeout_seconds 등.
- **`crates/installer/src/action.rs` 신설**:
  - `ActionExecutor::execute()` — `PlatformInstall` 받아서 dispatch (`tokio::process` 또는 `webbrowser`).
  - `download_and_run` — Downloader로 fetch → `spawn_and_wait` (cancel-safe + 15분 timeout + Inno/MSI 성공 코드 인식 + reboot-required 분기).
  - `open_url` — `webbrowser::open()` (Win/mac/Linux 통합).
  - `download_and_extract` / `shell.curl_pipe_sh` — Phase 1A.3.b.2 — 현재 `Unsupported` 에러.
  - `ActionOutcome` (kebab serde) — `Success` / `SuccessRebootRequired` / `OpenedUrl`.
  - `ActionError` (thiserror) — Download/Io/ExitCode/NoExitCode/Timeout/Cancelled/OpenUrl/Unsupported/InvalidSpec.
  - `parse_sha256` + `derive_filename` 헬퍼.
- **11건 추가 테스트 (lib unit)**: derive_filename / parse_sha256 / ActionOutcome serde / OpenedUrl serde / ollama 풀 install 섹션 파싱 / lm-studio open_url 파싱 / for_current_platform 분기 / spawn zero exit / spawn nonzero exit 7 / spawn cancel(100ms) / spawn timeout(200ms) → **11/11 passed**.
- 워크스페이스 검증: fmt + clippy + test 모두 ✅. 통합 테스트 합계: core-gateway 3 + runtime-detector 20 + hardware-probe 15 + **installer 18** (lib 11 + integration 7) = **56건 / 0 failed**.

#### Phase 1A.3.b.2 — download_and_extract + shell.curl_pipe_sh + post_install_check 실평가 (2026-04-26)
- 보강 리서치 — zip 8.x (CVE-2025-29787 fix 포함, sync API), tar 0.4 + flate2 1.x per-entry streaming. `tokio::task::spawn_blocking` + `Arc<AtomicBool>` cancel flag (entry 사이마다 polling).
- **`crates/installer/src/extract.rs` 신설**:
  - `ExtractFormat` enum (`Zip` / `TarGz` / `Dmg`, kebab-case serde) + `detect_format()` 확장자 기반 추정.
  - `ExtractError` (thiserror): Io / Zip / ZipSlip / Cancelled / DmgRequiresMac / Join.
  - `ExtractOutcome { entries, total_bytes, format }` (serde).
  - `extract()` async dispatcher — `spawn_blocking` 작업 + 외부 `CancellationToken` watcher가 atomic flag 설정.
  - `extract_zip_blocking()` — `zip::ZipArchive` + **dual zip-slip 방어**: `enclosed_name()` (zip 8.x 안전) + 자체 `lexical_safe_subpath()` (RootDir/Prefix/escape ParentDir 거부).
  - `extract_tar_gz_blocking()` — `flate2::read::GzDecoder` + `tar::Archive::entries()` per-entry streaming. **부모 디렉터리 자동 생성** + 디렉터리 엔트리(`is_dir`)는 `create_dir_all` 후 continue (tar는 `Entry::unpack`이 부모 dir 자동 생성 안 함 — Win에서 NotFound 발생하던 버그 수정).
  - `extract::Dmg` 항상 `DmgRequiresMac` 반환 — Phase 1A.3.b.3에서 macOS hdiutil + plist + ditto 구현 예정.
- **`crates/installer/src/action.rs` 보강**:
  - `run_download_and_extract()` — Downloader로 fetch → `detect_format()` (manifest의 `format` override 가능) → `extract_archive()` → `target_path_is_safe()` 검증 → `evaluate_post_install_check_opt()`.
  - `run_shell_curl_pipe_sh()` — **`#[cfg(any(target_os = "linux", target_os = "macos"))]`** 게이트. Win은 `Unsupported` 반환. `validate_shell_safe_url()`로 shell metachar(`;`/`&`/`|`/`` ` ``/`$`/`(`/`)`/`<`/`>` 등) + `http(s)://` scheme 강제. `bash -c "curl -fsSL URL | sh"` via `spawn_and_wait` (15분 timeout + cancel-safe).
  - `evaluate_post_install_check()` — manifest의 `post_install_check` 실평가. HTTP 200 OK까지 1.5s 간격 polling, `wait_seconds` deadline, `tokio::select!`로 cancel 즉시 응답. 타임아웃은 `Timeout` 에러로 surface.
  - `target_path_is_safe()` — extract_to 경로 검증 (절대경로 + ParentDir 컴포넌트 없음).
  - `validate_shell_safe_url()` — shell injection 방어. `#[allow(dead_code)]` (Win build에서만 unused).
- **8건 action 테스트 추가**: `validate_shell_safe_url_accepts_normal_https` / `_rejects_metachars` / `_requires_http_scheme` / `target_path_is_safe_accepts_absolute_no_parent` / `evaluate_post_install_check_http_get_passes` / `evaluate_post_install_check_unreachable_times_out` / `evaluate_post_install_check_unsupported_method` / `evaluate_post_install_check_cancellation`.
- **9건 extract 테스트**: `detect_format_by_extension` / `lexical_safe_subpath_normal_paths` / `lexical_safe_subpath_collapses_inner_parent` / `lexical_safe_subpath_rejects_escape` / `lexical_safe_subpath_rejects_absolute` / `extract_zip_roundtrip_with_subdirs` / `extract_tar_gz_roundtrip` (parent dir 자동 생성 검증) / `extract_dmg_returns_unsupported_on_non_mac` / `extract_zip_with_zipslip_entry_rejected`.
- 워크스페이스 검증: fmt + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace` 모두 ✅. 통합 테스트 합계: core-gateway 3 + runtime-detector 20 + hardware-probe 15 + **installer 35** (lib 28 + integration 7) = **73건 / 0 failed**. (이전 56건 → +17.)

#### Phase 1A.3.b.3 — dmg macOS 추출 구현 완료 (2026-04-26)
- 보강 리서치 결과를 `docs/research/phase-1a3b3-decision.md`에 종합 — Apple `hdiutil` man page + Homebrew Cask `unpack_strategy/dmg.rb` + `dmg` crate Drop 패턴 + node-appdmg busy-retry + ditto 메타데이터 보존 근거.
- **workspace dependencies 추가**: `plist = "1"` + `walkdir = "2"` (target-cfg = macos로 installer가 사용).
- **`crates/installer/src/extract.rs` 보강 — `dmg_macos` 모듈 (`#[cfg(target_os = "macos")]`)**:
  - `MountGuard { device }` Drop impl — `hdiutil detach` 1차 → 실패 시 `-force` fallback. read-only 마운트라 force-detach 안전. return/?/panic 어디서든 detach 보장.
  - `parse_attach_plist()` — `plist::Value::from_reader` + `system-entities` 배열 순회 + `dev-entry` & `mount-point` 둘 다 있는 첫 entity 매칭 (Apple_partition_scheme entry는 mount-point가 없어 자동 skip).
  - `extract_dmg_blocking()`:
    - `tempfile::Builder::tempdir()` for `-mountrandom` (`/Volumes/<name>` 충돌 회피).
    - `hdiutil attach -plist -nobrowse -readonly -noautoopen -noverify -mountrandom <tmp> <dmg>` 호출. EULA pager 등장 시 stdin `"qn\n"` (quit + decline 자동 동의).
    - plist 파싱 → `(device, mount_point)` 추출 → `MountGuard` 즉시 생성.
    - `/usr/bin/ditto <mount_point> <target>` 절대경로 호출 (PATH 의존 X). xattr/resource fork/ACL/코드사인 보존.
    - 100ms `try_wait` 폴링 — cancel 발생 시 `child.kill()` + `Cancelled` 반환. drop으로 detach.
    - 종료 후 `walkdir`로 entries/total_bytes 집계 (ditto 진행률 미보고).
- **non-macOS 경로**: `extract()` dispatcher가 `cfg(not(target_os = "macos"))` 분기에서 `Err(ExtractError::DmgRequiresMac)` 반환 (기존 동작 유지).
- **새 `ExtractError` 변형**: `DmgAttachFailed(String)` / `DmgPlist(String)` / `DittoFailed(Option<i32>)`.
- **3건 macOS-only unit 테스트** (cargo test 일반 실행에 포함): `dmg_parse_plist_finds_first_mountable_entity` (실제 hdiutil 출력 mock XML) / `dmg_parse_plist_rejects_when_no_mount_point` / `dmg_parse_plist_rejects_when_not_dict`.
- **1건 macOS-only `#[ignore]` 통합 테스트**: `extract_dmg_macos_roundtrip` — `hdiutil create -size 2m -fs HFS+ -volname LMmasterTest`로 픽스처 생성 + sentinel 파일 작성 + extract 호출 + target 복원 검증 + `hdiutil info`로 dangling mount 확인. macOS runner에서 `cargo test -p installer -- --ignored extract_dmg`로 실행.
- **Win/Linux 검증** (현재 세션, Win11): fmt + clippy --all-targets -D warnings + test --workspace 모두 ✅. 73건 / 0 failed (Win에서 macOS 코드는 cfg로 미컴파일이므로 카운트 변동 없음). macOS runner에선 +3 unit (76건) + ignored 1건이 추가됨.

#### Phase 1A.3.c — Tauri 2 Channel + IPC 통합 완료 (2026-04-26)
- 보강 리서치 결과를 `docs/research/phase-1a3c-decision.md`에 종합 — `tauri::ipc::Channel<T>` per-invocation stream 패턴 + capability TOML 스키마 + InstallRegistry RAII + manifest BaseDirectory 해석.
- **`crates/installer` 보강**:
  - `install_event.rs` 신설 — `InstallEvent` (`#[serde(tag = "kind", rename_all = "kebab-case")]` 7-variant: Started/Download/Extract/PostCheck/Finished/Failed/Cancelled). `DownloadEvent`는 wrapper struct `Download { download: DownloadEvent }`로 감싸 안쪽 tag 보존. `ExtractPhase`/`PostCheckStatus` enum. `InstallSink` trait + closure blanket impl + `NoopInstallSink` + `InstallSinkClosed` 신호.
  - `install_runner.rs` 신설 — `run_install(manifest, cache_dir, cancel, sink) -> Result<ActionOutcome, InstallRunnerError>` 순수 함수. `InstallRunnerError` (NoInstallSection/NoPlatformBranch/Init/Action/SinkClosed) + `code()` i18n key 매핑. `DownloadBridge`로 ProgressSink → InstallSink 어댑터 (sink close 시 cancel propagate). `manifest_path(dir, id)` 헬퍼.
  - `lib.rs` re-export: `InstallEvent`, `InstallSink`, `InstallSinkClosed`, `NoopInstallSink`, `ExtractPhase`, `PostCheckStatus`, `run_install`, `manifest_path`, `InstallRunnerError`.
- **lib unit tests** +13: install_event 7건 (kebab serde + Download inner-tag preserve + ExtractPhase/PostCheckStatus iter + Failed code+message + Cancelled unit + closure FnImpl), install_runner 6건 (manifest_path 헬퍼 + NoInstallSection / NoPlatformBranch errors + open_url Started→Finished/Failed sequence + sink close→SinkClosed+cancel trigger + error_codes kebab-case).
- **`crates/installer/tests/install_runner_test.rs` 신설** — wiremock 기반 통합 3건: download phase event 시퀀스 (`Started`+`Download::Started`+`Download::Verified`+`Download::Finished`+terminal) / 404 → `Failed { code: "download-failed" }` / cancellation 100ms 후 `Cancelled` 또는 `Failed`.
- **`apps/desktop/src-tauri` 신설/보강**:
  - `src/install/mod.rs` — `InstallApiError` (Serialize, kebab-tagged: AlreadyInstalling/ManifestNotFound/ManifestParse/CacheDirCreate/Runner). `ChannelInstallSink` (Tauri `Channel<InstallEvent>` → `InstallSink` 어댑터, 닫힘 → `InstallSinkClosed`). `InstallGuard` Drop으로 registry.finish 보장. `manifests_dir(app)` (`BaseDirectory::Resource` 우선, dev에서 `CARGO_MANIFEST_DIR`-relative 폴백). `cache_dir(app)` (`BaseDirectory::AppLocalData/cache/installer/`). `#[tauri::command]` `install_app` (try_start → InstallGuard → manifest 로드 → cache_dir → run_install) + `cancel_install` (idempotent).
  - `src/install/registry.rs` — `InstallRegistry { Mutex<HashMap<String, CancellationToken>> }` + `try_start` (중복 시 AlreadyInstalling 거부) / `finish` (idempotent) / `cancel` (id별, idempotent) / `cancel_all` (앱 종료 시) / `in_flight_count` (디버그).
  - `src/lib.rs` — `app.manage(Arc<InstallRegistry>)` setup + `invoke_handler!` 4 commands + `ExitRequested`에서 `registry.cancel_all()`.
  - `Cargo.toml` — `installer` + `runtime-detector` + `scopeguard` + `thiserror` 추가.
  - `permissions/install.toml` 신설 — `allow-install-app` / `allow-cancel-install` 권한 정의 (`commands.allow = [...]`).
  - `capabilities/main.json` 갱신 — `permissions: ["core:default", "allow-install-app", "allow-cancel-install"]`.
  - `tauri.conf.json` — `bundle.resources: { "../../../manifests/apps/*.json": "manifests/apps/" }`.
- **lmmaster-desktop unit tests** +10: registry 7건 (try_start 신규/중복 / finish 제거+미존재 / cancel 단일/미존재 / cancel_all 일괄), install api 3건 (InstallApiError serde kind + Runner code/message + InstallGuard Drop calls finish).
- **`apps/desktop/src/ipc/install-events.ts` + `install.ts` 신설** — TypeScript discriminated union 미러 (`InstallEvent` / `DownloadEvent` / `ActionOutcome` / `InstallApiError`) + `installApp(id, { onEvent })` / `cancelInstall(id)` 헬퍼. Tauri 2 `Channel`/`invoke` API. `tsc -b` 통과.
- **워크스페이스 검증**: fmt + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace` 모두 ✅. 통합 테스트 합계: core-gateway 3 + runtime-detector 20 + hardware-probe 15 + **installer 51** (lib 41 + download_test 7 + install_runner_test 3) + **lmmaster-desktop 10** = **99건 / 0 failed**. (이전 73 → +26.)

#### Phase 1A.4.a — React 첫실행 마법사 골격 + Step 1/4 완료 (2026-04-26)
- 보강 리서치 결과를 `docs/research/phase-1a4a-decision.md`에 종합 — xstate v5 `setup({...}).createMachine()` + `createActorContext()`, `@ark-ui/react` Steps headless, `react-error-boundary` per-step + `resetKeys`, `framer-motion` `AnimatePresence` + `MotionConfig reducedMotion="user"`, Toss 8원칙 해요체 카피 (계속할게요/이전으로/나중에 할게요/닫기/문제가 생겼어요 다시 시도해 볼까요?), 로안워드 유지 (런타임/모델/GPU 가속).
- **`apps/desktop/package.json` 의존성 추가**: `xstate ^5.19` + `@xstate/react ^5.0` + `@ark-ui/react ^5.36.2` + `react-error-boundary ^4.1.2` + `framer-motion ^11.11`. `pnpm install` ✅ (94 packages added in 8.7s).
- **`apps/desktop/src/onboarding/`** 신설 (8 files):
  - `machine.ts` — xstate v5 머신 (4-state: language → scan → install → done). install은 idle/running 서브상태 분리 — 1A.4.c에서 `fromPromise` actor 합류 예정. `sanitizeSnapshotForPersist`로 `install.running`을 idle로 정규화 (휘발 상태 비영속화).
  - `persistence.ts` — `loadSnapshot/saveSnapshot/markCompleted/isOnboardingCompleted/resetOnboarding`. localStorage IO는 try/catch silent fallback. key 분리: `lmmaster.onboarding.v1` (snapshot) + `lmmaster.onboarding.completed` (flag).
  - `context.tsx` — `createActorContext` Provider + module-scope 동기 hydrate (race 회피). `PersistBridge`가 transition마다 sanitize 후 save. Slice hooks: `useOnboardingStep` / `useOnboardingInstallSub` / `useOnboardingLang` / `useOnboardingError` / `useOnboardingSend` / `useOnboardingDone`.
  - `OnboardingApp.tsx` — `<MotionConfig reducedMotion="user">` + `<Steps.Root linear count=4 step={machineValue}>` + `<AnimatePresence mode="wait">` 200ms transform+opacity transition + per-step `<ErrorBoundary FallbackComponent={StepErrorFallback} resetKeys={[step]}>`.
  - `StepErrorFallback.tsx` — 한국어 해요체 + `resetErrorBoundary` 버튼.
  - `steps/Step1Language.tsx` — ko/en 라디오 그룹 (role="radiogroup" + aria-checked) + i18n.changeLanguage 즉시 반영 + SET_LANG 머신 이벤트.
  - `steps/Step2Scan.tsx` / `steps/Step3Install.tsx` — placeholder ("다음 업데이트에서 만나요") + BACK/NEXT/SKIP 버튼 정상 동작 (1A.4.b/c에서 실 IPC 합류).
  - `steps/Step4Done.tsx` — ✓ 마크 + 시작할게요 CTA → `onFinish()` (App.tsx의 markCompleted + setCompleted 트리거).
  - `onboarding.css` — design-system tokens.css 전용 — `--surface`/`--border`/`--primary`/`--shadow-glow`/`--space-*`/`--radius-*`/`--ease-emphasized` 활용. dark + 네온 그린 일관 + `:focus-visible` 2-layer ring.
- **`apps/desktop/src/App.tsx` 갱신** — `isOnboardingCompleted()` 게이팅 → `<OnboardingApp onComplete={handleComplete} />` 또는 기존 `<MainShell />` 분기. MainShell은 기존 본문을 컴포넌트로 추출.
- **`apps/desktop/src/i18n/{ko,en}.json` 보강** — `onboarding.*` 키 22+ 항목 (steps/actions/language/scan/install/done/error/aria). 한국어는 해요체, 영어는 친근한 톤.
- **검증**: `pnpm exec tsc -b` ✅ / `pnpm run build` (Vite production) ✅ (542 modules, 389KB JS / 125KB gzipped, 11.5KB CSS / 3KB gzipped, 1.69s) / `cargo clippy --workspace --all-targets -- -D warnings` ✅ / `cargo test --workspace` ✅ 99건 (Rust 변경 없음). dev 시각 검증은 `pnpm tauri:dev` 사용자 측 확인 대기 — 기대: 첫 실행 시 ko 기본 마법사 표시, ko↔en 토글, NEXT/BACK 동작, Step 4 완료 시 MainShell 전환.

#### Phase 1A.4.b — Step 2 환경 점검 통합 완료 (2026-04-27)
- 결정 노트 `docs/research/phase-1a4b-decision.md` — 새 보안/성능 결정점 없어 인라인 압축. 결과 타입은 `crates/runtime-detector::EnvironmentReport`에 두고 hardware-probe를 의존성으로 추가 (의존 방향 깨끗).
- **`crates/runtime-detector` 보강**:
  - `Cargo.toml` — `hardware-probe.workspace = true` 추가.
  - `lib.rs` — `EnvironmentReport { hardware: HardwareReport, runtimes: Vec<DetectResult> }` + `probe_environment()` async 합성 함수 (`tokio::join!(hardware_probe::probe(), detector.detect_all())`). detector init 실패 시 빈 runtimes로 graceful fallback.
- **`apps/desktop/src-tauri/src/commands.rs` 보강**:
  - `EnvApiError` (Serialize, kebab-tagged: Internal — 미래 확장용).
  - `#[tauri::command] async fn detect_environment() -> Result<EnvironmentReport, EnvApiError>` — 한 번의 invoke로 hardware + runtimes 반환.
  - 단위 테스트 1건 — env_api_error_serializes_with_kind_tag.
- **`apps/desktop/src-tauri/permissions/probe.toml` 신설** — `allow-detect-environment` app-defined 권한.
- **`apps/desktop/src-tauri/capabilities/main.json` 갱신** — 새 권한 추가.
- **`apps/desktop/src-tauri/src/lib.rs`** — invoke_handler에 `commands::detect_environment` 추가.
- **`apps/desktop/src/ipc/environment.ts` 신설** — TS 미러 (HardwareReport / DetectResult / EnvironmentReport / EnvApiError) + `detectEnvironment()` 헬퍼 + 표현 헬퍼 (`formatGiB` / `osFamilyLabel` / `runtimeKindLabel`).
- **`apps/desktop/src/onboarding/machine.ts` 보강**:
  - context에 `env`/`scanError` 추가.
  - `actors: { scan: fromPromise(detectEnvironment) }`.
  - `actions: setEnv / setScanError / clearScanResult`.
  - `guards: hasEnv`.
  - `scan` state를 4 substate로 분할 — `idle` (always 분기) / `running` (invoke) / `done` / `failed` (RETRY → idle). 캐시된 env 있으면 재진입 시 즉시 done.
  - `sanitizeSnapshotForPersist` 보강 — env/scanError 제거 + scan 서브상태 정리 → 다음 부팅에 자동 재점검.
- **`apps/desktop/src/onboarding/context.tsx` 보강** — `useOnboardingScanSub` / `useOnboardingEnv` / `useOnboardingScanError` slice hooks 추가.
- **`apps/desktop/src/onboarding/steps/Step2Scan.tsx` 재작성** — substate별 분기:
  - running: 4 카드 skeleton + "잠시만 기다려 주세요" caption (aria-busy + animation).
  - done: 4 카드 (OS / 메모리 / GPU / 런타임 리스트) + status pill (ok/warn/muted) + 한국어 hint (RAM 8GB 미만 / 디스크 가용 20GB 미만 → 경고).
  - failed: ScanFailure 카드 + RETRY 버튼.
  - NEXT는 `hasEnv` 가드로 활성/비활성 — `disabled` 속성 일관 적용.
- **`apps/desktop/src/onboarding/onboarding.css` 보강** — scan 카드 + status pill (color-mix로 warn 토널 톤) + runtime row + skeleton shimmer (prefers-reduced-motion 처리) + disabled button. ~120 lines 추가.
- **`apps/desktop/src/i18n/{ko,en}.json` 보강** — `onboarding.scan.subtitle.*` / `card.*` / `status.*` / `body.*` / `hint.*` / `runtime.*` / `failure.*` 30+ 키.
- **검증**: `pnpm exec tsc -b` ✅ / `pnpm run build` ✅ (543 modules, 396KB JS / 128KB gzipped, 14.2KB CSS / 3.5KB gzipped, 1.66s) / `cargo clippy --workspace --all-targets -- -D warnings` ✅ / `cargo test --workspace` ✅ **100건 / 0 failed** (이전 99 → +1 env_api_error). dev 시각 검증 기대: Step 2 진입 시 자동 점검 시작 → 1.0~2.5s 후 카드 노출 → NEXT 활성. ko 기본 + RAM/디스크 warn 분기.

#### 메타 — 초격차 비전 + 경쟁 리서치 + PRODUCT.md (2026-04-27, 코드 변경 없음)
- **글로벌 경쟁 리서치** (별도 에이전트, ~2,400 단어) — LM Studio · Ollama · Jan · Msty · AnythingLLM · Cherry Studio · Open WebUI · GPT4All · Page Assist · Pinokio · Foundry Local · LLaMA-Factory · HF AutoTrain · AI Toolkit / Foundry Toolkit · Tailscale · JetBrains Toolbox 3.3. 카테고리별 unique strengths + LMmaster opportunity gap 매핑.
- **`docs/PRODUCT.md` 신설** — 11 절 + 4 부록:
  - §1 한 줄 / §2 개요 / §3 사용 목적 + 5 페르소나 + 6 시나리오 / §4 USP (6 pillar + 7-점 초격차 thesis + 컴패니언 4가지 입증) / §5 기대 효과 (개인/사이드 운영자/사내/튜닝 공급자) / §6 차별화 매트릭스 (10 경쟁자 비교) / §7 기능 일람 12 영역 (✅/🟡/⏳ 상태 표기) / §8 인터페이스 가이드 (마법사·환경 점검·첫 모델·홈/카탈로그·워크벤치·진단·웹앱 통합) / §9 한국어 카피 톤 (해요체) / §10 로드맵 / §11 운영 원칙. 부록 A 경쟁 학습 / 부록 B Korean-tuned 모델 카탈로그 (EXAONE/HCX-SEED/Polyglot-Ko/K-EXAONE) / 부록 C G1~G8 갭 / 부록 D 참고 링크.
- **`docs/PHASES.md` 보강** — 페이즈 표에 "초격차 강화" 컬럼 추가 + Phase 4.5' (Knowledge Stack RAG) 신설 행 + 7-점 thesis 분배 표 + G1~G8 갭 표 + ADR-0022/0023/0024 후보 + Top-3 위험 + 키 컴파스 ("Ollama/LM Studio 다음 분기 출시 시 USP 생존?").
- **`memory/competitive_thesis.md` 신설** — 7-점 thesis + 8 갭을 향후 세션 자동 로드.
- **검증**: 코드 변경 없음. fmt/clippy/test 실행 안 함. 문서만 추가/갱신.
- **결과**: 다음 페이즈 진입 직전 standby 상태. 7-점 thesis와 G1~G8을 Phase 1A.4.c → 1A.4.d → Phase 1' 잔여 → Phase 2'~6'에 자연스럽게 반영하도록 설계 보강 완료.

#### Phase 1A.4.c — Step 3 첫 런타임 설치 통합 완료 (2026-04-27)
- 보강 리서치 결과를 `docs/research/phase-1a4c-decision.md`에 종합 — xstate v5 `fromPromise` + `signal.abort` → `cancelInstall` 브리지, module-scope `installEventBridge` 패턴, 14건 에러 코드 한국어 매핑, 5-substate (decide/skip/idle/running/failed) 머신 확장, Open WebUI/JetBrains Toolbox/Linear 카드 패턴 차용.
- **스코프**: 모델 큐레이션은 Phase 2'로 분리. 본 sub-phase는 **런타임 설치 (Ollama/LM Studio)** 전용. Korean 모델 1순위는 카탈로그 단계에서 본격 도입.
- **`apps/desktop/src/onboarding/install-bridge.ts` 신설** — module-scope `installEventBridge` getter/setter. 단일 onboarding actor 가정 → singleton.
- **`apps/desktop/src/onboarding/machine.ts` 대폭 확장**:
  - context: `installLatest` / `installLog`(<=10) / `installProgress` / `installOutcome` / `installError` / `retryAttempt` 추가.
  - events: `INSTALL_EVENT { event }` / `RESET_INSTALL`.
  - actor: `install` = `fromPromise<ActionOutcome, { id: string }>` — `signal.aborted` 시 `cancelInstall(input.id)` + 늦은 이벤트 드롭 + cancel 시 sentinel outcome 반환 (xstate detached).
  - actions: `applyInstallEvent` (Download.progress → context.installProgress, Retrying.attempt → context.retryAttempt) / `setOutcome` / `setInstallError` (InstallApiError.runner.code 파싱) / `clearInstallState`.
  - guards: `anyRuntimeRunning` (env.runtimes에서 ollama/lm-studio running 검사).
  - `install` state: 5 substates — `decide` (always 분기 → skip 또는 idle) / `skip` (`after: { 1200: '#onboarding.done' }`) / `idle` (SELECT_MODEL → running) / `running` (invoke install actor + onDone → done + onError → failed) / `failed` (RETRY → running with `reenter: true`).
  - 부모 install state의 `BACK` → scan + `clearInstallState` (어디서든 BACK 가능).
  - `sanitizeSnapshotForPersist` 보강 — install 모든 서브상태 → idle 정규화 + install* context 모두 비움.
- **`apps/desktop/src/onboarding/context.tsx` 보강** — `<InstallEventBridge />` 컴포넌트 신설 (mount 시 `setInstallEventBridge((e) => actorRef.send({ type: "INSTALL_EVENT", event: e }))`, unmount 시 null) + 7개 새 selector hooks (`useOnboardingInstallSub` 5-state, `useOnboardingInstallLatest` / `Log` / `Progress` / `Outcome` / `Error` / `RetryAttempt` / `ModelId`).
- **`apps/desktop/src/onboarding/steps/Step3Install.tsx` 전체 재작성** (~430 lines):
  - sub 분기 (decide/idle → CardGrid · skip → SkipBanner · running → InstallRunningPanel · failed → InstallFailedPanel).
  - **CardGrid**: `RuntimeCard` 2개 (Ollama "추천" pill + LM Studio "EULA 안내" pill). 각 카드 = title + 한 줄 reason + 라이선스/용량 메타 (tabular-nums) + RAM low hint + already-running disabled. SELECT_MODEL invoke.
  - **InstallRunningPanel**: phase 표시 ("받고 있어요"/"압축 풀고 있어요"/"확인하고 있어요"/"거의 끝났어요") + retry attempt suffix + ProgressBar (250ms debounce + ETA 분/초 분기 + speed MB/s/KB/s) + 자세히 보기 details (마지막 10건 describeEvent) + 취소 버튼 (BACK 이벤트, signal.abort가 cancelInstall trigger — 단일 진실원).
  - **InstallFailedPanel**: code → i18n key 매핑 (14건 + default fallback) + raw message `<pre>` + RETRY 500ms debounce (Rust InstallRegistry race window 회피) + BACK / SKIP.
  - **OpenedUrlPanel**: outcome.kind === "opened-url"일 때 표시 (수동 NEXT 안내).
- **`apps/desktop/src/onboarding/onboarding.css` 보강** (~280 lines 추가):
  - Runtime card grid (auto-fit minmax 260px) + status-aware 토널 (running/installed → opacity 0.7) + pill (recommended primary-a-2 / eula warn 토널) + meta dl/dt/dd (tabular-nums) + cta align-self stretch.
  - Install progress bar (`<progress>` 네이티브 + WebKit/Moz 색상 token 매핑 + transition).
  - Meta 3-column grid (% / speed / ETA).
  - Details log (collapsible + 회전 marker + max-height 180px scroll + ellipsis).
  - Skip banner (primary-a-2 톤).
- **`apps/desktop/src/i18n/{ko,en}.json` 보강** — onboarding.install.* 60+ 키 (cards/cta/pill/meta/running/phase/retrySuffix/speedPending/etaPending/etaSeconds/etaMinutes/skip/failed/openedUrl/error 14 코드 + default). 모든 한국어 카피 해요체 일관.
- **검증**: `pnpm exec tsc -b` ✅ / `pnpm run build` ✅ (545 modules, 413KB JS / 133KB gzipped, 18.1KB CSS / 4KB gzipped, 1.67s) / `cargo clippy --workspace --all-targets -- -D warnings` ✅ / `cargo test --workspace` ✅ **100건 / 0 failed** (Rust 변경 없음). dev 시각 검증 기대 — 사용자 측: Step 3 진입 시 (a) 둘 다 running이면 자동 SKIP banner 1.2s 후 done, (b) Ollama 받을게요 클릭 → 진행률 + 단계 라벨 + ETA → finished → done, (c) 그만두기 → cancelled → 카드로 복귀, (d) 실패 → 한국어 메시지 + RETRY/BACK/SKIP.

#### Phase 1A.4.e — Polish & Palette 완료 (2026-04-27)
- 보강 리서치 결과를 `docs/research/phase-1a4e-decision.md`에 종합 — Aurora 3-radial 정적 + Apple recipe glass (blur 20 + saturate 160 + inset top highlight + 2-layer drop) + pointermove rAF spotlight + Ark UI Combobox 5.36 컨트롤드 + document-level ⌘K + 한국어 단순 substring 필터.
- **Step 1 — 디자인 시스템 토큰 + 클래스** (3 files):
  - `packages/design-system/src/tokens.css` +25 — `--aurora-1/2/3` (primary-a × 2 + accent 6% whisper) + `--aurora-mask` + `--glass-blur` / `--glass-saturate` / `--shadow-glass` (Apple inset-top-highlight 포함).
  - `packages/design-system/src/components.css` 신규 채움 — `.surface-aurora` (::before z-index -1 + mask + isolation) + `.glass` (color-mix bg + backdrop-filter blur+saturate + inset shadow) + `.spotlight` (::before radial at var(--mx, --my)) + `<kbd>` 칩 + `prefers-reduced-motion` / `prefers-reduced-transparency` degradation.
  - `packages/design-system/src/base.css` +20 — button/a/role=button 80~120ms transition baseline + `:active scale(0.98)` (button only) + reduced-motion override.
  - `apps/desktop/src/main.tsx` — `@lmmaster/design-system/components.css` import 추가.
- **Step 2 — 마법사 적용 + SpotlightCard** (2 files):
  - `apps/desktop/src/onboarding/onboarding.css` — `.onb-root` 인라인 radial 제거 (`.surface-aurora` 클래스로 대체) + spotlight 자식 stacking 보장 + `inset 0 1px 0 white-a-2` 카드 하이라이트 추가.
  - `apps/desktop/src/components/SpotlightCard.tsx` 신설 — pointermove + rAF 게이트 + var(--mx/--my) 쓰기. role/semantic 영향 0.
  - `OnboardingApp.tsx` — `.onb-root` 클래스에 `surface-aurora` 추가.
  - `Step3Install.tsx` — RuntimeCard `<article>` → `<SpotlightCard>` (div) 교체. hover 시 mouse-following primary glow.
- **Step 3 — Command Palette 신설** (8 files, ~440 LOC):
  - `apps/desktop/src/components/command-palette/types.ts` — `Command { id, group, label, keywords?, shortcut?, perform, isAvailable? }` + `CommandGroup` (4종).
  - `apps/desktop/src/components/command-palette/filter.ts` — `matchesQuery` (lowercase substring on label + keywords) + `groupCommands` (그룹 정렬 wizard→navigation→system→diagnostics).
  - `apps/desktop/src/components/command-palette/context.tsx` — `CommandPaletteProvider` (open/setOpen/toggle/commands/register Map-기반 idempotent) + `useCommandPalette` + `useCommandRegistration` (useEffect cleanup으로 자동 unregister).
  - `apps/desktop/src/components/command-palette/CommandPalette.tsx` — Ark UI `Combobox.Root` (open/onOpenChange/inputValue/onValueChange + loopFocus + positioning fixed) + `useListCollection` (itemToValue/itemToString/isItemDisabled) + framer-motion `<AnimatePresence>` (backdrop fade 120ms + dialog scale 0.96→1 y -8→0 150ms) + `createPortal(document.body)` + `<kbd>` shortcut hint + 빈 결과 한국어 안내.
  - `apps/desktop/src/components/command-palette/palette.css` — `.palette-backdrop` (z-palette overlay) + `.palette-frame` (top 120px width 540px 가로 중앙) + `.palette-dialog.glass` + `.palette-input` 56px + `.palette-list` 60vh scroll + `.palette-group/-label` (non-sticky) + `.palette-item` `[data-highlighted]` primary-a-2 + 좌측 2px primary border + `[data-disabled]` + 단축키 우측 정렬 + 작은 화면 fallback.
  - `apps/desktop/src/hooks/useCommandPaletteHotkey.ts` — document keydown ⌘K/Ctrl+K + e.repeat skip + preventDefault + Esc 닫기.
  - `apps/desktop/src/i18n/{ko,en}.json` — `palette.*` 12 항목 (aria/placeholder/empty/group×4/cmd×8).
- **Step 4 — 마운트 + 시드 명령** (2 files):
  - `apps/desktop/src/App.tsx` — `<CommandPaletteProvider>` wrap (마법사+MainShell 모두 감쌈) + `<CommandPalette>` 글로벌 portal + `<PaletteHotkey>` hotkey 마운트 + MainShell 시드 4건 (`nav.home`, `nav.diagnostics`, `system.gateway.copyUrl` (status 기반 disabled + clipboard.writeText), `system.wizard.reopen` (resetOnboarding + setCompleted false)).
  - `apps/desktop/src/onboarding/OnboardingApp.tsx` — wizard 시드 4건 (`wizard.lang.ko/en` (i18n.changeLanguage + SET_LANG), `wizard.scan.retry` (step==='scan' 한정), `wizard.restart` (3× BACK chain)). 한국어 jamo cheat keywords 포함 ("ㅎㄱ", "ㅎㄱㅈㄱ", "ㅁㅂㅅ").
- **검증**: `pnpm exec tsc -b` ✅ / `pnpm run build` ✅ (623 modules, **509KB JS / 163KB gzipped**, 22.7KB CSS / 4.9KB gzipped, 2.04s — Vite chunk size warning은 framer-motion + Ark UI 합계라 예상치 안에 있음, 후속 코드 분할 가능) / `cargo clippy --workspace --all-targets -- -D warnings` ✅ / `cargo test --workspace` ✅ **100건 / 0 failed** (Rust 변경 없음).
- **dev 시각 검증 기대 (사용자 측)**:
  1. 마법사 진입 시 `surface-aurora` 3-radial glow + 카드 inset 1px 하이라이트.
  2. Step 3 진입 시 Ollama/LM Studio 카드 hover → mouse-following primary glow (spotlight).
  3. ⌘K (mac) / Ctrl+K (Win) → backdrop fade + glass 패널 scale 0.96→1 + 입력 즉시 focus.
  4. "한국어"/"korean"/"ㅎㄱ" 모두 wizard.lang.ko 매칭, ↑↓ Enter 선택 → 머신 SET_LANG → i18n 즉시 반영.
  5. 마법사 끝난 뒤 ⌘K → MainShell 시드 4건 노출. 게이트웨이 listening 아닐 때 copyUrl는 회색.
  6. `prefers-reduced-motion`/`reduced-transparency` 활성화 시 spotlight flat / glass 불투명 surface로 자동 degradation.

#### 메타 — 자율 실행 권한 셋업 (2026-04-27, 코드 변경 없음)
- 사용자가 GPT와 논의한 권장안 적용 — 다음 세션부터 대기 상태 없이 자동 진행 가능.
- **`.claude/settings.json`** (프로젝트 공유, git 추적): `defaultMode: "bypassPermissions"` + 광범위 allow (Read/Edit/Write/Glob/Grep/Bash·PowerShell의 cargo·pnpm·npm·node·rustup·git status/diff/log·헬퍼 스크립트) + ask (git push / git reset --hard / rm -rf / .env / secrets/**) + deny (.git/** / node_modules/** / target/** / dist/** / curl|sh / iex). **이 파일이 source of truth** — IDE auto-permission tracker가 손대지 않음.
- **`.claude/settings.local.json`** (로컬, git 무시): base는 `defaultMode: bypassPermissions` + WebSearch/WebFetch만. IDE auto-tracker가 자동 누적해도 무해 (allow는 union).
- **`~/.claude/settings.json`** (사용자 전역): `skipDangerousModePermissionPrompt: true` 추가 — bypass 진입 시 사용자 확인 prompt 자동 스킵. 기존 `model: opus` / `effortLevel: medium` 보존.
- **`CLAUDE.md`** (프로젝트 루트, 신설): 7 섹션 — 권한 파일 분담 / 자율 정책 (확인 vs 즉시) / 페이즈 4단계 운영 / 빌드 검증 명령 (PowerShell 동적 exe 금지, PATH 또는 헬퍼 스크립트) / 코드 품질 / 메모리 / 안전 가드 / 다음 페이즈 진입 체크리스트.
- **`.claude/scripts/`** (신설, 6 PowerShell 헬퍼): `cargo-clippy.ps1` / `cargo-test.ps1` / `cargo-fmt.ps1` / `frontend-tsc.ps1` / `frontend-build.ps1` / `verify.ps1` (풀 검증 일괄). 각 스크립트는 PATH 보강(`$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"`) + 출력 트리밍 + `ErrorActionPreference = "Continue"` (cargo stderr를 NativeCommandError로 감싸지 않게).
- **메모리 갱신**: `autonomous_mode.md`에 권한 인프라 정식화 내역 추가.
- **smoke test**: `.claude/scripts/cargo-clippy.ps1` 실행 → `Finished `dev` profile in 1.26s` ✅.
- **IDE auto-tracker 노출**: settings.local.json은 새 명령마다 자동 갱신되지만 base에 bypass mode만 두고 비워뒀고, 진짜 정책은 settings.json(공유)이 보장.

#### Phase 1A.4.d.1 — vitest infra + 머신 테스트 + 1A.4.c 잔여 fix 완료 (2026-04-27)
- 보강 리서치 결과를 `docs/research/phase-1a4d1-decision.md`에 종합 — vitest 2.1 + jsdom 25 + RTL 16 + Tauri mock 2-layer + xstate v5 `provide({ actors })` + `vi.waitFor` + 페이크 타이머 + Issue A/B 정확한 fix.
- **1A.4.c 잔여 Issue A — OpenedUrl outcome 자동 done 차단**:
  - `machine.ts` 가드 추가: `isOpenedUrlOutcome: ({event}) => event.output?.kind === "opened-url"`.
  - `running.onDone`을 guarded array로 — `[{target:"openedUrl",guard:"isOpenedUrlOutcome",actions:"setOutcome"}, {target:"#onboarding.done",actions:"setOutcome"}]`.
  - 새 substate `openedUrl: { on: { NEXT: "#onboarding.done", SKIP: "#onboarding.done", BACK: { target: "idle", actions: "clearInstallState" } } }`.
  - `useOnboardingInstallSub` union에 `"openedUrl"` 추가.
  - `Step3Install.tsx` switch에 `case "openedUrl"` 분기 → `OpenedUrlPanel`. InstallFailedPanel 안 OpenedUrl 조기-반환 제거. OpenedUrlPanel CTA를 `SKIP` → `NEXT` 변경.
- **1A.4.c 잔여 Issue B — failed → RETRY 시 stale outcome/error 정리**:
  - `running.entry: "clearInstallState"` 추가. 첫 진입 + RETRY (`reenter: true`) 양쪽에서 발화 → installLatest/Log/Progress/Outcome/Error/retryAttempt 일괄 초기화.
  - `setInstallError` 보강 — Error 인스턴스도 message에서 JSON.parse 시도하도록 분기 강화.
- **vitest infra 신설**:
  - `apps/desktop/package.json` — devDeps `vitest ^2.1.0` + `@vitest/coverage-v8 ^2.1.0` + `@vitest/ui` + `@testing-library/{react ^16.3, jest-dom ^6.6, user-event ^14.5}` + `jsdom ^25` + `@types/jsdom ^21.1`. 스크립트 `test` / `test:watch` / `test:ui` / `test:coverage`. 94 패키지 설치 4.8s.
  - `apps/desktop/vitest.config.ts` 신설 — `mergeConfig(viteConfig, ...)` + `globals: false` + 기본 `node` env (per-file `@vitest-environment jsdom` pragma로 opt-in) + setupFiles + v8 coverage (60/60/50/60 thresholds).
  - `apps/desktop/src/__tests__/setup.ts` 신설 — `jest-dom/vitest` 매처 + beforeEach `setInstallEventBridge(null)` + jsdom env에서 localStorage clear + RTL cleanup (동적 import).
- **테스트 4건 신설**:
  - `src/onboarding/install-bridge.test.ts` (4 케이스) — singleton get/set/replace + null clear.
  - `src/onboarding/persistence.test.ts` (6 케이스, jsdom) — round-trip + missing + JSON parse error + completed flag + reset.
  - `src/components/command-palette/filter.test.ts` (10 케이스) — substring/case-insensitive/keywords/jamo cheat (cho-only) + group order + insertion order + empty.
  - `src/onboarding/machine.test.ts` (22 케이스) — 머신 초기 상태 (3) + scan 서브 (3) + install 서브 (8 — Issue A 가드 분기 + openedUrl substate + BACK + Issue B RETRY clear) + INSTALL_EVENT 누적 (4) + sanitizeSnapshotForPersist (4). xstate v5 `machine.provide({ actors: { foo: fromPromise(mocked) } })` 패턴 + `vi.waitFor` + `vi.useFakeTimers/advanceTimersByTimeAsync`.
- **`InstallActorInput` 타입 export** (machine.ts) — 테스트에서 `fromPromise<ActionOutcome, InstallActorInput>` 명시 시그니처에 사용.
- **검증**:
  - `pnpm test` ✅ — **42 vitest passed** (4 files, 4.33s).
  - `pnpm exec tsc -b` ✅ / `pnpm run build` ✅ (Vite 545 modules → 509KB JS / 22.65KB CSS, 2.23s).
  - `cargo clippy --workspace --all-targets -- -D warnings` ✅ (헬퍼 스크립트 사용).
  - `cargo test --workspace` ✅ **100 passed** (헬퍼 스크립트로 카운트).
  - **합계: vitest 42 + cargo 100 = 142건 / 0 failed**.
- 1A.4.d.1 standby 진입 조건 충족 — 1A.4.d.2 (컴포넌트 테스트), 1A.4.d.3 (axe-core 접근성).

#### Phase 1A.4.d.2 — 컴포넌트 테스트 완료 (2026-04-27)
- 결정 노트 `docs/research/phase-1a4d2-decision.md` — 1A.4.d.1 리서치 재활용. context hooks `vi.mock` + i18n mock + IPC mock + jsdom polyfill (IntersectionObserver/matchMedia for framer-motion).
- **`apps/desktop/src/__tests__/setup.ts` 보강** — IntersectionObserver / matchMedia jsdom polyfill (framer-motion 안정화).
- **5 컴포넌트 테스트 신설**:
  - `Step1Language.test.tsx` (5) — 렌더 + ko 활성 + en 클릭 시 changeLanguage+SET_LANG + 동일 클릭 noop + NEXT.
  - `Step2Scan.test.tsx` (4) — running skeleton + done 4카드 + failed RETRY + BACK.
  - `Step3Install.test.tsx` (6) — idle 카드 + SELECT_MODEL + skip banner + running progressbar/cancel + failed RETRY + openedUrl NEXT (Issue A 후속).
  - `Step4Done.test.tsx` (2) — 렌더 + CTA onFinish.
  - `CommandPalette.test.tsx` (6) — 닫힘 + ⌘K open + Ctrl+K open + 명령 리스트 + 검색 필터 + empty state.
- **검증**: `pnpm test` ✅ — **65 vitest 통과** (9 files, 7.13s, 이전 42 → +23).

#### Phase 1A.4.d.3 — vitest-axe 접근성 자동 검증 완료 (2026-04-27)
- `vitest-axe ^0.1.0` + `axe-core ^4.10` 추가. setup.ts에 매처 등록 시도했으나 vitest 2.x Assertion 타입과 type-param 충돌 → `expect(results.violations).toEqual([])` 직접 검사 패턴으로 우회.
- **`apps/desktop/src/__tests__/a11y.test.tsx` 신설** (10 케이스):
  - Step1Language / Step2Scan(running/done/failed) / Step3Install(idle/running/failed/openedUrl) / Step4Done — 9 컴포넌트 substate axe 검사.
  - CommandPalette open 상태 + portal 마운트된 body 전체 검사 (1).
- jsdom 한계로 `color-contrast` / `html-has-lang` / `landmark-one-main` / `region` 룰은 자동 disable. 나머지 60+ 룰 통과.
- **검증**: `pnpm test` ✅ — **75 vitest 통과** (10 files, 7.64s, 이전 65 → +10). cargo 100 그대로.

#### Phase 1' — `crates/registry-fetcher` 완료 (2026-04-27)
- 결정 노트 `docs/research/phase-1p-registry-fetcher-decision.md` — Pinokio + Foundry + Helm + jsDelivr + GitHub Releases 패턴 합성. rusqlite + spawn_blocking 채택 (sqlx 회피), 4-tier sequential fallback, JSON parse error 비-폴백, stale-while-error.
- **`crates/registry-fetcher` 신설** (workspace member 추가):
  - `Cargo.toml` — reqwest + rusqlite + sha2 + tokio + futures + backon + thiserror + tracing + time. Dev: wiremock 0.6 + tempfile.
  - `src/error.rs` — `FetcherError` 11 변형 (한국어 사용자 메시지 + structured tier/status).
  - `src/source.rs` — `SourceTier` (Vendor/Github/Jsdelivr/Bundled) + `SourceConfig` URL template 치환 + manifest_id sanitize + `default_sources(github_tag, jsdelivr_ref)`. **5 unit tests**.
  - `src/cache.rs` — `Cache` (rusqlite + tokio::sync::Mutex<Connection>) + WAL/WITHOUT ROWID/CHECK 제약 + body_sha256 무결성 검증 → 손상 시 자동 drop. `get`/`put`/`bump_fetched_at`/`invalidate`. **5 unit tests**.
  - `src/fetcher.rs` — `FetcherCore` 4-tier fallback + 조건부 GET (If-None-Match/If-Modified-Since) + 304 → cached body + JSON parse error 비-폴백 + `try_stale_cache` (모든 네트워크 실패 → 24h 내 가장 fresh row 반환). **5 unit tests**.
  - `src/lib.rs` — `RegistryFetcher::new/fetch/fetch_all (4-buffer_unordered)/invalidate/parse<T>` + `FetcherOptions` builder.
  - `tests/integration_test.rs` — wiremock 9 통합 케이스 (first-tier-success / 500 fallback / all-500 bundled / etag 304 round-trip / TTL skip network / invalid JSON 비-폴백 / bundled-only offline / invalidate / parse helper).
- **검증**: `cargo test -p registry-fetcher` ✅ 25 (16 lib + 9 integration). `cargo clippy --workspace --all-targets -- -D warnings` ✅. `cargo test --workspace` ✅ **125건 누적** (이전 100 → +25 registry-fetcher). vitest 75 그대로.

#### Phase 1' — `crates/scanner` 완료 (2026-04-27)
- 결정 노트 `docs/research/phase-1p-scanner-decision.md` — tokio-cron-scheduler 0.15 + Ollama API + cascade 캐시 1h + Korean 30% 검증 + deterministic fallback.
- **`crates/scanner` 신설** (workspace member 추가):
  - `src/error.rs` — `ScannerError` 10 변형 (Probe/OllamaUnreachable/OllamaTimeout/OllamaModelMissing/LlmValidationFailed soft/AlreadyRunning/Scheduler/Io/Http/Json) + `JobSchedulerError` From impl.
  - `src/checks.rs` — `Severity` (Info/Warn/Error) + `CheckResult` + `run_all` (RAM 4단/Disk 2단/GPU 4단/WebView2/VC++/NVIDIA driver 530+ Win/CUDA/Vulkan/Runtime). hardware-probe 실 필드 매핑 (`runtimes.vcredist_2022`, `runtimes.cuda_toolkits/cuda_runtime`, `runtimes.vulkan: bool`, GpuInfo.model/driver_version/backend/device_type/apple_family). **7 unit tests**.
  - `src/templates.rs` — `render_summary` deterministic Korean 해요체 (errors > warns > 정상). **4 unit tests**.
  - `src/llm_summary.rs` — `OllamaClient` (cascade TTL 1h, /api/tags + /api/generate, keep_alive 30s, options temperature 0.4). `validate_korean_summary` (≥30% hangul + <800 chars + chat-template 누수 차단). **DEFAULT_CASCADE** (EXAONE3.5 2.4B → 1.2B → 7.8B → HCX-SEED → Qwen → Llama). **6 unit tests**.
  - `src/scheduler.rs` — `tokio-cron-scheduler` 0.15.1 wiring + `Job::new_async` Box::pin 패턴 + on-launch grace tokio::spawn + cron `"0 0 */6 * * *"`.
  - `src/lib.rs` — `EnvironmentProbe` async trait + `DefaultProbe` (runtime-detector::probe_environment) + `Scanner` (try_lock concurrency guard + LLM/deterministic 분기) + `ScannerService` wrapper (start/shutdown) + `tokio::sync::broadcast` 8-capacity 결과 채널 + `ScanSummary { source: SummarySource, model_used }`.
  - `tests/integration_test.rs` — wiremock + MockProbe **8 통합 테스트**: deterministic-only / Ollama unreachable fallback / LLM happy path / LLM 영어만 → fallback / cascade no match → fallback / RAM <8GB Warn 검증 / broadcast 구독자 수신 / cascade 캐시 1h (`expect(1)`로 한 번만 hit 검증).
- **검증**: `cargo test -p scanner` ✅ **25** (17 lib + 8 integration). `cargo clippy --workspace --all-targets -- -D warnings` ✅. `cargo test --workspace` ✅ **150건 누적** (이전 125 → +25 scanner). vitest 75 그대로.

#### Phase 1' — runtime-manager + 어댑터 보강 완료 (2026-04-27)
- 결정 노트 `docs/research/phase-1p-runtime-manager-decision.md` — Ollama API + LM Studio OpenAI 호환 인라인 매핑.
- **`crates/runtime-manager` 보강**:
  - `src/manager.rs` 신설 — `RuntimeManager { adapters: HashMap<RuntimeKind, Arc<dyn RuntimeAdapter>> }` + `register/get/list_kinds/priority`. ADR-0004 trait object 분기 금지. v1 priority: Ollama=LMStudio=1 / LlamaCpp=2 / KoboldCpp=3 / vLLM=4. **4 unit tests**.
  - `shared-types::RuntimeKind`에 `Hash` derive 추가 (HashMap 키).
- **`crates/adapter-ollama` 본격 구현** (~280 LOC, 8 tests):
  - HTTP attach: detect (`GET /api/version`), health (latency 측정), list_models (`/api/tags`), pull_model (non-stream POST + ProgressUpdate 2회 emit), remove_model (`DELETE /api/delete`), warmup (`POST /api/generate { keep_alive: "5m" }`).
  - start/stop/restart no-op (외부 데몬), install/update bail (한국어 안내 — installer crate에 위임).
  - wiremock 8 cases: detect 200/404/unreachable + list_models + health + pull progress + warmup + install bail.
- **`crates/adapter-lmstudio` 본격 구현** (~200 LOC, 7 tests):
  - HTTP attach: detect (`GET /v1/models`), health, list_models (OpenAI `data[].id`), warmup (`POST /v1/chat/completions { max_tokens: 1 }`).
  - pull_model/remove_model/install bail (EULA — LM Studio UI에서만).
  - wiremock 7 cases: detect 200/unreachable + list_models + warmup + pull EULA bail + install EULA bail + health.
- **검증**: 모든 어댑터 wiremock 통과 ✅. `cargo clippy --workspace --all-targets -- -D warnings` ✅. `cargo test --workspace` ✅ **169건 누적** (이전 150 → +19: manager 4 + ollama 8 + lmstudio 7). vitest 75 그대로. **누적 vitest 75 + cargo 169 = 244건 / 0 failed**.

#### Phase 1' — React 추가 화면 + manifests fixture 완료 (2026-04-27)
- **`apps/desktop/src/components/InstallProgress.tsx` 신설** — Step3Install의 InstallRunningPanel을 props 기반 재사용 가능한 컴포넌트로 추출. 250ms debounce ProgressBar + phaseOf/describeEvent 헬퍼 export.
- **`apps/desktop/src/components/ToastUpdate.tsx` 신설** — JetBrains Toolbox 스타일 우하단 토스트. glass 패널 + slide-up animation + reduced-motion/transparency fallback.
- **`apps/desktop/src/pages/Home.tsx` 신설** — Tailscale-style 큰 status pill (10px dot + 4px primary glow) + 추천 모델 카드 그리드 (SpotlightCard 사용, 시드 3종 EXAONE/HCX-SEED/Qwen) + 자가 점검 미니 카드.
- **`apps/desktop/src/pages/home.css` 신설** + components.css에 `.toast-update` 추가.
- **App.tsx 갱신** — placeholder를 `<Home gw={gw} />`로 교체.
- **i18n 키 추가** — `home.hero/recommend/scan` + `toast.update.*` (ko + en).
- **`manifests/snapshot/`** 신설 — registry-fetcher 4-tier fallback Bundled tier fixture (ollama.json + lm-studio.json 복사 + README).
- **`manifests/prompts/scan-summary.json`** 신설 — scanner crate LLM prompt + validation 정의.
- **검증**: TS build ✅ / Vite (547 modules, 512KB JS, 27.4KB CSS, 2.32s) ✅ / vitest 75 회귀 없음 / cargo clippy ✅ / cargo 169건 그대로. 누적 vitest 75 + cargo 169 = **244건 / 0 failed**.

#### Phase 1' — Tauri scanner IPC 연결 완료 (2026-04-27)
- **`apps/desktop/src-tauri`**:
  - `Cargo.toml` — `scanner.workspace = true` 추가.
  - `src/commands.rs` — `LastScanCache` (Mutex<Option<ScanSummary>>) + `ScanApiError` (already-running / internal) + `start_scan` async (Arc<Scanner> State 사용) + `get_last_scan` (캐시 read). 2 unit tests.
  - `src/lib.rs` — `setup` 안에서 `tauri::async_runtime::spawn`으로 `ScannerService::new(defaults_with_ollama)` mount. broadcast subscriber → `app.emit("scan:summary", &summary)` + `LastScanCache.set` forward. scheduler 6h cron + 5분 grace + on-demand 모두 동작.
  - `permissions/scan.toml` 신설 — `allow-start-scan` / `allow-get-last-scan`.
  - `capabilities/main.json` 갱신.
- **`apps/desktop/src/ipc/scanner.ts`** 신설 — `ScanSummary`/`CheckResult` TS 미러 + `startScan()` / `getLastScan()` / `onScanSummary(cb)` listener.
- **`apps/desktop/src/App.tsx` MainShell 보강** — 캐시된 scan을 `getLastScan()`으로 첫 렌더에 표시 + `onScanSummary` 구독으로 자동 갱신. `<Home gw={gw} scanSummary={scan?.summary_korean} />` props 연결.
- **검증**: `cargo check -p lmmaster-desktop` ✅ / `cargo clippy --workspace --all-targets -- -D warnings` ✅ / `cargo test --workspace` ✅ **171건** (이전 169 → +2 LastScanCache/ScanApiError unit). TS build ✅. **누적 vitest 75 + cargo 171 = 246건 / 0 failed**.

#### Phase 2'.a — 카탈로그 + 추천 + IPC 완료 (2026-04-27)
- **결정 노트**: `docs/research/phase-2pa-catalog-decision.md` — Foundry Local + Pinokio 2-tier + HF metadata + Cherry Studio 패턴 종합. 5가지 알고리즘 보정 채택 (headroom bonus, asymmetric category match, lexicographic tie-breaker, lightweight cliff prevention, ExclusionReason enum).
- **`crates/shared-types/src/lib.rs`**: `ModelCategory`에 `Hash` derive 추가 (filter용).
- **`crates/model-registry`** 본격화:
  - `manifest.rs` — `VerificationInfo`/`VerificationTier` (Verified/Community, default Community) + `HfMeta` (schema-now-data-later) + `use_case_examples` (Workbench 시드용). 5 unit tests + legacy 호환.
  - `recommender.rs` — deterministic compute. `ExclusionReason` tagged enum (insufficient-vram/insufficient-ram/incompatible-runtime/deprecated). `evaluate` 함수에 5가지 보정 적용. lexicographic tie-breaker는 `sort_by_key((Reverse(score), Reverse(maturity), install_size, id))`. 5 unit tests.
  - `category.rs` — `filter_by_category` + `count_by_category`. 3 unit tests.
  - `cache.rs` — `load_from_dir` (재귀 + paths.sort 결정성 + 첫 id 우선) + `load_layered` (snapshot+overlay 머지). `CacheError` (io/json/schema-unsupported). 9 unit tests.
  - `lib.rs` — `Catalog::{load_from_dir, load_layered, from_entries, entries, filter, recommend, category_counts}`. 5 unit tests.
  - `tests/recommender_test.rs` — host_low/mid/high/tiny + huge_vram + 결정성 invariant 100회 + lightweight cliff + lex tie-breaker + maturity override + deprecated 제외 + 카테고리 비대칭 + verified boost + id 충돌 + GPU 미장착 VRAM 컷. 16 integration tests.
  - `Cargo.toml` — `thiserror` + dev `tempfile` 추가.
- **`manifests/snapshot/models/`** 시드 8건:
  - `agents/exaone-4-1.2b.json` (760MB, 한국어 9, lightweight 후보)
  - `agents/exaone-3.5-7.8b.json` (4900MB, balanced 한국어)
  - `agents/hcx-seed-8b.json` (5100MB, 한국어 strength 10)
  - `coding/qwen-2.5-coder-3b.json` (1900MB, coding 9)
  - `coding/exaone-4-32b.json` (19500MB, host_high 전용)
  - `roleplay/polyglot-ko-12.8b.json` (7700MB, 한국어 RP)
  - `slm/llama-3.2-3b.json` (2000MB, fallback)
  - `sound-stt/whisper-large-v3-korean.json` (3094MB, 한국어 STT)
  - sha256은 v1 placeholder (64 zeros) — Phase 6' 자동 갱신에서 실제 hash로 교체.
- **`apps/desktop/src-tauri`**:
  - `tauri.conf.json` — `bundle.resources`에 `manifests/snapshot/{*.json, models/{agents,coding,roleplay,slm,sound-stt}/*.json}` 추가.
  - `src/commands.rs` — `CatalogApiError` (not-loaded/host-not-probed/internal) + `get_catalog(category?)` + `get_recommendation(category)` (runtime_detector::probe → HostFingerprint 변환). `host_fingerprint_from_report` 헬퍼 (mem.total_bytes/(1024²) → ram_mb, gpus.first.vram_bytes → vram_mb).
  - `src/lib.rs` — setup에서 `load_bundled_catalog(app)`로 resource_dir + dev fallback (cwd ancestors) 패턴. 실패 시 빈 카탈로그로 graceful 시작.
  - `permissions/catalog.toml` 신설 — `allow-get-catalog` / `allow-get-recommendation`.
  - `capabilities/main.json` 갱신.
- **`apps/desktop/src/ipc/catalog.ts`** 신설 — `ModelEntry`/`Recommendation`/`ExclusionReason` TS 미러 + `getCatalog(category?)` / `getRecommendation(category)`.
- **검증**: `cargo fmt --all` ✅ / `cargo clippy --workspace --all-targets -- -D warnings` ✅ / `cargo test --workspace` ✅ **215건** (model-registry +44: 28 lib + 16 integration). `pnpm exec tsc -b` ✅. `pnpm exec vitest run` ✅ 75건. **누적 vitest 75 + cargo 215 = 290건 / 0 failed**.

#### Phase 2'.b — React Catalog 화면 + 카드 + 추천 패널 완료 (2026-04-27)
- **결정 노트**: `docs/research/phase-2pb-catalog-ui-decision.md` — Foundry Local + LM Studio + Ollama + Cursor 추천 슬롯 + HF model card 패턴 종합. 7가지 디자인 결정 + Best=null 빈 상태 처리 + "추천만" 토글 OFF 기본값 확정.
- **`apps/desktop/src/i18n/{ko,en}.json`** — `catalog.*` / `recommendation.*` / `model.*` / `drawer.*` 키 추가 (ko-KR 해요체 + 명사구 혼용).
- **`packages/design-system/src/tokens.css`** — `--info-a-2/3` / `--warn-a-2/3` / `--error-a-2/3` 알파 토큰 추가.
- **`packages/design-system/src/base.css`** — `.nav-item`을 button 호환으로 보강 (appearance/background/border/text-align/font/width/focus-ring).
- **`apps/desktop/src/components/catalog/`** 신설:
  - `format.ts` — `formatSize` (MB→GB 한국어 단위) + `compatOf` (호스트 호환 레벨) + `idOf` + `languageStars` + `modelHasFlag`.
  - `ModelCard.tsx` — 3행 정보(배지/제목+카테고리+별점+사용처/메트릭+호환 chip). excluded 시 dim + reason chip. Enter/Space로 선택.
  - `RecommendationStrip.tsx` — 4슬롯 가로 그리드. Best는 강조 보더+glow. Best=null은 빈 상태 메시지.
  - `ModelDetailDrawer.tsx` — 우측 슬라이드 portal. role="dialog" + Esc/배경클릭 닫기 + quant_options 라디오 + warnings + use_case_examples + license. 첫 quant 권장 chip.
  - 테스트: `format.test.ts` 11건 (formatSize/compat/stars/idOf/flag).
- **`apps/desktop/src/pages/{Catalog.tsx, catalog.css}`** 신설:
  - 좌측 sidebar(검색 + 6 카테고리 라디오) + 우측 main(추천 strip + 필터 chips + 그리드).
  - `getCatalog()` 1회 + `getRecommendation()` 카테고리 변경마다.
  - 필터 chip 4종 (tool/vision/structured/recommendedOnly) + sort 3종 (score/korean/size).
  - 카드 클릭 → Drawer / 슬롯 클릭 → 그리드 scroll + 1.2s pulse animation.
  - 빈 상태: 카탈로그 미로드 vs 필터 미일치 분리.
  - 테스트: `Catalog.test.tsx` 9건 (렌더/카테고리 필터/excluded 표시/Drawer 열림+Esc/추천만 토글/getRecommendation 실패 graceful/빈 카탈로그/sort).
- **`apps/desktop/src/App.tsx`** — `MainShell`에 `activeNav` state 추가, sidebar `<a>` → `<button>` 변경, `activeNav==="catalog"`이면 `<CatalogPage>` 렌더.
- **검증**: `cargo fmt --check` ✅ / `cargo clippy --workspace --all-targets -- -D warnings` ✅ / `pnpm exec tsc -b` ✅ / `pnpm exec vitest run` ✅ **95건** (이전 75 → +20: 11 format + 9 Catalog). **누적 vitest 95 + cargo 215 = 310건 / 0 failed**.

#### Phase 2'.c — bench-harness 코어 + 한국어 prompt 시드 완료 (2026-04-27)
- **결정 노트**: `docs/research/phase-2pc-bench-decision.md` (~2300단어) — llama.cpp llama-bench / vLLM benchmark_serving / Ollama native counter / Foundry/Azure 캐시 정책 / NVML 1Hz polling 보강. 7가지 디자인 결정 명시.
- **`crates/bench-harness/` 신설**:
  - `Cargo.toml` — shared-types/serde/tokio/tokio-util/tracing/thiserror/async-trait + dev tempfile.
  - `src/types.rs` — `BenchSample/BenchReport/BenchKey/BenchErrorReport/PromptSeed/PromptTask/BenchMetricsSource(Native|WallclockEst)`. `fingerprint_short` 헬퍼 (DefaultHasher 기반 16-hex). 6 unit tests.
  - `src/error.rs` — 한국어 메시지 7-variant `BenchError` enum + io/json From. 2 unit tests.
  - `src/adapter.rs` — `BenchAdapter` async_trait (`run_prompt(model, prompt_id, text, keep_alive, cancel) → BenchSample`). Ollama/LMStudio 어댑터 측에서 impl 예정.
  - `src/runner.rs` — `run_bench(adapter, plan, cancel)` 30s 절대 타임아웃 + cooperative cancel. warmup 1회 + 3 prompts × 2 패스 = 6 측정. partial report 정책 (timeout/cancelled/실패 시 sample_count=0). `aggregate` 평균 합성. 7 unit tests (fake adapter Ok/Fail/Cancel + aggregate Native/Mixed).
  - `src/cache.rs` — JSON 디스크 캐시 + 30일 TTL + host fingerprint/digest mismatch invalidate. 7 unit tests (round-trip / missing / fingerprint change / digest mismatch / TTL expired / invalidate / nested dir create).
  - `src/lib.rs` — `baseline_korean_seeds()` (3 한국어 시드 inline). 2 unit tests (한글 포함 / 3 task 커버).
- **`manifests/prompts/bench-ko.json`** 신설 — chat/summary/reasoning 3 시드 + `purpose_ko` 메타.
- **`Cargo.toml` workspace** — `bench-harness` member + path dependency 등록.
- **검증**: `cargo fmt --check` ✅ / `cargo clippy --workspace --all-targets -- -D warnings` ✅ / `cargo test --workspace` ✅ **238건** (이전 215 → +23 bench-harness lib). **누적 vitest 95 + cargo 238 = 333건 / 0 failed**.

#### 메타 — 품질·컨텍스트 인프라 보강 완료 (2026-04-27, 코드 변경 없음)
- **CLAUDE.md v1.1**: §4 카피 톤 매뉴얼 + 코드/UI/테스트 컨벤션 매트릭스 + §4.5 결정 노트 6-섹션 + §7 Sub-phase DoD + §8 negative space 보존 원칙. lint급 두께로 보강.
- **`docs/DECISION_NOTE_TEMPLATE.md`**: 모든 결정 노트의 6-섹션 의무 구조 (특히 §2 기각안+이유). negative space가 다음 세션이 같은 함정에 빠지지 않게 하는 장치.
- **메모리 신설 2종**: `decision_note_structure.md` (feedback) + `quality_gates.md` (feedback). MEMORY.md 인덱스 갱신.
- **계기**: 사용자가 "1회 세션 max 처리량 한계로 제품 퀄리티 떨어질 위험" 명시(2026-04-27) — "제품 퀄리티 최우선" 정책 재확인 + 컨텍스트 보존 인프라 강화.

#### Phase 2'.c.2 — bench 어댑터 + Tauri IPC + React UI 완료 (2026-04-27)
- **`crates/adapter-ollama`** — `BenchAdapter` impl 추가:
  - `Cargo.toml` — bench-harness/tokio-util/tokio-stream/futures + reqwest stream feature.
  - `/api/generate { stream: true, keep_alive }` NDJSON 파싱 + 첫 non-empty `response` chunk = TTFT + 마지막 `done: true` chunk의 `eval_count`/`eval_duration`/`prompt_eval_*`/`load_duration`/`total_duration` 추출.
  - `metrics_source: Native`. cancel 시 stream drop = server abort.
  - 5 통합 테스트 (native counter 추출 / model not found / unreachable / done 누락 / runtime_label).
- **`crates/adapter-lmstudio`** — `BenchAdapter` impl 추가:
  - `/v1/chat/completions { stream: true }` SSE `data: ...\n\n` 파싱 + `[DONE]` 마커 + `delta.content` 첫 non-empty = TTFT.
  - `usage.completion_tokens` 1순위, 없으면 chunk 카운트 fallback. wall-clock TTFT.
  - `metrics_source: WallclockEst` + `pp_tps: None` (분리 불가).
  - 5 통합 테스트 (chunk count fallback / usage 우선 / model not found / unreachable / runtime_label).
- **`apps/desktop/src-tauri/src/bench/`** 신설:
  - `registry.rs` — `BenchRegistry { Mutex<HashMap<id, CancellationToken>> }` (install/registry.rs 패턴 차용). 7 unit tests.
  - `cache_store.rs` — `app_data_dir/cache/bench/` Tauri path resolve + bench-harness::cache wrap.
  - `commands.rs` — `BenchApiError` (already-running/host-not-probed/unsupported-runtime{runtime}/internal{message}, **struct variant** — tagged enum + tuple variant 충돌). `start_bench` async (host probe → 어댑터 dispatch → run_bench → cache save → emit `bench:finished`). `cancel_bench` sync. `get_last_bench_report` async (host probe → cache load_if_fresh). 2 unit tests.
  - `mod.rs` — re-export.
- **`apps/desktop/src-tauri/`**: `Cargo.toml` workspace adapter-ollama/adapter-lmstudio 추가. `lib.rs` BenchRegistry State + 명령 핸들러 등록 + ExitRequested cancel_all. `permissions/bench.toml` 3 권한. `capabilities/main.json` 갱신.
- **`apps/desktop/src/ipc/bench.ts`** 신설 — `BenchReport`/`BenchSample`/`BenchErrorReport` TS 미러 + `startBench`/`cancelBench`/`getLastBenchReport` + `onBenchStarted`/`onBenchFinished` listener.
- **`apps/desktop/src/components/catalog/BenchChip.tsx`** 신설 — 4 상태(idle CTA / running spinner+cancel / report ok-summary / partial / error+retry / wallclock-est 추정 배지). prefers-reduced-motion 가드. data-testid 마커 (bench-ok/partial/timeout/error-chip). 8 vitest.
- **`ModelDetailDrawer.tsx`** 통합 — Drawer 안에 "30초 측정" 섹션 + `BenchChip` + 첫 응답 미리보기 텍스트 (sample_text_excerpt). `getLastBenchReport`로 캐시 자동 조회 + `onBenchFinished` 구독 + `pickRuntime(model)` 헬퍼 (ollama > lm-studio).
- **i18n**: `bench.*` 12 키 ko/en (해요체 카피: "초당 N토큰 · 첫 응답 N초", "측정하고 있어요", "이 PC에는 무거워요" 등). `drawer.section.bench` 추가.
- **`packages/design-system/src/...`**: 별도 신규 토큰 없음 (Phase 2'.b의 info/warn/error 알파 재사용). `catalog.css`에 `.bench-chip` 변형 4종 + spinner + 추정 배지 스타일.
- **검증**: `cargo fmt --check` ✅ / `cargo clippy --workspace --all-targets -- -D warnings` ✅ / `cargo test --workspace` ✅ **257건** (이전 238 → +19: adapter-ollama +5, adapter-lmstudio +5, bench/registry +7, bench/commands +2). `pnpm exec tsc -b` ✅. `pnpm exec vitest run` ✅ **103건** (이전 95 → +8 BenchChip). **누적 vitest 103 + cargo 257 = 360건 / 0 failed**.

#### Phase 3'.a — Gateway core 라우팅 + SSE pass-through 완료 (2026-04-27)
- **결정 노트**: `docs/research/phase-3p-gateway-decision.md` (~2500단어, 6-섹션 의무 구조 — §2 기각안 10건 명시: LiteLLM 임베드, Open WebUI 대체, SSE 자체 reparse, SQLCipher default ON, per-runtime semaphore, 자체 SDK, 마스터 패스워드, 무인증, 일괄 invalidate, prefix 4자).
- **ADR-0022 신설**: Gateway 라우팅 정책 + scoped key 모델. 10가지 결정 + Alternatives considered (negative space mirror) + 검증 invariant 8종.
- **`crates/core-gateway`** 본격화:
  - `Cargo.toml` — reqwest stream + async-trait + thiserror + futures + bytes + dev wiremock 추가.
  - `upstream.rs` — `UpstreamProvider` trait (어댑터에 직접 의존 안 함) + `UpstreamRoute`/`ModelDescriptor` + `StaticProvider` 테스트용 + `runtime_label` 헬퍼. 3 unit tests.
  - `state.rs` — `AppState { provider, semaphore, http }`. 글로벌 `Semaphore(permits=1)` (GPU contention 직렬화). reqwest 클라이언트 1개 재사용 (no_proxy + tcp_keepalive 10s + pool_idle_timeout 30s).
  - `openai_error.rs` — OpenAI 호환 envelope 헬퍼: `error_response/model_not_found/upstream_unreachable/upstream_status/invalid_body`. 3 unit tests.
  - `routes/chat.rs` — `POST /v1/chat/completions` 본격 구현: model 필드 inspect → provider lookup → semaphore acquire → 업스트림 forward → byte-perfect `bytes_stream()` relay (axum::Sse 재포맷 안 함). `ReleaseOnDrop` stream wrapper로 permit RAII. `x-lmmaster-queue-wait-ms` 헤더 노출.
  - `routes/models.rs` — `GET /v1/models` 합산 + `GET /v1/models/:id` 단일.
  - `lib.rs` — chat 라우트는 별도 600s TimeoutLayer (장시간 generation 대비), 다른 라우트는 30s. `build_router(cfg, state)` 시그니처로 변경.
- **`apps/desktop/src-tauri/src/gateway.rs`** — 새 시그니처 호출 (현재 빈 StaticProvider — Phase 3'.b에서 RuntimeManager wrap한 RegistryProvider 주입 예정).
- **통합 테스트** `tests/routing_test.rs` 13건: invalid body 400 / unknown model 404 / upstream forward JSON / upstream 5xx propagate / unreachable 502 / **byte-perfect SSE relay** / list_models 합산 + owned_by / retrieve missing 404 / retrieve found / **semaphore 직렬화 (200ms × 2 ≥ 380ms 측정)** / queue-wait 헤더 numeric / upstream 404 envelope / ModelDescriptor consistency.
- **검증**: `cargo fmt --check` ✅ / `cargo clippy --workspace --all-targets -- -D warnings` ✅ / `cargo test --workspace` ✅ **276건** (이전 257 → +19: gateway lib +6 (upstream 3 + openai_error 3) + routing_test +13). `pnpm exec tsc -b` ✅ / vitest 103 변동 없음. **누적 vitest 103 + cargo 276 = 379건 / 0 failed**.

#### Phase 3'.b — Per-webapp scoped key + key-manager 본격화 + Settings GUI 완료 (2026-04-27)
- **`crates/key-manager`** 본격화:
  - `Cargo.toml` — argon2 0.5 + rand 0.8 + thiserror + dev tempfile.
  - `scope.rs` — 5차원 `Scope { models[], endpoints[], allowed_origins[], expires_at, project_id, rate_limit }` + glob 매칭 (`*`, `?`) + `is_expired()`. 14 unit tests.
  - `hash.rs` — argon2id (mem 64MB / iter 3 / par 1, OWASP 2024) + PHC 형식. 4 unit tests.
  - `plaintext.rs` — `lm-{prefix8}{secret24}` (총 35자), 모호 문자 제외 (0/O/1/I/L/l) — "lm-" prefix 자체는 검사 제외. 6 unit tests.
  - `store.rs` — SQLite (rusqlite bundled) + idx_api_keys_prefix/revoked + RFC3339. 7 unit tests.
  - `manager.rs` — `KeyManager::{open, open_memory, issue, list, revoke, verify}` + `AuthOutcome` enum. server-only key는 origin 없어도 허용, web-only key (allowed_origins 있음)는 origin 없으면 거부. 14 unit tests.
- **`crates/core-gateway`** auth 활성:
  - `auth.rs` — `require_api_key` 미들웨어 (Bearer 추출 + KeyManager.verify + Origin 매칭) + `preflight_response` (OPTIONS는 origin echo + 204) + 통과 시 ACAO를 키 origin으로 정확히 echo (`*` 절대 금지) + 거부 시 OpenAI 호환 envelope.
  - `state.rs` — `key_manager: Option<Arc<KeyManager>>` + `with_key_manager()`.
  - `lib.rs` — KeyManager 주입 시 `/v1/*` 라우트에 auth layer mount. 외부 `CorsLayer::permissive()` **제거** (ACAO를 *로 덮어쓰는 충돌 차단).
- **통합 테스트** `tests/auth_test.rs` 11건: missing key 401 / invalid plaintext 401 / origin mismatch 403 / revoked 401 / expired 401 / endpoint scope 403 / OPTIONS preflight 무인증 통과 / **ACAO=정확한 origin echo (`*` 아님)** / no origin + web-only key 403 / server-only key + no origin 통과 / health 무인증.
- **`apps/desktop/src-tauri`**:
  - `Cargo.toml` — time 추가.
  - `src/keys/{mod.rs, commands.rs}` 신설 — `create_api_key` / `list_api_keys` / `revoke_api_key` Tauri commands + `KeyApiError` (kebab-case tagged enum) + `CreatedKey`/`ApiKeyView` DTO. 2 unit tests.
  - `src/lib.rs` — KeyManager State 등록 (app_data_dir/keys.db, 실패 시 메모리 폴백) + 핸들러 등록.
  - `src/gateway.rs` — `app.try_state::<Arc<KeyManager>>()`로 KeyManager 가져와 AppState 주입 (Tauri Manager trait import).
  - `permissions/keys.toml` 신설 — 3 권한.
  - `capabilities/main.json` 갱신.
- **React UI**:
  - `apps/desktop/src/ipc/keys.ts` — TS 미러 + `defaultWebScope` 헬퍼.
  - `components/keys/ApiKeyIssueModal.tsx` — 1회 reveal modal (alias 필수 + multi-origin + glob model + 8s auto-mask + 5분 auto-close + 클립보드 카피). a11y: dialog/aria-modal/aria-labelledby/auto-focus/Esc 닫기.
  - `components/keys/ApiKeysPanel.tsx` — Settings 본체. table + revoked dim + revoke confirm.
  - `components/keys/keys.css` — 디자인 토큰만, button reset, focus-visible, prefers-reduced-motion 가드.
  - `App.tsx` — sidebar nav `keys` 라우팅 추가.
  - i18n `keys.*` 30+ 키 ko/en (해요체: "이 키는 지금만 보여드려요", "회수할게요").
  - vitest `ApiKeysPanel.test.tsx` 8건: 빈 상태 / active+revoked 표시 / revoked는 revoke 버튼 없음 / revoke confirm 호출 / confirm 거부 시 호출 안 함 / alias 빈 거부 / 발급+reveal step / 발급 실패 에러.
- **검증**: `cargo fmt --check` ✅ / `cargo clippy --workspace --all-targets -- -D warnings` ✅ / `cargo test --workspace` ✅ **337건** (이전 276 → +61: key-manager +46, core-gateway 통합 +11, gateway lib +2, keys/commands +2). `pnpm exec tsc -b` ✅ / `pnpm exec vitest run` ✅ **111건** (이전 103 → +8 ApiKeysPanel). **누적 vitest 111 + cargo 337 = 448건 / 0 failed**.

#### Phase 3'.c — Portable workspace fingerprint 3-tier repair 완료 (2026-04-27)
- **`crates/portable-workspace`** 본격화 (스캐폴딩 → production):
  - `Cargo.toml` — thiserror + sha2 + hex + dev tempfile.
  - `fingerprint.rs` — `WorkspaceFingerprint { os, arch, gpu_class, vram_bucket_mb, ram_bucket_mb, fingerprint_hash }` + `GpuClass` enum (Nvidia/Amd/Intel/Apple/None/Other) + 16GB RAM bucket / 8GB VRAM bucket으로 미세 변동 흡수 + sha256-trunc(16-hex) hash. `classify(prev, current) → green/yellow/red`. 11 unit tests.
  - `repair.rs` — `evaluate_and_repair` 메인 entry + `apply_repair` (yellow/red에서 cache/{bench,scan} invalidate, red에서 manifest.runtimes_installed[] 비움) + atomic temp+rename save_fingerprint. 8 unit tests (first run / green reload / yellow GPU change / red OS change / red arch change / corrupted file / round-trip / last_repaired_at).
- **`apps/desktop/src-tauri/src/workspace/`** 신설 — `WorkspaceRoot` lazy app_data_dir/workspace + `get_workspace_fingerprint` (read-only 분류) + `check_workspace_repair` (실제 repair 적용). 2 unit tests + permissions/workspace.toml + capabilities/main.json.
- **React UI**: `apps/desktop/src/ipc/workspace.ts` TS 미러 + `components/workspace/WorkspaceRepairBanner.tsx` (green=unmount / yellow=토스트 / red=모달, data-testid 마커) + `workspace.css` (디자인 토큰만, prefers-reduced-motion 가드) + i18n `workspace.repair.*` 11키 ko/en (해요체) + App.tsx mount + vitest 7건 (green/yellow/red/dismiss/repair 호출/ping fail silent/repair fail silent).
- **검증**: cargo fmt+clippy+test ✅ **358건** (+21: portable-workspace +19, workspace/commands +2). vitest ✅ **118건** (+7).

#### Phase 3'.d — TS SDK 강화 + 기존 웹앱 통합 데모 완료 (2026-04-27)
- **`packages/js-sdk`** 본격 보강:
  - `types.ts` — OpenAI 호환 `ChatCompletion` / `ChatCompletionChunk` / `ChatUsage` / `ApiErrorEnvelope` + `LMmasterApiError` 클래스 (status/type/code/message 보존).
  - `client.ts` — `LMmasterClient` (baseUrl trailing slash 정규화 + apiKey + fetchImpl) + `ensureOk()` 헬퍼 (4xx/5xx envelope을 LMmasterApiError로 변환).
  - `chat.ts` — `chatCompletions` 단발 + `streamChat` SSE iterator (JSON.parse + `[DONE]` + malformed skip) + `streamChatText` 누적 헬퍼.
  - `discovery.ts` — `pingHealth` (실패는 null) + `buildLaunchUrl` (lmmaster:// custom scheme) + `autoFindGateway` (후보 포트 + AbortController timeout).
  - `models.ts` 신설 — `listModels` / `retrieveModel`.
  - vitest 21건 (chat 단발+streaming+envelope 8 / client 6 / discovery 7).
- **`examples/web-demo`** 신설 — vite + ts + `@lmmaster/sdk: workspace:*` 단일 의존성. dark 테마 HTML + status pill + baseUrl/apiKey/model/prompt 입력 + `streamChat` 누적 표시 + `LMmasterApiError` 한국어 친절 에러. README 트러블슈팅 매트릭스 (origin_denied / invalid_api_key / model_not_found). vite build ✅.
- **검증**: 모든 검증 명령 ✅. **누적 vitest 118 (desktop) + 21 (js-sdk) + cargo 358 = 497건 / 0 failed**.

#### Phase 4.a + 4.h 부분 — 공통 컴포넌트 + Korean preset 5 sample 완료 (2026-04-27)
- **결정 노트**: `docs/research/phase-4-screens-decision.md` (~3000단어, 6-섹션 의무 — §2 기각안 8건 명시: 9 화면 1 페이지 tab / preset 영어 fallback / react-window / workbench 일부 노출 / keys-projects 통합 / settings에 Gemini 키 입력 / 카탈로그 초성 검색 / page transition).
- **`packages/design-system/src/react/`** 신설:
  - `StatusPill.tsx` + `StatusPill.css` — 5 상태 (booting/listening/failed/stopping/idle) + sm/md/lg + a11y `role="status"` + `aria-live="polite"` + dot 색 토큰 swap.
  - `VirtualList.tsx` + `VirtualList.css` — `@tanstack/react-virtual` 기반 24px row + sticky group header + empty state slot.
  - `index.ts` re-export (`@lmmaster/design-system/react`).
  - `package.json` — react / @tanstack/react-virtual peer dependency.
  - `apps/desktop/package.json` — @tanstack/react-virtual ^3.10 추가.
- **`crates/preset-registry`** 신설:
  - `Cargo.toml` — shared-types/serde/tracing/thiserror + dev tempfile.
  - `lib.rs` — `Preset` 스키마 (id/version/category/display_name_ko/subtitle_ko/system_prompt_ko/user_template_ko/example_user_message_ko/example_assistant_message_ko/recommended_models[]/fallback_models[]/min_context_tokens/tags/verification/license) + `PresetCategory` enum (7종) + `requires_disclaimer()` (legal/medical only) + `load_all` 재귀 로더 + `ensure_disclaimer` (legal/medical은 변호사/disclaimer 키워드 필수) + `ensure_id_prefix` + `validate_cross_links` + `group_by_category`. 11 lib + 5 integration tests.
- **`manifests/presets/`** 5 sample (각 카테고리 1):
  - `coding/refactor-extract-method.json` (시니어 엔지니어 메서드 추출).
  - `translation/ko-en-tech.json` (한↔영 기술 번역).
  - `legal/contract-clause-review.json` (disclaimer 의무 — "변호사 상담을 권해드려요").
  - `marketing/instagram-copy.json` (hook + 본문 + 해시태그 + 광고법 회피).
  - `education/middleschool-math-tutor.json` (단계별 풀이 + 한자어 풀어쓰기).
  - 모두 `recommended_models[]`가 Phase 2'.a 카탈로그 8 시드와 cross-link 통과.
- **`Cargo.toml`** — preset-registry workspace member + path dep.
- **검증**: cargo fmt+clippy+test ✅ **374건** (이전 358 → +16: preset-registry 11 lib + 5 integration). desktop tsc ✅. **누적 vitest 139 + cargo 374 = 513건 / 0 failed**.

#### Phase 4.b ~ 4.h 잔여 + cleanup 완료 (2026-04-27, 5 sub-agent 병렬)
- **5 sub-agent 병렬 실행** — 사용자 명시 "크레딧 풀파워 + 미친듯이 빠르게" 요청 반영. 충돌 없는 단위로 분할: 각 agent는 자기 페이지/모듈만 책임, lib.rs/capabilities/App.tsx 통합은 메인이 마지막에.
- **Phase 4.b — Install 화면**: `apps/desktop/src/pages/{Install.tsx, install.css, Install.test.tsx}` + `InstallProgress` `compact?: boolean` prop 추가 (default false). 카드 2개 (Ollama/LM Studio) + drawer + 진행 패널 + 빈 상태. 22 i18n 키 + 9 vitest. 결정 노트 `phase-4b-install-screen-decision.md` (§2 기각안 3건).
- **Phase 4.c — Runtimes 화면**: `pages/{Runtimes.tsx, runtimes.css, Runtimes.test.tsx}` + `ipc/runtimes.ts` + `src-tauri/src/runtimes/{mod.rs, commands.rs}` + `permissions/runtimes.toml`. 좌 320px 어댑터 카드 column + 우 모델 VirtualList. 19 i18n 키 + 4 cargo + 8 vitest. `phase-4c-runtimes-decision.md` §2 기각안 3건.
- **Phase 4.d — Projects 화면**: `pages/{Projects.tsx, projects.css, Projects.test.tsx}` — alias prefix 그룹화 dashboard + drawer 사용량 sparkline (24 SVG rect mock). 21 i18n 키 + 8 vitest.
- **Phase 4.e — Workbench placeholder**: `pages/{Workbench.tsx, workbench.css, Workbench.test.tsx}` — hero + 5 단계 미리보기 disabled cards + 알림 받기 토글 (localStorage). 17 i18n 키 + 6 vitest.
- **Phase 4.f — Diagnostics 4-grid**: `pages/{Diagnostics.tsx, diagnostics.css, Diagnostics.test.tsx}` — 4 섹션 (자가스캔/게이트웨이 헬스/벤치/워크스페이스) + 종합 헬스 score + sparkline mock. 21 i18n 키 + 8 vitest. `phase-4ef-workbench-diagnostics-decision.md`.
- **Phase 4.g — Settings 화면**: `pages/{Settings.tsx, settings.css, Settings.test.tsx}` + `ipc/settings.ts` (localStorage helpers). 4 카테고리 nav (일반/워크스페이스/카탈로그/고급) + Gemini/STT-TTS/SQLCipher disabled slot. 40 i18n 키 + 13 vitest. `phase-4dg-projects-settings-decision.md` §2 기각안 4건.
- **Phase 4.h 잔여 — Korean preset 99+ + IPC**: `manifests/presets/{coding 14, translation 13, legal 13, marketing 15, medical 15, education 14, research 14}/*.json` = **98 신규 + 5 기존 = 103 preset 총합**. 각 system_prompt_ko 200+ 자, 페르소나+절차+원칙+형식. 의료/법률 모두 disclaimer 키워드 포함. 모든 recommended_models[]가 카탈로그 8 시드 cross-link. `src-tauri/src/presets/{mod.rs, commands.rs}` (`PresetCache` lazy 로더 + `get_presets(category?)` + `get_preset(id)` + 3 unit) + `permissions/presets.toml` + `ipc/presets.ts` + ModelDetailDrawer "이 모델 추천 프리셋" 섹션. 결정 노트 `phase-4h-presets-decision.md` §2 기각안 4건.
- **Phase 4 cleanup**: StatusPill 마이그레이션 (Home/App.tsx sidebar pill markup + main.tsx pill.css import + base.css `.sidebar-pill { margin-top: auto }`) + ko.json voice audit 7건 수정 (8원칙: "기동 중입니다…" → "켜고 있어요…", "Loading…" → "기다려 주세요" 등). `phase-4-cleanup-decision.md`.
- **메인 통합 작업** (5 sub-agent 결과를 한 번에 머지):
  - `apps/desktop/src-tauri/Cargo.toml` — `preset-registry.workspace = true` 추가.
  - `lib.rs` — `runtimes` + `presets` 모듈 등록 + `PresetCache` Arc State + 4 신규 핸들러 (`list_runtime_statuses` / `list_runtime_models` / `get_presets` / `get_preset`).
  - `capabilities/main.json` — 4 신규 권한.
  - `tauri.conf.json` — `bundle.resources`에 `manifests/presets/{7 카테고리}/*.json` 추가.
  - `App.tsx` — 6 신규 페이지 import + activeNav 분기 6개 추가 + `lmmaster:navigate` custom event listener (cross-page navigation).
- **검증**: `cargo fmt --check` ✅ / `cargo clippy --workspace --all-targets -- -D warnings` ✅ / `cargo test --workspace` ✅ **381건** (이전 374 → +7: runtimes 4 + presets 3) / `pnpm exec tsc -b` ✅ / `pnpm exec vitest run` ✅ **170건** (이전 118 → +52: Install 9 + Runtimes 8 + Projects 8 + Workbench 6 + Diagnostics 8 + Settings 13). **누적 vitest 170 (desktop) + 21 (js-sdk) + cargo 381 = 572건 / 0 failed**.

#### Phase 4 cleanup + Phase 4.5'/5' 결정 노트 부분 완료 (2026-04-28)
- **Phase 4 cleanup 잔여 마감 (메인이 직접)**:
  - `apps/desktop/src/i18n/ko.json` `model.maturity.*` 4 키 한국어화: stable→안정, beta→베타, experimental→실험, deprecated→지원 종료. en.json은 영문 그대로.
  - `manifests/presets/research/` **6 preset 신규** (5개 목표 → 6개 산출): literature-review-table / methodology-section-write / data-cleaning-plan / peer-review-feedback / reproducibility-checklist / research-question-refine. 모두 `system_prompt_ko` 200+ 자, 시니어 연구자 페르소나 + 4~6 단계 절차 + 체크리스트 / 표 형식. **Korean preset 총 109건** (이전 103 → +6).
  - en.json voice audit은 미수행 (sub-agent limit hit, 다음 세션 standby).
- **Phase 4.5' / 5' 결정 노트 (sub-agent K, L 부분 완료, limit hit 직전)**:
  - `docs/research/phase-4p5-rag-decision.md` 256줄 — 6-섹션 + ADR-0023 (RAG 아키텍처) 신설 명시. `crates/knowledge-stack` 미작성 (다음 세션 본격 진입).
  - `docs/research/phase-5p-workbench-decision.md` 233줄 — 6-섹션 + 5단계 state machine + 4 trait + Mock impl + cache layout. `crates/workbench-core` 미작성 (다음 세션 본격 진입).
  - **결정 노트의 "검증 결과" 섹션은 sub-agent가 limit hit 전에 *희망 사항*으로 미리 적은 것**. 실 cargo test 미실행. 다음 세션 진입 시 crate scaffold 본격 작성 + 실 검증.
- **Sub-agent 4건 모두 limit hit (1:20am 리셋)**: Phase 4.5' RAG / Phase 5' Workbench / Phase 6' auto-updater+Pipelines / Phase 4 cleanup 모두 limit hit. 메인이 cleanup 일부 (maturity + research preset)만 직접 처리.
- **검증**: `cargo test -p preset-registry` ✅ **16건** (preset 109개 cross-link 통과). `cargo fmt --check` ✅ / `cargo clippy --workspace -- -D warnings` ✅ / `pnpm exec tsc -b` ✅ / `pnpm exec vitest run` ✅ **170건** (변동 없음 — i18n 키 변경이 테스트에 영향 0). **누적 vitest 170 (desktop) + 21 (js-sdk) + cargo 381 = 572건 / 0 failed**.

### 🟢 다음 자동 진입 — Phase 4.5'.a / 5'.a (crate scaffold) — sub-agent 1:20am 리셋 후

**현재 상태**:
- ✅ Phase 4.5' RAG **결정 노트 + ADR-0023 권장** (256줄, sub-agent 부분 작업).
- ✅ Phase 5' Workbench **결정 노트** (233줄, sub-agent 부분 작업).
- ❌ `crates/knowledge-stack` 미존재.
- ❌ `crates/workbench-core` 미존재.
- ❌ `crates/auto-updater` 미존재.
- ❌ ADR-0023 (RAG) / ADR-0024 (Pipelines) 미작성.
- ❌ Phase 6' 결정 노트 미작성.
- ❌ en.json voice audit 미수행.

**다음 sub-phase 권장 순서** (sub-agent 1:20am 리셋 후 4 agent 병렬):
1. **Phase 4.5'.a (heavy)**: ADR-0023 작성 + `crates/knowledge-stack` 본격 scaffold (chunker NFC + Embedder trait + MockEmbedder + KnowledgeStore SQLite per-workspace + ingest_path + tests 20+).
2. **Phase 5'.a (heavy)**: `crates/workbench-core` 본격 scaffold (5단계 state machine + JSONL 4 포맷 변환 + Modelfile generator + Quantizer/LoRATrainer trait + Mock impl + 10 baseline Korean eval cases + tests 25+).
3. **Phase 6'.a (heavy)**: ADR-0024 (Pipelines) + Phase 6' 결정 노트 + `crates/auto-updater` scaffold (semver compare + UpdateSource trait + GitHubReleasesSource + Poller + tests 15+).
4. **en.json voice audit (light)**: 라인 단위 Casual + Concise 톤 통일.

각 heavy agent는 결정 노트 §2 기각안 5+건 의무 (DECISION_NOTE_TEMPLATE 준수). 메인이 통합 단계에서 workspace Cargo.toml + 검증 책임.

**자동 시퀀스**:
1. 1:20am 리셋 후 4 sub-agent 병렬 dispatch.
2. 각 agent 결과 받은 뒤 메인이 workspace Cargo.toml 통합 + cargo test 검증.
3. RESUME + 메모리 갱신.
4. Phase 4.5'.b (Tauri IPC + React UI) / Phase 5'.b (CLI subprocess 통합) / Phase 6'.b (Tauri IPC) 후속 sub-phase chain.

**진입 시 자동 시퀀스** (CLAUDE.md §7 DoD + 결정 노트 6-섹션 준수):
1. Task 생성 + in_progress.
2. 결정 노트 `phase-3p-gateway-decision.md` §1.3 (portable repair) 재활용.
3. 구현 → 5 시나리오 테스트 → 검증 → RESUME 갱신 → Phase 3'.d chain.
- **Gateway proxy** — `crates/core-gateway` 본격 라우팅: `/v1/chat/completions` (OpenAI 호환) + `/api/generate` (Ollama 호환) → 등록된 어댑터로 dispatch. 직렬화 정책 (GPU contention 방지) + SSE pass-through.
- **per-webapp scoped key** — Phase 1A.4.e의 key-manager 확장: 웹앱별 key 발급 + Origin 검증 + scope (allowed models / rate limit).
- **portable workspace fingerprint repair** — `crates/portable-workspace`: 사용자가 USB로 다른 PC에 옮겨도 fingerprint 자동 갱신 + 모델 캐시 invalidate (필요 시) + 설정 보존.
- **OpenAI-compatible SDK shape** — `packages/sdk` (TypeScript): 기존 OpenAI client 호환 인터페이스로 게이트웨이 호출. Cherry/Cursor/Open WebUI 등이 그대로 endpoint URL 변경만으로 사용 가능.
- **기존 웹앱 통합 데모** — 사용자의 기존 HTML 웹앱이 우리 gateway URL을 호출 → Korean 모델로 답변 데모.
- **API 키 발급 GUI** — Settings 화면에서 새 key 생성 / 기존 key 회수 / scope 설정.
- **G6 / G8 갭 채우기** + **ADR-0022 (게이트웨이 라우팅 정책)** 신설 — local-first / cloud-fallback / observability hooks.

**진입 시 자동 시퀀스** (CLAUDE.md §7 DoD + 결정 노트 6-섹션 준수):
1. Task 생성 + in_progress.
2. 보강 리서치 — Axum SSE proxy / OpenAI 호환 라우팅 / Tailscale fingerprint repair / per-app key scoping (Open WebUI Pipelines, AnythingLLM workspace).
3. 결정 노트 `docs/research/phase-3p-gateway-decision.md` (DECISION_NOTE_TEMPLATE 6-섹션 — 특히 §2 기각안+이유 의무).
4. ADR-0022 신설.
5. 구현 → wiremock + 통합 테스트 → 검증 → RESUME 갱신 → Phase 4 chain.

### ⏸ Phase 1' 완료 → Phase 2' 진입 (2026-04-27 — 본 세션 막바지)

Phase 2'.a 완료 시점에서 누적 290건. PC 재부팅 standby 정보는 아래 보존(이전 세션 산출물).

### ⏸ PC 재부팅 standby (2026-04-27)

사용자가 PC 재부팅 후 작업을 그대로 이어가야 함. 조치 완료:
- ✅ 모든 파일 변경은 디스크에 영속화 (Write/Edit는 atomic).
- ✅ Claude Code conversation history는 `~/.claude/projects/.../*.jsonl`에 자동 저장.
- ✅ 메모리 스냅샷 — `memory/resume_context_2026_04_27.md` 신설 (다음 자동 진입 시퀀스 명시).
- ✅ MEMORY.md 인덱스 갱신.
- ✅ CLAUDE.md / settings.json (project + user 전역) 영속.

**재부팅 후 절차**:
1. PC 재부팅.
2. VS Code 실행 + 프로젝트 폴더 열기 (`C:\Users\wind.WIND-PC\Desktop\VVCODE\LMmaster`).
3. 통합 터미널(PowerShell) 열고 `claude --continue` (가장 최근 대화 이어감) 또는 `claude --resume` (목록에서 선택).
4. 첫 응답에서 bypass 모드 자동 활성 (skipDangerousModePermissionPrompt: true). 메모리 + CLAUDE.md + RESUME.md 자동 로드.
5. 위 §"다음 자동 진입" 시퀀스대로 chain — 사용자 추가 신호 없이 즉시 진행.

**프로젝트 절대 경로**:
```
C:\Users\wind.WIND-PC\Desktop\VVCODE\LMmaster
```

**`crates/scanner` 범위**:
- `tokio-cron-scheduler` 6h cron + on-launch grace + UI on-demand 트리거.
- `summarize_via_local_llm()` — Ollama HTTP `/api/generate` `keep_alive: "30s"`, 모델 cascade (EXAONE → HCX-SEED → Qwen2.5-3B).
- LLM 미실행 / 실패 시 한국어 deterministic Korean template fallback.
- 외부 통신 0 정책 — localhost only.
- ADR-0020 (Self-scan local LLM) 구현체.
- Tauri 측: `start_scan` / `get_scan_results` IPC + `scan:summary` event.

**진입 시 자동 시퀀스**:
1. Task 생성 + in_progress.
2. 보강 리서치 Agent dispatch — 글로벌 health-check 패턴 (macOS Storage Recommendations / VS Code Doctor / Discord /health / Tailscale status pill / Vercel deployment panel) + tokio-cron-scheduler + Ollama keep_alive 패턴.
3. 결정 노트 `docs/research/phase-1p-scanner-decision.md`.
4. 구현: `crates/scanner/{Cargo.toml, src/lib.rs, src/checks.rs, src/scheduler.rs, src/llm_summary.rs, src/templates.rs, src/error.rs, tests/integration_test.rs}`.
5. 검증 + RESUME 갱신.
6. 즉시 다음 sub-phase (`crates/runtime-manager` 보강)로 chain.

**현재 누적 상태** (2026-04-27 시점):
- Crates: 16 (core-gateway, runtime-manager, runtime-detector, installer, registry-fetcher, hardware-probe, model-registry, portable-workspace, key-manager, shared-types + 5 adapter + lmmaster-desktop).
- 테스트: vitest 75 + cargo 125 = **200건 / 0 failed**.
- 산출물: ADR 21 + manifest 2 + 마법사 4 step + Command Palette + Aurora/Glass 디자인 폴리시 + 자율 실행 셋업.

**진입 조건 충족** — Phase 1A 시리즈 (1A.1 ~ 1A.4.d) 모두 완료. 마법사 + 머신 + 테스트 안전망 (75 vitest + 100 cargo = **175건 / 0 failed**) 갖춤.

**Phase 1' 권장 진입 순서** (memory `autonomous_mode.md`):
1. **`crates/registry-fetcher` 신설** ← 다음 즉시
2. `crates/scanner` 신설
3. `crates/runtime-manager` 보강 (OllamaAdapter / LMStudioAdapter 실제 attach)

**`crates/registry-fetcher` 범위**:
- Manifest 4-tier fallback — vendor API ‖ GitHub releases → jsdelivr → bundled.
- ETag / If-Modified-Since 자동 적용.
- SQLite cache + TTL 1h.
- Pinokio Verified/Community 2-tier 거버넌스 (G5 갭).
- 새 ADR 후보 — manifest registry 정책 (의존성 방향, 캐시 일관성).

**다음 세션 시작 시 자동 진행 시퀀스**:
1. Task 생성 + in_progress.
2. 보강 리서치 Agent dispatch — 글로벌 manifest fetcher 패턴 (Pinokio / Foundry / Homebrew Cask / npm registry / Helm chart repo).
3. 결정 노트 `docs/research/phase-1p-registry-fetcher-decision.md`.
4. 구현: `crates/registry-fetcher/{Cargo.toml, src/lib.rs, src/source.rs, src/cache.rs, src/etag.rs, src/error.rs, tests/integration_test.rs}`.
5. 검증: `cargo clippy --workspace` + `cargo test --workspace`.
6. RESUME 갱신 + 즉시 다음 sub-phase (`crates/scanner`)로 chain.

**현재 누적 상태**:
- 산출물: ADR 21건 + crate 14개 + manifest 2건 + Tauri capability/permissions 셋업 + 마법사 4 step 완전 동작 + Command Palette + 자율 실행 셋업 (settings.json + CLAUDE.md + 6 헬퍼 스크립트).
- 테스트: vitest 75 (machine 22 + persistence 6 + filter 10 + bridge 4 + Step1 5 + Step2 4 + Step3 6 + Step4 2 + CommandPalette 6 + a11y 10) + cargo 100 = **175건 / 0 failed**.
- 디자인: Aurora + Glass + Spotlight 토큰 + 해요체 i18n 200+ 키 + Korean jamo cheat keywords.

**Phase 1A.4.d 후보**:
- vitest + @testing-library/react + @tauri-apps/api/mocks 셋업
- machine.ts transition 단위 테스트 (xstate-test 또는 actor.subscribe 검증)
- Step 1~4 컴포넌트 테스트 (IPC mock + Channel mock + 에러 boundary fallback + cancelInstall 호출 확인)
- axe-core WCAG 2.2 AA 자동 검증
- 1A.4.c에 노출된 작은 잔여 — OpenedUrl outcome 시 자동 done 대신 manual NEXT 분기 substate 추가, install.failed에서 stale outcome 영향 차단

**Phase 2' 후보** (대 페이즈):
- `crates/registry-fetcher` — manifest 4-tier fallback (vendor API ‖ GitHub releases → jsdelivr → bundled), ETag/If-Modified-Since, SQLite cache TTL 1h
- 카테고리 큐레이션 카탈로그 5×5~10 (한국어 1순위 EXAONE/HCX-SEED/Polyglot-Ko)
- 2-tier 거버넌스 (G5)
- 하드웨어-인지 배지 + 30s 벤치 (G4)

**권장 순서**: 1A.4.d (테스트 안전망) → Phase 1' 잔여 (scanner/registry-fetcher) → Phase 2' (카탈로그). 1A.4.d 없이 Phase 2'로 직진하면 회귀 위험 큼.

설계는 확정됐으나, 구현은 토큰 한계로 다음 세션으로 분리한다(완성도 우선 정책).

#### Phase 1' 구현 산출물 (구체 작업 목록)

**A. Rust crate 신설 / 보강**
1. `crates/runtime-detector` (신설)
   - `detect_ollama()` — `GET http://127.0.0.1:11434/api/version` (1.5s timeout) + fallback `which ollama`.
   - `detect_lm_studio()` — `GET http://127.0.0.1:1234/v1/models` (1.5s) + fallback `lms version` + 레지스트리 `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\<GUID>`.
   - `detect_environment()` — Win 레지스트리 (NVIDIA driver / CUDA / WebView2 / VC++ 2022) + mac (`uname -m`, Metal) + Linux (glibc, libstdc++, libcuda).
   - `detect_gpu()` — nvml-wrapper(NVIDIA) + sysinfo(나머지).
   - 단위 테스트 + 통합 테스트.
2. `crates/installer` (신설)
   - manifest 기반 `install(app: &str)` — manifest의 platform 분기 + download + sha256 verify + atomic rename + spawn.
   - `tauri-plugin-shell::Command::new` 사용. capability JSON에서 명시적으로 허용된 cmd만.
   - mid-flight 실패 복구: `*.partial` + 1회 silent retry + Squirrel-style blue/green.
3. `crates/registry-fetcher` (신설)
   - `fetch_with_fallback(sources: Vec<Source>) -> Manifest` — 4-tier 폴백 (vendor API ‖ GitHub releases → jsdelivr → bundled).
   - ETag/If-Modified-Since 자동 적용.
   - 결과 SQLite cache + TTL 1h.
4. `crates/scanner` (신설)
   - `tokio-cron-scheduler` 6h cron + on-launch grace + UI on-demand.
   - `summarize_via_local_llm(input)` — Ollama HTTP `/api/generate` `keep_alive: "30s"` 사용. 모델 cascade(EXAONE → HCX-SEED → Qwen2.5-3B).
   - LLM 미설치 → deterministic 한국어 템플릿.
   - 결과 `app.emit("scan:summary", ...)`.
5. `crates/runtime-manager` (보강)
   - 어댑터 우선순위: OllamaAdapter / LMStudioAdapter 1순위 실제 attach 구현.
   - LlamaCppAdapter는 unimplemented 유지 (Phase 6' 또는 v1.x).

**B. Manifest 파일**
1. `manifests/apps/ollama.json` — Pinokio-style declarative (detect / install / update).
2. `manifests/apps/lm-studio.json` — `redistribution_allowed: false`, install은 `open_url`만.
3. `manifests/snapshot/` — 빌드 시 stale-but-known-good snapshot (CI에서 자동 갱신).
4. `manifests/prompts/scan-summary.json` — 자가스캔 LLM 프롬프트.

**C. Tauri 보강**
1. `tauri-plugin-shell` 추가 + capability JSON에 strict ACL.
2. `tauri-plugin-http` 추가 (manifest fetch + installer download).
3. `tauri-plugin-updater` 추가 + 사인 키페어 생성 안내.
4. tauri commands: `detect_environment`, `install_app(id)`, `start_scan`, `get_scan_results`, `get_install_progress`.
5. event 채널: `install://progress`, `scan:summary`, `update://available`.

**D. React (apps/desktop/src) 신설**
1. `pages/Onboarding.tsx` — 4단계 stepper (언어 / 환경 점검 / 첫 모델 / 완료).
2. `pages/Home.tsx` — gateway pill + 추천 카드 + 자가스캔 결과 표시.
3. `components/InstallProgress.tsx` — stepper + size/ETA + 접힘식 "자세히 보기".
4. `components/ToastUpdate.tsx` — JetBrains Toolbox 스타일 "다음 실행 때 적용돼요".
5. `i18n/ko.json` 키 확장: `onboarding.*`, `install.*`, `scan.*`, `update.*`. 해요체 일관 적용.

**E. 패키지 설정**
1. `apps/desktop/package.json`에 `@tauri-apps/plugin-{shell,http,updater}` 추가.
2. `apps/desktop/src-tauri/Cargo.toml`에 `tauri-plugin-{shell,http,updater}` 추가.
3. workspace dependencies에 `tokio-cron-scheduler = "0.13"`, `nvml-wrapper`, `winreg` (Win) 등 추가.

**F. 검증**
1. `crates/runtime-detector/tests/` — mock HTTP 서버로 detect 테스트.
2. `crates/installer/tests/` — mock manifest로 install path 테스트 (실제 spawn은 dry-run).
3. `crates/scanner/tests/` — fallback 한국어 템플릿이 LLM 미동작 시 valid string 반환.
4. e2e: dev 실행 시 4단계 마법사 정상 표시.

#### 4 sub-phase 분할 (Phase 1A 전체)

- ✅ **Phase 1A.1**: 보강 리서치 + ADR-0021 + manifest 2건 + runtime-detector HTTP probe + 통합 테스트 9건.
- ✅ **Phase 1A.2.a**: 보강 리서치(GPU/Win 레지스트리/mac+Linux 3영역) + hardware-probe 전체 재작성 (wgpu drop → DXGI 직접 사용) + 통합 테스트 10건.
- ✅ **Phase 1A.2.b**: 2영역 보강(manifest evaluator + ash Vulkan) + manifest evaluator 11건 + Vulkan probe 5건 + NVML 레지스트리 fallback + libstdc++ symbol probe.
- ✅ **Phase 1A.3.a**: `crates/installer` 신설 + production resumable downloader + 7건 통합 테스트.
- ✅ **Phase 1A.3.b.1**: AppManifest install/update 스키마 + ActionExecutor (download_and_run + open_url) + 11건 lib 테스트.
- ✅ **Phase 1A.3.b.2**: download_and_extract (zip/tar.gz, spawn_blocking + AtomicBool cancel + dual zip-slip 방어) + shell.curl_pipe_sh (Linux/macOS 게이트 + injection 방어) + post_install_check 실평가 (HTTP polling + cancel + timeout) + 17건 추가 테스트.
- ✅ **Phase 1A.3.b.3**: dmg 추출 (macOS 전용 — `hdiutil attach -mountrandom` + plist 파싱 + `MountGuard` Drop으로 detach 보장 + `/usr/bin/ditto` 복사 + 100ms `try_wait` cancel polling) + plist 파싱 unit 3건 + `#[ignore]` macOS 통합 1건.
- ✅ **Phase 1A.3.c**: `tauri::ipc::Channel<InstallEvent>` IPC + `install_runner::run_install` 순수 함수 + `InstallRegistry` (id↔CancellationToken) + capability TOML/JSON ACL + bundled manifest resource + TS discriminated-union 미러 + 26건 추가 테스트.
- ✅ **Phase 1A.4.a**: 마법사 골격 — xstate v5 머신 + `createActorContext` Provider + localStorage 동기 hydrate + Ark UI Steps + per-step ErrorBoundary + Framer Motion + 해요체 i18n + Step 1(언어) + Step 4(완료) + App.tsx 게이팅.
- ✅ **Phase 1A.4.b**: Step 2 환경 점검 — `runtime-detector::probe_environment` 합성 함수 + `detect_environment` Tauri command + capability 갱신 + xstate `fromPromise(scan)` actor + always 캐시 분기 + RETRY 흐름 + 4 카드 UI + 한국어 카피.
- ✅ **Phase 1A.4.c**: Step 3 런타임 설치 — xstate `fromPromise(install)` actor + signal.abort → cancelInstall + module-scope event bridge + 5 substates (decide/skip/idle/running/failed) + Ollama/LM Studio 카드 + InstallProgress (debounce/ETA/speed/details log) + Failed 14 에러 한국어 매핑 + already-running 자동 SKIP. 모델 큐레이션은 Phase 2'로 분리.
- ✅ **Phase 1A.4.e**: Polish & Palette — Aurora glow + Glass surface + Spotlight 토큰 + 마법사/카드 적용 + Command Palette 신설 (Ark UI Combobox + ⌘K/Ctrl+K 글로벌 hotkey + 한국어 substring + jamo cheat keywords + framer-motion 애니메이션) + wizard 4건 + MainShell 4건 시드 명령.
- ✅ **Phase 1A.4.d.1**: vitest 2.1 + jsdom 25 + RTL 16 인프라 + Issue A (OpenedUrl substate) + Issue B (running entry clearInstallState) fix + 머신 22 + persistence 6 + filter 10 + bridge 4 = 42 vitest.
- ✅ **Phase 1A.4.d.2**: 컴포넌트 테스트 5 file (Step1~4 + CommandPalette) — context hooks vi.mock + jsdom polyfill = +23 (누적 65).
- ✅ **Phase 1A.4.d.3 (이번 세션 마지막)**: vitest-axe 도입 + 9 컴포넌트 substate + CommandPalette WCAG 자동 검증 = +10 (누적 75).
- ✅ **Phase 5'.a (2026-04-28)**: `crates/workbench-core` production-grade scaffold — error/jsonl/modelfile/quantize/lora/eval/flow 7 모듈 + ADR-0023 신설. 산출 = 76 unit + 5 integration = 81 tests pass / clippy 0 / fmt 0. trait-first 5단계 state machine (Data→Quantize→LoRA→Validate→Register) + Mock impl 2종 + 4 포맷 JSONL 자동 변환 + Modelfile generator (escape + multi-stop) + Korean QA evals 10 baseline (factuality 4 + instruction-following 3 + tone-korean 3). workspace `Cargo.toml` member + path dep 추가.
- ✅ **Phase 4.5'.a (2026-04-28)**: `crates/knowledge-stack` production-grade scaffold — error/chunker/embed/store/ingest 5 모듈 + ADR-0024 신설. 산출 = 47 unit + 9 integration = 56 tests pass / clippy 0 / fmt 0. NFC normalize chunker (문단/문장/char-window) + Embedder trait + MockEmbedder (sha256-deterministic 384dim) + KnowledgeStore (rusqlite per-workspace + cosine in-memory ranking) + IngestService (mpsc progress + cancel). Decision note 256줄. workspace `Cargo.toml` member + path dep 추가.
- ✅ **Phase 6'.a (2026-04-28)**: `crates/auto-updater` production-grade scaffold + ADR-0025 (Pipelines architecture) + ADR-0026 (Auto-Updater source). 산출 = 44 unit + 7 integration + 1 doctest = 52 tests pass / clippy 0 / fmt 0. UpdaterError 6 variants (해요체) + is_outdated semver compare (build metadata strip — Rust crate Ord와 spec 차이 보정) + UpdateSource trait + GitHubReleasesSource + MockSource (set_release/set_force_error/call_count) + Poller (CancellationToken + 6h interval + dedup last_notified). 결정 노트 261줄(Pipelines + Auto-Updater 합본). semver = "1" workspace dep 추가.
- ✅ **Phase 4 cleanup 잔여 (2026-04-28)**: en.json voice audit — 427 키 중 ~57건 톤 보정 (Linear/Stripe casual+concise+active voice). apologetic openers / question-form CTA / passive / wordy phrasing / 의문문 호명 제거. 사과체 9 + 의문문 7 + please 5 + passive 4 + wordy 10 + gerund 6 + punctuation 16. JSON 구조 보존, 키 추가/삭제 0. ko.json 영향 없음.
- ✅ **Phase 5'.b/4.5'.b 보강 리서치 (2026-04-28)**: `docs/research/phase-5pb-4p5b-ipc-reinforcement.md` 351줄 + 14 외부 인용. Channel<T> + Mutex<HashMap<id, CancellationToken>> + scopeguard Drop + RunEvent::ExitRequested cancel_all + app-local TOML — Phase 1A.3.c install + Phase 2'.c.2 bench와 동일 패턴 검증. 신규 추상화 도입 없이 replicate. ingest 어댑터: 기존 mpsc<IngestProgress> → Channel<IngestEvent> bridge task (installer DownloadBridge 패턴). registry key: workbench=`run_id` UUID / ingest=`workspace_id` (per-workspace SQLite write 직렬화).
- ✅ **Phase 5'.b (2026-04-28)**: Workbench Tauri IPC + 5-step React UI 승격. `src-tauri/src/workbench.rs` (1270 LOC, 28 unit tests) + `permissions/workbench.toml` 5 permissions + `ipc/workbench.ts` (136 LOC) + `Workbench.tsx` (875 LOC) + `Workbench.test.tsx` (12 vitest, axe a11y 통과) + `workbench.css` 590 LOC. Channel<WorkbenchEvent> kebab-case tagged enum + `WorkbenchRegistry` (run_id ↔ CancellationToken) + `RunEvent::ExitRequested` cancel_all. 5단계 stepper는 Ark UI `Steps` 대신 semantic `<button>` 리스트(a11y `aria-required-children` 위반 회피). i18n `screens.workbench.*` ko/en 동기 갱신.
- ✅ **Phase 6'.b 백엔드 (2026-04-28)**: `crates/pipelines` (PipelineError 5 + Pipeline trait + PipelineChain forward/reverse + PiiRedactPipeline 4 패턴 OnceLock regex + TokenQuotaPipeline + ObservabilityPipeline) + `core-gateway/src/pipeline_layer.rs` (PipelineLayer Tower middleware + SSE byte-perfect pass-through + OpenAI envelope error mapping). 산출 = 40 unit + 6 integration + 6 layer = **52 tests pass / 0 clippy / 0 fmt**. ADR-0025 §1 trait method `pre_request/post_response` → spec 따라 `apply_request/apply_response` (axum service style). PromptSanitize는 v1.x 이월(PII redact가 thesis #7 가치 우선).
- ✅ **Phase 4.5'.b (2026-04-28)**: Knowledge Stack Tauri IPC + Workspace UI Knowledge tab. `src-tauri/src/knowledge.rs` 890 LOC + 32 unit tests + permissions/knowledge.toml + ipc/knowledge.ts 150 LOC + Workspace.tsx 535 LOC + Workspace.test.tsx 13 vitest + workspace.css 290 LOC. **Registry key = workspace_id**(NOT ingest_id) — single ingest per workspace로 SQLite write 직렬화. Dual cancel signal (CancellationToken bridge + AtomicBool ingest cooperative). mpsc → Channel bridge `tauri::async_runtime::spawn`. 5 commands: `ingest_path / cancel_ingest / search_knowledge / list_ingests / knowledge_workspace_stats`. i18n `screens.workspace.*` ko/en 동기 갱신. 미해결: `KnowledgeStore::get_document_path` 메소드 부재로 SearchHit.document_path는 placeholder(document_id)— v1.1.
- ✅ **Phase 6'.b UI (2026-04-28)**: Auto-updater Tauri IPC + Settings 자동 갱신 토글 + JetBrains-style ToastUpdate. `src-tauri/src/updater.rs` 840 LOC + 32 unit tests + permissions/updater.toml + ipc/updater.ts 115 LOC + ToastUpdate.tsx rewrite (props release/currentVersion/onSkip/onDismiss + localStorage `lmmaster.update.skipped.{version}` + Esc + 한국어 날짜) + ToastUpdate.test.tsx 10 vitest + Settings.tsx AutoUpdatePanel (toggle + 1h/6h/12h/24h interval radio + "지금 확인할게요") + Settings.test.tsx +11 vitest. 5 commands: `check_for_update / cancel_update_check / start_auto_update_poller / stop_auto_update_poller / get_auto_update_status`. interval_secs 검증 [3600, 86400] (ADR-0026 §3). `@tauri-apps/plugin-shell` 미사용 → `window.open` 사용 (v1.x 후속). Last-checked는 outdated 콜백에서만 갱신(Poller 콜백 한정).
- ✅ **Phase 5'.c+5'.d (2026-04-28)**: Workbench Validate(bench-harness) + Register(model-registry) 실 연동.
  - `workbench-core/src/eval.rs`: `Responder` trait + `MockResponder` + `run_eval_suite`(cancel-aware) + 12 신규 unit tests (88 total).
  - `bench-harness/src/workbench_responder.rs` 신설(95 LOC, 4 tests, 27 total) — `WorkbenchResponder` adapter, v1은 MockResponder delegate, Phase 5'.e HTTP 후속.
  - `model-registry/src/register.rs` 신설(370 LOC, 13 tests, 58 total) — `ModelRegistry` (in-memory + disk JSON `app_data_dir/registry/custom-models.json`) + `CustomModel` + Korean error variants.
  - `workbench.rs`: `run_stage_validate`가 `run_eval_suite` 사용 + `EvalCompleted` 신규 event + summary `eval_report` 필드. `run_stage_register`가 `ModelRegistry::register` 호출 + `RegisterCompleted { model_id }` event. 신규 command `list_custom_models`. `RegistryFailed` error variant. workbench tests 30 (+2 net).
  - `Workbench.tsx` ValidateStep: per-case pass/fail badge + by-category aggregate + 정답률 라벨. RegisterStep: registered model id + "모델 카탈로그에서 확인할게요" 링크 (`#/catalog`).
  - i18n `screens.workbench.{validate,register}.*` 확장 (ko/en 동기).
  - Open: 실 HTTP wiring(Ollama/LM Studio 호출)은 Phase 5'.e. `ollama create -f Modelfile` shell-out도 5'.e. Catalog가 `listCustomModels`로 사용자 정의 모델 노출은 Catalog UI 후속.
- ✅ **Phase 6'.c (2026-04-28)**: Pipelines UI — Settings 토글 + 감사 로그 viewer. `src-tauri/src/pipelines.rs` 767 LOC + 32 unit tests + permissions/pipelines.toml + ipc/pipelines.ts 86 LOC + PipelinesPanel.tsx 368 LOC + pipelinesPanel.css 228 LOC + PipelinesPanel.test.tsx 14 vitest + Settings.test.tsx +2 smoke. 5 commands: `list_pipelines / set_pipeline_enabled / get_pipelines_config / get_audit_log / clear_audit_log`. RingBuffer cap 200 + atomic JSON 디스크 persist + corrupted JSON graceful fallback. `record_audit` helper 노출 (Phase 6'.d gateway wiring 후속). 감사 로그 wrapper `<section role="log" aria-live="polite">` + `<ol>` 자식 (axe `<li>` 부모 list role 요구 + role="log" override 충돌 회피).
- ✅ **Phase 5'.e + 7' 보강 리서치 (2026-04-28)**: `phase-5pe-runtime-http-reinforcement.md` 398 LOC + 22 인용 / `phase-7p-release-prep-reinforcement.md` 492 LOC + 24 인용. 합계 46 외부 docs/repos. 핵심 결정: 5'.e는 `/api/generate` + stream:false + 60s timeout + backon 5xx-only retry / LM Studio `GET /v1/models` 사전 polling / `kill_on_drop(true)` shell-out / fixture binary CI 패턴. 7'는 NSIS perUser + OV cert + dmg + AppImage + minisign updater + Korean EULA clickwrap + opt-in telemetry / 본체 tauri-plugin-updater(self-update)와 ADR-0026 auto-updater(외부 추적) endpoint 분리.
- ✅ **Phase 5'.e (2026-04-28)**: 실 HTTP runtime wiring + ollama create shell-out.
  - `crates/bench-harness/src/workbench_responder.rs` 100→987 LOC: `RuntimeKind` 4-variant + `ResponderConfig`(timeout 60s / max_retries 2 / temperature 0.0 / num_ctx 2048 / connect_timeout 5s / no_proxy) + Ollama `/api/generate` (stream:false + keep_alive:5m) + LM Studio `/v1/chat/completions` (사전 `/v1/models` polling으로 model 미로드 진단) + backon ExponentialBuilder 5xx/429 only retry / 4xx 즉시 실패. wiremock 24 신규 tests.
  - `apps/desktop/src-tauri/src/workbench.rs` +330 LOC: `OllamaCreate{Started,Progress,Completed,Failed}` 4 신규 event variants + `build_responder()` + `run_ollama_create_stage()` (Modelfile 디스크 작성 → `tokio::process::Command::new("ollama").arg("create"...)` + kill_on_drop(true) + 60s timeout + cancel cooperative + stdout/stderr 라인별 emit) + stderr→한국어 매핑 6 시나리오 + `MOCKED_OLLAMA_PATH` env override (CI에서 ollama 의존 0). 24 신규 tests.
  - `apps/desktop/src/pages/Workbench.tsx` +160 LOC: `RuntimeSelector` 컴포넌트(radio Mock/Ollama/LM Studio + base_url + model_id + register_to_ollama checkbox) + state `ollamaCreateLines`/`ollamaOutputName` + RegisterStep ollama-create-log 표시. +4 vitest.
  - `WorkbenchConfig`에 `responder_runtime / responder_base_url / responder_model_id` 3 필드 추가 + `Default` derive. flow.rs 수정.
  - i18n `screens.workbench.runtime.*` + `screens.workbench.register.ollamaCreate.*` 11 키 ko/en 동기.
  - 검증: cargo 816 / vitest 232 / 0 failed. clippy 0 warnings (collapsible_if + default_constructed_unit_structs + dead_code 3건 fix).
  - Open: `fake_ollama` Rust binary fixture는 미생성, env override만 사용 — Phase 5'.f에서 LlamaQuantizer fixture와 함께 도입. `keep_alive` 5m 하드코딩(v1.x config 노출). 사용자 동의 dialog는 Phase 7' EULA 흐름과 통합.
- 🧹 **docs 정리 (2026-04-28)**: `docs/RESUME.md` 903→112줄 (Claude attention 최적화 / `<300줄`). `docs/PROGRESS.md` 신설(97줄, 6 pillar dashboard). `docs/CHANGELOG.md` 신설(909줄, 본 시간순 이력). `docs/adr/README.md` 0022~0026 추가. 엘리트 ref: HumanLayer CLAUDE.md 가이드 / Anthropic best practices ("context injection 파일은 짧게, 상세는 별도").
- ✅ **Phase 6'.d (2026-04-28)**: Gateway audit wiring — `PipelineLayer` ↔ `PipelinesState` 라이브 mpsc 채널.
  - `crates/pipelines/src/pipeline.rs` +50 LOC: `AuditEntry::timestamp_iso()` (RFC3339, cross-crate boundary 변환용 — Tauri DTO 의존 회피). +4 unit tests.
  - `crates/core-gateway/src/pipeline_layer.rs` +250 LOC: `PipelineLayer`에 `Option<mpsc::Sender<AuditEntry>>` + `with_audit_channel()` 빌더 + `drain_audit()` (try_send 전용, Full/Closed 모두 `tracing::warn!` + drop, 절대 block 안 함). 4 drain 시점 wired (apply_request 성공 / 에러 short-circuit / apply_response 성공 / 에러). +7 unit tests.
  - `crates/core-gateway/src/lib.rs` +95 LOC: `with_pipelines_audited(router, chain, audit_sender)` 신규 빌더. `with_pipelines` 그대로 유지(역호환). +2 tests.
  - `apps/desktop/src-tauri/src/pipelines.rs` +210 LOC: `AUDIT_CHANNEL_CAPACITY = 256` + `AuditEntryDto::from_audit_entry` + `From<AuditEntry>` impl + `audit_task: Mutex<Option<JoinHandle>>` 필드 + `with_audit_channel()` 헬퍼(idempotent — 재호출 시 이전 task abort). Drop impl로 task 정리. +7 tests.
  - `apps/desktop/src-tauri/src/gateway.rs` +50 LOC: `gateway::run`이 `audit_sender` 받음 + `build_chain_from_state(&app)` (PipelinesState config snapshot 읽어 v1 시드 3종 chain 빌드 + Phase 6'.e에서 hot-reload 예정).
  - `apps/desktop/src-tauri/src/lib.rs` +5 LOC: setup 단계에서 `pipelines_state.with_audit_channel()` → sender 주입.
  - 검증: cargo 837 / vitest 232 / 0 failed. clippy 0 warnings. fmt clean.
  - Open: Phase 6'.e — `set_pipeline_enabled` 시 chain hot-reload (현재 boot snapshot only) + per-key matrix activation (ADR-0025 §3).
- ✅ **Phase 7'.a (2026-04-28)**: v1 Release scaffold (사용자 결정 블로커 없는 부분).
  - `tauri.conf.json` bundler 매트릭스 — NSIS `currentUser` (perUser는 schema에 없어서 의미 동일한 currentUser 사용) + dmg 11.0+ + AppImage. createUpdaterArtifacts: true. plugins.updater placeholder pubkey + endpoint. entitlements.plist 참조. license / publisher / copyright.
  - `apps/desktop/src-tauri/entitlements.plist` 신설 (22 LOC) — mac codesign (network client/server, sandbox off, JIT 거부, library validation disable).
  - tauri-plugin-updater 통합 — Cargo.toml `tauri-plugin-updater = "2"` + package.json `@tauri-apps/plugin-updater ^2.0.0` + lib.rs `Builder::new().build()` plugin 등록 + capabilities `updater:default`.
  - `apps/desktop/src-tauri/src/telemetry.rs` 신설 (230 LOC, +8 tests) — TelemetryConfig (enabled / anon_id / opted_in_at) + 디스크 영속 `app_data_dir/telemetry/config.json` + 첫 활성 시 anonymous UUID 발급. **실제 전송 미구현 (Phase 7'.b GlitchTip endpoint 후속)** — config + UUID 관리만.
  - `EulaGate.tsx` 신설 (310 LOC, +11 vitest) — 첫 실행 EULA 클릭랩 + version-bound localStorage `lmmaster.eula.accepted.{version}` + 한국어/English 토글 + 스크롤 끝까지 도달해야 동의 버튼 enable (다크 패턴 회피) + focus-trap + Esc 비활성. minimal markdown renderer (react-markdown 의존성 회피, h1~h3 + ul + bold + code + escapeHtml + XSS 단언 테스트).
  - `eula-{ko,en}-v1.md` placeholder (40 LOC each) — 7 섹션 + TODO 마커 (적용 시점 / §5 책임 한계 / 공식 이메일).
  - `TelemetryPanel.tsx` 신설 (125 LOC, +8 vitest) — Settings 토글 + 한국어 명시 ("기본은 비활성. 활성하면 익명 사용 통계만 보내요. 프롬프트·모델 출력은 절대 전송 안 해요").
  - `App.tsx` — `<EulaGate eulaVersion="1.0.0">` 래핑 (Onboarding/MainShell 앞).
  - `Settings.tsx` — `<TelemetryPanel />` 추가.
  - i18n `screens.eula.*` (12 keys) + `screens.settings.telemetry.*` (10 keys) ko/en 동기.
  - **ADR-0027 신설** — bundler / 사인 / EULA / telemetry 정책 (Phase 7'.a). 기각: MSI vs NSIS / Sentry SaaS vs GlitchTip self-hosted / EULA always-show vs version-bound.
  - 검증: cargo 845 / vitest 251 / 0 failed. clippy 0. fmt 0. tsc 0. vite build 679 modules / 2.21s.
  - **사용자 결정 TODO 6건**: OV cert thumbprint / Apple Developer signingIdentity / minisign pubkey / repo URL / EULA 법무 검토 / publisher명.
  - Open: Phase 7'.b — release.yml CI matrix + minisign 자동 서명 + GlitchTip endpoint + 베타 토글 + README 다국어 + ADR-0028(audit channel) + ADR-0029(GlitchTip).
- 📊 **누적 검증 (2026-04-28 최종 — v1 코드 100%)**: cargo 845 + vitest 251 = **1096 tests / 0 failed**. Crates 22. ADR 0001~0027. 결정 노트 34건. **v1 코드 완료** — 사용자 결정 6건 + Phase 7'.b CI 자동화만 남음.
- 🧹 **stale .js 청소 (2026-04-28)**: `tsc -b --clean` 누락으로 `src/pages/Workbench.js` 등 컴파일 산출물이 vitest 모듈 해상도를 오염시킴 → 해결: 모든 `src/**/*.js` (test 포함) 삭제 후 재빌드. 다음 세션 trap 노트.
- ⏳ **Phase 1A.4**: React 첫실행 마법사 — `@ark-ui/react` Steps + xstate v5 + react-error-boundary + 4 stage 한국어 카피(해요체) + 디자인 토큰 적용 + e2e 검증.

### 다음 standby (2026-04-28)

**Phase 5'.b — Workbench Tauri IPC + React UI 승격**:
- `start_workbench_run(config) -> run_id` (tauri::command + Channel<WorkbenchEvent>).
- `cancel_workbench_run(run_id)` (CancellationToken registry lookup).
- `list_workbench_runs() -> Vec<RunSummary>`.
- capabilities/main.json 4 신규 permission.
- React `pages/Workbench.tsx` 5-step state machine UI (현재 placeholder).

**Phase 4.5'.b — Knowledge Stack Tauri IPC + Workspace UI 통합**:
- `ingest_path(workspace_id, path, opts) -> ingest_id` (Channel<IngestEvent>).
- `cancel_ingest(workspace_id)`.
- `search_knowledge(workspace_id, query, k) -> Vec<SearchHit>`.
- React Workspace banner + Knowledge tab 신설.

**Phase 6'.b — Pipelines / Auto-Updater UI**:
- `Pipeline` trait 3종 v1 시드 (PII redact / token quota / observability).
- gateway `PipelineLayer` middleware.
- `check_for_update / start_auto_update_poll / cancel_auto_update_poll` IPC.
- React 토스트 + Settings 자동 갱신 토글.

각 sub-phase 끝에 `cargo test` + dev 실행으로 검증, RESUME 갱신. 토큰 한계 시 추가 sub-sub-phase 분할 허용.

### 검증 명령 (Phase 0 + Phase 1' sub-phase 종료 시)

```bash
pnpm install
cargo fetch
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
pnpm -r build
pnpm --filter @lmmaster/desktop tauri dev
```

기대 (Phase 1A 완료 시):
- 첫 실행 시 한국어 4단계 마법사 표시.
- 환경 점검 단계에서 Ollama / LM Studio 감지 결과 표시.
- 미설치 시 silent install (Ollama) 또는 공식 페이지 안내 (LM Studio) 한국어 안내.

### 인계 사항
- Phase 1A.2를 시작하려면: **"Phase 1A.2 시작"** 메시지. 새 세션에서 메모리 자동 로드 + 이 RESUME 참조.
- Phase 1A.2의 가장 긴급한 보강 리서치 영역: nvml-wrapper graceful-fail 동작 확인 + wgpu 29 `Instance::enumerate_adapters` cross-vendor 정확도 + Win 레지스트리 32/64-bit redirection (KEY_WOW64_64KEY) + Pinokio manifest의 detect 메소드 표준화 (shell.which / registry.read / fs.exists / http.get 통합 evaluator).
- 환경 빌드 검증은 재현 가능: cargo + pnpm 설치 후 `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace` (vcvars64 경유, RESUME §사전 설치 참조).
