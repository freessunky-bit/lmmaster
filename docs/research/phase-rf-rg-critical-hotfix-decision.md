# Phase R-F+R-G 통합 결정 노트 — Critical Hotfix v0.3.x

> **상태**: 채택 (2026-05-08, 사용자 추천안 승인 + 옵션 B/B 채택)
> **선행 의존성**: R-A/B/C/D/E + 분리 #31/#38 모두 머지 완료. v0.0.1 release tag push 직전.
> **다음 페이즈**: Phase R-H (Boundary Polish) → Phase R-I (CI/Build Hygiene) → Phase R-J (Invariant Tests).
> **보강 리서치**: 5개 영역 병렬 Agent (SQLCipher / Ollama Linux / Workbench URL / KeyManager / Updater) — 각 1500단어 보고.
> **결정 일자**: 2026-05-08

---

## 1. 결정 요약

GPT Pro 확장 검수 리포트(2026-05-07, 19 finding)에서 critical/high 등급으로 식별된 5건을 **v0.3.1 hotfix**로 묶어 처리한다. 모두 cloud-zero / local-first 정체성을 직접 위협하는 신뢰 경계 결함.

| ID | 결정 | 영역 | Effort |
|---|---|---|---|
| **D1 (R-G.1)** | SQLCipher feature를 release build에서 실제 활성화 | 암호화 wiring | 1-2h |
| **D2 (R-F.1)** | Ollama Linux 설치를 `shell.curl_pipe_sh` → `open_url`로 전환 | 외부 통신 화이트리스트 | 1-2h |
| **D3 (R-F.2)** | Workbench `base_url`에 localhost-only allowlist 강제 | cloud-zero 정체성 | 1h |
| **D4 (R-G.4)** | Tauri Updater 옵션 B-3 hybrid (active=false) | release 정합 | 1-2h |
| **D5 (R-G.2)** | KeyManager migration logic을 단일 경로 + magic bytes 감지로 재작성 | 데이터 가용성 | 2-4h |

본 5건이 머지되면 *외부 통신 화이트리스트*, *cloud-zero*, *암호화 약속*, *데이터 가용성*, *release 정합* 5개 신뢰 축이 동시에 회복된다. 검수 리포트 critical 3건 중 2건(curl|sh, base_url) + high 5건 중 4건(SQLCipher, KeyManager, updater 정합 + bonus UPDATE_REPO bug) 종결.

---

## 2. 채택안

### 2.1 D1 — SQLCipher release wiring

**핵심 패턴**: `bundled`와 `bundled-sqlcipher-vendored-openssl`이 mutually exclusive이므로 workspace `Cargo.toml`에서 `default-features = false`로 두고 sub-crate가 명시 선택. desktop crate가 binary unification root이므로 forwarding feature 하나로 두 sub-crate 동시 활성화.

**변경 파일**:

1. `Cargo.toml` workspace.dependencies — `rusqlite = { version = "0.31", default-features = false, features = ["bundled"] }`로 변경 (`default-features = false` 명시).

2. `apps/desktop/src-tauri/Cargo.toml` `[features]`:
   ```toml
   custom-protocol = ["tauri/custom-protocol"]
   sqlcipher = [
     "key-manager/sqlcipher",
     "knowledge-stack/sqlcipher",
   ]
   ```

3. `.github/workflows/release.yml` matrix `args` — 모든 platform에 `'--features sqlcipher'` 추가:
   ```yaml
   - platform: windows-x86_64
     args: '--features sqlcipher'
   - platform: darwin-aarch64
     args: '--target aarch64-apple-darwin --features sqlcipher'
   # ... (4 platform 모두)
   ```

4. release.yml Linux prerequisites — `perl pkg-config make nasm` 추가.

5. release.yml Windows 신규 step — `shogo82148/actions-setup-perl@v1` (strawberry distribution) + `ilammy/setup-nasm@v1`.

