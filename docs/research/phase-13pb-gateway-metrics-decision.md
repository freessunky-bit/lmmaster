# Phase 13'.b — Gateway metrics middleware + Diagnostics 실 데이터 결정 노트 (2026-04-30)

> **목적**: Diagnostics 페이지에 가짜 MOCK 4건 (latency / 최근 요청 / bench batch / repair history)이 사용자 신뢰도 직격. 실 IPC로 모두 wire.
> **참조**: `docs/research/phase-next-gui-audit-deferred.md` Phase 13'.b

## 1. 결정 요약

1. **메모리 전용 ring buffer** — SQLite 영속 미도입 (LMmaster 규모에서 과대). 60s sparkline + 50 recent requests in-memory.
2. **`axum::middleware::from_fn_with_state`** — TraceLayer와 분리해 lock contention 회피 (research §1).
3. **Path-only 저장** — query string drop (PII 누수 방지, research §4).
4. **bench batch는 file mtime scan** (research §6 옵션 A) — 옵션 B(메모리 인덱스) / C(SQLite) 거부.
5. **repair history는 JSONL append-only** — `app_data_dir/workspace/repair-log.jsonl` (research §7).
6. **Frontend 5초 polling** — websocket/SSE 없이 단순. Diagnostics 진입 시점만 활성.

## 2. 채택안

### 2.1 GatewayMetrics 메모리 store

```rust
pub struct GatewayMetrics {
    latency: RwLock<VecDeque<LatencySample>>,    // 60s evict
    recent: RwLock<VecDeque<RequestRecord>>,     // capacity 50
}

impl GatewayMetrics {
    pub fn record(&self, method, path, status, ms);  // middleware
    pub fn latency_sparkline(&self) -> Vec<u32>;     // 30 bucket 평균
    pub fn recent_requests(&self, limit) -> Vec<RequestRecord>;  // 최근 → 오래된
    pub fn percentiles(&self) -> Percentiles;        // p50/p95/count
}
```

### 2.2 Tower middleware

```rust
pub async fn record_metrics(
    State(metrics): State<Arc<GatewayMetrics>>,
    req: Request,
    next: Next,
) -> Response {
    let started = Instant::now();
    let method = req.method().as_str().to_string();
    let path = req.uri().path().to_string();  // query string DROP
    let response = next.run(req).await;
    let ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;
    metrics.record(&method, &path, response.status().as_u16(), ms);
    response
}
```

`build_router`의 ServiceBuilder 외측에 mount — RequestId 부여 후 메트릭 기록.

### 2.3 Bench batch IPC

```rust
pub fn list_recent(app, limit) -> Vec<BenchReport> {
    // bench_cache_dir read_dir + sort_by(mtime) + take(limit) + parse
}

#[tauri::command]
pub async fn list_recent_bench_reports(app, limit: Option<u32>) -> Vec<BenchReport>;
```

### 2.4 Repair history JSONL

```rust
struct RepairHistoryEntry { at, tier, invalidated_caches, note }

// check_workspace_repair 끝에서 tier != Green 시 append:
// app_data_dir/workspace/repair-log.jsonl

#[tauri::command]
pub async fn get_repair_history(app, limit) -> Vec<RepairHistoryEntry>;
```

### 2.5 Frontend Diagnostics

- `useEffect` mount 시: `listRecentBenchReports` + `getRepairHistory` 1회.
- `useEffect` polling 5s: `getGatewayLatencySparkline` + `getGatewayRecentRequests` + `getGatewayPercentiles` 동시 호출 (`Promise.all`).
- MOCK 상수 4건 모두 삭제. `BenchEntry` / `RecentRequest` / `RepairHistoryRow` 인터페이스도 제거 (각각 backend serde 미러로 교체).

## 3. 기각안 + 이유

**A. SQLite access log 영속 (research §2 hybrid)**
- ❌ 거부 (v1.x): LMmaster 규모 (사용자 1, 분당 ~100 req)에서 SQLite 도입은 과대. 메모리 50 entry로 Diagnostics "최근 5개" 충분. 7일+ 로그 분석은 v1.x 기능. tauri-plugin-sql 의존성 추가는 보수적으로.

