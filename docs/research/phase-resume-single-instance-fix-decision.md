# Phase Resume — `tauri-plugin-single-instance` 2.0.0 패닉 fix 결정 노트

> 작성일: 2026-04-29
> 상태: 확정 (lock 갱신 + 빌드 검증 완료 시점에 적용)
> 선행: ADR-0036 (Stability — single-instance + panic hook + WAL)
> 후행: v0.0.1 ship 직전 회귀 차단. 차후 plugin patch 회귀 감시 정책 (§3 §5).

## 0. 결정 요약 (3가지)

1. **`Cargo.lock`에 잠긴 `tauri-plugin-single-instance = 2.0.0`을 `cargo update`로 최신 2.x patch (≥ 2.2.2, 현재 2.4.1)로 갱신** — Cargo.toml caret(`"2"`) 명세는 그대로 유지.
2. **사용자 코드 (lib.rs Builder 첫 plugin 등록 패턴)·`identifier`·capability 설정은 모두 정상**, 변경 불필요. 회귀의 원인은 plugin 내부 Windows IPC 핸들 null-check 누락 (PR #2452).
3. **CI/RESUME 체크리스트에 "patch 회귀 감시 절차 (`cargo update` + 의존성 outdated 점검)" 추가**, caret 명세가 lock 정착 시 자동 갱신되지 않는 함정을 다음 세션이 다시 밟지 않도록 명문화.

## 1. 채택안

### 1.1 `cargo update -p tauri-plugin-single-instance` (즉시 적용)

- 명령: `cargo update -p tauri-plugin-single-instance` (Cargo.toml caret `"2"` 명세 유지, lock만 갱신).
- 결과 기대값: lock 내 `tauri-plugin-single-instance` version 줄이 `2.0.0` → `2.4.1` (또는 그 이상 2.x 최신 patch)로 변경. checksum 갱신.
- 차용한 글로벌 사례:
  - **GitHub `tauri-apps/plugins-workspace` PR #2452** — Windows IPC named pipe 핸들 null-check 5라인 추가. 2.2.2 (2025-02-24)에 머지/릴리즈.
  - **Issue #2405** — `STATUS_STACK_BUFFER_OVERRUN` + `windows.rs:121` null pointer dereference 보고. Rust 1.86 stable에서 정렬(align) 변경(rust-lang/rust#134424) 영향으로 회귀 노출. 본 환경 Rust 1.95에서도 동일 증상 ✅.
  - **2.4.1**까지의 모든 후속 release notes에 fix 포함 — 회귀 재발 없음.
- 트레이드오프: lock 변경 → diff 1~2줄 (plugin + 의존 sub-crate가 있으면 함께 갱신). 본 fix는 patch level이라 ABI 영향 없음.

### 1.2 사용자 코드 / 설정 변경 없음 (확인)

- `apps/desktop/src-tauri/src/lib.rs:64-72` — `tauri_plugin_single_instance::init` 등록이 Builder의 **첫 plugin** ✅ (공식 README 권고 준수).
- `apps/desktop/src-tauri/Cargo.toml:22` — `tauri-plugin-single-instance = "2"` caret 명세 유지 ✅.
- `apps/desktop/src-tauri/tauri.conf.json` — `identifier`가 ASCII reverse-DNS이고 Win32 reserved 문자 없음 ✅ (mutex 이름 sanitize 무관).
- capability `main.json`에 single-instance 권한 등록 불필요 — 공식 plugin은 capability 시스템 비사용 (Tauri 2 plugins-workspace v2 docs 확인).

### 1.3 CI / RESUME 절차 보강 — patch 회귀 감시

- **RESUME 체크리스트에 한 줄 추가**: "v1.x 진입 / 새 sub-phase 진입 시 `cargo update --workspace --dry-run` 1회 실행하여 lock-stale 의존성 표면화."
- **선택적 확장 (v1.x)**: `cargo outdated --workspace --depth 1` (cargo-outdated crate)을 verify.ps1 마지막 단계로 추가 — caret 명세가 잠긴 patch 회귀를 정기적으로 catch.

## 2. 기각안 + 이유 (Negative space — 의무 섹션)

### 2.1 `tauri-plugin-single-instance = "=2.0.X"` 또는 `"=2.4.1"` 정확 버전 핀

- **시도 / 검토 내용**: Cargo.toml에 등호 prefix로 정확 버전 핀하면 lock 재생성 시 강제 해상도. 회귀가 다시 나타나는 미래 patch 회귀(2.x post-regression)를 봉쇄.
- **거부 이유**: 정확 핀은 *향후 보안 patch 자동 수신*을 차단. v1 운영 정책상 plugin은 patch는 자동 수신, minor 이상은 명시 점검이 합리적. 등호 핀은 새 보안 fix가 늦게 들어오는 위험이 회귀 위험보다 큼. 1순위 대신 *2순위 백업*으로만 보존: `cargo update`가 의존성 그래프 충돌로 거부될 때만 수동 핀.
- **재검토 트리거**: tauri-plugin-single-instance 가 2.x 내에서 두 번째 회귀(2.5.x 등)를 일으키면 `=2.4.1` 핀 + 수동 캐치업 정책으로 전환.

### 2.2 git rev pin (`git = "https://github.com/tauri-apps/plugins-workspace", branch = "v2"`)

- **시도 / 검토 내용**: crates.io 게시 지연 회피용. 핫 fix가 main에는 있고 crates.io 미발행일 때 사용.
- **거부 이유**: 본 케이스는 **이미 crates.io 2.4.1까지 발행됨** — git pin 불필요. git pin은 lockfile reproducibility는 유지되지만 release 검증 절차를 우회하는 *불안 요소*. v1 ship 직전에는 회피.
- **재검토 트리거**: 미래에 crates.io 미발행 핫 fix가 필요한 1회성 상황.

### 2.3 plugin 비활성화 + 자체 named-pipe / Mutex 직접 구현

- **시도 / 검토 내용**: ADR-0036 §A에서 이미 거부한 자체 mutex 파일 패턴의 *named pipe 변형*. Rust로 `CreateNamedPipeW` 직접 호출.
- **거부 이유**: ADR-0036 §A 거부 사유 그대로 — cross-platform stale handle 검증 + permission 문제 + 비정상 종료 시 stale 잔재. **공식 plugin에 5라인 fix가 이미 머지된 상태에서 자체 구현은 비합리적**. v1 시점 부담 가장 큼.
- **재검토 트리거**: tauri-plugin-single-instance가 2.x 라인 EOL 선언 + 3.x로 강제 마이그레이션 비용이 너무 클 때.

### 2.4 plugin 자체 fork → mutex 이름 sanitize 로직 추가 PR

- **시도 / 검토 내용**: identifier에 비ASCII / Win32 reserved 문자가 들어가면 mutex 이름 생성 시 null로 떨어진다는 가설. fork해서 sanitize 로직 PR.
- **거부 이유**: 본 환경 `identifier = "com.lmmaster.desktop"`는 ASCII reverse-DNS, **reserved 문자 0**. 가설 자체가 본 케이스 원인 아님. 또한 PR #2452의 fix가 이미 null-check를 추가해 sanitize와 무관하게 안전. 공헌 가치는 있지만 *우리 ship 일정에 비핵심*.
- **재검토 트리거**: 추후 한국어 identifier 지원 등 i18n 확장 필요 시.

### 2.5 `Builder` 등록 순서 재배열 / panic-safe wrapper (`AssertUnwindSafe`)

- **시도 / 검토 내용**: panic이 나는 *위치*가 plugin init이라 wrapper로 감싸면 abort을 면할 수 있는지.
- **거부 이유**: panic은 `non-unwinding panic` (abort 직행) — `catch_unwind`가 잡지 못함. 또한 등록 순서는 이미 공식 권고 (첫 plugin) 준수. **원인은 plugin 내부 코드 결함**이라 호출자 측 wrapping은 무효.
- **재검토 트리거**: 미래에 plugin이 *recoverable* panic으로 전환되면 이 패턴 도입 검토.

## 3. 미정 / 후순위 이월

| 항목 | 이월 사유 | 진입 조건 / 페이즈 |
|---|---|---|
| `cargo outdated --workspace --depth 1`을 `.claude/scripts/verify.ps1`에 통합 | v1 ship 우선. cargo-outdated 자체 의존성 추가 + 빌드 시간 영향 검증 필요 | v1.x 안정화 페이즈 |
| 모든 Tauri plugin patch 회귀 자동 catch CI job | GitHub Actions runner 비용 / dependabot 충돌 검토 필요 | v1.x post-ship |
| Rust 1.86+ align 변경 영향 받는 *다른* 의존성 정밀 점검 | rust-lang/rust#134424 영향 plugin은 single-instance 외 미보고. 발견 시 별도 fix | 회귀 보고 시 |
| 한국어 identifier 지원 (i18n 확장) | v1은 reverse-DNS ASCII만. UI/Branding 영향 큼 | v1.x 또는 v2 |
| 자체 mutex 구현으로의 fallback 경로 | 공식 plugin 채택이 ship 정책. EOL 신호 없음 | EOL 선언 시 |

## 4. 테스트 invariant

> single-instance plugin은 OS-level IPC 의존이라 unit test로 catch 불가. invariant는 *컴파일 타임 + 런타임 smoke + lock 정책*으로 분산.

- **컴파일 타임**: `tauri::Builder::default().plugin(tauri_plugin_single_instance::init(...))` 가 컴파일 — type / API 호환 검증 (이미 ADR-0036 §Test invariants).
- **런타임 smoke (수동, RESUME 체크리스트)**: 빌드 산출물 1회 실행 → panic 없이 메인 윈도우 표시 → 두 번째 인스턴스 실행 시 첫 윈도우 포커스 + 두 번째 즉시 종료. 본 결정 노트 적용 후 **이 smoke가 panic 없이 통과**해야 ship.
- **Lock 정책 invariant**: `Cargo.lock` 내 `tauri-plugin-single-instance` version이 `>= 2.2.2` (PR #2452 이후 버전) 인지 RESUME 체크리스트에서 확인. 다음 세션이 `cargo update --offline` 등으로 lock을 옛 상태로 reset하면 RESUME에서 catch.
- **Identifier invariant**: `apps/desktop/src-tauri/tauri.conf.json` 의 `identifier`가 ASCII reverse-DNS만 허용 (Win32 reserved 문자 거부). 추후 i18n identifier 도입 시 plugin sanitize 능력 검증부터.
- **회귀 감시 invariant**: 새 plugin patch 채택 전 changelog의 "fix" / "windows" / "panic" / "null" 키워드 grep 1회.

## 5. 다음 페이즈 인계

- **선행 의존성**: ADR-0036 (Stability — single-instance) — 본 결정은 ADR-0036의 *patch 회귀 fix*이지 설계 변경 아님. ADR-0036 자체는 그대로 Accepted.
- **이 페이즈 산출물**:
  - `Cargo.lock` 1개 행 갱신 (`tauri-plugin-single-instance 2.0.0` → `2.4.1`).
  - `docs/research/phase-resume-single-instance-fix-decision.md` (본 노트).
  - `docs/adr/0036-stability-single-instance-panic-wal.md` References 1줄 추가 (PR #2452 + 2.4.1 lock 갱신 트리거).
  - `docs/RESUME.md` "누적 검증" 줄에 fix 메모 1줄.
- **다음 sub-phase로 가는 진입 조건**:
  - `cargo build -p lmmaster-desktop` 성공.
  - `run_dev.bat` 1회 실행 → panic 없이 메인 윈도우 도달.
  - 위 2개 통과 시 Phase 9'.c (Multi-runtime adapters) standby로 복귀.
- **위험 노트** (next session이 빠질 수 있는 함정):
  - **caret + lock의 함정**: `Cargo.toml`의 caret은 lock이 없을 때만 resolve. 한 번 잠긴 lock은 `cargo update` 없이 자동 갱신되지 않음. 다음 세션이 "Cargo.toml에 `"2"`라 적혀 있으니 최신이겠지"라고 가정하면 같은 함정에 다시 빠짐. **§4 Lock 정책 invariant**가 이를 막는다.
  - **Rust 1.86+ stable 회귀의 일반화 가능성**: rust-lang/rust#134424 align 변경 영향 plugin은 single-instance 외 현재 미보고. 다른 plugin이 비슷한 회귀를 일으키면 본 결정 노트의 *패턴*을 재활용 (§1.3 patch 회귀 감시 절차).
  - **2.x 라인 회귀 재발 시 핀 전환**: §2.1 핀 옵션은 거부했지만 *재검토 트리거*로 보존. 두 번째 회귀가 보고되면 즉시 `=2.4.1` (또는 안전 마지막) 핀 전환.

## 6. 참고

### 인용 (외부)

- PR fix: <https://github.com/tauri-apps/plugins-workspace/pull/2452>
- Issue 원본: <https://github.com/tauri-apps/plugins-workspace/issues/2405>
- 2.2.2 release notes: <https://github.com/tauri-apps/plugins-workspace/releases/tag/single-instance-v2.2.2>
- crates.io 버전 이력: <https://crates.io/crates/tauri-plugin-single-instance/versions>
- Rust 회귀 관련: <https://github.com/rust-lang/rust/pull/134424>
- V1 백포트 PR: <https://github.com/tauri-apps/plugins-workspace/pull/2657>

### 관련 ADR / 결정 노트

- ADR-0036 — Stability (본 fix의 모태 설계).
- `docs/research/phase-8p-9p-10p-residual-plan.md` §1.6.2 — single-instance 보강 리서치 원본.

### 메모리 항목 (선택)

- **추가 후보**: "tauri-plugin-single-instance 2.0.0 lock 회귀 — Cargo.lock이 caret 명세보다 옛 patch에 잠길 수 있음. 회귀 catch는 RESUME 체크리스트의 `cargo update --dry-run` 단계." → 본 노트 자체로 충분, 메모리 추가 보류 (코드/락에 박힘 + ADR-0036 References 보강으로 회상 경로 확보).

---

**핵심 한 줄**: 사용자 코드는 정상, `Cargo.lock`이 fix-이전 2.0.0에 잠긴 게 회귀 원인. `cargo update -p tauri-plugin-single-instance`로 lock 갱신 + RESUME 체크리스트에 patch 회귀 감시 절차 추가가 v0.0.1 ship 직전의 표준 응답.
