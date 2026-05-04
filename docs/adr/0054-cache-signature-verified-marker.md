# ADR-0054 — manifest_cache schema v2: signature_verified marker (cache poisoning 방어)

* **상태**: Accepted (2026-05-03). Phase R-B 머지와 함께 적용.
* **선행**: ADR-0047 (minisign 카탈로그 서명) — verifier 자체. ADR-0044 (live catalog refresh) — fetch 흐름. Phase 13'.g.2.b/c — `fetch_one_with_signature` + `verify_catalog_signature` 데스크톱 헬퍼.
* **컨텍스트**: 2026-05-02 GPT Pro 검수에서 v0.0.1 ship-blocker S4로 보고됨. 현재 구조의 핵심 결함:
  1. `Cache::put`은 `body_sha256`만 기록 — *서명이 검증된 row인지*는 트래킹 X.
  2. `fetch_one_with_signature`가 cache hit 시 `verify` skip — "이전에 검증됐겠지" 가정 (코드 주석 자체가 명시적으로 가정).
  3. 사용자 PC에 v1 cache row가 적재된 후 verifier가 비활성/재활성된다면(빌드 변종, env var 변동), 미검증 row가 verifier 활성 빌드에서 그대로 신뢰됨 → cache poisoning.
* **결정 노트**: `docs/research/phase-r-b-catalog-trust-pipeline-decision.md` §S4.

## 결정

1. **`manifest_cache` 스키마 v2 — `signature_verified INTEGER NOT NULL DEFAULT 0` 컬럼 추가** — `SCHEMA_VERSION_MAX = 2`. 신규 설치는 CREATE TABLE에 컬럼 직접 포함, 기존 v1 사용자는 `ALTER TABLE ADD COLUMN`로 마이그레이션. ALTER는 idempotent (duplicate column 에러 무시).
2. **신규 설치는 `version='2'`로 직접 진입** — `manifest_cache` row count == 0 인지로 신규/기존 구분. row 있으면 v1로 두고 ALTER 마이그레이션, 비어 있으면 v2.
3. **`CachePutInput`에 `signature_verified: bool` 필드 추가** — caller가 적재 시점에 명시.
   - `try_source` (네트워크 fresh) → `false` (검증 전, 후속 verify가 mark)
   - `try_bundled` → `true` (빌드 시점 큐레이터 검증 + 코드 서명, 신뢰 가능)
   - 사용자 caller (`fetch_one` 단독) → `false` (검증 미수행)
4. **`CacheRow`에 `signature_verified: bool` 추가** — `get` 시 row의 검증 상태를 함께 반환. `FetchedManifest`도 `#[serde(default)] signature_verified` 필드 추가 (백워드 호환).
5. **`Cache::mark_verified(source, manifest_id)` 신규 메서드** — verify 성공 후 row를 verified로 마킹. 별개 `RegistryFetcher::mark_signature_verified` 외부 노출 (desktop helper용).
6. **`fetch_one_with_signature` 정책 강화**:
   - cache 적중 + `signature_verified=true` → 즉시 OK (이전 검증 명시 trust).
   - cache 적중 + `signature_verified=false` → invalidate + 재페치 + verify 강제 (무한 재귀 방지: 재귀 1회만, 다음 호출은 from_cache=false 또는 Bundled).
   - 네트워크 fresh → verify 강제 + 통과 시 mark_verified + 반환 manifest의 `signature_verified=true`.
   - Bundled → verify skip (은전).
7. **데스크톱 `verify_catalog_signature` 정책 동기화** — `signature_verified_marker` 인자 신설:
   - cache 적중 + marker=true → 즉시 `Verified { source: "X (cache+verified)" }`.
   - cache 적중 + marker=false → 네트워크 verify 흐름 진입 (cache poisoning 방어).
   - verify 성공 → `RegistryFetcher::mark_signature_verified(source, "catalog")` 호출 + `Verified` 반환.

## 근거

- **schema v2 with ALTER COLUMN**: SQLite는 `WITHOUT ROWID` 테이블에서도 ADD COLUMN 지원. 기존 사용자 데이터 손실 0, 다음 fetch 시 자동 verify로 마커 채움.
- **default 0 (unverified) for legacy rows**: 안전 default — verifier 활성 빌드면 invalidate + 재페치로 자연스럽게 마이그레이션. verifier 비활성 빌드는 marker 무시 (현재 코드처럼 작동).
- **caller-controlled marker**: 정책 결정(verify 통과 의미)을 fetcher core가 아닌 *caller*가 가짐. registry-fetcher는 mechanism만 제공, policy는 desktop layer가 결정 — clean architecture.
- **재귀 1회 보장**: invalidate 후 재페치는 cache miss → 네트워크 또는 Bundled로 빠짐. 두 경로 모두 verify를 통과하거나(network) skip(Bundled)하므로 무한 재귀 0.
- **`#[serde(default)]` 백워드 호환**: 기존 직렬화된 `FetchedManifest`(snapshot/replay 시나리오)가 `signature_verified` 누락이어도 deserialize OK (기본값 false).

