# ADR-0057 — Architecture cleanup + protocol decoupling (R-E 7 sub-phase 통합)

* **상태**: Accepted (2026-05-04). Phase R-E 7 sub-phase 머지와 함께 적용.
* **선행**: ADR-0050 (chat protocol). ADR-0024 (Knowledge Stack RAG). ADR-0009 (Portable workspace). ADR-0035 (KeyManager SQLCipher). ADR-0050~0056 (R-A/B/C/D 누적 ship-blocker fix).
* **컨텍스트**: 2026-05-02 GPT Pro 검수 30건 중 *POST v0.0.1 architecture cleanup* 7건. ship-blocker는 R-A/B/C/D로 모두 해소(17건). R-E는 v0.0.1 정상 출시 후 진입한 *코드 품질 + 성능 + 신뢰성 cleanup* — 사용자 가시 동작 변경 없음 (회귀 0).
* **결정 노트**: `docs/research/phase-r-e-architecture-cleanup-decision.md`

## 결정

### R-E.1 (T3) — wiremock 자동 회귀 가드

R-C.2의 `delta_emitted` graceful disconnect 동작을 raw `tokio::TcpListener`로 mid-stream disconnect 시뮬레이션해 자동 회귀 가드:
- `Content-Length: 99999` + 실 body 짧게 송신 → hyper transport error 유발
- 3 어댑터(ollama/lmstudio/llama-cpp) × 2 케이스(delta 1+ → Completed / delta 0 → Failed) = 6 신규 invariant

### R-E.2 (C2) — OpenAI compat DTO 공통화

`adapter-lmstudio`와 `adapter-llama-cpp`가 inline 정의하던 8 struct/enum 중복 제거:
- 신규 crate `crates/openai-compat-dto/`
- ChatRequest / ChatTurn / Content (untagged) / ContentPart (tagged) / ImageUrl / ChatChunk / ChatChoice / ChatDelta
- 두 어댑터가 alias import (`ChatChunk as OpenAIChatChunk`) — 사용처 코드 0 변경
- 6 invariant 테스트 (untagged/tagged serialize, optional default, request 라운드트립)

### R-E.3 (A1) — chat 프로토콜 decoupling

`adapter-ollama`에 정의된 `ChatMessage` / `ChatEvent` / `ChatOutcome`을 다른 두 어댑터가 역의존하던 구조 → `chat-protocol` crate로 추출:
- 신규 crate `crates/chat-protocol/`
- `adapter-ollama`는 `pub use chat_protocol::{...}` re-export로 백워드 호환
- 7 invariant (서로 다른 wire format 보존 + serde tag/case + None skip)

### R-E.4 (A2 재스코프) — adapter-ollama 역의존 완전 제거

원안(RuntimeAdapter trait split)은 현 trait이 이미 lifecycle-focused라 ROI 낮음 → 재스코프:
- `adapter-lmstudio` + `adapter-llama-cpp`의 `adapter_ollama::Chat*` import → `chat_protocol::Chat*` 직접 import
- 두 어댑터의 `adapter-ollama` Cargo dep 완전 제거
- `apps/desktop/src-tauri/src/chat/mod.rs`의 import도 `chat_protocol::Chat*`로 분할
- 결과: 어댑터 의존 그래프 정상화 (ollama가 다른 어댑터의 부모 노릇 X)

### R-E.5 (P1) — KnowledgeStorePool

`apps/desktop/src-tauri/src/knowledge.rs::open_store`가 매 IPC 호출마다 SQLite open/close 반복 → Arc 캐시:
- `KnowledgeStorePool { inner: HashMap<store_path, Arc<Mutex<KnowledgeStore>>>, order: Vec<String>, max_size: usize }`
- get_or_open: cache hit → Arc clone, miss → open + 적재
- FIFO eviction (max=4 default) — workspace 전환 잦은 패턴 대응
- Tauri State로 관리 + `app.manage(Arc::new(KnowledgeStorePool::new()))`
- 4 IPC 갱신: ingest_path, search_knowledge, search_knowledge_with_embedder, knowledge_workspace_stats
- 5 invariant 테스트 (cache hit / 다른 path 별개 Arc / FIFO eviction / 빈 path / default)

