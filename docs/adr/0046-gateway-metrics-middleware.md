# ADR-0046 — Gateway metrics middleware + Diagnostics 실 데이터

* **상태**: 채택 (2026-04-30)
* **컨텍스트**: Phase 13'.b — Diagnostics 페이지의 4개 MOCK (latency / 최근 요청 / bench batch / repair history)이 사용자 신뢰도 직격. middleware + IPC + frontend wire.
* **결정 노트**: `docs/research/phase-13pb-gateway-metrics-decision.md`

## 결정

1. **`crates/core-gateway::usage_log::GatewayMetrics`** — 메모리 ring buffer (60s latency + 50 recent requests).
2. **`record_metrics` middleware** — `axum::middleware::from_fn_with_state`로 mount. path만 저장 (query strip).
3. **3 IPC 신규 (`commands.rs`)** — `get_gateway_latency_sparkline` / `get_gateway_recent_requests` / `get_gateway_percentiles`.
4. **`bench/cache_store::list_recent`** + IPC `list_recent_bench_reports` — 파일 mtime 정렬.
5. **`workspace/commands::get_repair_history`** + JSONL append-only at `app_data_dir/workspace/repair-log.jsonl`.
6. **Frontend 5초 polling** — Diagnostics 페이지 진입 시 active. mount 시점 1회 bench/repair fetch + 5s gateway metrics poll.

## 근거

- 메모리 ring buffer는 LMmaster 규모(1 user, ~100 req/min)에 충분 — SQLite 영속은 v1.x로 deferred.
- TraceLayer와 분리된 from_fn middleware로 lock contention 회피.
- path-only 저장 — query string PII 누수 사전 차단.
- file mtime scan은 process 재시작 시에도 데이터 유지 (메모리 인덱스 거부 이유).

## 거부된 대안

- SQLite access log — v1.x deferred (인프라 과대).
- TraceLayer 내부 push — sync 콜백 lock contention.
- 메모리 bench 인덱스 — 재시작 시 빈 상태.
- WebSocket streaming — Diagnostics는 진입 시점만 보는 페이지.
- Authorization → key_id fingerprint — middleware chain 의존성 늘어남, v1.x.

## 결과 / 영향

- Diagnostics 4개 MOCK 모두 실 데이터로 교체.
- 사용자 화면 가짜 숫자 0 — 신뢰도 직접 회복.
- 미들웨어 hot path overhead 미미 (RwLock 쓰기 µs 단위).
- repair-log.jsonl 무한 증가 위험 — 5MB rotation v1.x로 deferred.

## References

- 결정 노트: `docs/research/phase-13pb-gateway-metrics-decision.md`
- 관련 ADR: ADR-0001 (gateway 정책), ADR-0022 (semaphore + KeyManager), ADR-0044 (live catalog refresh)
- 코드:
  - `crates/core-gateway/src/usage_log.rs` (GatewayMetrics + record_metrics)
  - `crates/core-gateway/src/state.rs` (AppState.metrics 필드)
  - `crates/core-gateway/src/lib.rs` (build_router metrics_layer mount)
  - `apps/desktop/src-tauri/src/commands.rs` (3 IPC)
  - `apps/desktop/src-tauri/src/bench/{cache_store,commands,mod}.rs` (list_recent)
  - `apps/desktop/src-tauri/src/workspace/{commands,mod}.rs` (repair history JSONL)
  - `apps/desktop/src/pages/Diagnostics.tsx` (MOCK 제거 + 5s polling)
- ACL: 5 신규 identifier (`allow-get-gateway-{latency-sparkline,recent-requests,percentiles}`, `allow-list-recent-bench-reports`, `allow-get-repair-history`).