6. release.yml 신규 verify step:
   ```yaml
   - name: Verify SQLCipher feature wiring
     shell: bash
     run: |
       cargo tree -e features -p key-manager --features sqlcipher \
         | grep -E "rusqlite v0\.31\.[0-9]+ .*bundled-sqlcipher-vendored-openssl" \
         || (echo "::error::sqlcipher feature 미전파"; exit 1)
       cargo tree -e features -p knowledge-stack --features sqlcipher \
         | grep -E "bundled-sqlcipher-vendored-openssl" \
         || (echo "::error::knowledge-stack sqlcipher 미전파"; exit 1)
   ```

7. `crates/key-manager/src/store.rs` mod tests — runtime invariant test:
   ```rust
   #[cfg(feature = "sqlcipher")]
   #[test]
   fn sqlcipher_runtime_active_in_release_feature() {
       let dir = tempfile::tempdir().unwrap();
       let path = dir.path().join("verify.db");
       let passphrase = "deadbeef".repeat(8);
       let store = KeyStore::open(&path, &passphrase).unwrap();
       let version: Option<String> = store.conn
           .query_row("PRAGMA cipher_version", [], |r| r.get(0)).ok();
       assert!(
           version.as_deref().filter(|v| !v.is_empty()).is_some(),
           "SQLCipher 빌드인데 cipher_version이 비어 있어요"
       );
   }
   ```

8. `crates/knowledge-stack/src/store.rs` — 동일 패턴 invariant test.

**dev/test 빌드 호환**: workspace level `bundled` 유지. sqlcipher feature는 release build에서만 명시 활성화. `cargo test --workspace --exclude lmmaster-desktop` (Strawberry Perl 미보장 환경 호환) 그대로.

### 2.2 D2 — Ollama Linux `open_url` 전환

**핵심 패턴**: `https://github.com/ollama/ollama/blob/main/docs/linux.mdx`을 `open_url` 대상으로 사용. capability scope `https://github.com/**`로 이미 화이트리스트 — ACL 변경 0. Ollama 공식 docs 페이지가 4단계 manual install 안내 + `sha256sum.txt` 보유.

**변경 파일**:

1. `manifests/apps/ollama.json` linux 분기 + `manifests/snapshot/ollama.json` 동일:
   ```json
   "linux": {
     "method": "open_url",
     "url": "https://github.com/ollama/ollama/blob/main/docs/linux.mdx",
     "reason_ko": "리눅스는 배포판마다 패키지 매니저가 달라요. 공식 가이드를 열어드릴게요.",
     "reason_en": "Linux distros differ by package manager. Opening the official manual install guide."
   }
   ```

2. `crates/runtime-detector/src/manifest.rs` — `validate_install_methods_safe()` + `ManifestValidationError::ForbiddenShellCurlPipeSh` variant 신설:
   ```rust
   #[derive(Debug, thiserror::Error)]
   pub enum ManifestValidationError {
       #[error("매니페스트 '{app_id}' ({os}): shell.curl_pipe_sh는 더 이상 허용되지 않아요. open_url 또는 download_and_extract로 전환해 주세요.")]
       ForbiddenShellCurlPipeSh { app_id: String, os: &'static str },
   }
   ```

3. 호출 site = `crates/installer/src/install_runner.rs::run_install` 진입 시 `manifest.validate_install_methods_safe()?`.

4. `crates/installer/src/action.rs::run_shell_curl_pipe_sh` — feature flag 격리:
   ```rust
   #[cfg(feature = "legacy-curl-install")]
   async fn run_shell_curl_pipe_sh(...) -> Result<ActionOutcome, ActionError> {
       tracing::error!("legacy-curl-install feature is enabled — must NOT ship to production");
       /* 기존 로직 */
   }

   #[cfg(not(feature = "legacy-curl-install"))]
   async fn run_shell_curl_pipe_sh(...) -> Result<ActionOutcome, ActionError> {
       Err(ActionError::Unsupported(
           "shell.curl_pipe_sh — supply-chain RCE 표면이라 v0.X부터 제거됐어요",
       ))
   }
   ```

5. `crates/installer/Cargo.toml` `[features]`에 `legacy-curl-install = []` 선언 (default OFF).

