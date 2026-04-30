# Phase 13'.c — API 키 scope 편집 + Crash 뷰어 결정 노트

* **상태**: 채택 (2026-04-30)
* **컨텍스트**: Phase 13'.b 완료 후, Diagnostics에서 가짜 데이터를 모두 걷어낸 다음의 두 가지 미구현. (1) 부분 노출이던 API 키 관리에서 사용자가 발급된 키의 *필터를 평문 재발급 없이* 갱신하는 길이 없음 — revoke + re-issue 패턴은 평문 재배포 부담이 큼. (2) panic_hook + crash 파일 작성은 이미 완성돼 있는데, 사용자가 그 파일을 *앱 안에서 볼 길이 없음* — 진단 화면에 "최근 크래시" 슬롯이 비어 있음.

## 1. 결정 요약

1. **`KeyManager::update_scope`** 추가 — `scope` 전체 교체 (key_prefix / key_hash / created_at은 보존). revoked 상태도 편집 허용.
2. **IPC `update_api_key_scope`** — `models + endpoints 둘 다 빈 scope`는 `EmptyScope`로 거부. 무용 키 차단.
3. **`ApiKeyEditModal`** 컴포넌트 — alias / key_prefix는 read-only. origins / models / endpoints / expires_at / pipelines override 모두 편집.
4. **`crash::list_crash_reports` + `read_crash_log`** — panic_hook의 기존 `CRASH_DIR` static을 `pub fn crash_dir()`로 노출하고 IPC가 그것을 source of truth로 사용.
5. **파일명 안전성 검증** — `crash-` prefix + `.txt` suffix + path separator/`..`/null byte 차단. 1 MB cap.
6. **`CrashSection` (Diagnostics 5번째 row, full-width)** — mtime DESC 목록 + 토글 expand로 본문 조회.

## 2. 채택안

### 2.a `KeyManager::update_scope`

* `key-manager/src/store.rs::update_scope(id, &Scope) -> Result<(), StoreError>` — 단일 UPDATE 쿼리로 `scope_json` 컬럼 통째로 교체. row 미존재 시 `NotFound`.
* `key-manager/src/manager.rs::update_scope(id, Scope)` — Mutex lock 후 store에 위임.
* revoked 키 편집 허용: 회수된 키도 정책상 *필터 정정*은 가능 (재활성은 별도 정책 — v1 unsupported, scope 변경만 함). `verify`는 revoked_at 검사로 여전히 거부.
* **테스트 invariant** (3 추가):
  - `update_scope_replaces_all_fields_preserves_hash_and_prefix` — key_prefix / key_hash 변하지 않음.
  - `update_scope_unknown_id_returns_not_found`.
  - `update_scope_works_on_revoked_keys`.
  - 매니저 레벨: `update_scope_replaces_all_filters` + `update_scope_then_verify_uses_new_filters` (live edit이 verify 결과에 반영됨을 검증).

### 2.b IPC `update_api_key_scope`

* `commands.rs::UpdateKeyScopeRequest { id, scope }` — Scope를 통째로 전송.
* 서버 측 검증: `scope.models.is_empty() && scope.endpoints.is_empty()` → `KeyApiError::EmptyScope` (한국어 메시지).
* `keys.toml`에 `allow-update-api-key-scope` identifier 추가, `capabilities/main.json`에 등록.
* 한국어 카피: "scope의 endpoints / models 둘 다 비어 있으면 모든 호출이 차단돼요. 최소 1개는 채워주세요."

### 2.c `ApiKeyEditModal`

* `ApiKeysPanel`에 "필터 편집" 버튼 추가 — revoked 키는 노출 안 함.
* Modal 디자인 패턴은 `ApiKeyIssueModal`과 동일 (Esc 닫기, 배경 클릭, focus trap, role="dialog" + aria-modal).
* alias / key_prefix는 `keys-field is-readonly` 클래스로 read-only 표시 — 평문 재발급이 아님을 시각 신호.
* save 시 `EmptyScope` 에러 kind를 받아 한국어 안내 (`keys.errors.emptyScope`). `updateFailed`는 일반 fallback.
* i18n ko / en 동시 갱신: `keys.editModal.*`, `keys.actions.edit`, `keys.errors.{emptyScope, updateFailed}`.

