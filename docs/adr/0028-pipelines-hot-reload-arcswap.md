# ADR-0028: Pipelines hot-reload via ArcSwap

- Status: Accepted
- Date: 2026-04-28
- Related: ADR-0025 (Pipelines architecture), ADR-0029 (Per-key Pipelines override), ADR-0030 (SSE chunk transformation)
- Phase: 8'.c.2

## Context

Phase 6'.b/6'.c에서 PipelineLayer가 부팅 시점의 `PipelineChain` 스냅샷을 정적으로 보유했어요. 사용자가 Settings에서 "PII redact 끄기" 같은 토글을 바꾸면 디스크 영속(config.json)은 갱신되지만, 게이트웨이 다음 재시작 전까지는 새 정책이 적용되지 않아요. 다음 페이즈를 위해 다음 4가지가 필요했어요:

1. **사용자 토글 즉시 반영** — UX: 토글 클릭 → 다음 요청부터 효과 확인.
2. **race-free** — 진행 중인 요청은 옛 chain으로 안전하게 끝나야 함.
3. **lock-free read** — 매 요청마다 lock 경합이 추가되면 안 됨.
4. **gateway 재시작 없이** — 사용자가 chat 중인데 게이트웨이를 재시작하면 connection 끊어져요.

## Decision

`PipelineLayer`의 chain 필드를 `Arc<ArcSwap<PipelineChain>>`로 보관. 사용자 토글 → `PipelinesState::apply_set` → `build_chain(new_config)` → `chain_swap.store(Arc::new(new_chain))`. 게이트웨이의 `PipelineMiddleware::call`은 매 요청 시작 시 `chain_swap.load_full()`로 1회 snapshot을 잡고 그 future가 끝날 때까지 사용해요.

### 핵심 invariant

- **read는 lock-free**: `ArcSwap::load`는 wait-free. 동시 100k+ 요청에서도 경합 0.
- **swap은 atomic**: `Arc::store`는 RCU 패턴. 옛 Arc는 마지막 strong reference가 drop될 때 free.
- **진행 중 요청 안전**: future가 잡고 있는 Arc는 strong_count로 보존. swap 후에도 옛 chain 인스턴스가 살아있다가 future 종료 시 자연 drop.
- **다음 요청부터 새 chain**: 다음 `call()` 진입 시 `load_full()`이 새 Arc를 반환.

### 외부 인터페이스

`core_gateway::with_pipelines_audited_swap(router, chain_swap, audit_sender)` — Tauri 측에서 `PipelinesState::chain_swap()` 핸들을 mount. 한 swap을 PipelinesState + PipelineLayer 양쪽이 공유.

### 시드 4종 chain build

`pipelines.rs::build_chain(&PipelinesConfig)` — config의 boolean 4개로부터 chain 생성. 순서: prompt-sanitize → pii-redact → token-quota → observability (request 흐름의 자연 순서).

## Consequences

### 긍정

- 사용자 토글 ↔ 다음 요청 사이 latency 0 (즉시 반영).
- 매 요청 lock 추가 0 — `ArcSwap::load_full`은 atomic load + clone Arc만.
- 진행 중 요청 안전 — Arc strong_count로 자동 보존.
- 게이트웨이 재시작 없이 정책 hot-reload.

### 부정

- ArcSwap dependency 추가 (~2KB binary cost, well-tested in vector / linkerd / tonic).
- 옛 chain 인스턴스가 *진행 중 요청이 끝날 때까지* 메모리에 남음 — 일반적으로 ms 단위, streaming 600s timeout 고려 시 최대 600s. 메모리 누수 아님 (자동 drop).
- 토글 직후의 *진행 중 요청*은 옛 정책을 따라 끝남 (사용자 기대와 약간 다를 수 있음 — UI에서 명시).

## Alternatives considered + rejected

### 1. `RwLock<Arc<PipelineChain>>` — write-occasional pattern

매 read에 read-lock, swap 시 write-lock. 정합성은 보장되지만:

- 매 요청마다 read-lock 진입/종료 비용 (수십 ns).
- write-lock 잡을 때 모든 in-flight read를 기다려야 함 (toggle latency).
- ArcSwap이 동일 시맨틱을 lock-free로 제공하는데 굳이 lock 도입할 이유 없음.

### 2. 매 요청마다 chain 재빌드 (build_chain in `call`)

- 빌드 비용: Arc 4개 alloc + Vec push. 단일 요청당 micros 수준이지만 매 요청에 곱해지면 무시 못 함.
- 토글 빈도는 *세션당 0~수회*인데 그 비용을 모든 요청에 균등 분산하는 건 비효율.

### 3. global gateway 재시작

- UX 부담 큼 (chat 중인 connection 끊어짐).
- supervisor + listener bind + router build cost (수십 ms).
- "토글 한번에 chat 끊는다"는 메시지가 사용자 적대적.

### 4. Pipeline trait에 `enabled()` 메서드 추가, chain은 정적

- 모든 Pipeline이 자기 활성화 상태를 알아야 함 → 단일 책임 원칙 위반.
- chain.apply_request 안에서 매 Pipeline에 enabled() 호출 → 분기 비용 + 모듈 결합도 증가.

## References

- `crates/core-gateway/src/pipeline_layer.rs::PipelineLayer::swap_chain` — atomic swap 구현.
- `apps/desktop/src-tauri/src/pipelines.rs::PipelinesState::apply_set` — 토글 → swap 와이어링.
- `apps/desktop/src-tauri/src/gateway.rs::run` — chain_swap 공유 mount.
- ArcSwap docs: `https://docs.rs/arc-swap/latest/arc_swap/`.
- vector / linkerd / tonic 사례 — config hot-reload + service discovery refresh.
