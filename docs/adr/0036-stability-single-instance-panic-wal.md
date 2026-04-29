# ADR-0036: Stability — single-instance + panic hook + SQLite WAL (Phase 8'.0.b)

- Status: Accepted
- Date: 2026-04-28
- Related: ADR-0001 (companion 데스크톱 + localhost gateway), ADR-0002 (Tauri 2), ADR-0008 (SQLite + 옵션 SQLCipher), ADR-0035 (SQLCipher activation)
- 결정 노트: 본 ADR + `docs/research/phase-8p-9p-10p-residual-plan.md` §1.6.2

## Context

v1 ship 전 안정성 측면 3가지가 미정이었어요:

1. **다중 인스턴스 충돌**: 사용자가 LMmaster.exe를 두 번 더블클릭하면 두 번째 인스턴스가 같은 SQLite DB / 같은 :PORT (gateway)에 접근 시도. 첫 인스턴스가 깨질 수 있음.
2. **panic 안내 부재**: `unwrap_or_panic` 같은 마지막 보루가 발생해도 사용자는 "윈도우가 사라졌어요"만 보임. crash report 없음.
3. **SQLite 기본 journal mode (DELETE)**: sync write 비용 + reader/writer mutex 충돌. 작은 DB라도 매 commit 시 fsync 2회 발생.

이 셋은 같은 sub-phase에 모아 처리. v1 ship critical path.

## Decision

### 1. Tauri single-instance plugin — 첫 창 포커스

- `apps/desktop/src-tauri/Cargo.toml`: `tauri-plugin-single-instance = "2"`.
- `apps/desktop/package.json`: `@tauri-apps/plugin-single-instance ^2.0.0`.
- `lib.rs::run()` Builder의 첫 plugin로 등록:
  ```rust
  .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
      if let Some(window) = app.get_webview_window("main") {
          let _ = window.set_focus();
          let _ = window.unminimize();
          let _ = window.show();
      }
  }))
  ```
- 두 번째 실행 시 first 인스턴스의 webview window가 focus + unminimize + show. 두 번째 프로세스는 그대로 종료 (plugin이 자체 처리).

### 2. Rust panic hook — 한국어 메시지 + crash report

- `apps/desktop/src-tauri/src/panic_hook.rs` 신설.
- `panic_hook::install(crash_dir)` — `std::panic::set_hook` 등록. `Tauri::Builder` 호출 *전*에 한 번만 호출.
- hook 본문은 `std::panic::catch_unwind` 안에서 동작 — 자체 panic 시 무한 재귀 차단.
- 동작:
  1. `tracing::error!` 기록.
  2. `<crash_dir>/crash-<rfc3339>.txt` 파일 작성: payload + location + 한국어 메시지 + backtrace.
  3. previous hook chain 호출 (default stderr 출력 등 보존).
  4. (Tauri runtime 살아 있으면) MessageDialog로 한국어 알림 표시 — i18n 키 `dialogs.crash.body`. v1.0은 알림 표시는 v1.x로 미룸 (Tauri 다이얼로그 sync 호출 패턴은 setup() 외부에서 어려움).
- crash 디렉터리: `%LOCALAPPDATA%/lmmaster/crash` (Win) / `~/Library/Application Support/lmmaster/crash` (mac) / `$XDG_DATA_HOME/lmmaster/crash` (Linux).

### 3. SQLite WAL + busy_timeout=5s + synchronous=NORMAL — 4 SQLite 사용처

