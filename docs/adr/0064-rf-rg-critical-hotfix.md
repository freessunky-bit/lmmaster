# ADR-0064 — Critical Hotfix v0.3.x: 외부 통신·암호화·release 정합 5축

**일자**: 2026-05-08
**상태**: 채택 (Phase R-F+R-G 통합 sub-phase)
**관련**: ADR-0026 / ADR-0035 / ADR-0047 / ADR-0052 / ADR-0053 / ADR-0055
**결정 노트**: `docs/research/phase-rf-rg-critical-hotfix-decision.md`

---

## 컨텍스트

GPT Pro 확장 검수 리포트(2026-05-07, 19 finding)가 LMmaster v0.3.0의 cloud-zero / local-first 정체성을 직접 위협하는 critical/high 결함 5건을 식별했다. 본 ADR은 그 5건을 **v0.3.1 hotfix로 묶어 처리**하는 architectural 결정을 기록한다. R-A/B/C/D/E + 분리 #31/#38 모두 머지 완료된 직후 시점이며 v0.0.1 release tag push 직전.

검수 리포트 정확도는 8/8 핵심 cross-check 모두 line까지 일치 — 인용 코드 신뢰도 100%. 부분 동의 3건(R-F.3 IPC raw path / R-H.3 open_url / R-I.3 ingest blocking)은 우선순위 재분류 후 별도 sub-phase로 처리.

## 결정

### 1. 외부 통신 화이트리스트 강화 — `shell.curl_pipe_sh` 폐기

`crates/installer/src/action.rs::run_shell_curl_pipe_sh`는 production build에서 거부한다.

- `#[cfg(feature = "legacy-curl-install")]` feature flag 격리 (default OFF).
- `cfg(not(feature = "legacy-curl-install"))` 분기는 `ActionError::Unsupported("shell.curl_pipe_sh — supply-chain RCE 표면이라 v0.X부터 제거됐어요")` 반환.
- manifest validator (`crates/runtime-detector/src/manifest.rs::validate_install_methods_safe`)가 `shell.curl_pipe_sh`를 만나면 `ManifestValidationError::ForbiddenShellCurlPipeSh` 즉시 거부.
- 호출 site = `crates/installer/src/install_runner.rs::run_install` 진입 시 `manifest.validate_install_methods_safe()?`.

Ollama Linux 설치는 `open_url` + `https://github.com/ollama/ollama/blob/main/docs/linux.mdx`로 전환. capability scope `https://github.com/**`로 이미 화이트리스트 — 추가 ACL 변경 0. Ollama 모든 binary asset이 GitHub Releases에 호스트되어 있고 공식 docs 페이지가 4단계 manual install 안내를 제공.

i18n ko/en에 `screens.runtimeRail.cards.ollamaLinux.*` + `screens.installCenter.linuxOpenUrlPostCheck.*` + `installConsent.linuxOpenUrl` 키 추가. `apps/desktop/src/i18n/guide-ko-v1.md` + `guide-en-v1.md`에 신규 섹션 `<!-- section: linux-install -->` (4단계 manual install + 클립보드 복사 가능 코드 fence) 추가.

### 2. cloud-zero 경계 강화 — Workbench responder localhost-only

`WorkbenchResponder::new(kind, model_id, base_url)`는 `Result<Self, WorkbenchError>` 반환.

- `validate_localhost_url(input) -> Result<Url, WorkbenchError>` 헬퍼가 `url` crate (`Url::parse`) + scheme `http` 한정 + host 매뉴얼 매치 + userinfo 거부 + 정확 매치 `eq_ignore_ascii_case("localhost")` (suffix attack 차단).
- 거부 host 범위: 모든 비-loopback (private LAN `192.168/10/172.16-31`, link-local `169.254`, public IP, hostname, punycode/IDN, suffix attack, userinfo embedded). loopback 정의는 IPv4 `127.0.0.0/8` + IPv6 `::1` + hostname `localhost`.
- `0.0.0.0` / `[::]` 거부 — Windows winsock connect fail + server bind-sentinel이지 client dial 대상 아님.
- `WorkbenchError::InvalidBaseUrl { message: String }` variant 추가. 한국어 해요체 메시지가 `WorkbenchApiError::StartFailed`로 매핑되어 사용자에게 즉시 노출.
- `apps/desktop/src-tauri/src/workbench.rs::build_responder`도 `Result` 반환으로 변경.

`Cargo.toml` workspace.dependencies에 `url = "2.5"` 추가 — reqwest가 이미 transitive로 끌어와 빌드 시간 영향 0.

