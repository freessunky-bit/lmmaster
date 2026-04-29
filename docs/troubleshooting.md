# LMmaster Troubleshooting

> Phase Env'.a (2026-04-28) — 알려진 환경 이슈와 우회 방법.

LMmaster는 Tauri 2 + Rust + React 스택으로 동작해요. 이 문서는 개발/사용 중 마주칠 수 있는 환경 이슈와 우회법을 정리해 둔 것이에요. **재현 안 되면 보고서로 부탁드려요** — 이슈는 `docs/risks.md`와 함께 관리해요.

## 알려진 환경 이슈

### 1. STATUS_ENTRYPOINT_NOT_FOUND on `cargo test -p lmmaster-desktop`

**증상**

```
error: test failed, to rerun pass `-p lmmaster-desktop --lib`
Caused by:
  process didn't exit successfully: ... (exit code: 0xc0000139, STATUS_ENTRYPOINT_NOT_FOUND)
```

`cargo build -p lmmaster-desktop` 또는 `cargo run -p lmmaster-desktop`은 정상 동작해요. `pnpm tauri:dev`도 정상이에요. 단지 `cargo test -p lmmaster-desktop`(unit test exe)만 위 오류로 즉시 종료돼요.

**근본 원인**

- Windows 11 25H2 build 26200에서 `api-ms-win-core-synch-l1-2-0.dll` ApiSet routing이 손상.
- 이 ApiSet은 `WaitOnAddress` / `WakeByAddressSingle` / `WakeByAddressAll` (kernelbase.dll re-export)을 가리켜요.
- `parking_lot` crate가 lock-free synchronization에 위 함수들을 import하는데, 테스트 exe의 import resolution이 실패해서 즉시 STATUS_ENTRYPOINT_NOT_FOUND.

**영향 범위**

- `cargo test -p lmmaster-desktop`(unit-test exe)만 실패해요.
- 다른 crate (`auto-updater`, `knowledge-stack`, `core-gateway` 등) 테스트는 모두 정상.
- `cargo run -p lmmaster-desktop` / `pnpm tauri:dev` / `pnpm tauri:build` 등 실 앱 실행은 영향 없음.
- 통합 테스트(`apps/desktop/src-tauri/tests/*.rs`)도 영향 없음 (rlib + 별도 test exe인데 cdylib 의존성 패턴이 다름).

### 해결 (관리자 PowerShell)

근본 해결은 시스템 파일 복구예요. 관리자 권한 PowerShell에서:

```powershell
# 1. 시스템 파일 무결성 검사 + 복구
sfc /scannow

# 2. Windows 이미지 복구
DISM /Online /Cleanup-Image /RestoreHealth

# 3. Windows Update 누적 패치 재설치
#    설정 → Windows Update → "업데이트 확인" → 누적 보안 업데이트 설치
```

3단계 후 시스템 재부팅하면 ApiSet routing이 정상화되고 `cargo test -p lmmaster-desktop`도 통과해요.

### 우회 (개발 편의)

근본 해결까지 시간이 걸리거나, CI 환경에서는 다음 우회법을 사용해요.

#### A. 워크스페이스 테스트 시 `lmmaster-desktop` 제외

```bash
cargo test --workspace --exclude lmmaster-desktop
```

`apps/desktop/src-tauri/src/*.rs`의 단위 테스트는 이렇게 하면 빠지지만, **로직의 95%는 별도 crate에 있어요** (`crates/auto-updater`, `crates/knowledge-stack`, `crates/core-gateway`, `crates/workbench-core`, `crates/installer`, `crates/scanner`, `crates/key-manager` 등). desktop crate 안의 commands는 얇은 wrapper라 영향이 작아요.

`run-tests.bat`은 이 옵션이 기본 적용돼 있어요.

#### B. 통합 테스트만 실행

```bash
# desktop crate의 통합 테스트는 별도 exe라 영향 없어요 (실 앱과 같은 cdylib 경로).
cargo test --test '*' -p lmmaster-desktop
```

현재는 통합 테스트가 desktop crate에 없지만, v1.x에 추가될 가능성이 있어요.

#### C. 실 앱 실행으로 검증

```bash
# 정상 동작.
cargo run -p lmmaster-desktop
pnpm tauri:dev
pnpm tauri:build
```

#### D. 프론트엔드 단위 테스트는 무관

```bash
cd apps/desktop
pnpm exec vitest run
```

vitest는 Node.js + jsdom 기반이라 ApiSet 의존성과 무관해요.

### CI 전략

GitHub Actions Windows runner는 클린 이미지라 정상 동작 가정.
로컬 개발 머신만 손상된 경우라면 매트릭스에 손상 머신을 추가하지 않으면 돼요.

```yaml
# .github/workflows/ci.yml (예시)
strategy:
  matrix:
    os: [ubuntu-latest, windows-latest, macos-latest]
steps:
  - run: cargo test --workspace
```

CI에서 ApiSet 손상은 일반적이지 않아요 — clean Windows runner라면 기본 OK.

## 기타 환경 이슈

### 빈 카탈로그로 시작했어요 / `카탈로그 로드 실패` 경고

빌드 시 `manifests/snapshot/models/`가 resource directory로 복사되는데, dev 빌드(`pnpm tauri:dev`)에서는 가끔 이 디렉터리 시드가 누락되기도 해요.

**해결**: `pnpm tauri:build`로 release 빌드를 해보세요. resource bundling이 명시적으로 진행돼요.
또는 `apps/desktop/src-tauri/tauri.conf.json`의 `bundle.resources`에 `manifests/snapshot/**`이 포함됐는지 확인.

### Workspace fingerprint mismatch (오래된 cache 폴더)

자가스캔 캐시가 stale인 경우 — `~/.lmmaster/cache/scan-*.json`을 삭제하거나, 앱 안의 "환경 다시 점검할게요" 버튼을 눌러 주세요.

### Ollama 자동 감지 실패

Windows: `%LOCALAPPDATA%\Programs\Ollama\ollama.exe`가 PATH 아니어도 LMmaster가 자동 감지해요.
실패 시: 설정 → 런타임에서 수동 경로 입력. (Phase 4'.c.2.)

## 참고 링크

- [Microsoft Docs — Windows API Sets](https://learn.microsoft.com/en-us/windows/win32/apiindex/windows-apisets)
- [parking_lot crate — Implementation overview](https://github.com/Amanieu/parking_lot/blob/master/src/parking_lot.rs)
- [Tauri 2 — IPC reference](https://v2.tauri.app/reference/javascript/api/)
- [Phase 8'/9'/10' residual plan §5 (Env'.a)](research/phase-8p-9p-10p-residual-plan.md)

## 변경 이력

| 날짜 | 항목 |
|---|---|
| 2026-04-28 | Phase Env'.a 초안 — STATUS_ENTRYPOINT_NOT_FOUND 진단 + 우회 |
