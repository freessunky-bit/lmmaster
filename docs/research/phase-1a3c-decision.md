# Phase 1A.3.c — Tauri Channel + IPC 결정 노트

> 보강 리서치 (2026-04-26) 종합. Tauri 2.10.x Channel<T> + capability ACL + in-flight install registry.

## 1. 핵심 결정

| 항목 | 결정 | 근거 |
|---|---|---|
| 이벤트 채널 | `tauri::ipc::Channel<InstallEvent>` (`Emitter::emit` 아님) | per-invocation 스코프, ordered, 자체 IPC 큐. Tauri 공식 권장 (calling-frontend/) |
| 이벤트 enum 형태 | `#[serde(tag = "kind", rename_all = "kebab-case")]` 7-변형 union | 프론트 discriminated union 깔끔 |
| `DownloadEvent` 합성 | wrapper struct `Download { download: DownloadEvent }` | 내부 tag 충돌 회피 (이미 `kind` field 존재) |
| `Channel::send` | **sync 호출**, 30~50ms IPC eval (큰 payload 시) | source `crates/tauri/src/ipc/channel.rs` 확인. 우리 페이로드는 <1KB라 무시 가능 |
| 이벤트 throttle | 기존 256KB / 100ms 다운로드 throttle 유지 | downloader 레벨에서 이미 처리 |
| 채널 close 감지 | `channel.send().is_err()` → `cancel.cancel()` 후 `Err(ChannelClosed)` | window 닫힘 = 사용자 이탈 → install 종료 |
| 프론트 Channel API | `import { Channel } from '@tauri-apps/api/core'`, `new Channel<InstallEvent>()` + `ch.onmessage = ...` | Tauri 2.10.x 공식 |
| 권한 모델 | app-defined permissions in `permissions/install.toml` + `capabilities/main.json` 명시 allowlist | `removeUnusedCommands = true` 호환 + 미래 보안 강화 (default-allow는 deprecated 흐름) |
| In-flight registry | `Mutex<HashMap<String, CancellationToken>>` `app.manage(...)` | tokio Mutex 불필요 (sync 락만 잠깐) |
| 동시 install 같은 id | 거부 (`AlreadyInstalling` 에러) | 사용자 예측 가능, 대부분 installer 패턴 |
| Cancel-all | `RunEvent::ExitRequested`에서 `registry.cancel_all()` | `CancellationToken::Drop`은 cancel 안 함 — 명시 호출 필수 |
| Manifest 소스 | bundled `manifests/apps/*.json` via `tauri.conf.json` `bundle.resources` + `app.path().resolve(..., BaseDirectory::Resource)` | dev/prod 모두 동작. 원격 fetch는 Phase 1' 후순위 |
| 순수 함수 분리 | `crates/installer::install_runner::run_install(manifest, cache_dir, cancel, sink) -> Result<ActionOutcome>` | Tauri command는 30줄 shim — 테스트 용이 |
| RAII 가드 | `scopeguard` crate — `defer!` 매크로로 registry.finish() 보장 | 조기 `?` 누락 방지 |
| 테스트 | wiremock + tempdir + `Vec<InstallEvent>` capture sink로 pure-function 테스트. Tauri command 자체는 mock_app 안 함 (가성비 낮음) | |
| Specta TS 바인딩 | 이번 sub-phase는 manual TS 미러 (`apps/desktop/src/ipc/install-events.ts`) + serde JSON contract test. specta 통합은 후순위 | ADR-0015 명시는 했으나 미통합 |

## 2. 새 타입

### `crates/installer/src/install_event.rs`

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum InstallEvent {
    Started { id: String, method: String, display_name: String },
    Download { download: DownloadEvent },
    Extract { phase: ExtractPhase, entries: u64, total_bytes: u64 },
    PostCheck { status: PostCheckStatus },
    Finished { outcome: ActionOutcome },
    Failed { code: String, message: String },
    Cancelled,
}
```

### `crates/installer/src/install_runner.rs`

`run_install(manifest, cache_dir, cancel, sink) -> Result<ActionOutcome, InstallRunnerError>`:
1. `manifest.install.for_current_platform()` → method 결정
2. `Started` emit
3. method별 dispatch — ActionExecutor::execute() 호출, 진행 이벤트는 sink로 전달
4. 종료 시 `Finished` 또는 `Failed` 또는 `Cancelled` emit
5. ActionOutcome 반환

### `apps/desktop/src-tauri/src/install/mod.rs`

```rust
#[tauri::command]
pub async fn install_app(
    app: tauri::AppHandle,
    registry: tauri::State<'_, InstallRegistry>,
    id: String,
    channel: tauri::ipc::Channel<InstallEvent>,
) -> Result<ActionOutcome, InstallApiError> { ... }

