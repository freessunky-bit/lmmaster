# Phase R-B — Catalog Trust Pipeline 결정 노트

> 2026-05-03. GPT Pro 정적 검수 30건 중 v0.0.1 ship-blocker 보안 + 신뢰 카테고리(S3+S4+S5+R4+T2)를 본 sub-phase에서 해소. SQLCipher caller wiring(S3 후속)은 #38로 분리.

## 1. 결정 요약

- **D1 (T2)**: minisign-verify 0.2.5 자체 fixture(`RWQf6LRC...` + body `b"test"` + 1556193335 timestamp prehashed sig)로 round-trip 테스트 활성화. 4 invariant — 정상 verify / 변조 거부 / dual-key fallback / 키 미일치 거부.
- **D2 (T2 보강)**: `from_minisign_strings`에 bare base64 + multi-line 두 형식 모두 수용. CI env var 친화 + `rsign generate` 출력 그대로 둘 다 작동.
- **D3 (S3)**: `crates/knowledge-stack/Cargo.toml`에 `sqlcipher` feature gate 추가 + `KnowledgeStore::open_with_passphrase()` 메서드 신설. caller wiring은 #38 분리.
- **D4 (S4)**: `manifest_cache` 스키마 v2 — `signature_verified` 컬럼 추가 + ALTER 마이그레이션. cache poisoning 방어.
- **D5 (S4)**: `CachePutInput`/`CacheRow`/`FetchedManifest`에 `signature_verified` 필드 추가. caller가 적재 시점에 명시.
- **D6 (S4)**: `Cache::mark_verified` + `RegistryFetcher::mark_signature_verified` 외부 노출. desktop helper가 verify 통과 시 호출.
- **D7 (S5)**: `verify_catalog_signature` 시그니처에 `signature_verified_marker` 인자 추가. cache 적중 + marker=true → 즉시 Verified, marker=false → 네트워크 verify 강제.
- **D8 (S5)**: `fetch_one_with_signature` cache 적중 시 row의 marker 검사 → false면 invalidate + 재페치 1회. 무한 재귀 방지(Bundled 또는 fresh로 빠짐).
- **D9 (R4)**: 기존 release.yml + sign-catalog.yml 검수 — sound. SECRETS_SETUP.md 가이드 완비. 코드 변경 0.
- **D10**: ADR-0053 (knowledge-stack SQLCipher) + ADR-0054 (cache verified marker) 두 ADR로 분기.

## 2. 채택안

### D1 — minisign round-trip 4 invariant

```rust
const FIXTURE_PUBKEY: &str = "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
const FIXTURE_SIG: &str = "untrusted comment: signature from minisign secret key
RUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=
trusted comment: timestamp:1556193335\tfile:test
y/rUw2y8/hOUYjZU71eHp/Wo1KZ40fGy2VJEDl34XMJM+TX48Ss/17u3IvIfbVR1FkZZSNCisQbuQY+bHwhEBg==";
const FIXTURE_BODY: &[u8] = b"test";

#[test] fn round_trip_with_real_keypair_verifies() { ... }
#[test] fn round_trip_rejects_tampered_body() { ... }   // body 변조 → VerifyFailed
#[test] fn round_trip_rejects_wrong_primary_with_correct_secondary() { ... }  // dual-key
#[test] fn round_trip_rejects_when_no_key_matches() { ... }   // 두 키 모두 불일치 → VerifyFailed
```

### D2 — `parse_pubkey` private helper

```rust
fn parse_pubkey(s: &str) -> Result<PublicKey, SignatureError> {
    if s.lines().count() >= 2 {
        return PublicKey::decode(s)...;  // multi-line ("untrusted comment: ...\nRWQ...")
    }
    PublicKey::from_base64(s.trim())...   // bare base64 ("RWQ...")
}
```

CI env var는 보통 한 줄 base64로 등록 — `from_base64` fallback이 친화적. `rsign generate` 출력의 두 줄 형식도 그대로 작동.

### D3 — knowledge-stack SQLCipher feature gate

```toml
[features]
default = []
embed-onnx = ["dep:ort", "dep:tokenizers", "dep:ndarray"]
sqlcipher = ["rusqlite/bundled-sqlcipher-vendored-openssl"]   # 신규
```

```rust
pub fn open_with_passphrase(path: &Path, passphrase: &str) -> Result<Self, KnowledgeError> {
    let conn = Connection::open(path)?;
    apply_passphrase(&conn, passphrase)?;       // PRAGMA key + sqlite_master 검증
    apply_stability_pragmas(&conn)?;             // WAL/busy_timeout/synchronous
    let mut store = Self { conn };
    store.init_schema()?;
    Ok(store)
}
```

