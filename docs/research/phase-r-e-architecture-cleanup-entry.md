# Phase R-E — Architecture Cleanup 진입점 노트 (POST v0.0.1)

> 2026-05-03 작성. R-A/R-B/R-C/R-D 4 페이즈 머지 완료(17건 ship-blocker). R-E는 아키텍처 cleanup 7건으로 v0.0.1 ship 후 진입.

## 진입 조건

- ✅ Phase R-A (commit `6ab55d3`) push 완료 — Security Boundary
- ✅ Phase R-B (commit `1191f6c`) push 완료 — Catalog Trust Pipeline
- ✅ Phase R-C (commit `b760eb0`) push 완료 — Network + Correctness
- ✅ Phase R-D (commit `56eb4c7`) push 완료 — Frontend Polish
- ⏳ v0.0.1 release tag push (사용자 결정 — `git tag v0.0.1 && git push origin v0.0.1`)
- ⏳ release.yml 자동 트리거 후 4-platform 빌드 + draft Release 확인

R-E는 ship-blocker 아님 — v0.0.1 release 후 사용자 피드백 + 추가 보강 단계로 진입.

## 7 sub-phase 구체적 정의

### R-E.1 (T3) — wiremock chat_stream graceful 자동 회귀 가드

R-C.2(2026-05-03 머지)에서 추가한 `delta_emitted` graceful early disconnect 동작이 *코드 audit으로만* 검증 중. 다음 refactor에서 silently break 가능 → wiremock으로 자동 회귀 가드.

**테스트 시나리오 (3 어댑터 × 2 케이스 = 6 invariant)**:
1. delta 1건 emit + transport error → ChatOutcome::Completed (graceful)
2. delta 0건 + transport error → ChatOutcome::Failed (실 에러)

**구현 후보**:
- (a) wiremock `Content-Length` 명시 + body 짧게 → hyper transport error 유발
- (b) tokio TcpListener로 raw HTTP 응답 + 중간 socket drop (확실, 복잡)

**파일**:
- `crates/adapter-ollama/src/lib.rs` — chat_stream 새 wiremock 테스트 2건
- `crates/adapter-lmstudio/src/lib.rs` — 동 2건
- `crates/adapter-llama-cpp/src/lib.rs` — 동 2건

**예상 시간**: 작은 ~ 중간 (wiremock 트릭 검증 필요)

---

### R-E.2 (C2) — OpenAI compat DTO 공통화

`adapter-lmstudio`와 `adapter-llama-cpp` 모두 OpenAI compat 6+ struct 중복 (`OpenAIChatRequest` / `OpenAIChatTurn` / `OpenAIContent` / `OpenAIContentPart` / `OpenAIImageUrl` / `OpenAIChatChunk` / `OpenAIChoice` / `OpenAIDelta`).

**구현**:
- 신규 crate `crates/openai-compat-dto/` 또는 `shared-types::openai_compat` 모듈
- 두 어댑터의 inline 정의 제거 → import
- 시그니처 100% 보존 (기존 chat_stream 동작 0 변경)

**검증**: cargo workspace clippy clean / 기존 chat_stream 12 (lmstudio) + 10 (llama-cpp) 테스트 통과.

**예상 시간**: 작음. 순수 코드 이동.

---

### R-E.3 (A1) — chat 프로토콜 decoupling

`ChatMessage` / `ChatEvent` / `ChatOutcome`이 `adapter-ollama`에 정의 → 다른 두 어댑터가 `use adapter_ollama::ChatMessage` 역의존.

**구현**:
- 신규 crate `crates/chat-protocol/` 또는 `shared-types::chat`
- 3 struct + helper 추출
- `adapter-ollama`는 re-export 유지(백워드 호환) 또는 caller 모두 갱신
- `apps/desktop/src-tauri/src/chat/mod.rs` import 갱신

**리스크**: workspace member 1건 추가 + Cargo dep 그래프 변경.

**예상 시간**: 중간. import path 다수 갱신.

**의존**: R-E.2(C2)와 묶기 가능 — chat 영역 추출이 같은 본질.

---

### R-E.4 (A2) — RuntimeAdapter trait 분리