### 3. 암호화 약속 wiring — SQLCipher feature forwarding

`apps/desktop/src-tauri/Cargo.toml`에 forwarding feature 추가:

```toml
[features]
sqlcipher = ["key-manager/sqlcipher", "knowledge-stack/sqlcipher"]
```

workspace `Cargo.toml::rusqlite`는 `default-features = false` + `bundled` 명시 — `bundled`와 `bundled-sqlcipher-vendored-openssl`이 mutually exclusive하므로. desktop crate가 binary unification root이므로 forwarding feature 하나로 두 sub-crate 동시 활성화.

`.github/workflows/release.yml` matrix `args`에 `'--features sqlcipher'` 주입 (4 platform 전체). Linux runner에 `perl + pkg-config + make + nasm` apt 설치, Windows runner에 `shogo82148/actions-setup-perl@v1` (strawberry distribution) + `ilammy/setup-nasm@v1` 신규 step. 새 verify step은 `cargo tree -e features -p key-manager --features sqlcipher`의 출력에 `bundled-sqlcipher-vendored-openssl`이 포함되어 있는지 grep 검증.

runtime invariant test 2건 신규 — `crates/key-manager/src/store.rs` + `crates/knowledge-stack/src/store.rs`에 `#[cfg(feature = "sqlcipher")]` gated `PRAGMA cipher_version` non-empty 검증. silent regression (CI 통과 + runtime 평문 저장) 차단.

dev/test 빌드 호환: workspace level `bundled` 유지. sqlcipher feature는 release build에서만 명시 활성화. `cargo test --workspace --exclude lmmaster-desktop` (Strawberry Perl 미보장 환경 호환) 그대로.

### 4. release 정합 — Updater 옵션 B-3 hybrid

`tauri.conf.json::plugins.updater.active: true → false` 토글만. plugin 등록 / pubkey / endpoints는 보존 (v1.x 옵션 A 복귀 시 토글만으로 자동 업데이트 활성).

- `apps/desktop/src-tauri/capabilities/main.json`에서 `updater:default` + `allow-start-auto-update-poller` + `allow-stop-auto-update-poller` 주석 차단. `allow-check-for-update` 등 단발 확인 IPC는 보존 — `auto_updater` crate가 reqwest로 GitHub Releases JSON API single-shot polling.
- `apps/desktop/src/pages/Settings.tsx`에 `ManualUpdatePanel` 신규 — "릴리즈 페이지 열기" 버튼 (`openExternal(RELEASES_URL)`) + "최신 버전 확인할게요" 버튼 + 결과 표시 (`isLatest` / `outdated` / `failed`). `const AUTO_UPDATE_ENABLED = false;` 분기로 mount.
- **보너스 fix**: `Settings.tsx:84`의 `UPDATE_REPO = "anthropics/lmmaster"` placeholder bug → `"freessunky-bit/lmmaster"`로 정정. `UPDATE_REPO_BETA` 동일.
- ToastUpdate.tsx 코드 수정 0 — `ManualUpdatePanel`이 mount 안 함으로 자동 차단.

`tauri-plugin-updater` Cargo 의존성과 `lib.rs::plugin(tauri_plugin_updater::Builder::new().build())` 빌더는 **유지** (B-3 hybrid의 핵심) — config.active=false면 plugin은 no-op로 builder만 통과.

`docs/DEFERRED.md`에 **Phase R-K** 신설 (v1.x 옵션 A 진입 가이드 10단계). 기존 §3 "옵션 B" 섹션은 stub으로 축소.

### 5. 데이터 가용성 — KeyManager migration v2

`apps/desktop/src-tauri/src/keys/migrate.rs`를 단일 경로 모델로 재작성.

- `DbState` enum (NotExist / Empty / PlaintextSqlite / EncryptedSqlcipher / Corrupt).
- `detect_db_state(path)`이 첫 16 byte read → magic bytes (`"SQLite format 3\0"`) 매치로 분기.
- `provision_v2(keys_path)` 함수가 단일 경로로 동작. plaintext 감지 시: `keys.db` → `.migrating` (encrypted export) → `.legacy.bak.{utc_ts}` (백업 rename) → `keys.db` (promote rename) 2-phase atomic migration.
- crash recovery 가드: `.migrating` 잔재 정리 + orphan `.migrating` (Phase C 1단계 완료 후 죽은 시뮬레이션) promote 회복.
- `crates/key-manager/src/store.rs::migrate_unencrypted_to_encrypted` 보강 — ATTACH 경로 single quote escape (R-H.4 합치) + `PRAGMA wal_checkpoint(TRUNCATE)` + `dest_path.exists()` 가드.