caller wiring(knowledge.rs가 keyring 통한 passphrase 적용 + 마이그레이션 + per-workspace secret)은 sub-phase #38로 분리. 본 ADR은 *infrastructure only*.

### D4~D6 — schema v2 + signature_verified 마커

```sql
-- 신규 설치 (CREATE TABLE):
CREATE TABLE manifest_cache (
    ...,
    signature_verified  INTEGER NOT NULL DEFAULT 0,
    ...
);

-- 기존 v1 → v2 마이그레이션:
ALTER TABLE manifest_cache ADD COLUMN signature_verified INTEGER NOT NULL DEFAULT 0;
UPDATE schema_meta SET value = '2' WHERE key = 'version';
```

```rust
pub struct CacheRow { ..., pub signature_verified: bool, }
pub struct CachePutInput { ..., pub signature_verified: bool, }
pub struct FetchedManifest { ..., #[serde(default)] pub signature_verified: bool, }

impl Cache {
    pub async fn mark_verified(source, manifest_id) -> Result;
}

impl RegistryFetcher {
    pub async fn mark_signature_verified(source, manifest_id) -> Result;  // delegate
}
```

put 시점 정책:
- `try_source` (네트워크 fresh) → `false`
- `try_bundled` → `true` (빌드 시점 신뢰)
- 사용자 caller (`fetch_one` 단독, no verifier) → `false`

### D7 — desktop verify_catalog_signature signature 보강

```rust
async fn verify_catalog_signature(
    fetcher: &RegistryFetcher,
    body: &[u8],
    source: SourceTier,
    from_cache: bool,
    signature_verified_marker: bool,    // 신규
) -> Result<CatalogSignatureStatus, ()> {
    if !source.is_network() {
        return Ok(BundledFallback { at_ms });
    }
    if from_cache && signature_verified_marker {
        return Ok(Verified { source: "X (cache+verified)" });
    }
    // network verify path...
    match verifier.verify(body, &sig_text) {
        Ok(()) => {
            fetcher.mark_signature_verified(source, "catalog").await;
            Ok(Verified { ... })
        }
        Err(e) => Ok(Failed { reason: e.to_string() }),
    }
}
```

### D8 — fetch_one_with_signature 재귀 1회

```rust
if manifest.from_cache {
    let row = self.cache.get(manifest.source, id).await?;
    if matches!(row, Some(ref r) if r.signature_verified) {
        return Ok(manifest);
    }
    self.cache.invalidate(Some(id)).await;
    return Box::pin(self.fetch_one_with_signature(id, verifier)).await;
}
// 재귀 후: from_cache=false 또는 Bundled → 무한 재귀 X
```

### D9 — release workflow 검수 결과

기존 `.github/workflows/release.yml`:
- ✅ Cross-platform matrix (Win/Mac/Linux)
- ✅ TAURI_SIGNING_PRIVATE_KEY + PASSWORD secrets wired
- ✅ pre-flight: fmt + clippy + tests (`--exclude lmmaster-desktop`)
- ✅ tauri-action: bundle + sign + GitHub Release(draft)
- ✅ releaseDraft: true (사용자가 publish — 안전 게이트)
- ✅ Auto-generated release notes via `gh release edit --generate-notes`
- ✅ SHA256 + size in job summary
- ✅ Korean install notes

기존 `.github/workflows/sign-catalog.yml`:
- ✅ catalog.json push 시 자동 트리거
- ✅ rsign2 install + sign
- ✅ Graceful skip if `CATALOG_MINISIGN_SECRET_KEY` unset
- ✅ Self-check verify with `CATALOG_MINISIGN_PUBKEY`
- ✅ Auto-commit + push with `[skip-sign]` (recursion 방지)

`.github/SECRETS_SETUP.md`: §1 (Tauri Updater) + §1.b (Catalog signing) 모두 한국어 가이드.

**결론**: R-B.5 코드 변경 0. workflows는 sound.

## 3. 기각안 + 이유

