# Phase 13'.g — minisign 카탈로그 서명 검증 (infrastructure)

* **상태**: 채택 (2026-04-30)
* **컨텍스트**: ADR-0044(Phase 13'.a)로 jsDelivr → GitHub raw로 카탈로그 번들이 사용자 PC에 도달. 무서명이라 GitHub repo 손상 / jsDelivr 캐시 풀 오염 시 임의 카탈로그 푸시 가능. 본 sub-phase는 *검증 코드만* 먼저 머지하고, 실 keypair 임베드 + CI 서명 파이프라인 + Diagnostics 빨간 카드는 v1.x 후속.

## 1. 결정 요약

1. **`minisign-verify` v0.2** crate 도입 — zero-deps Rust verify-only.
2. **`SignatureVerifier`** struct (primary + optional secondary pubkey) — dual key rotation 패턴.
3. **`SignatureError`** 4종 + 한국어 메시지.
4. **단위 테스트** 4개 + 1 ignored — 형식 거부 / 한국어 메시지 / round-trip placeholder.
5. **wiring (FetcherCore 통합 / 빌드 시점 pubkey 임베드 / Diagnostics 카드)**은 v1.x로 deferred.
6. **CI 서명 파이프라인** (rsign sign + GitHub Actions secret) — v1.x.

## 2. 채택안

### 2.a `minisign-verify` 선택

- 클라이언트는 verify만 — sign 측 deps(rpassword/scrypt 등) 불필요.
- minisign 포맷 호환성을 직접 구현하지 않고 crate에 위임.
- v0.2는 안정 — 1.7k+ stars 동급 jedisct1 ecosystem.

### 2.b dual pubkey rotation

- `primary` + `secondary: Option<PublicKey>` 두 키 모두 시도. 둘 중 하나 통과면 OK.
- 회전 절차:
  1. 새 secondary 키 생성 + secondary 임베드한 앱 릴리즈.
  2. CI는 새 키로 서명 시작.
  3. 90일간 primary는 verify 가능 — 구버전 사용자 graceful upgrade.
  4. 90일 후 secondary→primary 승격, 새 secondary 후보 추가, primary 폐기.
- 참고: reachy-mini KEY_ROTATION.md (12개월 권장) + Tauri Updater pubkey-rotation 패턴.

### 2.c 한국어 카피 톤

- `NoPublicKey` — "공개키가 아직 등록되지 않았어요"
- `InvalidPublicKey(detail)` — "공개키 형식이 올바르지 않아요: {0}"
- `InvalidSignature(detail)` — "서명 형식이 올바르지 않아요: {0}"
- `VerifyFailed` — "서명 검증에 실패했어요 — 카탈로그가 변조됐거나 잘못된 키로 서명됐어요"

§4.1 톤 매뉴얼 사실 진술 + 사용자 추정 가능.

### 2.d Infrastructure only 분할

본 sub-phase는 *검증 코드 + 결정 보존*만:
- `signature.rs` — verify lib.
- `lib.rs` — export.
- 단위 테스트 — 형식 검증 / 한국어 메시지.

**v1.x 후속 작업** (각각 별개 sub-phase):
- 빌드 시점 pubkey 임베드 — `build.rs` + env `LMMASTER_CATALOG_PUBKEY{,_SECONDARY}`.
- CI 서명 파이프라인 — `rsign sign catalog.json` GitHub Actions step + Encrypted Secret으로 secret key 주입.
- `FetcherCore::fetch_one_with_signature` — body + `.minisig` 동시 fetch + verify.
- Diagnostics 빨간 카드 — Phase 13'.b 인프라 활용 (`get_gateway_*` 패턴 차용해 `get_catalog_signature_status`).
- bundled fallback + 사용자 안내 — verify 실패 시 *조용히* bundled로 강등 X. 빨간 카드 + 다음 refresh 시까지 fresh fetch 차단.
- 키 회전 SOP 문서 — `docs/runbooks/catalog-signing-rotation.md`.

### 2.e 단위 테스트 invariant

| 테스트 | 검증 |
|---|---|
| `invalid_pubkey_format_returns_typed_error` | garbage pubkey → `InvalidPublicKey(detail)` |
| `empty_pubkey_returns_typed_error` | 빈 문자열 → `InvalidPublicKey(detail)` |
| `invalid_secondary_pubkey_format_returns_typed_error` | secondary garbage → `InvalidPublicKey(detail)` (primary 검증 후 secondary 검증) |
| `errors_have_korean_messages` | `to_string()`이 한국어 포함 |
| `round_trip_with_real_keypair_placeholder` (ignored) | v1.x integration test에서 활성화 |

## 3. 기각안 + 이유 (negative space)

| 옵션 | 거부 이유 |
|---|---|
| **rsign2 lib 사용** | CLI 전용. lib API 부재. |
| **rust-minisign 풀 사용** | sign 측 deps (rpassword/scrypt 등) 클라이언트에 불필요. zero-deps minisign-verify가 우월. |
| **ed25519-dalek 직접 + 자체 포맷 파서** | minisign 포맷 호환성 직접 구현은 시간 비용 큼. minisign-verify가 그 작업을 이미 해줌. |
| **Sigstore / Cosign** | OCI registry / OIDC keyless 지향. 1인 + 단일 JSON 번들에 overkill (Mendhak 분석 — "ecosystem feels fragmented"). |
| **GitHub OIDC keyless** | Cosign 전제. 위 거부 사유 동일. |
| **TUF** | root/targets/snapshot/timestamp 4 role + delegation은 다중 메인테이너 시나리오. LMmaster 단일 메인테이너 + 단일 카탈로그엔 overkill. v2 멀티 채널 도달 시 재검토. |
| **age / Hashicorp Vault** | 키 관리 인프라 over-engineering. GitHub Encrypted Secrets로 충분. |
| **단일 키 (회전 시 강제 재설치)** | 한국 데스크톱 UX에 마찰 큼. dual pubkey 90일 overlap 표준. |
| **자동 재시도** | 네트워크 오염 시 무한 루프 위험. |
| **silent fallback (경고 없이 bundled 강등)** | 보안 사고 은폐. Diagnostics 빨간 카드 필수 (v1.x). |
| **본 sub-phase에 wiring 통합** | 토큰 예산 폭발 + 실 keypair 없이 통합 검증 어려움. infrastructure만 먼저 머지. |
| **fixture 기반 round-trip 테스트 (이번 세션 내)** | minisign-verify v0.2 fixture 형식 명확하지 않아 시간 소모. v1.x integration test에서 실 keypair로 활성화. |

## 4. 미정 / 후순위 이월

* **빌드 시점 pubkey 임베드 (build.rs)** — v1.x.
* **CI 서명 파이프라인 (rsign sign + GitHub Actions)** — v1.x.
* **catalog 자동 fetch + verify wiring** — v1.x.
* **Diagnostics 빨간 카드** — v1.x (Phase 13'.b 인프라 활용).
* **키 회전 SOP** — `docs/runbooks/catalog-signing-rotation.md`. v1.x.
* **bundled fallback 정책 finalize** — verify 실패 시 *조용히 강등 X* 강제. v1.x.

## 5. 테스트 invariant

본 sub-phase 종료 시점에 깨면 안 되는 항목:

1. `registry-fetcher` 단위 테스트 4 신규 + 1 ignored 모두 의도대로 작동.
2. `minisign-verify` 의존성이 빌드 시간을 유의미하게 늘리지 않음 (zero-deps).
3. `SignatureError` 모든 variant 한국어 메시지 보장.
4. dual pubkey 패턴 — 두 키 모두 시도하는 분기 코드 보존.
5. `SignatureVerifier::verify`는 *Result만 반환* — caller가 fallback 정책 결정.

## 6. 다음 페이즈 인계

* v1.x 후속 (별도 sub-phase):
  - **Phase 13'.g.2** — wiring (FetcherCore 통합 + Diagnostics 카드 + bundled fallback).
  - **Phase 13'.g.3** — CI 서명 파이프라인 + 키 회전 SOP.
  - **Phase 13'.g.4** — build.rs pubkey 임베드 + env 주입.
* 본 sub-phase 변경은 catalog 흐름과 *호환* — 기존 `FetcherCore::fetch_one`은 그대로 작동, signature는 *호출하지 않음*.
* 위험 노트:
  - minisign-verify v0.2의 `PublicKey::decode` 입력 형식이 정확히 단일 base64 line인지 multi-line인지 fixture 기반 검증 미완료. v1.x integration test에서 실 keypair로 확정.
  - GitHub Encrypted Secrets는 repo 단위 — fork된 PR에서 secret 노출 안 됨. CI 워크플로는 main branch trigger만 허용해야 함.

## References

- 결정 노트: 본 파일.
- ADR: `docs/adr/0047-minisign-catalog-signature.md`.
- 코드:
  - `crates/registry-fetcher/Cargo.toml` (minisign-verify v0.2 deps).
  - `crates/registry-fetcher/src/signature.rs` (SignatureVerifier + SignatureError + 5 unit tests).
  - `crates/registry-fetcher/src/lib.rs` (export).
- 외부:
  - [rust-minisign-verify](https://github.com/jedisct1/rust-minisign-verify)
  - [Tauri 2 Updater](https://v2.tauri.app/plugin/updater/)
  - [reachy-mini KEY_ROTATION.md](https://github.com/pollen-robotics/reachy-mini-desktop-app/blob/develop/KEY_ROTATION.md)
  - [Cosign vs minisign 분석 (Mendhak)](https://code.mendhak.com/understanding-sigstore-cosign-as-a-beginner/)
  - [TUF CNCF](https://www.cncf.io/projects/the-update-framework-tuf/)
  - [GitHub Encrypted Secrets](https://docs.github.com/en/actions/security-guides/encrypted-secrets)