6. i18n ko/en 신규 키:
   - `screens.runtimeRail.cards.ollamaLinux.{consent, running, done, failed, openGuide, checkAgain}`
   - `screens.installCenter.linuxOpenUrlPostCheck.{title, body, openGuideAction, retryAction}`
   - `screens.installCenter.installConsent.{default, linuxOpenUrl}` (platform 분기 안전)

7. `apps/desktop/src/i18n/guide-ko-v1.md` + `guide-en-v1.md` — 신규 섹션 `<!-- section: linux-install -->` (4단계 manual install + 클립보드 복사 가능 코드 fence).

8. `apps/desktop/src/pages/Guide.tsx::SECTION_IDS`에 `"linux-install"` 추가 + `SECTION_KEYWORDS`에 한국어 키워드 (`["linux", "리눅스", "tar", "zst", "systemd", "ollama-linux", "ㄹㄴㅅ"]`).

9. frontend platform 분기 — `Step3Install` / `RuntimeRail` / `InstallCenter`가 `os.platform() === "linux" && manifest.id === "ollama"` 조건에서 `installConsent.linuxOpenUrl` 카피 사용.

10. `manifests/apps/catalog.json` 재생성 — `node .claude/scripts/build-catalog-bundle.mjs` (CLAUDE.md §3 카탈로그 갱신 흐름). 단 ollama.json은 `category: external-runtime`라 model catalog가 아님 — bundle 영향 없음 검증.

**capability scope**: 변경 없음. `https://github.com/**`가 docs 페이지 + Releases asset 모두 커버.

### 2.3 D3 — Workbench localhost-only allowlist

**핵심 패턴**: `url` crate (`Url::parse`) + `IpAddr` std 매뉴얼 매치 (loopback / private LAN / link-local 거부) + `Host::Domain`은 `eq_ignore_ascii_case("localhost")` 정확 매치 (suffix attack 차단). 거부 사유 한국어 해요체 즉시 노출.

**변경 파일**:

1. `Cargo.toml` workspace.dependencies — `url = "2.5"` 추가.

2. `crates/bench-harness/Cargo.toml` `[dependencies]` — `url = { workspace = true }`.

3. `crates/bench-harness/src/workbench_responder.rs`:
   ```rust
   use std::net::IpAddr;
   use url::{Host, Url};

   pub(crate) fn validate_localhost_url(input: &str) -> Result<Url, WorkbenchError> {
       let trimmed = input.trim();
       if trimmed.is_empty() {
           return Err(invalid("주소를 입력해 주세요."));
       }
       let url = Url::parse(trimmed).map_err(|_| {
           invalid("주소 형식이 올바르지 않아요. 예: http://localhost:11434")
       })?;
       if url.scheme() != "http" {
           return Err(invalid("http만 사용할 수 있어요. https는 외부 서비스라 차단했어요."));
       }
       if !url.username().is_empty() || url.password().is_some() {
           return Err(invalid("주소에 사용자명/비밀번호를 넣을 수 없어요."));
       }
       let host = url.host().ok_or_else(|| invalid("주소를 입력해 주세요."))?;
       let allowed = match host {
           Host::Domain(name) => name.eq_ignore_ascii_case("localhost"),
           Host::Ipv4(ip) => ip.is_loopback(),
           Host::Ipv6(ip) => ip.is_loopback(),
       };
       if !allowed {
           return Err(invalid(
               "내 PC 안에서 돌아가는 모델만 평가할 수 있어요. http://localhost 주소로 입력해 주세요.",
           ));
       }
       Ok(url)
   }

   pub fn new(runtime_kind: RuntimeKind, model_id: impl Into<String>, base_url: impl Into<String>)
       -> Result<Self, WorkbenchError>
   {
       let base = base_url.into();
       let url = validate_localhost_url(&base)?;
       let normalized = url.as_str().trim_end_matches('/').to_string();
       Ok(Self { /* ... */ base_url: normalized, /* ... */ })
   }
   ```

4. `crates/workbench-core/src/error.rs` — `WorkbenchError::InvalidBaseUrl { message: String }` variant 추가.

