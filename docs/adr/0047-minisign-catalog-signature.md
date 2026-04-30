# ADR-0047 — minisign-verify로 카탈로그 번들 서명 검증 (infrastructure)

* **상태**: 채택 (2026-04-30 인프라, 2026-05-01 wiring 완료) — Phase 13'.g(인프라) + 13'.g.2.a(option_env! pubkey loader) + 13'.g.2.b(resolve_signature_url helper) + 13'.g.2.c(FetcherCore wiring + caller 통합 + Diagnostics SignatureSection) + 13'.g.2.d(CI 서명 파이프라인 YAML) 모두 머지 완료. v1 ship 가능.
* **컨텍스트**: Phase 13'.a로 카탈로그 번들이 jsDelivr → GitHub raw 흐름으로 사용자 PC에 도달. 무서명이라 GitHub repo가 손되거나 jsDelivr 캐시가 오염되면 *임의 카탈로그가 사용자 PC에 들어옴*. 서명 검증 추가 필수. 단 v1 ship 전 실 키페어 + CI 서명 파이프라인 통합은 별도 운영 작업이라 본 ADR은 *infrastructure only* — verify 코드 + 결정 보존, wiring은 v1.x.
* **결정 노트**: `docs/research/phase-13pg-catalog-signature-decision.md`

## 결정

1. **`minisign-verify` v0.2** crate를 `registry-fetcher`에 추가 — zero-deps Rust verify-only.
2. **`signature::SignatureVerifier`** struct — primary + (optional) secondary pubkey. dual key 패턴 (Tauri Updater + reachy-mini의 90일 overlap 키 회전 차용).
3. **`SignatureError`** 5종 (`NoPublicKey`, `InvalidPublicKey`, `InvalidSignature`, `VerifyFailed`) — 한국어 사용자 향 메시지.
4. **runtime constructor** `from_minisign_strings(primary, secondary)` — 빌드 시점 임베드는 v1.x에 별도 wire (env var 또는 build.rs).
5. **catalog 자동 fetch + Diagnostics 빨간 카드 + bundled fallback wiring**은 v1.x로 deferred — 본 ADR은 검증 인프라만.
6. **실 keypair fixture 기반 round-trip 테스트**도 v1.x — CI에서 `rsign generate` 후 임베드.

## 근거

- **minisign-verify zero-deps**: rust-minisign(전체)은 sign 측 의존성 (rpassword/scrypt 등)을 끌고 옴. verify-only면 no-runtime 부담. 클라이언트는 검증만 하므로 충분.
- **dual pubkey rotation**: 단일 키 + 강제 앱 업데이트는 한국 데스크톱 UX에 부적합 (사용자 마찰 큼). 90일 overlap이 사실상 표준 (reachy-mini KEY_ROTATION.md 참조).
- **infrastructure only 분할**: 실 키페어 + CI secret + GitHub Actions 워크플로 + Diagnostics 카드는 *각각 별개의 운영 작업*. 본 sub-phase에 묶으면 토큰 폭발. 검증 코드만 먼저 머지.
- **한국어 카피 톤**: `VerifyFailed` 메시지는 "카탈로그가 변조됐거나 잘못된 키로 서명됐어요" — 사용자가 무엇이 일어났는지 추정 가능 + 패닉 톤 회피.

## 거부된 대안

- **rsign2 lib 직접 사용** — CLI 전용. lib API 미제공. 거부.
- **rust-minisign 풀 사용** — sign 측 deps (rpassword, scrypt) 끌고 옴. 클라이언트엔 verify-only로 충분. 거부.
- **ed25519-dalek 직접 + 자체 minisign 포맷 파서** — minisign 포맷 호환성 직접 구현 부담. minisign-verify가 그 일을 이미 해줌. 거부.
- **Sigstore / Cosign** — OCI registry / OIDC keyless 시나리오 지향. 1인 메인테이너 + 단일 JSON 번들에 overkill. ecosystem fragmentation 우려 (Mendhak 분석). 거부.
- **GitHub OIDC keyless** — Cosign 전제. 거부.
- **TUF (The Update Framework)** — root/targets/snapshot/timestamp 4 role + delegation 모델. 다중 메인테이너 + 다단계 신뢰 위임 시나리오. LMmaster 1인 + 단일 카탈로그에 overkill. v2 멀티 채널 도달 시 재검토.
- **age / Hashicorp Vault** — 키 관리 인프라 over-engineering. GitHub Encrypted Secrets로 충분.
- **단일 키 + 강제 재설치** — UX 마찰 큼. 거부.
- **자동 재시도** — 네트워크 오염 시 무한 루프 위험. 거부.
- **silent fallback (경고 없이 bundled로 강등)** — 보안 사고 은폐. 거부 — Diagnostics 빨간 카드 필수 (v1.x).

## 결과 / 영향

- `registry-fetcher` 의존성: minisign-verify v0.2 추가 (zero-deps이라 빌드 부담 미미).
- `SignatureVerifier` API 노출 — 외부에서 verify 호출 가능.
- **현재 catalog 흐름은 변경 없음** — `FetcherCore`는 signature를 *아직 호출하지 않음*. 본 ADR은 인프라만.
- **검증 4 + 1 ignored 단위 테스트** — 형식 거부 / 한국어 메시지 / round-trip placeholder.
- v1.x 후속 작업 (별도 ADR 필요할 수 있음):
  - 빌드 시점 pubkey 임베드 (`build.rs` + env `LMMASTER_CATALOG_PUBKEY{,_SECONDARY}`).
  - GitHub Actions 워크플로 (rsign sign + `<id>.json.minisig` 산출 + 동시 게시).
  - `FetcherCore::fetch_one`에 sig 부탁 — `<id>` body fetch 후 `<id>.minisig` 추가 fetch + verify.
  - 검증 실패 시 bundled fallback + Diagnostics 빨간 카드 (Phase 13'.b 인프라 활용).
  - 키 회전 SOP — 12개월 권장.

## References

- 결정 노트: `docs/research/phase-13pg-catalog-signature-decision.md`
- 관련 ADR: ADR-0001 (gateway 정책), ADR-0044 (live catalog refresh), ADR-0046 (gateway metrics — Diagnostics 인프라).
- 코드:
  - `crates/registry-fetcher/Cargo.toml` (minisign-verify v0.2 deps).
  - `crates/registry-fetcher/src/signature.rs` (SignatureVerifier + SignatureError + 5 unit tests).
  - `crates/registry-fetcher/src/lib.rs` (export).
- 외부:
  - [rust-minisign-verify](https://github.com/jedisct1/rust-minisign-verify) — zero-deps Rust verify-only.
  - [Tauri 2 Updater](https://v2.tauri.app/plugin/updater/) — 동일 Ed25519 minisign 포맷.
  - [reachy-mini KEY_ROTATION.md](https://github.com/pollen-robotics/reachy-mini-desktop-app/blob/develop/KEY_ROTATION.md) — 12개월 회전 + dual pubkey 패턴.
  - [GitHub Encrypted Secrets](https://docs.github.com/en/actions/security-guides/encrypted-secrets) — libsodium sealed boxes.