### 2.d Crash 디렉터리 노출

* `panic_hook::crash_dir() -> Option<PathBuf>` — 기존 `CRASH_DIR` static을 read-only로 외부에 공개. `install`이 한 번만 등록하므로 race 없음.
* `crash::list_crash_reports(Option<u32>)` — read_dir + filter (`crash-*.txt`) + sort by mtime DESC + truncate(limit). limit 미지정 시 50.
* `crash::read_crash_log(filename)` — `is_safe_crash_filename` 검증 후 1 MB cap + `read_to_string`.
* `CrashIpcError` 5종 (`NotInitialized`, `InvalidFilename`, `NotFound`, `TooLarge { bytes }`, `Io { message }`) — 모두 `kebab-case` tag + 한국어 `#[error(...)]`.

### 2.e 파일명 안전성 검증

* `is_safe_crash_filename(name)` 단위 테스트 (5건):
  - `crash-` prefix + `.txt` suffix 강제.
  - `/` `\` `..` null byte 차단.
  - 잘못된 prefix/suffix 거부.
* path traversal 시도가 `dir.join(name)`에 닿기 전에 모두 거부됨.

### 2.f `CrashSection` UI

* Diagnostics grid에 5번째 섹션으로 full-width row 배치 (`.diag-section-fullrow { grid-column: 1 / -1 }`).
* 목록은 `<ul role="list">`로 a11y 보장. 각 항목: timestamp / filename / size / View 버튼 + `aria-expanded`.
* expand 시 `<pre>`로 backtrace 표시. `prefers-reduced-motion` 준수 — 기본 토큰 사용.
* 빈 상태 / 미초기화 / TooLarge / 일반 에러 4개 분기 모두 한국어.

## 3. 기각안 + 이유 (negative space)

| 옵션 | 거부 이유 |
|---|---|
| **revoke + re-issue로만 scope 변경 강제** | 평문 1회 노출 비용 + 사용자 작업 흐름(웹앱 재배포)이 무거워짐. 단일 사용자 데스크톱 앱에서는 *과한 보안 가정*. |
| **scope 부분 patch (PUT 대신 PATCH 스타일)** | 현재 `enabled_pipelines`만 PATCH인데 5개 더 늘리면 IPC 표면이 N개로 폭발. PUT 통째로 + 빈 scope 거부가 더 단순. |
| **revoked 키 scope 편집 차단** | revoke가 실수였을 때 복원 시나리오를 막아 UX 손해. revoke 자체를 풀지 않으므로 보안 침해 없음. |
| **panic_hook 자체에 IPC 추가** | panic_hook 모듈은 sync + low-level. tauri::command를 그 안에 넣으면 테스트 격리 어려움. 별도 `crash` 모듈로 IPC 분리. |
| **`CRASH_DIR`를 `OnceLock<PathBuf>`로 변경** | 기존 `Mutex<Option<PathBuf>>`는 idempotent install + test reset을 지원. OnceLock는 test reset 불가. 변경 비용 > 이득. |
| **JSONL 또는 SQLite로 crash 영속** | 텍스트 파일 + read_dir로도 v1.x 규모(연 10건 이하 panic)는 충분. 영속화는 ADR-0046 access log 전례에 따라 v1.x deferred. |
| **자동 crash 업로드** | telemetry opt-in이 이미 panic_hook에 통합되어 있음 (별도 backend submit). 사용자 동의 없이 외부 통신 0 정책 위반은 거부. |
| **재시작 없이 panic 재현 테스트** | `std::panic::set_hook`은 process-wide static이라 cross-test 격리가 어려움. 기존 `_reset_for_tests` + grand mutex 패턴 유지. |
| **alias 편집 허용** | alias 변경은 *키 정체성 표지*가 흔들리는 효과. v1.x deferred (필요 시 별도 IPC). |
| **expires_at에 사용자 친화 datepicker** | i18n + timezone 처리 부담. RFC3339 raw 입력 + placeholder 안내가 1인 데스크톱 앱 규모에 적정. v1.x 폴리싱 후보. |

## 4. 미정 / 후순위 이월

* **scope 편집 audit log** — "누가 언제 어떤 scope을 어떻게 바꿨다" 영속 기록. 단일 사용자 앱이라 v1.x deferred. 기록 대상 결정 시 `repair-log.jsonl` 패턴 차용.
* **crash 파일 자동 청소** — 30일 이상 오래된 crash auto-delete. v1.x.
* **crash search / filter** — 키워드/날짜 범위. 50개 cap이라 v1 충분.
* **alias rename IPC** — 위 §3 참조.
* **revoke 취소 (un-revoke)** — UX 함정 + 보안 위험. 명시적 미지원.

## 5. 테스트 invariant

본 sub-phase 종료 시점에 깨면 안 되는 항목:

1. `key-manager` — `update_scope` 가 key_prefix / key_hash 보존.
2. `key-manager` — 빈 scope (model + endpoint 둘 다 빔) 거부 (commands.rs 레벨).
3. `key-manager` — revoked 키 scope 편집 후 `revoked_at` 보존.
4. `crash` — `is_safe_crash_filename` 5건 (path traversal / null / wrong suffix / etc.) 모두 거부.
5. `crash` — `filename_to_rfc3339` round-trip 정확.
6. `crash` — `CrashIpcError` 모든 variant `kebab-case` tag.
7. `commands.rs` — `KeyApiError::EmptyScope` `kebab-case` tag.

## 6. 다음 페이즈 인계

* Phase 13'.f 진입 조건 — manifest 작성 시 `commercial: false` 라벨 + `kind: embedding` 분류 필요. 현재 catalog 스키마가 받아주는지 확인 필요.
* Phase 13'.g (가능) — minisign 서명 검증 (ADR-0047). Phase 13'.c 변경은 카탈로그 흐름과 무관 — 서명 통합과 충돌 X.
* 위험 노트:
  - `CRASH_DIR` poisoned mutex가 `crash_dir()` 호출 시 `None` 반환 → 사용자에게 `not-initialized` 표시되지만 panic 적재는 계속 됨. 회복은 앱 재시작.
  - IPC `update_api_key_scope`는 SQLCipher passphrase 의존. dev 빌드(`sqlcipher` feature OFF)에서 잘못된 scope을 넣어도 평문 DB라 복구 가능 — production 빌드는 정상.

## References

- 결정 노트 본문: 본 파일.
- 코드:
  - `crates/key-manager/src/{store,manager}.rs::update_scope` + tests.
  - `apps/desktop/src-tauri/src/keys/commands.rs::update_api_key_scope` + `KeyApiError::EmptyScope`.
  - `apps/desktop/src-tauri/src/crash.rs` (신규 모듈).
  - `apps/desktop/src-tauri/src/panic_hook.rs::crash_dir` (read-only export).
  - `apps/desktop/src-tauri/permissions/{keys,crash}.toml`.
  - `apps/desktop/src-tauri/capabilities/main.json` (3 신규 identifier).
  - `apps/desktop/src/components/keys/ApiKeyEditModal.tsx` (신규).
  - `apps/desktop/src/components/keys/ApiKeysPanel.tsx` (Edit 통합).
  - `apps/desktop/src/ipc/{keys,crash}.ts`.
  - `apps/desktop/src/pages/Diagnostics.tsx::CrashSection`.
  - `apps/desktop/src/i18n/{ko,en}.json` (`keys.editModal`, `keys.errors.{emptyScope, updateFailed}`, `screens.diagnostics.sections.crash`).
- 관련 ADR: ADR-0022 (KeyManager 5차원 scope), ADR-0029 (per-key Pipelines override), ADR-0036 (panic_hook + crash report).
