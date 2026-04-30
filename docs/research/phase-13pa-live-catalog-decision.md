# Phase 13'.a — 모델 카탈로그 라이브 갱신 결정 노트 (2026-04-30)

> **목적**: registry-fetcher가 app manifest만 fetch하던 한계를 풀어 새 모델(Gemma 등)이
> 앱 재배포 없이 사용자에게 보이도록.
> **참조**: `docs/research/phase-next-gui-audit-deferred.md` Phase 13'.a 항목

## 1. 결정 요약

1. **단일 bundle JSON** 채택 — 50-200 모델 규모에서 per-file fetch보다 단순/안정 (research §3 vs Homebrew formulae 사례).
2. **jsDelivr 1순위 → GitHub Releases 2순위 → Bundled 3순위** — 한국 latency + 영구캐시 우위 (research §2).
3. **Repo URL hardcode 수정** — `lmmaster/lmmaster` → `freessunky-bit/lmmaster` (실제 GitHub repo).
4. **bundled_dir path fix** — `manifests/snapshot/apps`(없는 경로) → `manifests/apps`(실제 위치).
5. **CatalogState hot-swap** — 받은 body를 `Catalog::from_entries`로 deserialize 후 atomic swap. 진행 중 사용자 액션은 Arc 캡처로 무영향.
6. **build script** `.claude/scripts/build-catalog-bundle.mjs` — per-file → 단일 bundle 자동 빌드. 매니페스트 추가 후 1회 실행.

## 2. 채택안

### 2.1 Bundle 구조

`manifests/apps/catalog.json` — 단일 `ModelManifest` 형식, 모든 모델 entries 합본.

```json
{
  "$schema_hint": "model-registry ModelManifest schema_version=1. Auto-generated.",
  "schema_version": 1,
  "generated_at": "2026-04-30T14:30:00Z",
  "entries": [
    { "id": "deepseek-r1-7b", ... },
    { "id": "exaone-3.5-7.8b-instruct", ... },
    ... (id 알파벳 순)
  ]
}
```

build script가 `manifests/snapshot/models/**/*.json`을 walk → 각 manifest의 entries 병합 → 중복 id 검사 → id 정렬 → `manifests/apps/catalog.json` 출력.

### 2.2 Source 우선순위

```rust
default_sources(github_tag, jsdelivr_ref) → vec![
    SourceConfig::Jsdelivr {                                      // 1순위
        url: "https://cdn.jsdelivr.net/gh/freessunky-bit/lmmaster@{ref}/manifests/apps/{id}.json",
        timeout: 6s,
    },
    SourceConfig::Github {                                        // 2순위
        url: "https://github.com/freessunky-bit/lmmaster/releases/download/manifests-{tag}/{id}.json",
        timeout: 8s,
    },
    SourceConfig::Bundled { timeout: 500ms },                     // 3순위 (네트워크 0 fallback)
]
```

manifest_id "catalog"이면 위 URL 템플릿이 `catalog.json`으로 resolve.

### 2.3 Hot-swap 흐름

```
[6h cron tick] OR [사용자 "다시 불러오기" 클릭]
  → RegistryFetcherService::refresh_once
    → fetch_all(["ollama", "lm-studio", "catalog"])
      → 각 id별 4-tier fallback (jsdelivr → github → bundled)
      → ETag/If-Modified-Since로 304 시 캐시 그대로
    → catalog.json body 추출
      → CatalogState::swap_from_bundle_body
        → ModelManifest deserialize + schema_version 체크 + entries 비어있지 않음 검사
        → Catalog::from_entries → Arc::new → RwLock::write::swap
    → emit "catalog://refreshed" event
  → Catalog.tsx onCatalogRefreshed listener fires
    → reload() — getCatalog() 재호출 → 새 entries로 UI 갱신
```

### 2.4 Failure modes (graceful)

| 단계 | 실패 | 처리 |
|---|---|---|
| 모든 네트워크 tier 404 | catalog 갱신 X | bundled tier 시도 → bundled도 없으면 stale fallback 유지 (`reload_from_bundled` 호출) |
| body JSON parse 실패 | catalog 갱신 X | warn 로그 + `reload_from_bundled` 폴백 |
| schema_version != 1 | catalog 갱신 X | error 로그 + stale 유지 — 앱 업데이트 신호 |
| entries 빈 배열 | catalog 갱신 X | error 로그 — 실수로 빈 bundle 푸시 방지 |

## 3. 기각안 + 이유 (negative space)

**A. Per-file index + `*.json` per-model fetch (research Option B)**
- ❌ 거부: 200 모델 × 200 req → HTTP/2 필요 + ETag 관리 복잡. Homebrew formulae(수만 개)도 단일 bundle 사용. LMmaster 50-200 규모에선 overkill. Future bump (1000+ 모델) 시 schema_version 2로 마이그레이션 가능.

**B. GitHub Contents API listing (research Option C)**
- ❌ 거부: unauth 60/h rate limit — 첫 실행 spike 시 차단 위험. 신뢰성 0.