5. `apps/desktop/src-tauri/src/workbench.rs::build_responder` — `Result<WorkbenchResponder, WorkbenchError>` 반환. caller `start_workbench_run`이 `WorkbenchApiError::StartFailed { message }`로 매핑 (한국어 메시지 그대로 사용자 노출).

6. i18n ko/en — `screens.workbench.errors.baseUrl.{nonLocalhost, httpsBlocked, empty, malformed, userinfo}` 5키 추가 (frontend onBlur hint용; backend가 진짜 검증).

7. `apps/desktop/src/pages/Workbench.tsx` (선택) — base_url input onBlur hint:
   ```tsx
   const looksLocalhost = (s: string) =>
     /^http:\/\/(localhost|127\.\d+\.\d+\.\d+|\[::1\])(:\d+)?(\/|$)/.test(s.trim());
   ```

**기존 테스트 fixup**: `with_config_overrides_defaults` 등 비-localhost host(`http://x:11434`) 사용 테스트 → `http://127.0.0.1:11434`로 변경. `WorkbenchResponder::new()` Result 반환에 따라 `.unwrap()` 추가. wiremock `MockServer::start().await.uri()`는 `http://127.0.0.1:포트` 형태라 자연 통과.

### 2.4 D4 — Updater 옵션 B-3 hybrid

**핵심 패턴**: `active=true → false` 토글만 + `Settings.tsx`에 `ManualUpdatePanel` 신규. Tauri Updater plugin 등록은 *유지* (no-op 상태) → v1.x 옵션 A 복귀 시 토글만으로 자동 업데이트 활성. ToastUpdate.tsx 코드 수정 0 (mount 차단으로 자동 비활성).

**보너스 발견**: `Settings.tsx:84`의 `UPDATE_REPO = "anthropics/lmmaster"`는 placeholder bug. 실 repo `freessunky-bit/lmmaster` — 본 변경에서 함께 fix.

**변경 파일**:

1. `apps/desktop/src-tauri/tauri.conf.json`:
   ```json
   "plugins": {
     "updater": {
       "active": false,                       // v1.x Phase R-K 진입 시 true.
       "endpoints": [/* 보존 */],
       "dialog": false,
       "pubkey": "..."                         // ⚠ 짝 secret 미확인 — Phase R-K 진입 시 새 keypair.
     }
   }
   ```

2. `apps/desktop/src-tauri/capabilities/main.json`:
   - `updater:default` 주석 차단.
   - `allow-start-auto-update-poller` + `allow-stop-auto-update-poller` 주석 차단.
   - `allow-check-for-update` + `allow-cancel-update-check` + `allow-get-auto-update-status` 보존 (단발 확인 흐름 작동).

3. `apps/desktop/src/pages/Settings.tsx`:
   - line 84 `UPDATE_REPO`: `"anthropics/lmmaster"` → `"freessunky-bit/lmmaster"` (bug fix).
   - line 84 `UPDATE_REPO_BETA`: 동일 fix.
   - 상수 추가: `const AUTO_UPDATE_ENABLED = false;` + `const RELEASES_URL = "https://github.com/freessunky-bit/lmmaster/releases/latest";`.
   - line 307 `<AutoUpdatePanel />` → `{AUTO_UPDATE_ENABLED ? <AutoUpdatePanel /> : <ManualUpdatePanel />}`.
   - 신규 `ManualUpdatePanel` 컴포넌트 — "릴리즈 페이지 열기" 버튼 (`openExternal(RELEASES_URL)`) + "최신 버전 확인할게요" 버튼 (auto_updater crate single-shot polling) + 결과 표시 (`isLatest` / `outdated` / `failed`).

4. i18n ko/en — `screens.settings.autoUpdate.manualMode.{title, description, openReleases, currentVersion, checkLatest, checking, isLatest, newVersionAvailable, checkFailed}` 9키 추가. 기존 `autoUpdate` 키는 보존 (deprecated marker, v1.x 옵션 A 복귀 시 재사용).

5. `docs/DEFERRED.md` — 기존 §3 "옵션 B" 섹션 위에 **Phase R-K** 신설 (v1.x 옵션 A 진입 가이드 10단계). 기존 §3 섹션은 "Phase R-K로 통합되었어요" stub으로 축소.