| # | 기각안 | 이유 |
|---|---|---|
| 1 | T2 테스트에 `rust-minisign` dev-dep 추가해 ephemeral keypair 생성 | minisign-verify 0.2.5의 자체 fixture가 이미 검증된 known-good 벡터. 추가 dev-dep 불필요 |
| 2 | `from_minisign_strings`이 *오직* multi-line만 지원 | CI env var는 `\n` escape 부담. bare base64 fallback이 더 친화적 |
| 3 | knowledge-stack에 caller wiring까지 본 sub-phase 포함 | 8+ IPC + 마이그레이션 + keyring 관리 = 자체 sub-phase. R-B 범위 외 → #38로 분리 |
| 4 | passphrase를 `KnowledgeStore` struct에 저장 후 lazy apply | `Connection::open` 직후 race 위험. 즉시 apply가 정공 |
| 5 | knowledge-stack SQLCipher 별개 crate(`knowledge-stack-sqlcipher`)로 분리 | feature gate 1줄이 더 간결. 별개 crate는 호출자 cfg 분기 cost 큼 |
| 6 | OpenSSL 시스템 의존(`bundled-sqlcipher` 옵션) | Windows/macOS에서 시스템 OpenSSL 부재 → 빌드 깨짐. vendored가 portable |
| 7 | schema v1 그대로 + 별개 `signature_meta` 테이블 | JOIN + 트랜잭션 일관성 부담. 단일 컬럼 추가가 간결 |
| 8 | signature_verified 마커를 desktop layer 자체 cache로 처리 | fetcher core와 동기화 깨짐 위험. SQLite row가 single source of truth |
| 9 | cache poisoning 방어를 *항상 verify*(매번 .minisig 재페치) | 6시간 cron마다 GitHub rate limit 위험. marker로 last-verify trust가 ROI 균형 |
| 10 | invalidate 후 재페치를 caller에 위임 | caller 분기 누락 시 cache poisoning 경로. fetcher core 자체 처리가 안전 default |
| 11 | marker를 in-memory만 보관 | 앱 재시작 시 lost → 매번 verify → rate limit. SQLite 영속이 자연 |
| 12 | schema 마이그레이션을 별개 sub-phase | 코드 + 스키마 변경 = 같은 sub-phase atomic. 분리하면 중간 상태 빌드 깨짐 |
| 13 | `signature_verified`를 `Option<bool>` tri-state | 분기 복잡. bool default false로 충분 |
| 14 | bundled tier도 verify 강제 | bundled은 코드 서명 + 큐레이션. 앱 자체가 변조됐다면 catalog 변조도 의미 없음 |
| 15 | release.yml에 `--features sqlcipher` 빌드 step 추가 | OpenSSL 컴파일 ~30s 추가. v0.0.1 ship에 SQLCipher caller wiring 미적용이라 ROI 낮음. 추후 #38 머지 시 추가 |
| 16 | T2 테스트에 self-signed pubkey + body 전체 시퀀스를 fresh 생성 | minisign-verify 자체 fixture로 충분 — cryptographic primitive 검증 X (test 목적이 아님) |

## 4. 미정 / 후순위 이월

- **Sub-phase #38 (knowledge-stack caller wiring)** — `apps/desktop/src-tauri/src/knowledge.rs`가 `open_with_passphrase`를 사용:
  - keyring entry username 신설(`knowledge-secret`).
  - `provision()` 함수 mirror (key-manager 패턴).
  - 평문 → 암호화 마이그레이션 (`migrate_unencrypted_to_encrypted` mirror).
  - per-workspace passphrase 키 도출 (단일 keyring secret 재사용 또는 HKDF).
  - 평문 fallback (Linux headless, keyring 미접근).
- **CI 매트릭스에 `--features sqlcipher` 빌드 추가** — caller wiring 머지 후 추가.
- **`signature_verified` marker UI 노출** — Diagnostics SignatureSection이 cache hit + marker=true 시 "(cache+verified)" prefix 표시 (이미 구현, i18n 추가 필요 — Phase R-D 묶음).
- **catalog.json만 verify** — 다른 manifest(ollama.json, lm-studio.json 등)는 현재 `fetch_one`만 사용. catalog 외 manifest signature 정책은 v1.x.
- **lmmaster-desktop crate 단위 테스트 Windows DLL 한계** — Tauri 2 plugin DLL 의존성 변경 없음. R-A에서 documented + portable-workspace 통합 테스트 38건이 회귀 보호.

## 5. 테스트 invariant

본 sub-phase가 깨면 안 되는 invariant:

