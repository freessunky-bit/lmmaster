# Phase 1' — `crates/registry-fetcher` 결정 노트

> 보강 리서치 (2026-04-27) 종합. Pinokio + Foundry Local + Homebrew Cask + Helm + jsDelivr + GitHub Releases 패턴 합성.

## 1. 핵심 결정

| 항목 | 결정 | 근거 |
|---|---|---|
| 4-tier 우선순위 | **Vendor → GitHub Releases → jsDelivr → Bundled** (sequential, first-success) | 한국어 사용자 + 사내망 비율 고려, 오프라인 보장 |
| 폴백 의미 | network/5xx/4xx → fall-through. **JSON parse error는 폴백 안 함** (cache poisoning 방지) | 손상 응답이 다른 미러로 전파되면 위험 |
| jsDelivr 핀 | `@<commit-or-tag>`, **절대 `@main` 금지** | ETag 안정 + 영구 캐시 (jsDelivr 수동 purge 회피) |
| ETag/If-Modified-Since | reqwest header 자동 + 304 시 `bump_fetched_at` + body 캐시 그대로 | RFC 7232 준수 |
| Stale-while-error | TTL 1h / grace 24h. 24h 내 + 네트워크 fail → stale 반환 + warn. 24h 초과 → bundled로 | RFC 5861 |
| DB | **rusqlite + `tokio::task::spawn_blocking`** (sqlx 미사용) | 단일 테이블 read-mostly. sqlx의 compile-time check 오버헤드 회피 |
| Schema | `manifest_cache` (source, manifest_id) PK + body_sha256 무결성 + WITHOUT ROWID | 작은 테이블 + 손상 자동 복구 |
| 동시성 | 단일 `Arc<tokio::sync::Mutex<Connection>>`. WAL 모드. r2d2 미사용 | 캐시 read <1ms — 풀링 불필요 |
| 단일 플라이트 | manifest_id 키로 중복 in-flight 요청 dedup (`Mutex<HashMap<String, Arc<Notify>>>`) | concurrent fetch 시 origin 1번만 hit |
| API 반환 | **raw bytes** (`AppManifest` 미파싱). `parse::<T>()` 헬퍼 별도 노출 | 304 round-trip + 다중 manifest type 지원 |
| 거버넌스 필드 | manifest schema에 `verification: { tier: "verified"\|"community", curator, signature_url?, signed_at? }` 추가 | Pinokio 패턴. Phase 1' MVP는 Verified만 |
| Bundled 위치 | crate가 `bundled_dir: PathBuf` 옵션으로 받음. Tauri는 `BaseDirectory::Resource`로 해결 | crate를 Tauri-agnostic하게 유지 |
| 한국어 에러 메시지 | `tracing`은 영어 structured + Korean 사용자 메시지 thiserror로 별도 | 디버깅 + UX 분리 |
| Cargo deps | reqwest + rusqlite(workspace) + serde + sha2 + hex + thiserror + tracing + backon + futures + time(HTTP-date) | 모두 기존 workspace deps. 신규 0 |
| Dev deps | wiremock 0.6 + tempfile + tokio test-util | installer crate에 동일 패턴 |

## 2. API 시그니처

```rust
pub struct RegistryFetcher { ... }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceTier { Vendor, Github, Jsdelivr, Bundled }

pub struct SourceConfig { tier, url_template, timeout }
pub struct FetcherOptions { cache_db, sources, bundled_dir?, ttl, stale_grace, http? }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchedManifest {
    source, manifest_id, from_cache, stale, fetched_at, etag?, body
}

impl RegistryFetcher {
    pub fn new(opts: FetcherOptions) -> Result<Self, FetcherError>;
    pub async fn fetch(&self, id: &str) -> Result<FetchedManifest, FetcherError>;
    pub async fn fetch_all(&self, ids: &[&str]) -> Vec<(String, Result<FetchedManifest, FetcherError>)>;
    pub async fn invalidate(&self, id: Option<&str>) -> Result<(), FetcherError>;
    pub fn parse<T: DeserializeOwned>(&self, fm: &FetchedManifest) -> Result<T, FetcherError>;
}
```

## 3. SQLite schema

```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS schema_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
INSERT OR IGNORE INTO schema_meta VALUES ('version', '1');

CREATE TABLE IF NOT EXISTS manifest_cache (
  source        TEXT NOT NULL CHECK(source IN ('vendor','github','jsdelivr','bundled')),
  manifest_id   TEXT NOT NULL,
  url           TEXT NOT NULL,
  body          BLOB NOT NULL,
  body_sha256   BLOB NOT NULL,
  content_type  TEXT,
  etag          TEXT,
  last_modified TEXT,
  fetched_at    INTEGER NOT NULL,
  PRIMARY KEY (source, manifest_id)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS manifest_cache_fetched ON manifest_cache(fetched_at);
```

## 4. 폴백 매트릭스

| Tier | Network err | 5xx | 4xx | 200 + JSON ok | 200 + JSON err | 304 |
|---|---|---|---|---|---|---|
| Vendor | →next | →next | →next + log err | success | **stop, error** | success (cached) |
| GitHub Releases | →next | →next | →next | success | **stop, error** | success |
| jsDelivr | →next | →next | →next | success | **stop, error** | success |
| Bundled | n/a | n/a | n/a | success | **stop, error** | n/a |

모든 네트워크 tier 실패 → cache 검사 → 24h 내 → stale, 24h 초과 → bundled.

## 5. 파일 구조 (총 ~865 LOC)

```
crates/registry-fetcher/
├── Cargo.toml                     ~35
├── src/
│   ├── lib.rs                     ~120
│   ├── source.rs                  ~100
│   ├── cache.rs                   ~180
│   ├── fetcher.rs                 ~220
│   └── error.rs                   ~60
└── tests/
    └── integration_test.rs        ~250
```

## 6. 검증 (11 통합 테스트)

1. `fetch_vendor_200`
2. `fallback_vendor_500_to_github`
3. `fallback_all_500_to_bundled`
4. `etag_round_trip` (304)
5. `ttl_expiry_refetches`
6. `stale_grace_offline`
7. `stale_grace_exceeded_falls_to_bundled`
8. `corrupt_cache_recovers` (sha256 mismatch)
9. `concurrent_no_double_write` (single-flight)
10. `json_parse_error_no_fallthrough`
11. `verified_field_parsed`

## 7. 비목표 (이번 sub-phase 외)

- Signature 검증 (minisign / sigstore) — Phase 5'+
- Community 매니페스트 사용자 등록 UI — Phase 4
- jsDelivr purge API 통합 — 후순위 (수동 purge로 충분)
- HTTP/3 / QUIC — reqwest 기본