**ToastUpdate.tsx**: 수정 0. `ManualUpdatePanel`이 mount 안 함 → 자동 차단.

**ipc/updater.ts**: 수정 0. 단발 확인은 `auto_updater` crate가 reqwest로 GitHub Releases JSON API polling — Tauri Updater plugin과 무관.

### 2.5 D5 — KeyManager migration v2

**핵심 패턴**: 단일 경로 (`keys.db`) + magic bytes plaintext 감지 + `.migrating` temp + atomic 2-phase rename + crash recovery 가드. 기존 두 경로 분리 모델은 caller 인자 swap 회귀 원인.

**변경 파일**:

1. `apps/desktop/src-tauri/src/keys/migrate.rs`:
   ```rust
   const SQLITE_MAGIC: &[u8; 16] = b"SQLite format 3\0";

   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub enum DbState {
       NotExist,
       Empty,
       PlaintextSqlite,
       EncryptedSqlcipher,
       Corrupt,
   }

   pub fn detect_db_state(path: &Path) -> std::io::Result<DbState> {
       use std::io::Read;
       if !path.exists() { return Ok(DbState::NotExist); }
       let mut f = std::fs::File::open(path)?;
       let mut buf = [0u8; 16];
       let n = f.read(&mut buf)?;
       match n {
           0 => Ok(DbState::Empty),
           1..=15 => Ok(DbState::Corrupt),
           _ if &buf == SQLITE_MAGIC => Ok(DbState::PlaintextSqlite),
           _ => Ok(DbState::EncryptedSqlcipher),
       }
   }

   pub fn provision_v2(keys_path: &Path) -> MigrationOutcome {
       let migrating = with_suffix(keys_path, ".migrating");
       // Step 0: stale .migrating 정리 + orphan 회복.
       if migrating.exists() && !keys_path.exists() {
           // Phase C 1단계만 완료된 잔재 — promote 회복.
           let _ = std::fs::rename(&migrating, keys_path);
       } else if migrating.exists() {
           let _ = std::fs::remove_file(&migrating);
       }
       /* keyring resolve + state 분기 */
   }
   ```

2. `crates/key-manager/src/store.rs::migrate_unencrypted_to_encrypted` 보강:
   - ATTACH 경로 single quote escape (R-H.4 합치): `let escaped = plain_path.display().to_string().replace('\'', "''");`
   - `PRAGMA wal_checkpoint(TRUNCATE)` 호출 후 connection drop.
   - `dest_path.exists()` 가드 (`.migrating` 잔재 truncate).

3. `apps/desktop/src-tauri/src/lib.rs:251-265` — caller 1줄로 단순화:
   ```rust
   let keys_path = app.path().app_data_dir()
       .map(|d| d.join("keys.db"))
       .unwrap_or_else(|_| PathBuf::from("keys.db"));
   let outcome = keys::provision_v2(&keys_path);  // 단일 경로
   ```

4. i18n ko/en — `keys.migrating.{start, done, fail}` + `keys.fallback.headless` 키 추가.

**6개월 회귀 통과 원인 + 방어**: provision의 단위 테스트가 keyring OS 의존으로 dev 환경에서 스킵됨. 본 변경에 fake keyring trait 도입 — provision-level integration test에서 OS keyring 무관하게 검증.

---

## 3. 기각안 + 이유 (negative space — 다음 세션이 같은 함정 재방문 방지)