**C. minisign 서명 즉시 도입**
- ❌ 거부 (v1.x로 deferred): 보안 정당성 강함 (research §4). 그러나 Phase 13'.a의 핵심 가치는 "Gemma 보임" — 서명까지 추가하면 1 sub-phase 분량을 넘김. 별도 Phase 13'.a.1 또는 Phase 14로 분리. **위험 기록**: 서명 없으니 repo 탈취 시 임의 모델 catalog 주입 가능. 사용자 PC에 풀까지 트리거되진 않음 (사용자 명시 클릭 필요) — pull 자체가 사용자 동의 + Ollama 4xx fallback 있어 blast radius 제한적. v1.x에서 ADR-0046으로 minisign 추가.

**D. arc-swap::ArcSwap 즉시 마이그레이션**
- ❌ 거부 (다음 페이즈): RwLock<Arc>은 read 시 lock 취득(매우 짧음)이지만 wait-free 아님. ArcSwap이 더 우월하나, 현 throughput 문제 없음 (catalog read는 페이지 진입 시 1회). 측정 후 필요 시 마이그레이션. negative space: 측정 안 했는데 마이그레이션 = 검증 없는 최적화.

**E. `@main` ref 사용 (commit hash 대신)**
- ❌ 거부: jsDelivr 영구캐시 의미 사라짐 + 매 push마다 캐시 무효 + GC 위험 (research §흔한 함정 1). v1은 `CARGO_PKG_VERSION` 사용, v1.x에서 commit hash 핀 자동화 (release CI).

**F. catalog body 없으면 reload_from_bundled로 무조건 폴백**
- ❌ 거부: catalog id가 fetch 실패하면 frontend는 stale 유지. reload_from_bundled가 같은 stale 결과 반환 → 의미 없음. 본 결정은 "catalog body 받았을 때만 swap, 아니면 stale 유지"로 단순화.

**G. catalog manifest에 별도 path prefix (`/catalog/...`)**
- ❌ 거부 (단순화): `manifests/apps/catalog.json`에 두면 기존 SourceConfig URL 템플릿(`{id}.json`) 그대로 재사용. 별도 source set 불필요. 디렉터리 이름이 "apps"라도 의미상 "registry-fetched manifests" 통합 디렉터리로 재해석.

## 4. 미정 / v1.x 이월

- **minisign 서명** — 위 §3.C. ADR-0046 후보.
- **ArcSwap 마이그레이션** — 측정 후 결정.
- **per-file incremental fetch** — 1000+ 모델 도달 시 schema bump.
- **commit hash auto-pinning** — release CI에 자동화.
- **build-catalog-bundle.mjs CI 통합** — 현재 매뉴얼. release.yml에 통합해 PR 시 자동 빌드 + verify.
- **catalog.json size monitoring** — 1MB 넘으면 압축(gzip) 전송 검토.

## 5. 테스트 invariant

| 영역 | invariant |
|---|---|
| `build-catalog-bundle.mjs` | (a) 모든 per-file의 entries가 bundle에 포함, (b) 중복 id 거부 (exit 1), (c) id 알파벳 순 정렬, (d) UTF-8 출력 (BOM 없이) |
| `default_sources` | jsDelivr 1순위, GitHub 2순위, Bundled 3순위. URL에 `freessunky-bit/lmmaster` 포함. (테스트 추가됨) |
| `CatalogState::swap_from_bundle_body` | (a) 정상 ModelManifest body → swap + count 반환, (b) schema_version != 1 → Err, (c) entries 비어있음 → Err, (d) JSON parse 실패 → Err |
| `RegistryFetcherService::refresh_once` | catalog body 수신 → `swap_from_bundle_body` 호출. 실패 시 `reload_from_bundled` 폴백. event emit. |
| Frontend `Catalog.tsx::onCatalogRefreshed` | 이벤트 도착 시 `getCatalog()` 재호출, entries 즉시 갱신 |

## 6. 다음 페이즈 인계

**진입 조건**:
- 본 결정 노트 + ADR-0044 commit
- `manifests/apps/catalog.json` (12 entries) commit + push (jsDelivr가 24시간 내 propagate)

**Phase 13'.b** (다음): Diagnostics MOCK 4건 제거 + gateway metrics 미들웨어. 본 페이즈와 독립.

**위험 노트**:
- catalog.json 푸시 후 jsDelivr 캐시 propagation 지연 가능 — 첫 사용자가 24시간 내에 `freessunky-bit/lmmaster@HASH` ref가 미존재면 304 fallback이 안 일어남. 첫 release 시 `@main` 임시 사용 후 commit hash 핀으로 전환.
- 사용자 manifest add → build-catalog-bundle.mjs 재실행 잊으면 원격 stale. README + CONTRIBUTING.md에 명시 권장.
- frontend `onCatalogRefreshed` 리스너가 entry change 감지 못 함 — `setLastRefresh`만 갱신. **확인 필요**: 별도 reload effect가 정상 동작하는지.

## 7. 검증 결과 (예정)

```
✓ cargo test -p registry-fetcher (jsDelivr-first invariant)
✓ cargo check --workspace
✓ pnpm exec tsc -b
✓ build-catalog-bundle.mjs: 12 entries, no duplicates, alphabetical
✓ ACL drift 0
```

## 8. ADR

본 결정 노트와 짝으로 `docs/adr/0044-live-catalog-refresh.md` 신설.
