# ADR-0053 — knowledge-stack SQLCipher feature gate + open_with_passphrase

* **상태**: Accepted (2026-05-03). Phase R-B 머지와 함께 적용. 호출자 wiring(knowledge.rs가 keyring 통한 passphrase 적용)은 sub-phase #38로 분리.
* **선행**: ADR-0035 (KeyManager SQLCipher activation + OS keyring secret) — knowledge-stack에 동일 패턴 적용. ADR-0024 (Knowledge Stack RAG — per-workspace SQLite).
* **컨텍스트**: 2026-05-02 GPT Pro 검수에서 v0.0.1 ship-blocker S3로 보고됨. ADR-0035에서 key-manager만 SQLCipher가 적용되어 있었는데, knowledge-stack(사용자 RAG 컬렉션 — 파일 인덱스 + 임베딩 + 텍스트 chunks)은 평문 SQLite 그대로 노출. 사용자 PC가 손상/탈취되면 RAG에 ingested된 모든 문서 본문이 평문으로 디스크에 남음.
* **결정 노트**: `docs/research/phase-r-b-catalog-trust-pipeline-decision.md` §S3.

## 결정

1. **`crates/knowledge-stack/Cargo.toml`에 `sqlcipher = ["rusqlite/bundled-sqlcipher-vendored-openssl"]` feature 추가** — key-manager(`bundled-sqlcipher-vendored-openssl`)와 동일 vendored OpenSSL 빌드. Default OFF (테스트/dev 빌드 호환), 프로덕션 빌드는 `--features sqlcipher` 명시.
2. **`KnowledgeStore::open_with_passphrase(path, passphrase) -> Result<Self, KnowledgeError>` 신규 메서드** — key-manager `KeyStore::open` 패턴 차용. 호출 순서: `Connection::open` → `apply_passphrase` (PRAGMA key) → `apply_stability_pragmas` (WAL/busy_timeout/synchronous) → `init_schema`.
3. **기존 `open(path)`는 평문 모드 그대로 유지** — 테스트 / Linux headless 폴백 / SQLCipher 비활성 빌드 호환. CLAUDE.md §1 백워드 호환 원칙 충족.
4. **`apply_passphrase` private helper** — `PRAGMA key = '<escaped>'` 실행 후 `SELECT count(*) FROM sqlite_master`로 즉시 검증. 잘못된 키면 `NotADatabase` 에러 표면화. SQLCipher OFF 빌드(stock SQLite)에서는 PRAGMA key가 unknown pragma로 무시되며 평문 master row 정상 반환 (개발 빌드 graceful).
5. **테스트 invariant 2건** — `open_with_passphrase_creates_db_when_feature_gated` (default-feature 테스트, 빌드 양쪽 통과) + `open_wrong_passphrase_fails_on_existing_encrypted_db` (`#[cfg(feature = "sqlcipher")]` 게이트, sqlcipher 빌드에서만 잘못된 패스워드 거부 검증).
6. **caller wiring 분리(sub-phase #38)** — `apps/desktop/src-tauri/src/knowledge.rs`가 `KnowledgeStore::open_with_passphrase`를 사용하려면: (1) keyring entry username 신설(예: `knowledge-secret`), (2) `provision()` 함수 mirror, (3) 평문 → 암호화 마이그레이션, (4) per-workspace passphrase 키 도출(HKDF 또는 단일 keyring secret 재사용). 본 ADR 범위 외.

## 근거

- **per-workspace SQLite + SQLCipher = ADR-0024 정책 자연스러운 확장**: knowledge-stack은 이미 per-workspace DB로 격리됨. 같은 keyring secret을 모든 workspace가 공유해도 schema-level isolation은 그대로.
- **vendored-openssl 일관성**: key-manager가 `bundled-sqlcipher-vendored-openssl`을 쓰고 있어 동일 옵션. 시스템 OpenSSL 의존도 0.
- **default OFF**: dev 빌드(특히 Windows Strawberry Perl 미설치 환경)에서 빌드 가능. CI 매트릭스가 `--features sqlcipher`로 분기.
- **PRAGMA key 즉시 검증**: 잘못된 키 적용 시 `sqlite_master` 조회가 `NotADatabase`로 실패 — silent corruption 방지.
- **wiring 분리 정당화**: caller 변경은 keyring + 마이그레이션 + per-workspace secret 도출 = 3개 신규 결정 + 8+ 함수 수정. 본 ADR은 *infrastructure only* — 안전한 분할.

## 거부된 대안

1. **knowledge-stack도 caller wiring까지 본 sub-phase 포함**: 8+ IPC + 마이그레이션 + keyring 관리 = 자체 sub-phase 규모. R-B 범위(catalog trust + 보안 marker)에 끌어오면 ship-blocker 해소 늦어짐.
2. **passphrase를 store struct에 저장 후 lazy apply**: `Connection::open` 직후 모든 read/write 가능 — lazy하면 race 위험. 즉시 apply가 정공.
3. **별개 crate `knowledge-stack-sqlcipher`로 분리**: feature gate가 더 깨끗. 그러나 per-crate 분기 cost가 더 크고, 호출자가 `cfg(feature)`로 분기해야 함. feature gate 1줄이 더 간결.
4. **OpenSSL 시스템 라이브러리 의존(`bundled-sqlcipher-vendored-openssl` 대신 `bundled-sqlcipher`)**: Windows / macOS에서 시스템 OpenSSL 부재 흔함 → 사용자 빌드 깨짐. vendored가 portable.
5. **SQLCipher 대신 SQLite + 별개 AES file encryption**: rusqlite 통합 X → schema layer에서 query 못 함 (전체 파일 decrypt 후 메모리 SQLite). 성능 + 코드 복잡도 폭증.
6. **PRAGMA key 미설정 시 자동 평문 fallback**: 사용자가 의도와 다르게 평문 저장됨을 모를 수 있음 — 보안 회귀. Caller가 명시적으로 `open` (평문) vs `open_with_passphrase` (암호화) 선택.
7. **테스트를 `#[cfg(feature = "sqlcipher")]`로만 게이트(default OFF에서 0건)**: stock SQLite 빌드에서 `open_with_passphrase` 호출이 작동하는지(graceful) 회귀 보호 안 됨. default-feature 테스트 1건 + sqlcipher-only 테스트 1건이 양쪽 보호.
8. **per-workspace passphrase 별개 (workspace당 다른 키)**: 워크스페이스 export/import 시 키 마이그레이션 복잡. 단일 keyring secret + workspace_id 기반 schema isolation으로 보호 충분.

## 결과 / 영향

- **`crates/knowledge-stack/Cargo.toml`**: `[features]`에 `sqlcipher = ["rusqlite/bundled-sqlcipher-vendored-openssl"]` 추가. baseline 빌드 영향 0.
- **`crates/knowledge-stack/src/store.rs`**:
  - `KnowledgeStore::open_with_passphrase()` 신규 (~25 LOC).
  - `apply_passphrase()` private helper (~10 LOC).
  - `cfg(test)` 영역에 SQLCipher 테스트 invariant 2건.
- **백워드 호환**: `open(path)` 평문 모드 그대로 작동. 기존 사용자(평문 DB)에 영향 0 — sub-phase #38 마이그레이션 적용 전까지.
- **caller 영향 0** (현재): `apps/desktop/src-tauri/src/knowledge.rs`는 `KnowledgeStore::open(path)`만 사용. sub-phase #38에서 `open_with_passphrase`로 전환 + 마이그레이션 IPC.
- **CI 영향**: 신규 feature gate가 baseline 빌드에 영향 0. sqlcipher feature 활성 빌드는 vendored OpenSSL 컴파일 시간 추가 (~30s) — 프로덕션 릴리스 빌드에서만.

## References

- 결정 노트: `docs/research/phase-r-b-catalog-trust-pipeline-decision.md`
- GPT Pro 검수: 2026-05-02 30-issue static review (S3 본 ADR로 해소)
- 코드:
  - `crates/knowledge-stack/Cargo.toml` (feature gate)
  - `crates/knowledge-stack/src/store.rs::open_with_passphrase` (신규 메서드)
- 관련 ADR: 0024 (Knowledge Stack RAG), 0035 (KeyManager SQLCipher — 패턴 원본)
- 후속 sub-phase: #38 (knowledge.rs caller wiring + per-workspace passphrase + 마이그레이션 IPC)
