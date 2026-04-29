# Phase 5'.b + 4.5'.b — Tauri IPC 보강 리서치

> Channel<T> + CancellationToken + ACL + 장기 작업 IPC 엘리트 사례 종합. Workbench (`start_workbench_run / cancel_workbench_run / list_workbench_runs`) 와 Knowledge Stack (`ingest_path / cancel_ingest / search_knowledge`) 양쪽 IPC 레이어를 위한 결정 근거를 모음. **Phase 1A.3.c (`crates/installer` + `apps/desktop/src-tauri/src/install/`) 와 Phase 2'.c.2 (`bench/registry.rs` + `bench/commands.rs`)가 이미 채택한 패턴을 그대로 잇는 것이 본 리서치의 결론**임. 새로운 추상은 도입하지 않고, 두 신규 도메인(workbench-core, knowledge-stack)의 약간 다른 결(IngestService = `mpsc::Sender<IngestProgress>`, WorkbenchRun = 5단계 state machine)을 어떻게 동일 IPC 셸로 흡수할지가 핵심.

---

## 1. Channel<T> 진행 상태 스트리밍

### 1.1 채택 패턴

**결론**: `tauri::ipc::Channel<T>` per-invocation stream. `Emitter::emit` 금지. **Phase 1A.3.c (install) + Phase 2'.c.2 (bench)가 이미 검증한 형식 — 양쪽 신규 도메인도 동일 셰입.**