| # | 거부된 대안 | 사유 |
|---|---|---|
| 1 | **SQLCipher: workspace level에서 `bundled-sqlcipher-vendored-openssl` 직접 활성화** | release뿐 아니라 모든 dev/CI 빌드가 OpenSSL vendored 컴파일 강제 (Strawberry Perl + nasm 필수, ~30초 추가). dev 사이클 손상 |
| 2 | **SQLCipher: `tauri.conf.json::cargoExtraArgs` 필드 사용** | Tauri 2 schema에 존재하지 않음 (v2.tauri.app/reference/cli 검증). tauri-action `args` forwarding이 정공 |
| 3 | **SQLCipher: `RUSTFLAGS` 환경변수로 features 주입** | RUSTFLAGS는 features와 무관 (rustc compile flag). cargo features는 별개 메커니즘 |
| 4 | **Ollama Linux: capability scope에 `ollama.com/**` 추가** | 외부 통신 화이트리스트 *명시적 확장* — ADR-0055 reconcile 부담. github.com docs 페이지로 우회하면 zero-cost |
| 5 | **Ollama Linux: `download_and_extract` (옵션 A)로 자동화** | `.tar.zst` extract 미지원, sha256 자동 갱신 cron 부재, sudo 우회 미해결, AMD/NVIDIA/Jetson variant 분기 미설계. v1.x로 deferred |
| 6 | **Ollama Linux: `run_shell_curl_pipe_sh` 함수 완전 제거** | git history 보존 안 됨. 6개월 후 코드 제거가 더 안전한 점진적 deprecation |
| 7 | **Workbench: env var `LMMASTER_WORKBENCH_TRUST_ANY_HOST=1`로 우회 옵션** | 공격자가 Tauri 프로세스 환경에 변수 주입만으로 모든 보호 무효화. supply-chain 위험 비용 > 편의성. enterprise mode는 v2.x ADR (별개 빌드 feature flag) |
| 8 | **Workbench: 0.0.0.0 / `[::]` 허용** | Windows winsock connect 거부. server bind-sentinel이지 client dial 대상 아님 |
| 9 | **Workbench: substring 매치 `host_str().contains("localhost")`** | `localhost.evil.com` 즉시 우회. 정확 매치 `eq_ignore_ascii_case("localhost")` 필수 |
| 10 | **Workbench: DNS rebinding hardening (`reqwest::resolve()` 정적 매핑)** | 본 sub-phase 범위 외 — Phase 5'.e' 후속 reinforce. host 검증으로는 막을 수 없는 별개 attack vector |
| 11 | **Workbench: 수동 split host 파싱** | `http://localhost@evil.com/`에서 `localhost`를 host로 잘못 인식 — 검증 자체 우회. `url::Url`이 host와 username을 분리해 주므로 두 필드 각각 검사 필수 |
| 12 | **Updater: `active=false`만 (옵션 B-1, 빈 pubkey)** | plugin schema는 빈 pubkey 허용하지만 unused dependency warning + capability 표면. B-3 hybrid가 가장 깨끗 |
| 13 | **Updater: tauri-plugin-updater 통째 제거 (옵션 B-2)** | 옵션 A 복귀 시 의존성/builder/capability/test 4곳 동시 복구 — 회귀 위험 ↑ |
| 14 | **Updater: `active=false` 런타임 감지로 ToastUpdate 자동 hide (옵션 3)** | 새 IPC 추가 필요 (over-engineering). Settings.tsx 분기로 mount 차단이 zero-cost |
| 15 | **KeyManager: PRAGMA probe로 plaintext 감지 (Method B)** | 실패 모드 다양 (잘못된 키 vs 손상 vs 빈 파일). 잘못된 PRAGMA key를 평문에 적용하면 후속 fingerprint 변형 — 디버깅 어려움. magic bytes(Method A)가 단순+신뢰성 우위 |
| 16 | **KeyManager: 두 경로 분리 모델 (`keys_path` + `legacy_plain_path`) 유지 + caller fix** | 현재 인자 swap이 6개월 무사 통과한 근본 원인. 단일 경로 모델로 단순화하면 같은 회귀 재발 차단 |

---

## 4. 미정 / 후순위 이월 (v1.x)