1. **minisign round-trip OK**: `RWQf6LRC...` pubkey + 정상 body → `verify` Ok.
2. **minisign 변조 거부**: 같은 pubkey + 변조 body → `VerifyFailed`.
3. **minisign dual-key**: secondary 키만 일치해도 통과 (90일 overlap).
4. **minisign no-match 거부**: 두 키 모두 불일치 → `VerifyFailed`.
5. **pubkey 두 형식**: bare base64 / multi-line 모두 parse OK.
6. **knowledge-stack default open()**: 평문 모드 — 백워드 호환.
7. **knowledge-stack open_with_passphrase()**: SQLCipher feature 양쪽 빌드에서 작동.
8. **knowledge-stack sqlcipher only — wrong passphrase 거부**: 잘못된 키 → `NotADatabase`.
9. **cache schema v2 fresh**: 신규 설치 → `version='2'` + signature_verified 컬럼 존재.
10. **cache schema v1→v2 ALTER**: 기존 v1 사용자 → ALTER 자동 + 데이터 보존.
11. **cache marker round-trip**: put true → get true / put false → get false.
12. **cache mark_verified**: false 행을 true로 갱신.
13. **fetcher try_source put marker**: 네트워크 fresh → false.
14. **fetcher try_bundled put marker**: bundled → true.
15. **fetch_one_with_signature cache hit verified**: marker=true → 즉시 OK.
16. **fetch_one_with_signature cache hit unverified**: marker=false → invalidate + 재페치.
17. **FetchedManifest serde 백워드**: `signature_verified` 누락 deserialize → false default.
18. **desktop verify_catalog_signature marker=true**: 즉시 Verified.
19. **desktop verify_catalog_signature marker=false → network verify**: cache poisoning 방어.
20. **desktop verify success → mark_verified**: 다음 fetch에서 cache+verified 표시.

본 sub-phase 신규 invariant: **+9** (registry-fetcher 4 + signature 4 + knowledge-stack 1, 기존 24 → 33). 기존 invariant 0건 깨짐.

## 6. 다음 페이즈 인계

### 진입 조건

- ✅ R-B.1 (T2 minisign round-trip) 완료
- ✅ R-B.2 (S3 SQLCipher feature gate) 완료
- ✅ R-B.3 (S4 cache verified marker) 완료
- ✅ R-B.4 (S5 signed fetch wiring) 완료
- ✅ R-B.5 (R4 release workflow 검수) 완료
- ✅ R-B.6 (ADR-0053 + 0054 + 결정 노트) 완료
- ⏳ commit + push (사용자 승인 대기)

### 의존성

- **Phase R-C** (Network + Correctness) — S7 reqwest no_proxy + allowlist + C1 chat_stream EOF + R3 Client::new() fallback + C3 installer URL filename validation. R-B의 verifier 정책과 무관 → 병렬 진행 가능.
- **Phase R-D** (Frontend Polish) — K1+K2+K3 i18n emoji 제거 + Catalog hardcoded fallback + thiserror Korean + `errors.path-denied` 키 추가 + `(cache+verified)` 한국어 라벨.
- **Phase R-E** (Architecture v1.x) — A1 chat protocol decoupling + A2 bench trait + C2 OpenAI compat 공통화 + P1 KnowledgeStorePool + P4 channel cancel + R2 cancellation token + T3 wiremock — POST v0.0.1 release.
- **#38 (knowledge.rs caller wiring)** — R-B.2 후속. keyring + 마이그레이션 + per-workspace secret. POST R-B.

### 위험 노트

- **schema v2 ALTER idempotency**: 사용자가 빌드 변종 (v2 → v1 다운그레이드 → v2 업그레이드) 시 ALTER가 두 번 실행될 수 있음 → 두 번째는 "duplicate column" 에러로 silent ignore (`let _ = conn.execute_batch(...)`). 데이터 손실 0.
- **재귀 무한 방지**: `fetch_one_with_signature` 재귀 1회 — invalidate 후 재페치는 cache miss 또는 Bundled로 빠짐. 두 경로 모두 verify 통과 또는 skip → 추가 재귀 0.
- **marker 우회 가능성**: 사용자가 SQLite를 직접 편집해 `signature_verified=1`로 변조 시 우회 가능. 그러나 사용자 PC 자체가 신뢰 경계 — *원격 attacker*가 cache poisoning 못 한다는 게 본 sub-phase의 ScopedScope. 사용자 PC 변조는 ADR-0053 SQLCipher 적용 후 caller wiring 머지 시 (#38) 추가 보호.
- **catalog.json만 verify**: 다른 manifest(ollama.json 등)는 verify X — 현재 cache poisoning 가능. 그러나 ollama.json은 **app manifest** (어떻게 설치할지 안내 — URL/sha256 명시), catalog.json보다 영향 작음. v1.x 후속 ADR로.

### 다음 standby

**Phase R-C.1** (S7 reqwest no_proxy + allowlist) — `crates/registry-fetcher::FetcherCore::http`와 `apps/desktop/src-tauri/src/auto_updater.rs`의 reqwest::Client 생성을 audit. proxy 환경변수(`HTTP_PROXY`/`HTTPS_PROXY`)가 무시되는지 + 외부 통신이 화이트리스트 내인지 검증. ADR-0026 §1 외부 통신 0 정책 일관성.