### R-E.6 (P4) — Channel close → cancel cascade

`Channel::send` 실패 시 backend `CancellationToken` 발화:
- `chat/mod.rs::start_chat`: Ollama + LmStudio branch에 `cancel_for_emit.cancel()` 추가
- `workspace/portable.rs::ChannelExportSink + ChannelImportSink`: `cancel: CancellationToken` 필드 추가, emit 실패 시 cancel
- 기존 `emit_or_cancel` 패턴 (knowledge / workbench / updater / install) 변경 없음 (이미 cancel cascade 적용)
- 결과: 사용자 화면 닫음 → backend stream 다음 chunk 대기 없이 즉시 drop (GPU + 네트워크 자원 절약)

### R-E.7 (R2) — WorkspaceCancellationScope

Workspace 전환 시 이전 workspace의 in-flight op cascade cancel:
- 신규 `apps/desktop/src-tauri/src/workspace/cancel_scope.rs::WorkspaceCancellationScope`
- inner: `HashMap<workspace_id, Vec<CancellationToken>>`
- API: `register(workspace_id, token)` / `cancel_workspace(id)` / `cancel_all()` / cfg(test) 진단 helper
- `set_active_workspace` IPC가 `prev_active_id`와 신규 `id` 비교 → 다르면 `cancel_scope.cancel_workspace(prev)` 호출
- opt-in 정책 — 기존 op는 register 안 함 → 영향 0. 점진적 wiring은 v2.x.
- 5 invariant 테스트 (cascade / 다른 workspace 영향 X / cancel_all / unknown noop / register 카운트)

## 근거

- **R-E.1 wiremock 회귀 가드**: R-C.2 fix가 코드 audit으로만 검증되던 위험을 자동 가드로. raw TcpListener는 wiremock의 abrupt-disconnect 미지원 한계 우회.
- **R-E.2~R-E.4 dep 그래프 정상화**: `adapter-ollama` 단방향 의존 제거 → 새 어댑터(koboldcpp/vllm) 추가 시 ollama 빠져도 빌드. workspace cycle 위험 0.
- **R-E.5 KnowledgeStorePool**: SQLite open은 빠르지만 IPC 호출당 반복은 누적 비용. Arc 캐시는 *zero-cost over single-store* 사용자 케이스(대부분 1 workspace 활성).
- **R-E.6 Channel close**: 사용자 UX — 페이지 이탈 후 GPU가 한참 더 돌아가는 사례 0. backend logs(tracing::debug!)로 audit 가능.
- **R-E.7 opt-in scope**: 모든 op register 강제는 큰 리팩토링 → 인프라만 깔고 점진적 wiring. v2.x에서 chat/ingest 우선 적용.
- **모든 R-E.* 회귀 0 정책**: 기존 invariant 모두 통과 + 신규 invariant 추가만. wire format / IPC 시그니처 100% 보존.

## 거부된 대안

