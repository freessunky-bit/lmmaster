# ADR-0044 — 모델 카탈로그 라이브 갱신 (single signed bundle via jsDelivr+GitHub fallback)

* **상태**: 채택 (2026-04-30)
* **컨텍스트**: Phase 13'.a — `registry-fetcher`가 app manifest만 fetch하던 한계로 새 모델 (Gemma 등)이 앱 재배포 없이 사용자에게 보이지 않음.
* **결정 노트**: `docs/research/phase-13pa-live-catalog-decision.md`

## 결정

1. **Single bundle JSON** — `manifests/apps/catalog.json` 단일 파일에 모든 모델 entries 합본.
2. **4-tier source 우선순위 변경** — `jsDelivr → GitHub Releases → Bundled`. (기존: github → jsdelivr → bundled).
3. **`CatalogState::swap_from_bundle_body`** — 받은 body를 deserialize 후 atomic swap.
4. **build script** — `.claude/scripts/build-catalog-bundle.mjs`로 per-file → bundle 자동 빌드.
5. **manifest_id "catalog"** — 기존 manifest_ids 리스트에 추가, fetcher가 동일 4-tier로 처리.

## 근거

- **single bundle** — 50-200 모델 규모에서 per-file fetch (200 req)보다 단순. Homebrew formulae(수만 개)도 동일 패턴.
- **jsDelivr 1순위** — 한국 latency 우선 (Seoul/Incheon POP 2-3ms vs GitHub 100-200ms). jsDelivr는 Cloudflare+Fastly+Bunny 다중 CDN으로 가용성도 높음. 영구 캐시(immutable on commit hash)도 우위.
- **hot-swap with RwLock<Arc>** — 진행 중 사용자 액션은 시작 시점 Arc 캡처로 무영향. arc-swap 마이그레이션은 측정 후 결정 (deferred).

## 거부된 대안

- **Per-file index + per-model fetch** — HTTP/2 + ETag 관리 복잡. 1000+ 모델 도달 시 schema_v2로 마이그레이션 가능.
- **GitHub Contents API** — unauth 60/h rate limit 위험.
- **minisign 서명** — 보안 정당하나 Phase 13'.a 범위 초과. v1.x ADR-0046 후보.
- **arc-swap 즉시 마이그레이션** — 측정 없는 최적화 거부.
- **`@main` ref** — 영구캐시 의미 상실 + GC 위험.

## 결과 / 영향

- 새 모델 추가 흐름: `manifests/snapshot/models/<cat>/<model>.json` 추가 → `node build-catalog-bundle.mjs` 실행 → commit + push → jsDelivr 24시간 내 propagate → 사용자 6h cron 또는 수동 클릭 시 자동 적용.
- 사용자 측: "다시 불러오기" 클릭 시 모델 카탈로그 + app 정보 모두 갱신. Catalog.tsx의 `onCatalogRefreshed` listener가 entries reload.
- 위험 (잠재): repo 탈취 시 임의 모델 catalog 주입 가능. blast radius는 사용자 명시 클릭(pull) 필요로 제한적. v1.x에 minisign 서명 추가 (ADR-0046).

## References

- 결정 노트: `docs/research/phase-13pa-live-catalog-decision.md`
- 보강 리서치 (subagent): jsDelivr / GitHub / Homebrew JSON API / VS Code Marketplace 비교
- 관련 ADR: ADR-0026 (외부 통신 0 정책 예외 — registry fetch), ADR-0042 (real embedder cascade)
- 관련 코드:
  - `crates/registry-fetcher/src/source.rs` (default_sources)
  - `apps/desktop/src-tauri/src/commands.rs` (CatalogState::swap_from_bundle_body)
  - `apps/desktop/src-tauri/src/registry_fetcher.rs` (refresh_once)
  - `apps/desktop/src-tauri/src/lib.rs` (manifest_ids, bundled_dir)
  - `.claude/scripts/build-catalog-bundle.mjs` (build script)
  - `manifests/apps/catalog.json` (12 entries, auto-generated)