#[tauri::command]
pub fn cancel_install(
    registry: tauri::State<'_, InstallRegistry>,
    id: String,
) -> Result<(), InstallApiError> { ... }
```

## 3. Capability JSON / TOML

`permissions/install.toml` (app-defined permission set):
```toml
[[permission]]
identifier = "allow-install-app"
description = "Run install_app for an app id and stream events to caller window."
commands.allow = ["install_app"]

[[permission]]
identifier = "allow-cancel-install"
description = "Cancel an in-flight install by app id."
commands.allow = ["cancel_install"]
```

`capabilities/main.json` 갱신:
```json
"permissions": ["core:default", "allow-install-app", "allow-cancel-install"]
```

## 4. Cancel 전파 체인 (검증 완료)

| 단계 | hook | 위치 |
|---|---|---|
| Downloader | `tokio::select!` cancelled() | `downloader.rs` |
| extract zip/tar | `Arc<AtomicBool>` per-entry | `extract.rs` |
| extract dmg ditto | 100ms `try_wait` poll | `extract.rs::dmg_macos` |
| spawn_and_wait | `biased select!` start_kill | `action.rs` |
| post_install_check | `tokio::select!` cancelled() | `action.rs` |

**Gap (의도적 허용)**: `detect_format`, post-ditto `walkdir` 집계는 cancel 미체크 — 모두 CPU-only + <100ms.

## 5. 파일 추가/변경

### Rust
- (신규) `crates/installer/src/install_event.rs`
- (신규) `crates/installer/src/install_runner.rs`
- (수정) `crates/installer/src/lib.rs` — re-export
- (신규) `apps/desktop/src-tauri/src/install/mod.rs`
- (신규) `apps/desktop/src-tauri/src/install/registry.rs`
- (수정) `apps/desktop/src-tauri/src/lib.rs` — install module + state + commands + cancel_all on exit
- (수정) `apps/desktop/src-tauri/Cargo.toml` — installer + runtime-detector + scopeguard
- (수정) workspace `Cargo.toml` — `scopeguard = "1"` 추가

### Tauri config
- (신규) `apps/desktop/src-tauri/permissions/install.toml`
- (수정) `apps/desktop/src-tauri/capabilities/main.json`
- (수정) `apps/desktop/src-tauri/tauri.conf.json` — `bundle.resources`에 `manifests/apps/*.json`

### Frontend
- (신규) `apps/desktop/src/ipc/install-events.ts` — TS discriminated union
- (신규) `apps/desktop/src/ipc/install.ts` — `installApp(id, onEvent)` / `cancelInstall(id)` helpers

### Tests
- (신규) `crates/installer/tests/install_runner_test.rs` — wiremock + tempdir + capture sink, 3~5건

## 6. 비목표 (이번 sub-phase 외)

- Specta v2 통합 (TS 자동 생성) — 별도 sub-phase
- Manifest 원격 fetch (registry-fetcher crate) — Phase 1' 후순위
- React UI 컴포넌트 (`InstallProgress.tsx`) — Phase 1A.4
- ExtractEvent의 fine-grained progress (현재는 wrapper-side starting/done 2-checkpoint)
- 동시 install 여러 id (single-id reject만 구현, 다중은 자연스럽게 가능)

## 7. 참고 구현체

- [tauri-apps/plugins-workspace upload plugin](https://github.com/tauri-apps/plugins-workspace/blob/v2/plugins/upload/src/lib.rs) — `Channel<ProgressPayload>` 패턴
- [Tauri Calling-frontend docs](https://v2.tauri.app/develop/calling-frontend/) — Channel canonical
- [Tauri Capability reference](https://v2.tauri.app/reference/acl/capability/) — permissions JSON 스키마
- [Channel rustdoc 2.10.2](https://docs.rs/tauri/2.10.2/tauri/ipc/struct.Channel.html)
- [Discussion #11589 — Channel::send blocking](https://github.com/orgs/tauri-apps/discussions/11589)
- [tauri-specta v2](https://github.com/specta-rs/tauri-specta) — 후순위 통합 대상