- 적용 대상:
  - `crates/key-manager/src/store.rs` (Phase 8'.0.a 통합).
  - `crates/knowledge-stack/src/store.rs`.
  - `crates/registry-fetcher/src/cache.rs` — *이미* WAL 활성 (Phase 1' 시 적용).
  - `apps/desktop/src-tauri/src/commands.rs::CatalogState` — RwLock<Arc<Catalog>>로 SQLite 미사용. WAL 적용 불필요 (model-registry는 JSON, knowledge-stack과 분리).
  - `crates/model-registry/src/register.rs` — JSON 기반. WAL 적용 불필요.
- pattern (key-manager 기준):
  ```rust
  fn apply_stability_pragmas(conn: &Connection) -> Result<(), StoreError> {
      let _: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
      conn.busy_timeout(std::time::Duration::from_millis(5000))?;
      conn.execute_batch("PRAGMA synchronous = NORMAL;")?;
      Ok(())
  }
  ```
- in-memory DB는 WAL 무의미 → busy_timeout만 적용.
- 검증: 각 store에 `journal_mode()` getter 추가 + unit test로 file-backed DB가 "wal" 반환하는지 확인.

## Consequences

### Positive

- 사용자가 LMmaster를 두 번 켜도 자동으로 첫 창 활성. SQLite lock 충돌 / port 충돌 0.
- panic 발생 시 사용자가 빈 화면을 보는 대신 (적어도) crash 파일이 디스크에 쌓여 진단 가능. 향후 AppHandle 살아있는 시점에서 사용자 dialog로 안내 가능.
- WAL 활성으로 read while write 가능 → UI rendering이 backend write를 막지 않음. 작은 DB라도 sync overhead 절반 절감.

### Negative

- single-instance plugin의 callback이 *현재 process*에서 실행됨 — 즉, 두 번째 프로세스에서 `app.get_webview_window` 호출이지만 plugin이 IPC로 first 인스턴스에 forward. Tauri 2가 전부 처리하므로 본 ADR에서는 Recommended 패턴만 사용.
- panic hook 등록은 process-wide. 테스트 환경에서 hook 누수 방지를 위해 `_reset_for_tests()` helper + grand mutex 추가.
- WAL은 NFS / 네트워크 드라이브에서 동작 안 함. portable workspace 정신상 사용자 PC 로컬 가정 — 문제 없음.
- WAL 모드 활성 후 `.db-wal` / `.db-shm` 사이드카 파일 생성. 사용자가 `keys.db`만 백업하면 마지막 commit이 사라질 수 있음 — Settings → Workspace 정리 메뉴에서 자동 checkpoint 권장.

## Alternatives considered

### A. Mutex 파일 (~/.lmmaster.pid) 직접 관리

**거부 이유**: cross-platform stale pid 검증 + permission 문제 + 비정상 종료 시 stale pid가 남아 두 번째 실행 차단. tauri-plugin-single-instance는 OS 별 이미 검증된 IPC 채널 사용 — 자체 구현보다 안정.

### B. panic_hook 대신 process-level signal handler (Unix SIGABRT)

**거부 이유**: cross-platform 부재 (Windows는 SIGABRT 없음 — `_set_invalid_parameter_handler` 별도). Rust `set_hook`은 OS 추상화 + `catch_unwind` 친화. signal handler는 hook 안에서 다시 호출하면 deadlock 위험.

### C. WAL 미적용 + 기본 DELETE journal 유지

**거부 이유**: 사용자 PC가 SSD라도 fsync 2회 + reader 차단 → UI freeze 가능. Phase 1'에서 registry-fetcher가 WAL 적용한 것과 일관성 깨짐. 작은 DB라도 안정성 표준 통일이 v1 trust 빌드.

### D. `synchronous = OFF` (성능 우선)

**거부 이유**: power loss 시 마지막 commit 데이터 손상 가능. NORMAL은 WAL과 함께 쓰면 안정성 + 성능 절충 — SQLite 공식 권장. OFF는 desktop 사용자 trust violation 위험.

### E. panic 시 즉시 process 종료 (`std::process::abort`)

**거부 이유**: hook chain 호출 + crash report 작성 기회 차단. 사용자에게 안내 0. previous hook(default stderr)이 stderr에 적도록 두는 게 진단 친화.

## Test invariants

- single-instance: plugin 등록 자체가 Tauri Builder 컴파일에서 검증 (의존성 그래프 / 타입). 런타임 검증은 통합 테스트 (out-of-scope, v1.b).
- panic_hook:
  - `install(None)` 다중 호출 시 두 번째부터 no-op (idempotent).
  - panic 발생 시 crash 디렉터리에 `crash-<ts>.txt` 1개 이상 생성.
  - 한국어 메시지("예기치 못한 오류") + 사용자 payload 포함.
  - crash_dir 미지정 시 panic은 정상 처리 + 파일 작성만 skip.
- WAL:
  - file-backed DB는 `PRAGMA journal_mode == "wal"`.
  - in-memory DB는 `"memory"` (WAL 미적용).

## References

- [Tauri 2 single-instance plugin](https://v2.tauri.app/plugin/single-instance/)
- [SQLite WAL — Write-Ahead Logging](https://www.sqlite.org/wal.html)
- [Rust std::panic::set_hook](https://doc.rust-lang.org/std/panic/fn.set_hook.html)
- LMmaster 결정 노트: `docs/research/phase-8p-9p-10p-residual-plan.md` §1.6.2