## 거부된 대안

1. **schema v1 그대로 + 별개 `signature_meta` 테이블**: JOIN 2개 + 트랜잭션 일관성 추가 부담. 단일 컬럼 추가가 더 간결.
2. **`signature_verified` 검사를 desktop layer가 자체 cache로 처리**: ADR-0047의 verify_catalog_signature 위치에 Mutex<HashMap<id, bool>> 추가. cache를 별개 layer로 두면 fetcher core와 동기화 깨짐 위험. SQLite row에 함께 두는 것이 single source of truth.
3. **cache poisoning 방어를 *항상 verify로*** (cache hit이어도 매번 .minisig fetch + verify): 6시간 cron마다 catalog .minisig 재페치 → GitHub rate limit 위험. marker로 *마지막 verify 결과 trust*가 ROI 균형.
4. **invalidate 후 재페치를 caller에 위임**: `fetch_one_with_signature`가 미검증 cache row를 그대로 반환 + caller가 분기. caller 분기 누락 시 또 다른 cache poisoning 경로. fetcher core가 자체 처리하는 게 안전 default.
5. **marker를 in-memory만 보관(SQLite column 없음)**: 앱 재시작 시 lost — 매번 verify 강제됨 → rate limit. SQLite 영속이 cron 부하 0과 양립.
6. **schema 마이그레이션을 별개 sub-phase로**: 코드 변경 + 스키마 변경 = 동일 sub-phase가 atomic. 분리하면 중간 상태(코드는 v2 기대 + DB는 v1)에서 빌드 깨짐.
7. **`signature_verified`를 `Option<bool>`로 (NULL = 검증 시도 X)**: tri-state 분기 복잡. bool default false로 충분 (마커 의미: "이 row는 verify 통과한 적 있음").
8. **데스크톱 `verify_catalog_signature` 시그니처 변경 X (marker만 별개 IPC로)**: caller(refresh_once)가 두 호출 동기화 책임 — race 가능. 단일 함수 인자가 atomic.
9. **bundled tier도 verify 강제**: bundled은 빌드 시 코드 서명 + 큐레이션. 사용자 PC가 변조 못 함 (앱 자체가 변조됐다면 catalog 변조도 의미 없음). verify skip이 정합.

## 결과 / 영향

- **`crates/registry-fetcher/src/cache.rs`**:
  - `SCHEMA_VERSION_MAX` 1 → 2.
  - `CacheRow.signature_verified: bool` 필드 추가.
  - `CachePutInput.signature_verified: bool` 필드 추가.
  - `Cache::mark_verified()` 신규.
  - `init_schema`에 v1 → v2 ALTER 마이그레이션 + 신규 설치 v2 직접 진입 로직.
  - 기존 5개 테스트 invariant + 신규 4건(round_trip_true, round_trip_false, mark_verified_flips, fresh_install_v2).
- **`crates/registry-fetcher/src/fetcher.rs`**:
  - `FetchedManifest.signature_verified: bool` (`#[serde(default)]`).
  - `try_source` put: false / `try_bundled` put: true.
  - `fetch_one_with_signature` cache 적중 unverified → invalidate + 재페치 정책.
- **`crates/registry-fetcher/src/lib.rs`**:
  - `RegistryFetcher::mark_signature_verified()` 외부 노출.
- **`apps/desktop/src-tauri/src/registry_fetcher.rs`**:
  - `verify_catalog_signature` 시그니처에 `signature_verified_marker: bool` 추가.
  - cache 적중 + marker=true → 즉시 Verified.
  - verify 성공 → `mark_signature_verified` 호출.
  - `refresh_once`가 catalog row의 `signature_verified` marker 보존 + 전달.
- **백워드 호환**:
  - `FetchedManifest` 직렬화 백워드: `#[serde(default)]`로 누락 OK.
  - SQLite 스키마 마이그레이션은 자동 + 한 번만.
  - 사용자 PC의 기존 v1 cache는 첫 fetch에서 invalidate → 정상 verify 사이클 진입.
- **테스트**: registry-fetcher 33 unit (5 신규 invariant) + 9 integration. 결정 노트 §5 모든 invariant 충족.

## References

- 결정 노트: `docs/research/phase-r-b-catalog-trust-pipeline-decision.md`
- GPT Pro 검수: 2026-05-02 30-issue static review (S4 본 ADR로 해소)
- 코드:
  - `crates/registry-fetcher/src/cache.rs` (스키마 v2 + mark_verified)
  - `crates/registry-fetcher/src/fetcher.rs` (signature_verified 마커 plumbing)
  - `crates/registry-fetcher/src/lib.rs::mark_signature_verified` (외부 노출)
  - `apps/desktop/src-tauri/src/registry_fetcher.rs::verify_catalog_signature` (marker 검사)
- 관련 ADR: 0044 (live catalog refresh), 0047 (minisign verifier)