| 항목 | 진입 조건 | 위치 |
|---|---|---|
| **R-F.3 IPC raw path → selected_path_token registry** | Tauri dialog plugin 도입 + ADR-0052 S6 v0.3.x 승격 결정. 검수 리포트는 critical로 분류했으나 XSS surface 깨끗 + Tauri 2 capability 강제로 exploit surface 제한적이라 HIGH로 재분류 | DEFERRED.md §16-19, R-F.3 별도 sub-phase |
| **Ollama Linux 옵션 A (download_and_extract)** | Linux 사용자 점유율 > 15% + manual install 실패율 > 30% (텔레메트리 6주 모니터링) | DEFERRED.md 신규 (이번 변경에 포함) |
| **Updater 옵션 A (자동 업데이트)** | minisign keypair 발급 + GitHub Secret 등록 + 옛 v0.0.x 사용자 first-time-pubkey-bump 안내 | DEFERRED.md Phase R-K (이번 변경에 포함) |
| **Workbench enterprise gateway mode** | 비-localhost 요구 N건 누적 | v2.x ADR 후보 (가칭 ADR-006X) |
| **DNS rebinding hardening (reqwest resolve())** | 본 sub-phase 머지 후 reinforce | Phase 5'.e' 후속 |
| **KeyStore passphrase rotation** | KeyStore::rekey() API 설계 | 별도 sub-phase |
| **Guide 코드 fence 클립보드 복사 버튼** | _render-markdown.ts 후처리 + CodeBlock 컴포넌트 | Phase 0X.y로 분리 |
| **R-H ~ R-J 후속 14건** | 본 sub-phase 머지 + RESUME 갱신 | 본 결정 노트 §6 인계 |

---

## 5. 테스트 invariant (sub-phase DoD — 깨면 안 되는 것)

### 5.1 Rust unit/integration tests

| invariant | 위치 | 카운트 차분 |
|---|---|---|
| `validate_localhost_url`이 12 케이스 정확 처리 (loopback OK / private LAN / link-local / suffix attack / userinfo / https 모두 거부) | `crates/bench-harness/src/workbench_responder.rs::base_url_validation_tests` | +18 (validation 12 + responder.new Err 4 + base_url normalize 1 + 기존 fixup 1) |
| `WorkbenchResponder::new` Result 반환 — 비-localhost 거부 | 위 |  |
| manifest validator가 `shell.curl_pipe_sh` 자동 거부 | `crates/runtime-detector/src/manifest.rs::validation_tests` | +3 |
| 실제 `manifests/apps/ollama.json`이 validation 통과 | 위 |  |
| `detect_db_state` magic bytes 정확 분류 (5 case) | `apps/desktop/src-tauri/src/keys/migrate.rs::migration_tests` | +5 |
| `provision_v2` plaintext → encrypted migration 성공 + .bak 생성 | 위 |  |
| `provision_v2` `.migrating` 잔재 정리 | 위 |  |
| `provision_v2` orphan `.migrating` promote 회복 | 위 |  |
| `KeyStore::migrate_unencrypted_to_encrypted` ATTACH escape (`O'Brien` fixture) | `crates/key-manager/src/store.rs` | +1 |
| **`#[cfg(feature = "sqlcipher")] sqlcipher_runtime_active`** — PRAGMA cipher_version non-empty | `crates/key-manager/src/store.rs` + `crates/knowledge-stack/src/store.rs` | +2 (release-only) |

**총 Rust +30**.

### 5.2 React UI tests (vitest)

| invariant | 위치 | 카운트 |
|---|---|---|
| `ManualUpdatePanel`이 노출 + ToastUpdate 미렌더 | `apps/desktop/src/pages/Settings.test.tsx` | +1 |
| "릴리즈 페이지 열기" 클릭 시 GitHub URL 호출 | 위 | +1 |
| `ManualUpdatePanel` a11y violations 0 | 위 | +1 |
| AutoUpdatePanel 토글 분기가 `AUTO_UPDATE_ENABLED=false`에서 mount 안 됨 | 위 | +1 |
| Workbench base_url onBlur hint (선택) — 비-localhost 입력 시 hint 노출 | `apps/desktop/src/pages/Workbench.test.tsx` | +1 (선택) |

**총 React +4 (필수) + 1 (선택)**.

### 5.3 CI gate

| invariant | 위치 |
|---|---|
| `cargo tree -e features -p key-manager --features sqlcipher` 출력에 `bundled-sqlcipher-vendored-openssl` 포함 | `.github/workflows/release.yml` 신규 step |
| `cargo tree -e features -p knowledge-stack --features sqlcipher` 동일 |  |