1. **R-E.1 — wiremock으로 직접 abrupt disconnect**: wiremock-rs API가 raw socket drop 미지원. 우회 시 라이브러리 fork 필요 → ROI 낮음. raw TcpListener가 정공.
2. **R-E.2 — DTO를 `shared-types::openai_compat` 모듈로**: shared-types는 cross-cutting 도메인 타입 — adapter 와이어 DTO와 책임 분리. 별개 crate가 audit 친화.
3. **R-E.3 — adapter-ollama 그대로 두고 외부에서 분기**: 의존 방향 여전히 뒤집힌 채. 별개 crate 추출이 정공.
4. **R-E.4 — RuntimeAdapter trait를 ChatAdapter / BenchAdapter / LifecycleAdapter로 split (원안)**: 현 trait이 이미 lifecycle-focused — split ROI 낮음. `impl Fn(...)` async_trait 호환성 이슈도 있음. 재스코프된 "역의존 완전 제거"가 더 의미.
5. **R-E.5 — Arc<RwLock<KnowledgeStore>>로 read concurrency 확보**: SQLite WAL + busy_timeout으로 충분. RwLock은 추후 contention 측정 후 결정. v1.x는 Mutex로 단순.
6. **R-E.5 — proper LRU access ordering (move-to-front)**: HashMap + Vec FIFO만으로 max=4 환경에선 LRU와 차이 미미. proper LRU는 v2.x.
7. **R-E.6 — Tauri 2 `Channel::on_close` 콜백 사용**: 현 Tauri 2 API에 명시적 on_close 미지원. send-fail polling이 표준 패턴.
8. **R-E.7 — 모든 op CancellationToken을 parent.child_token()으로 강제**: 모든 op 시그니처 변경 → 큰 리팩토링. opt-in register가 점진적 적용 가능.
9. **R-E.7 — workspace 전환 시 모든 workspace cancel (not just prev)**: 다른 workspace에 활성 op가 있을 수 있음(미래 multi-workspace UX). prev만 cancel이 정합.
10. **모든 R-E를 단일 commit으로**: PR review 친화도 ↓. 7 commit으로 분할 — 각 sub-phase가 독립 verify 가능.

## 결과 / 영향

- **신규 crate 2건**: `crates/openai-compat-dto`, `crates/chat-protocol`
- **신규 모듈 1건**: `apps/desktop/src-tauri/src/workspace/cancel_scope.rs`
- **adapter-ollama**: 기존 chat 타입 정의 → re-export. 외부 사용처 0 변경
- **adapter-lmstudio + adapter-llama-cpp**: adapter-ollama 역의존 제거 (의존 그래프 정상화)
- **apps/desktop/src-tauri/src/chat/mod.rs**: `chat_protocol::*` 직접 import + Channel close cancel
- **apps/desktop/src-tauri/src/workspace/portable.rs**: ChannelSink에 cancel cascade
- **apps/desktop/src-tauri/src/knowledge.rs**: `open_store` → `KnowledgeStorePool::get_or_open`
- **apps/desktop/src-tauri/src/workspaces.rs**: `set_active_workspace`에 cancel_scope 트리거
- **apps/desktop/src-tauri/src/lib.rs setup**: 2 신규 State (`KnowledgeStorePool`, `WorkspaceCancellationScope`)

**테스트 신규 invariant**: 6 (R-E.1) + 6 (R-E.2) + 7 (R-E.3) + 5 (R-E.5) + 5 (R-E.7) = **29 신규**

**회귀**: 0건 (기존 24+ invariant 모두 통과)

**성능**: KnowledgeStorePool로 같은 workspace 연속 IPC의 SQLite open overhead 제거. Channel close cancel로 사용자 페이지 이탈 시 GPU 자원 즉시 회수.

## References

- 결정 노트: `docs/research/phase-r-e-architecture-cleanup-decision.md`
- GPT Pro 검수: 2026-05-02 30-issue static review (R-E 7건 본 ADR로 해소)
- 진입점 노트: `docs/research/phase-r-e-architecture-cleanup-entry.md` (사전 작성)
- 코드 (commit 순):
  - `34c3cf7` R-E.2 (openai-compat-dto)
  - `e421f24` R-E.5 (KnowledgeStorePool)
  - `8f2e8f9` R-E.1 (wiremock graceful 회귀 가드)
  - `59b8a0a` R-E.3 (chat-protocol)
  - `e102eee` R-E.4 (adapter-ollama dep 제거)
  - `cd4fc71` R-E.6 (Channel cancel cascade)
  - `39924ff` R-E.7 (WorkspaceCancellationScope)
- 관련 ADR: 0050 (chat protocol 원안), 0055 (R-C.2 graceful disconnect), 0058 = R-E 후속 sub-phase가 있을 시 (현재 7건이 모두 R-E.* 표기, ADR 번호는 0057 단일)
- 후속 (v2.x 잠재):
  - R-E.5 KnowledgeStorePool RwLock 전환 (concurrency 측정 후)
  - R-E.7 ingest/chat/bench register wiring (점진적)
  - chat/bench/install ChannelSink cancel 통합 audit