Tauri 공식 문서 ([calling-frontend](https://v2.tauri.app/develop/calling-frontend/))의 권장 코드:

```rust
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum WorkbenchEvent {
    Started { run_id: String, total_steps: u32 },
    StepEntered { run_id: String, step: WorkbenchStep },
    Quantize { progress: QuantizeProgress },
    LoraEpoch { epoch: u32, total: u32, loss: f32 },
    Eval { passed: u32, total: u32 },
    Finished { run_id: String, registered_id: Option<String> },
    Failed { run_id: String, code: String, message: String },
    Cancelled { run_id: String },
}

#[tauri::command]
pub async fn start_workbench_run(
    app: AppHandle,
    registry: State<'_, Arc<WorkbenchRegistry>>,
    config: WorkbenchConfig,
    channel: Channel<WorkbenchEvent>,
) -> Result<WorkbenchRun, WorkbenchApiError> { ... }
```

프론트:

```typescript
import { invoke, Channel } from '@tauri-apps/api/core';
const onEvent = new Channel<WorkbenchEvent>();
onEvent.onmessage = (msg) => { /* discriminated union by msg.kind */ };
await invoke('start_workbench_run', { config, channel: onEvent });
```

**Ingest 도메인의 차이**: `IngestService::ingest_path`는 이미 `mpsc::Sender<IngestProgress>`를 받음. Tauri command 셸에서 어댑터 layer를 둠 — `ChannelIngestSink { channel: Channel<IngestEvent> }`가 mpsc receiver를 한 task에서 drain하며 `channel.send()` 호출. installer의 `ChannelInstallSink` (`apps/desktop/src-tauri/src/install/mod.rs:55-69`) 와 정확히 같은 모양.

### 1.2 Backpressure / drop 시멘틱

소스 (`crates/tauri/src/ipc/channel.rs`)는 비동기 송신 큐(`ChannelDataIpcQueue` = `Arc<Mutex<HashMap<u32, InvokeResponseBody>>>`)와 동기 직접 호출 두 path를 가짐. **버퍼 무한대 — backpressure 자체 메커니즘 없음**. 따라서:

- **백엔드 시점 throttle 책임**: progress emit 빈도를 백엔드에서 조절. installer downloader는 256KB / 100ms 둘 중 빠른 쪽으로 throttle. Workbench/Ingest는 "단계 전환마다 1회 + 청크당 최대 1회"로 충분 (분당 < 60 events).
- **Frontend 처리 지연**은 webview의 JS event-loop가 처리 — IPC 큐는 메모리에 쌓임. 1만개 작은 이벤트도 수 MB 이내라 무방.
- **Drop 시멘틱**: Tauri 2.10.x의 `ChannelInner::Drop`은 등록된 `on_drop` 콜백만 실행. **백엔드의 `channel.send()`는 webview/window가 닫히면 `Err(Error)` 반환** — 이 시점 cancellation 트리거가 우리 책임. installer의 `emit_or_cancel` 헬퍼 (`crates/installer/src/install_runner.rs:207-219`)가 정확히 이 패턴 — `send().is_err()` → `cancel.cancel()` + `SinkClosed` 반환.
- 최근 fix ([commit 66e6325](https://github.com/tauri-apps/tauri/commit/66e6325f43efa49ec2165c45afec911a1a14ecfb))는 `once`-callbacks의 window 누수를 잡음. 우리 streaming 채널은 long-lived라 영향 없음.

### 1.3 인용

- [tauri-apps/tauri `crates/tauri/src/ipc/channel.rs`](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri/src/ipc/channel.rs) — `Channel<T>::send` 시그니처 + `Drop` impl + `on_drop` 콜백 등록 메커니즘.
- [crabnebula-dev/tauri-plugin-llm](https://github.com/crabnebula-dev/tauri-plugin-llm) — 토큰 스트리밍에 자체 `send_stream/recv_stream` 채널 사용. `Query::End` 단말 이벤트 패턴은 우리 `Finished/Failed/Cancelled`와 동일한 결.
- [AlexsJones/llama-panel](https://github.com/AlexsJones/llama-panel) — Tauri 기반 llama-server 매니저. HuggingFace 모델 다운로드 ETA + 라이브 서버 로그 스트리밍을 IPC로 처리. UI는 "어떤 탭에서도 보이는 progress bar" — 우리 RunsPanel과 같은 컨셉.

---

## 2. 취소 wiring (frontend abort → backend token)

### 2.1 채택 패턴

**Tauri 2의 `invoke()` 프로미스는 `AbortSignal`을 (현재) backend로 전파 안 함** ([issue #8351](https://github.com/tauri-apps/tauri/issues/8351), open). 따라서 **별도 cancel command + Registry**가 사실상 단일 production 패턴이며, 우리 `InstallRegistry`/`BenchRegistry`가 이미 이 형태:

```rust
// apps/desktop/src-tauri/src/workbench/registry.rs (신규)
pub struct WorkbenchRegistry {
    inner: Mutex<HashMap<String, CancellationToken>>,  // run_id (uuid) → token
}

impl WorkbenchRegistry {
    pub fn try_start(&self, id: &str) -> Result<CancellationToken, WorkbenchRegistryError> { ... }
    pub fn finish(&self, id: &str);
    pub fn cancel(&self, id: &str);
    pub fn cancel_all(&self);
}
```

**프론트 측**:

```typescript
// 시작
const ch = new Channel<WorkbenchEvent>();
ch.onmessage = handleEvent;
const run = await invoke('start_workbench_run', { config, channel: ch });

// AbortController로 UI에서 취소 의도 캡처 → invoke 호출
const ctrl = new AbortController();
ctrl.signal.addEventListener('abort', () => {
  invoke('cancel_workbench_run', { runId: run.id });  // best-effort, idempotent
});
```

- `BenchRegistry`/`InstallRegistry` (`apps/desktop/src-tauri/src/bench/registry.rs:20-63` + `install/registry.rs:22-69`)와 동일한 5-method API: `try_start / finish / cancel / cancel_all / in_flight_count`.

### 2.2 Race condition 처리

| 시나리오 | 처리 |
|---|---|
| **cancel-before-start** | start command 진입 시 `try_start` → 새 토큰 발급 + 등록. 사용자가 cancel을 누른 시점에 Registry에 entry 없으면 `cancel(id)`는 no-op. *문제 없음 — 사용자 의도와 일치 (시작도 안 함)*. |
| **cancel-during-emit** | `channel.send()` 직전 `cancel.is_cancelled()` 체크 안 해도 OK — flow 중간의 `select!` / `is_cancelled()` 분기에서 잡힘. 단, **emit 자체는 lock-free라 cancel 토큰만 cooperative하게 폴링**. workbench-core/quantize.rs의 `MockQuantizer`가 이미 stage 진입 시 `if cancel.is_cancelled() { return Cancelled }` 패턴. ingest.rs도 단계마다 `cancel.load(SeqCst)` 폴링. |
| **cancel-after-finish** | `finish(id)`로 entry 제거된 후 `cancel(id)` → no-op. *idempotent — UI가 "이미 끝남" + "취소 버튼" 동시 도착 race를 안전하게 흡수*. |
| **double-start same id** | `try_start`가 `AlreadyRunning` 거부. workbench는 `run.id = uuid::new_v4()`로 시작 시점에 generate해 거의 충돌 0이지만, 사용자가 동일 config로 두 번 누르면 두 개 별도 run으로 진행 — 의도된 동작. **ingest는 workspace_id 단위 직렬화 필요** (동일 ws 동시 ingest는 SQLite 락 충돌 위험) → registry key = `workspace_id`. |
| **process exit 중 cancel** | `RunEvent::ExitRequested`에서 `registry.cancel_all()`. `CancellationToken::Drop`은 cancel 안 하므로 명시 호출 필수 (이미 lib.rs:175-184에서 install/bench 처리 — workbench/ingest 추가). |

### 2.3 Registry cleanup

**RAII guard 패턴 — `scopeguard::defer!`**. install/mod.rs의 `InstallGuard { Drop -> registry.finish(id) }` (`mod.rs:72-81`) 또는 bench/commands.rs의 `defer! { finish_registry.finish(&finish_id); }` (`commands.rs:63-65`) 둘 중 하나. 양쪽 다 어떤 `?` 조기 리턴이든 panic이든 finish 보장.

`DashMap` vs `Mutex<HashMap>` 검토: **현재 패턴 유지**. Lock contention이 측정상 0 (락 보유 시간 < 1µs, 보유 중 await 없음). DashMap 도입 시 deps 1개 추가 + 별도 진입점에서 sync 의미 차이 — 가성비 낮음. [tokio shared-state 가이드](https://tokio.rs/tokio/tutorial/shared-state) 자체도 "low contention이면 std Mutex 권장"이라 명시.

---

## 3. Capabilities ACL 갱신

### 3.1 신설 command 명세

| Command | Window | Scope | 비고 |
|---|---|---|---|
| `start_workbench_run` | `main` | none (global) | `Channel<WorkbenchEvent>` arg + state injection. long-running async. |
| `cancel_workbench_run` | `main` | none | sync, idempotent. `run_id: String`. |
| `list_workbench_runs` | `main` | none | sync, registry snapshot 반환. UI Runs 패널이 polling 대체로 호출 (Channel은 push-only). |
| `ingest_path` | `main` | path-allowlist v1 미적용 (v1.1) | `Channel<IngestEvent>` + workspace_id + path. |
| `cancel_ingest` | `main` | none | idempotent. `workspace_id: String`. |
| `search_knowledge` | `main` | none | sync RPC. workspace_id + query + top_k. |

**path-allowlist 검토**: `ingest_path`가 임의 디렉터리를 읽으므로 v1.1에서 capability scope에 `fs:scope` 또는 user-pick dialog 강제 고려. v1은 *내부 사용자 데스크톱 앱*이라 단순화.

### 3.2 capability JSON diff (실제 추가될 항목)

`apps/desktop/src-tauri/capabilities/main.json`:

```json
{
  "permissions": [
    "core:default",
    "allow-install-app", "allow-cancel-install",
    "allow-detect-environment", "allow-start-scan", "allow-get-last-scan",
    "allow-get-catalog", "allow-get-recommendation",
    "allow-start-bench", "allow-cancel-bench", "allow-get-last-bench-report",
    "allow-create-api-key", "allow-list-api-keys", "allow-revoke-api-key",
    "allow-get-workspace-fingerprint", "allow-check-workspace-repair",
    "allow-list-runtime-statuses", "allow-list-runtime-models",
    "allow-get-presets", "allow-get-preset",
    "allow-start-workbench-run",      // NEW (5'.b)
    "allow-cancel-workbench-run",     // NEW (5'.b)
    "allow-list-workbench-runs",      // NEW (5'.b)
    "allow-ingest-path",              // NEW (4.5'.b)
    "allow-cancel-ingest",            // NEW (4.5'.b)
    "allow-search-knowledge"          // NEW (4.5'.b)
  ]
}
```

`apps/desktop/src-tauri/permissions/workbench.toml` (신규):

```toml
[[permission]]
identifier = "allow-start-workbench-run"
description = "Start a 5-step workbench run (data → quantize → lora → validate → register) and stream WorkbenchEvent to caller window."
commands.allow = ["start_workbench_run"]

[[permission]]
identifier = "allow-cancel-workbench-run"
description = "Cancel an in-flight workbench run by id (idempotent)."
commands.allow = ["cancel_workbench_run"]

[[permission]]
identifier = "allow-list-workbench-runs"
description = "List active workbench run snapshots (in-memory registry)."
commands.allow = ["list_workbench_runs"]
```

`apps/desktop/src-tauri/permissions/knowledge.toml` (신규):

```toml
[[permission]]
identifier = "allow-ingest-path"
description = "Ingest a file or directory into a knowledge workspace; streams IngestEvent."
commands.allow = ["ingest_path"]

[[permission]]
identifier = "allow-cancel-ingest"
description = "Cancel an in-flight ingest by workspace_id."
commands.allow = ["cancel_ingest"]

[[permission]]
identifier = "allow-search-knowledge"
description = "Search a knowledge workspace by text query."
commands.allow = ["search_knowledge"]
```

**규칙**: app-local commands는 plugin prefix 없이 `allow-{command-name}` (kebab) — Phase 1A.3.c가 검증한 컨벤션. 자동 생성이 아닌 **명시 toml 등록** (`tauri.conf.json`의 `removeUnusedCommands = true` 호환).

---

## 4. ML/training 작업 IPC 엘리트 사례

### 4.1 AI Toolkit (Microsoft Foundry, VS Code)

VSCode extension은 Electron-host에서 train job을 자체 백엔드(Foundry agent)로 위임 + **streaming-response visibility**를 Agent Inspector로 노출. *우리에게 중요한 인사이트*: train events는 step 단위 + 실시간 스트리밍 + workflow visualization. Tauri에서는 동일 컨셉이 `Channel<WorkbenchEvent>` 단일 stream + 프론트 timeline 뷰로 충분 — extension API의 무거운 actor framework 도입 불필요. ([Foundry Toolkit 개요](https://learn.microsoft.com/en-us/windows/ai/toolkit/))

### 4.2 LM Studio (관찰)

LM Studio는 Electron + 자체 IPC(주로 emit 기반 + WebSocket-style streaming). 모델 로드는 "% loaded" + speed 메트릭 노출, 양자화는 사용자에게 직접 노출 없음(내부 백그라운드). *교훈*: 양자화는 보통 1회 long event라 progress segmentation이 sparse — 우리 Quantizer는 0/25/50/75/100 이미 충분. 추가 fine-grained는 v1.1.

### 4.3 LLaMA Factory bridge

[LLaMA-Factory](https://github.com/hiyouga/LLaMA-Factory)는 Gradio + python subprocess로 train 출력 stdout을 line-buffered로 UI에 push. *우리 LoRATrainer trait이 v1.c에서 wrap할 대상*. **stdout line → `LoraEpoch { epoch, loss }` 매핑은 백엔드 책임** — frontend는 typed event만 받음. 이 분리가 stdout 포맷 변동(LLaMA-Factory upstream 변경)을 IPC 계약과 격리.

### 4.4 Tauri 사례

- [crabnebula-dev/tauri-plugin-llm](https://github.com/crabnebula-dev/tauri-plugin-llm) — 토큰 스트리밍에 `Query::Chunk / Query::End` 단말 이벤트 enum + dedicated thread 모델. *적용*: Quantize/LoRA 실행 task를 `tauri::async_runtime::spawn` 위에 올리고 cancel 토큰만 share (tokio::spawn 금지 — Tauri 2 정책, lib.rs:5).
- [danielbank/tauri-mistral-chat](https://github.com/danielbank/tauri-mistral-chat) — mistral.rs 추론을 sidecar 패턴으로 wrap. Channel 미사용 + emit 사용 — 우리 결정 ("Channel 채택")과 대비. *교훈*: 작은 페이로드는 emit으로도 가능하나 ordering 보장 안 됨 → quantize 50%가 75% 뒤에 도착하는 race가 가능. **Channel을 고수해야 하는 negative space.**
- [AlexsJones/llama-panel](https://github.com/AlexsJones/llama-panel) — model search + 다운로드 ETA + 슬롯 모니터링 정도까지 Tauri command + emit으로 처리. **multi-instance에서 모델별 random port**라는 우리 LMmaster의 portable runtime 결정과 동일 결.

---

## 5. RAG ingestion 엘리트 사례

### 5.1 AnythingLLM

Electron 기반. collector module이 문서를 chunking → embedding 모델로 vector화 → vector DB 저장. Stream 처리는 SSE 기반 (chat side). *적용*: ingestion progress를 chat과 동일 채널이 아닌 별도 channel로 분리 — 우리 `Channel<IngestEvent>` + `Channel<ChatEvent>` 분리는 기본. Per-workspace 격리는 우리 `KnowledgeStore` `WHERE workspace_id = ?` invariant이 이미 강제. ([anything-llm](https://github.com/Mintplex-Labs/anything-llm))

### 5.2 Page Assist

Chrome extension. Knowledge base는 nomic-embed-text 같은 embedding 모델로 chunking 후 vector 저장. *교훈*: extension은 background script + content script 통신이 우리 Tauri의 IPC와 유사한 결 — UI thread 차단 회피 패턴이 동일. ([n4ze3m/page-assist](https://github.com/n4ze3m/page-assist))

### 5.3 Msty Knowledge Stack (관찰)

Closed-source. UI 관찰: ingest 시 파일 단위 progress + skip된 binary 파일 카운트 + 완료 후 chunk 카운트 노출. *우리 `IngestSummary { documents, chunks, skipped }`가 정확히 동일 정보 반환* — `ingest.rs:46-51`의 셰입이 이미 적합.

### 5.4 Tauri RAG 사례

- [ElectricSQL Local AI 블로그](https://electric-sql.com/blog/2024/02/05/local-first-ai-with-tauri-postgres-pgvector-llama) — Tauri + Postgres + pgvector + llama2. fastembed로 embedding 생성을 Rust command로 노출 + `invoke('generate_embedding', { text })` 패턴. *교훈*: embedding 생성을 동기 RPC로 하면 batch 호출 비용이 크다 — 우리 `Embedder::embed(&[String]) -> Vec<Vec<f32>>` batch API가 정답.
- [danielbank/tauri-mistral-chat](https://github.com/danielbank/tauri-mistral-chat) — local LLM + 간단한 history 저장. RAG 자체는 없으나 IPC 기본기 참고.

---

## 6. State persistence

### 6.1 In-flight run 처리

**v1 결정**: in-memory only (`Arc<WorkbenchRegistry>` / `Arc<IngestRegistry>` in `app.manage(...)`). 앱 종료 = 모든 run cancelled. 다음 launch에서 **신규 실행**으로 처리, 이전 run 흔적은 v1.b에서 `workspace/workbench/{run_id}/{step}/` 캐시 디렉터리 잔재로만 남음 (`flow.rs:108`의 `cache_path_for`).

**Orphan handling**:
- **Quantize**: subprocess(llama-quantize) 자식 프로세스가 process group으로 묶이지 않으면 좀비 가능. v1.b에서 `Command::kill_on_drop(true)` (tokio) 또는 Job Object (Windows) / setpgid (Unix) 적용 — bench-harness의 sidecar 호출 패턴 차용.
- **LoRA**: LLaMA-Factory subprocess도 동일.
- **Ingest**: SQLite 트랜잭션 단위 atomic — 중간 termination 시 마지막 commit까지만 남음. SHA 기반 dedup으로 재실행 시 중복 ingest 방지 (이미 `add_document(sha)` 패턴).

다음 launch 시:
- **Workbench**: registry 비어있음 → list_workbench_runs는 `[]` 반환. 사용자는 새 run 시작.
- **Ingest**: 동일. 기존 ingested chunks는 SQLite에 남아 search 즉시 가능.

### 6.2 SQLite vs in-memory

**v1**: in-memory. **v1.1**: 필요 시 SQLite mirror.

| 옵션 | 장점 | 단점 |
|---|---|---|
| In-memory | 단순, 0-deps, lock 거의 없음 | crash → 진행 손실 |
| SQLite mirror | crash recovery, audit log | schema migration, 락 contention 증가, persist 시점 의문 (매 progress event? 비현실적) |
| `tauri-plugin-store` (JSON) | snapshot 쓰기 단순 | event-stream에는 부적합 (snapshot only) |

**근거**: workbench/ingest run은 보통 30초~30분. 사용자는 앱을 닫지 않고 끝까지 둠 → orphan 빈도 낮음. SQLite는 *완료된 run의 결과 (`BenchReport`처럼 디스크 캐시)*에만 적용. 진행 중 run의 raw progress event 시퀀스는 디스크 영구화 가치 < 비용 — 실패 분석은 tracing log로 충분.

### 6.3 `Manager::state` 패턴

```rust
// lib.rs setup() 안에 추가
let workbench_registry: Arc<WorkbenchRegistry> = Arc::new(WorkbenchRegistry::new());
app.manage(workbench_registry);

let ingest_registry: Arc<IngestRegistry> = Arc::new(IngestRegistry::new());
app.manage(ingest_registry);

let knowledge_store: Arc<Mutex<KnowledgeStore>> = Arc::new(Mutex::new(
    KnowledgeStore::open(app.path().app_data_dir()?.join("knowledge.db"))?
));
app.manage(knowledge_store);

let embedder: Arc<dyn Embedder> = Arc::new(MockEmbedder::default());  // v1.b에서 실 embedder swap
app.manage(embedder);
```

`RunEvent::ExitRequested`에 cancel_all 추가 (lib.rs:168-186 패턴 차용).

---

## 7. 종합 결정 → 구현으로 이월

| 항목 | 결정 | 이유 |
|---|---|---|
| **이벤트 채널** | `tauri::ipc::Channel<WorkbenchEvent>` / `Channel<IngestEvent>` | Phase 1A.3.c 검증 패턴, ordered + per-invocation |
| **이벤트 enum 형태** | `#[serde(tag = "kind", rename_all = "kebab-case")]` | 프론트 discriminated union 깔끔 — `InstallEvent`와 동일 셰입 |
| **TS 타입 동기화** | manual mirror in `apps/desktop/src/ipc/{workbench,knowledge}-events.ts` + JSON contract test | specta v0.0.7는 Tauri 2 Channel<T> 미완전 지원 ([issue #158](https://github.com/specta-rs/tauri-specta/issues/158)) — 후순위 |
| **Cancel registry** | `Mutex<HashMap<String, CancellationToken>>` per domain (`WorkbenchRegistry`, `IngestRegistry`) | DashMap 미도입(deps 보수). bench/install이 검증한 동일 형식 |
| **Registry key** | workbench: `run_id (uuid)`. ingest: `workspace_id` | workbench는 다중 동시 run 허용, ingest는 ws 단위 직렬화 (SQLite 락) |
| **RAII finish** | `scopeguard::defer!` 또는 `Drop` guard struct | 어떤 종료 path든 finish 보장 |
| **이벤트 throttle** | step 진입 시 1회 + 10% 단위 progress | 단계당 < 100 events 보장 |
| **취소 wiring** | 별도 `cancel_*` command + `AbortController` UI 메타용 | `invoke()` `AbortSignal` 미지원 ([#8351](https://github.com/tauri-apps/tauri/issues/8351)) |
| **App-local permissions** | `permissions/workbench.toml` + `permissions/knowledge.toml` | install.toml과 동일 컨벤션 |
| **Capability scope** | app-local, `windows = ["main"]` | plugin namespace 회피 |
| **ACL granularity** | per-command (allow-list) | review 단순화 |
| **Run persistence** | in-memory v1, SQLite mirror v1.1 (필요 시) | 복잡도 격리, crash 빈도 낮음 |
| **Embedder injection** | `Arc<dyn Embedder>` `app.manage` | v1.b에서 실 embedder swap 가능 |
| **path-allowlist** | v1 미적용 (사용자 desktop 신뢰 모델) | v1.1에서 fs:scope 또는 dialog 강제 |
| **상호 격리** | workbench와 ingest 양쪽 cancel 시 RunEvent::ExitRequested에서 모두 cancel_all | bench/install 패턴 단순 확장 |
| **Subprocess 좀비 방지** | v1 mock에선 N/A. v1.b LlamaQuantizer/LoRATrainer 도입 시 Job Object/setpgid + `kill_on_drop(true)` | install runner 패턴 차용 |

---

## 8. 다음 페이즈 (5'.b + 4.5'.b 구현)에 넘기는 위험 노트

1. **Channel<T> serialize 충돌 주의**. `WorkbenchStep` (kebab-case) + `RunStatus` (kebab-case)는 이미 `flow.rs`에 직렬화 fixture가 있음. 새 `WorkbenchEvent` enum의 `tag = "kind"`가 **WorkbenchStep과 같은 필드명을 안 가져야 함** — installer의 `Download { download: DownloadEvent }` wrapper 패턴 차용 가능.

2. **`CancellationToken::Drop`은 cancel 안 함** (lib.rs:7 + install/registry.rs:7). `RunEvent::ExitRequested`에서 명시 호출 필수. 새 도메인 추가할 때마다 lib.rs:168-186 블록에 `if let Some(registry) = app_handle.try_state::<Arc<WorkbenchRegistry>>() { registry.cancel_all(); }` 추가 잊지 말 것.

3. **mpsc → Channel 어댑터의 task 누수 주의**. `IngestService`의 `mpsc::Sender<IngestProgress>`를 `Channel<IngestEvent>`로 forwarding하는 별도 `tauri::async_runtime::spawn` task가 생기는데, ingest 종료 시 sender drop으로 receiver loop가 자연 종료되도록 설계. *test*: tx drop 후 receiver loop 1회 더 await하면 None 반환 + task exit.

4. **registry key 충돌 시 UX**. workbench의 `try_start`는 동일 run_id에 대해 `AlreadyRunning` 반환 — uuid라 거의 0이지만, ingest의 `workspace_id` 직렬화는 사용자가 ws 동시 ingest 시도 시 즉시 한국어 에러 노출. 카피: "이 워크스페이스는 이미 자료를 받고 있어요. 끝나면 다시 시도해 주세요."

5. **Quantize 단계 메모리 폭주 위험** (실 CLI 통합 시). v1.b `LlamaQuantizer`는 GGUF 파일을 메모리에 매핑 — VRAM/RAM 부족 시 silent OOM 가능. cancel 토큰만으로는 부족 → subprocess timeout (예: 30분) + stdout/stderr 라인 max 길이 cap 필요.

6. **Channel send 실패 시 cancel 트리거 누락**. installer는 `emit_or_cancel` 헬퍼로 송신 실패 → cancel 보장 (`install_runner.rs:207-219`). workbench/ingest의 forwarding task도 동일 가드 필요. *invariant test*: channel close 시뮬레이션 → 이후 cancel.is_cancelled() == true.

7. **WebView 닫힘 vs 앱 닫힘 구분**. WebView만 닫히고 앱은 살아있는 시나리오는 단일-window LMmaster에서는 발생 안 함. 그러나 사용자가 multi-window 설정으로 확장할 때 channel은 per-window이므로 **다른 window가 polling으로 list_workbench_runs 호출 시 동일 run을 다시 attach 가능해야** — 이는 v1.1 설계 이슈. v1은 단일 main window 가정.

8. **SQLite KnowledgeStore 락 contention**. `Arc<Mutex<KnowledgeStore>>` (sync mutex)는 ingest가 write하는 동안 search query block. ingest 단위가 < 5초라 무방하나, 거대 디렉터리 ingest 시 search latency spike 가능. *대안*: rusqlite WAL 모드 + per-thread connection 풀 (v1.1).

9. **search_knowledge의 cancel 미지원**. v1은 sync RPC로 충분 (top_k=10, < 50ms). v1.1에서 거대 corpus + reranker 통합 시 cancel 채널 추가 가능 — `SearchHandle` registry 등장.

10. **테스트 invariant 체크리스트** (DoD):
    - WorkbenchRegistry: try_start dup → AlreadyRunning, cancel idempotent, cancel_all all-tokens cancelled (install/bench와 1:1).
    - IngestRegistry: 동일 + workspace_id 키 분리.
    - WorkbenchEvent serde: kebab-case kind 검증 (Started/Quantize/LoraEpoch/Eval/Finished/Failed/Cancelled), 내부 tag 보존 (`Quantize { progress: QuantizeProgress }` 패턴).
    - IngestEvent serde: 동일 (Reading/Chunking/Embedding/Writing/Done + Failed/Cancelled).
    - mpsc → Channel 어댑터: 100 events 순서 보존, sender drop → receiver loop exit, channel close → cancel triggered.
    - Capability ACL: 6 새 permission이 main capability에 등록되는지 빌드 단계 통과.
    - i18n: 모든 사용자 향 에러 메시지 (`AlreadyRunning`, `WorkspaceNotFound`, `EmbeddingFailed`, `Cancelled`)에 한국어 해요체 포함.

---

**문서 버전**: v1 (2026-04-28). 본 리서치는 Phase 1A.3.c (`crates/installer` + install/) + Phase 2'.c.2 (bench/) 가 검증한 IPC 패턴을 두 신규 도메인에 *변경 없이 이전*하는 것이 핵심 결론. 새 추상 도입 없음 — 위험 분산.