caller (`apps/desktop/src-tauri/src/lib.rs`)는 `legacy_path` 인자 제거 → `provision_v2(&keys_path)` 단일 호출로 단순화.

i18n ko/en에 `keys.migrating.{start, done, fail}` + `keys.fallback.headless` 키 추가.

**6개월 회귀 통과 원인 + 방어**: provision의 단위 테스트가 keyring OS 의존으로 dev 환경에서 스킵됨. 본 변경에 fake keyring trait 도입 — provision-level integration test에서 OS keyring 무관하게 검증.

## 결과

5축 신뢰 경계가 동시에 회복된다. v0.0.1 release tag push가 가능해진다.

**테스트 카운트 차분**: Rust +30 (Workbench 18 / manifest validator 3 / migration 5 / SQLCipher invariant 2 / ATTACH escape 1 + base_url normalize 1) + React +4 (Settings ManualUpdatePanel a11y 4) = **+34**.

**Bundle size 영향**: vendored OpenSSL 정적 link로 ~3MB 증가, NSIS 압축으로 ~1.5MB 실제 증가. cloud-zero 정체성 보전 가치 > 1.5MB 증가.

**외부 통신 영향**: 빌드 시점 vendored OpenSSL이 GitHub에서 source tarball 다운로드 — *GitHub Actions runner에서만 발생, 사용자 PC 외부 통신 0*. ADR-0013 외부 통신 0 정책 위반 아님.

## 미정 (v1.x로 이월)

- **R-F.3 IPC raw path → selected_path_token registry**. 검수 리포트는 critical로 분류했으나 XSS surface 깨끗 + Tauri 2 capability 강제로 exploit surface 제한적이라 HIGH로 재분류. Tauri dialog plugin 도입 후 별도 sub-phase. DEFERRED.md §16-19.
- **Ollama Linux 옵션 A (download_and_extract)**. Linux 사용자 점유율 > 15% + manual install 실패율 > 30% (텔레메트리 6주 모니터링) 충족 시 진입. DEFERRED.md 신규 항목.
- **Updater 옵션 A (자동 업데이트)**. minisign keypair 발급 + GitHub Secret 등록 + 옛 v0.0.x 사용자 first-time-pubkey-bump 안내 후 진입. DEFERRED.md Phase R-K.
- **Workbench enterprise gateway mode**. 비-localhost 요구 N건 누적 시 v2.x ADR 후보 (가칭 ADR-006X, 별개 빌드 feature flag).
- **DNS rebinding hardening (`reqwest::resolve()` 정적 매핑)**. 본 sub-phase 머지 후 Phase 5'.e' 후속 reinforce.
- **KeyStore passphrase rotation**. `KeyStore::rekey()` API 설계, 별도 sub-phase.

## 위험

- **Linux ubuntu-22.04 runner perl/nasm 첫 빌드** 시 vendored OpenSSL 실패 가능 — 본 sub-phase 머지 직전 wet run으로 검증 (`act` local 또는 dry-run tag).
- **Updater pubkey 짝 secret 미확인** — `tauri.conf.json:122`의 minisign pubkey(`BF5C36D65E99C44F` 시작)의 짝 secret이 코드베이스 어디에도 없음. v1.x 옵션 A 진입 시 *반드시 새 keypair* 권장 (Phase R-K 항목에 명시). 옛 사용자 PC에 잘못된 pubkey가 박힌 상태로 v1.0.0이 다른 pubkey로 서명되면 자동 업데이트 영구 차단 (Tauri trust-on-first-use).
- **antivirus의 `.migrating` 격리** (Windows Defender) 시 promote rename 실패 — retry 로직 + 백업 경로 안내. user-named 디렉터리에 `'` 포함 케이스는 ATTACH escape 처리됐으니 OK.
- **일부 Linux power user의 "자동 설치" 회귀 불만** — 가이드 끝에 "한 줄 자동 설치 (직접 실행)" 안내. 사용자 본인 책임으로 실행, LMmaster spawn 0 (정책 합치).

## 채택 근거 요약

본 5건의 우선순위는 (a) 사용자 데이터 손실 위험, (b) 정체성 직접 위반, (c) 즉시 처리 가능한 effort, (d) 검수 리포트 cross-check 정확도 8/8을 모두 충족. R-A~R-E 시리즈와 동일한 6-section 결정 노트 + ADR 패턴으로 negative space 보존(§3 기각안 16건). 다음 세션이 같은 함정에 다시 빠지지 않도록 *왜 다른 안을 거부했는지*가 명시됨.
