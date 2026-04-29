# ADR-0035: SQLCipher activation for KeyManager (Phase 8'.0.a)

- Status: Accepted
- Date: 2026-04-28
- Related: ADR-0007 (key-manager 자체 구현), ADR-0008 (SQLite + 옵션 SQLCipher), ADR-0022 (gateway routing policy + key store), ADR-0036 (Stability — single-instance / panic / WAL)
- 결정 노트: 본 ADR + `docs/research/phase-8p-9p-10p-residual-plan.md` §1.6.1

## Context

ADR-0008은 "SQLite + 옵션 SQLCipher"를 약속했지만 v1까지 default off였어요. `crates/key-manager/src/store.rs` 첫 주석 자체가 "SQLCipher는 v1 default off (env opt-in 미구현, v1.1 ADR)"이라 표기. v1 ship 직전 사용자가 `keys.db` 파일을 hex editor로 열면 argon2 hash + scope JSON이 그대로 노출돼요. argon2id로 hash됐으니 평문 키는 회복 불가지만 — alias / scope / 발급 시각이 평문이라 사용자 사생활 노출. 보안 약속을 v1 ship 전에 닫아야 해요.

## Decision

### 1. rusqlite `bundled-sqlcipher-vendored-openssl` feature 활성

- `crates/key-manager/Cargo.toml`: `rusqlite = { workspace = true, features = ["bundled-sqlcipher-vendored-openssl"] }`. workspace는 default features만 두고 crate 단위에서 추가.
- 빌드 시 SQLCipher + OpenSSL 통합 — 시스템 OpenSSL 의존 0. portable 정신 일관.
- CI/Windows 빌드에서 zlib/openssl 컴파일 오버헤드 ~30s 추가. 허용 범위.

### 2. OS 키체인에 32 byte secret 저장

- `keyring = "3"` workspace dep 활용. service=`lmmaster`, username=`keymanager-secret`.
- 첫 실행 시: 32 byte random → hex 인코딩 → keyring 저장 → SQLCipher PRAGMA key.
- 재실행: keyring에서 hex 읽기 → 그대로 PRAGMA key.

### 3. 평문 → 암호화 마이그레이션 wizard

- `KeyStore::migrate_unencrypted_to_encrypted(plain_path, encrypted_path, passphrase)`. ATTACH plaintext + `sqlcipher_export` + DETACH.
- 호출 후 평문 DB는 `keys.db.legacy.bak`으로 rename. 자동 삭제 X — CLAUDE.md §1 destructive 액션 정책상 사용자 명시 동의 필요.
- `apps/desktop/src-tauri/src/keys/migrate.rs`: `provision()` 한 entry-point. keyring 접근 → migrate → mode 반환.

### 4. Linux headless / 키체인 미접근 시 graceful fallback

- `KeyStoreMode::UnencryptedFallback { reason }` variant — `KeyManager::open_unencrypted` 사용.
- Settings 화면에 한국어 hint: "키체인을 쓸 수 없으면 평문 모드로 폴백돼요" (이미 i18n에 박힘).
- 외부 KMS / 사용자 비밀번호 입력 거부 — UX 마찰 + 외부 통신 0 위반.

### 5. KeyManager API 시그니처 확장

- `KeyManager::open(path, passphrase: &str)` — production 경로. passphrase는 caller(=`lib.rs::run`) 책임.
- `KeyManager::open_unencrypted(path)` — Linux fallback / 테스트.
- `KeyManager::open_memory()` — 메모리 DB (테스트). SQLCipher 미적용 (의미 없음).

## Consequences

### Positive

- v1 ship 직전 ADR-0008의 "옵션 SQLCipher" 약속 실현 — 사용자 PC 도난 / 외장 SSD 분실 시 키 메타데이터 노출 차단.
- OS 키체인은 사용자가 이미 신뢰하는 시스템 — 사용자 비밀번호 입력 UX 마찰 0.
- 마이그레이션은 atomic + 원본 백업 보존 — rollback 가능.
- `KeyStore::journal_mode()` 추가로 WAL 검증 unit 가능.

### Negative

- rusqlite SQLCipher feature는 OpenSSL 빌드 시간 +30s. CI 캐시로 흡수.
- keyring crate는 OS dependency — Linux GNOME Keyring 미실행 환경에서는 fallback로 빠짐. macOS는 first-class.
- 마이그레이션 실패 시 (FS 권한 에러 등) 새 빈 암호화 DB로 시작. 평문 DB는 그대로 보존 — 사용자에게는 "기존 키 다시 발급해 주세요" 안내. v1.b에서 retry UI 보강.
- SQLCipher 자체 panic 시 → 평문 fallback이 실패하면 메모리 DB로 강제 → 사용자가 키 재발급 필요.

## Alternatives considered

### A. 사용자 입력 비밀번호 (Bitwarden 스타일)

**거부 이유**: 매 실행 입력 마찰 + 한국 일반 사용자에게 UX 부담. 비밀번호 잊으면 데이터 영구 소실 — recovery flow 추가 필요. OS 키체인 자동화는 동일 보안 + 마찰 0.

### B. 평문 DB 유지 + OS file permission (Linux 0600)

**거부 이유**: Windows / macOS는 file ACL 표준이 다양해 portable 약속 깨짐. 외장 SSD 분실 시 ACL 0 → 평문 노출. ADR-0008 "옵션 SQLCipher" 약속 정면 위반.

### C. 외부 KMS (AWS Secrets Manager / HashiCorp Vault)

**거부 이유**: 외부 통신 0 정책(ADR-0013) 정면 위반. 사용자가 인터넷 끊긴 상황에서 키 사용 불가 → portable workspace 정신 무너짐. v1 사용자(개인 / 소규모 팀) 운영 부담 vs 이득 비대칭.

### D. argon2 hash만으로 충분 (별도 암호화 X)

**거부 이유**: 평문 키는 hash로 보호되지만 alias / scope / 발급 시각은 평문 노출. 보안 메타데이터 자체가 사용자 패턴 분석 가능 (어떤 webapp이 언제 키 발급됐는지). ADR-0008 SQLCipher 약속 미실현.

## Test invariants

- 새 DB 생성 시 SQLCipher 헤더 자동 적용 (parametric test: 다른 passphrase로 다시 열면 sqlite 에러).
- 빈 passphrase 거부 (`StoreError::EmptyPassphrase`).
- 평문 → 암호화 round-trip: 2 row 작성 → 마이그레이션 → 동일 passphrase로 재열기 → 2 row 보존.
- 마이그레이션 missing source → `MigrationFailed` 에러 (panic X).
- file-backed DB는 `journal_mode == "wal"` (Phase 8'.0.b 통합 검증).

## References

- [SQLCipher API — sqlcipher_export](https://www.zetetic.net/sqlcipher/sqlcipher-api/#sqlcipher_export)
- [rusqlite features — bundled-sqlcipher-vendored-openssl](https://github.com/rusqlite/rusqlite/blob/master/Cargo.toml)
- [keyring crate — cross-platform OS keyring](https://github.com/hwchen/keyring-rs)
- LMmaster 결정 노트: `docs/research/phase-8p-9p-10p-residual-plan.md` §1.6.1