**총 테스트 차분 +34** (Rust 30 + React 4). RESUME에 명시 필수.

---

## 6. 다음 페이즈 인계

### 6.1 Phase R-H (Boundary Polish, 3-4h) — R-F+R-G 머지 후 진입

진입 조건: 본 sub-phase 검증 통과 + RESUME.md 갱신.

작업 항목:
- **R-H.1 (30m)**: `install_app(id)` IPC entry에 `^[A-Za-z0-9._-]+$` 검증 + `canonicalize()` prefix check.
- **R-H.2 (30m)**: `crates/registry-fetcher::fetch_one` bundled fallback에 id allowlist 검증.
- **R-H.3 (1-2h)**: installer `open_url` Rust action layer host allowlist (lmstudio.ai / github.com / huggingface.co) — webbrowser::open 직접 호출 감사.
- **R-H.4 (포함)**: `KeyStore::migrate_unencrypted_to_encrypted` ATTACH escape — D5에 합쳐 처리됨.

### 6.2 Phase R-I (CI/Build Hygiene, 4-5h)

- **R-I.1 (1h)**: CI `cargo test --workspace --exclude lmmaster-desktop` (no-run 제거) + Node `pnpm --filter @lmmaster/desktop test`.
- **R-I.2 (1h)**: `tsconfig.json` `noEmit: true` + 기존 .js 일괄 정리 + .gitignore + CI grep guard. 의도적 `.js` (i18n/init.js 등) 보존 수동 검사.
- **R-I.3 (1-2h)**: Knowledge ingest cancel token chunk-level 강화 + file size cap.
- **R-I.4 (30m)**: Trending Watcher decision note + deprecated workflow 삭제 gate 갱신.

### 6.3 Phase R-J (Invariant Tests, 2h)

- **R-J.1 (30m)**: XSS renderer invariant tests (escape 후 inline tag 외 모두 거부).
- **R-J.2 (30m)**: i18n parity script CI.
- **R-J.3 (30m)**: `unsafe` / `tokio::spawn` grep gate.
- **R-J.4 (30m)**: a11y/CSP/Korean copy modal checklist 강화.

### 6.4 위험 노트

- **R-G.1 SQLCipher**: Linux ubuntu-22.04 runner perl/nasm 미설치 환경에서 vendored OpenSSL 빌드 실패 시 release.yml first run이 fail. 본 sub-phase 머지 직전 wet run으로 검증 (`act` local 또는 dry-run tag).
- **R-F.1 Ollama**: 일부 Linux power user가 "왜 자동 설치 안 해주냐" 회귀 불만 가능. 가이드 끝에 "한 줄 자동 설치 (직접 실행)" 섹션 + 클립보드 복사 — 사용자 본인 책임으로 실행 안내.
- **R-G.4 Updater pubkey**: `tauri.conf.json:122`의 minisign pubkey(`BF5C36D65E99C44F` 시작) 짝 secret 보관 미확인. v1.x 옵션 A 진입 시 *반드시 새 keypair* 권장 (Phase R-K 항목에 명시).
- **R-G.2 KeyManager**: antivirus의 `.migrating` 격리 (Windows Defender) 시 promote rename 실패 가능. retry 로직 + 백업 경로 안내.

### 6.5 사용자 결정 대기 항목

본 sub-phase에서 추가 확인이 필요한 항목 **0건**. 추천안 (옵션 B/B) 사용자 승인 완료 (2026-05-08).

---

**문서 버전**: v1.0 (2026-05-08, Phase R-F+R-G 통합 hotfix 1차 작성).

**참조**:
- 보강 리서치 5개: SQLCipher / Ollama Linux / Workbench URL / KeyManager / Updater (각 1500단어 보고).
- 검수 리포트: `c:/Users/wind.WIND-PC/Downloads/LMmaster-review-report.md` (2026-05-07, 19 finding).
- 관련 ADR: ADR-0026 / ADR-0035 / ADR-0047 / ADR-0052 / ADR-0053 / ADR-0055 / **ADR-0064 신설**.