**B. tower_http::TraceLayer 안에서 직접 metrics push**
- ❌ 거부: TraceLayer의 `on_response` 콜백은 sync trait — `RwLock::write()` 보유 시간이 길면 contention. 별도 `from_fn` middleware가 정석 (research §1).

**C. bench 결과 메모리 인덱스 (옵션 B)**
- ❌ 거부: process 재시작 시 비어 있어 첫 Diagnostics 진입이 빈 상태 — 사용자 신뢰도 직격 (research §6).

**D. SQLite bench history (옵션 C)**
- ❌ 거부: KB 단위 JSON BenchReport를 DB에 넣으면 비대화. file scan 어차피 필요해 중복.

**E. SQLite repair_log 테이블 (single-row trigger)**
- ❌ 거부: append-only JSONL이 더 단순 + 가독성 ↑. Read는 전체 read 후 split (5MB 한계라 OK). v1.x에 5MB rotation + 3 generation 도입.

**F. WebSocket / SSE로 라이브 streaming**
- ❌ 거부: Diagnostics는 진입 시점만 보는 페이지. 5초 polling으로 충분, 인프라 단순.

**G. PII redaction at storage layer**
- ✅ 채택 (path-only): `request.uri().path()` — query string은 미들웨어가 자동 drop. caller (handler) 시점엔 query 접근 가능 (extractor가 자체 처리).
- ❌ 거부 (regex 후처리): 복잡 + 오탐 위험. path-only로 단순화.

**H. `Authorization` 헤더에서 key_id fingerprint 추출 (research §4)**
- ❌ 거부 (v1.x): 현재 KeyManager에 hashed key_id 메서드는 있으나 access log에 묶기엔 미들웨어 chain 의존성 늘어남. v1.x — `key_id` 컬럼 추가 후 도입.

## 4. 미정 / v1.x

- SQLite access_log + 7d retention + 사용자 retention 설정 (research §4).
- `time_to_first_byte_ms` 별도 필드 — SSE chat 정확 측정 (research §3).
- p99 percentile — 60 sample 한정이면 noise. v1.x에 1h window까지 늘리면 의미.
- `key_id` fingerprint 컬럼 — Authorization 헤더 마스킹 + audit 통합.
- UI: "메트릭 수집 끄기" Settings 토글 (privacy opt-out).

## 5. 테스트 invariant

| 영역 | invariant |
|---|---|
| `GatewayMetrics::record` | 60s 지난 latency entry 자동 evict. recent capacity 50 — 초과 시 oldest pop. |
| `latency_sparkline()` | 30 bucket 길이. 빈 bucket은 0. 가장 최근 sample은 마지막 bucket. |
| `percentiles()` | p50/p95 정확 (정렬 후 idx 산출). 빈 sample은 default(0/0/0). |
| `record_metrics` middleware | path만 저장 (query strip). next.run 직전·직후로 ms 산출. |
| `list_recent` (bench) | mtime 내림차순. 비-JSON 파일 skip. limit 50 cap. |
| `RepairHistoryEntry` JSONL | tier != green일 때만 append. append-only — 읽기 시 line-by-line parse. |
| Frontend polling | mount 시점 + 5s interval. cleanup 시 clearInterval. |

## 6. 다음 페이즈 인계

**Phase 13'.c** (다음 후보): API 키 필터 편집 + Crash 뷰어. 본 페이즈와 독립.

**진입 조건**:
- 본 결정 노트 + ADR-0046 commit ✓
- 메트릭 wiring 검증 (cargo test core-gateway 신규 5건 + tsc clean) ✓

**위험 노트**:
- middleware lock contention — `RwLock::write` 매 요청 호출. 100 req/s에서도 µs 단위라 문제 없으나, 1000+ req/s 도달 시 `parking_lot::RwLock` 또는 `arc-swap` 마이그레이션.
- bench cache 디렉터리 미존재 시 빈 결과 반환 — 첫 사용자 정상.
- repair-log.jsonl 무한 증가 — 5MB rotation v1.x.

## 7. ADR

본 결정 노트 짝: `docs/adr/0046-gateway-metrics-middleware.md`.