`RuntimeAdapter` god interface 책임 분할:
- `LifecycleAdapter`: install / start / stop / restart / health
- `ChatAdapter`: chat_stream / list_models / warmup
- `BenchAdapter`: run_prompt

`RuntimeAdapter`는 marker trait + super trait composition. 어댑터는 필요한 trait만 implement → bench-only / chat-only adapter 작성 가능.

**리스크**: 큰 리팩토링. 기존 caller (chat/mod.rs / bench/commands.rs) trait bound 다수 갱신.

**예상 시간**: 큼. 가장 영향 면적 큼.

**의존**: R-E.3(A1) 후순위 권장 — chat 타입 추출 후 ChatAdapter trait 정의.

---

### R-E.5 (P1) — KnowledgeStorePool 도입

`apps/desktop/src-tauri/src/knowledge.rs:484-488` — IPC 호출당 `KnowledgeStore::open` 반복.

**구현**:
- `KnowledgeStorePool { inner: Arc<Mutex<HashMap<WorkspaceId, Arc<Mutex<KnowledgeStore>>>>> }`
- 첫 호출 → open + 캐시. 같은 workspace 다음 호출 → Arc clone.
- LRU eviction (max 4 workspace 캐시 유지) — workspace 전환 시 cold workspace drop.
- Tauri State로 관리.

**예상 시간**: 작음. contained scope.

**검증**: knowledge-stack 70 + knowledge IPC 11 통과 + 신규 pool round-trip + LRU eviction 5 invariant.

---

### R-E.6 (P4) — Channel cancel detection

`tauri::ipc::Channel<ChatEvent>` close → backend cancel cascade 누락.

**구현**:
- Tauri 2 API 검토: `Channel::on_close` 콜백 가능?
- 또는 send 실패 → CancellationToken 발행 (polling pattern)
- chat_stream loop에서 `if on_event_send.is_err() { cancel.cancel(); }`

**범위**: chat_stream + portable export/import + bench + workbench (모든 Channel 사용처) 일괄.

**예상 시간**: 중간. Tauri 2 API 의존.

---

### R-E.7 (R2) — Workspace cancellation scope

매 operation별 별개 `CancellationToken::new()` → workspace 인지 cascade로 통합.

**구현**:
- `WorkspaceCancellationScope` 구조체 (workspace_id별 parent CancellationToken)
- 새 operation 시작 시 `parent_token.child_token()` 발급
- workspace 전환 시 이전 scope의 parent token cancel → 모든 child cancel cascade

**리스크**: 모든 operation 시그니처 갱신. cross-workspace operation(catalog 갱신 등)은 scope 외 처리.

**예상 시간**: 큼. 광범위.

**의존**: R-E.6(P4) 먼저 — Channel cancel 흐름이 잡힌 후 wider scope.

---

## 권장 순서

1. **R-E.2 (C2)** — 가장 작음, 순수 코드 이동, 위험 0. *워밍업으로 적합.*
2. **R-E.5 (P1)** — contained, 측정 가능한 perf 개선.
3. **R-E.1 (T3)** — wiremock 트릭 검증 단계, 회귀 가드.
4. **R-E.3 (A1)** — chat 타입 추출.
5. **R-E.4 (A2)** — trait 분리 (R-E.3 의존).
6. **R-E.6 (P4)** — Channel cancel.
7. **R-E.7 (R2)** — workspace scope (R-E.6 의존).

## 검증 명령

```powershell
.\.claude\scripts\check-acl-drift.ps1
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --exclude lmmaster-desktop
cd apps/desktop
pnpm exec tsc -b
pnpm exec vitest run
```

## 분리된 sub-phase (별개 진행)

R-A/R-B에서 분리된 sub-phase는 R-E와 무관하게 우선순위 결정:

- **#31** — Knowledge IPC tokenized path (R-A.3 분리)
- **#38** — knowledge-stack caller wiring (R-B.2 분리)

둘 다 v0.0.1 ship 후 사용자 피드백 보고 우선순위 재평가.

## 다음 세션 시작 시

1. `docs/RESUME.md` 누적 검증 라인 → 17건 ship-blocker 해소 확인.
2. 본 문서 §권장 순서 → R-E.2(C2)부터 진입.
3. 또는 v0.0.1 release tag 먼저 push → 사용자 피드백 들어오면 R-E 우선순위 재조정.
