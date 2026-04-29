# Phase 1A.4.d.1 — vitest infra + 머신 테스트 + 1A.4.c 잔여 결정 노트

> 보강 리서치 (2026-04-27) 종합. vitest 4.1 + xstate v5 + Tauri 2 mock 설계.

## 1. 핵심 결정

| 항목 | 결정 | 근거 |
|---|---|---|
| vitest 버전 | `^4.1.4` (Apr 2026 현재 stable) | Major rewrite, faster watcher |
| RTL | `@testing-library/react ^16.3.0` + `jest-dom ^6.6.0` + `user-event ^14.5.2` | React 18.3 호환 |
| DOM env | `jsdom ^25` (NOT happy-dom) | vitest-axe가 happy-dom에서 깨짐 (`Node.prototype.isConnected` 이슈) |
| globals | `false` — 명시 import | TS strict 친화 + grep 용이 |
| 기본 env | `node` + per-file `@vitest-environment jsdom` pragma | 순수 단위 테스트는 DOM 할당 안 함 |
| 설정 파일 | 별도 `vitest.config.ts` + `mergeConfig(viteConfig, ...)` | 기존 vite.config.ts는 Tauri-specific하게 유지 |
| coverage | v8 + 70/70/60/70 thresholds (lines/functions/branches/statements) | 기본기 — 세밀화는 1A.4.d.3 |
| 머신 actor mock | `machine.provide({ actors: { foo: fromPromise(mockedFn) } })` | xstate v5 권장. 실 IPC 미호출 |
| 비동기 대기 | `vi.waitFor(fn, { timeout, interval })` | subscribe-Promise보다 timeout/누수 안전 |
| Fake timers | `vi.useFakeTimers()` + `advanceTimersByTimeAsync` | `after` 분기 + 500ms debounce 테스트 |
| 모듈 mock 정책 | machine 테스트는 `provide` 사용. 컴포넌트는 `vi.mock("../../ipc/...")`. IPC wrapper는 `mockIPC` from `@tauri-apps/api/mocks` | 레이어드 — 1A.4.d.1은 첫 둘만 |
| 브리지 reset | `setInstallEventBridge(null)` in `beforeEach` (setup.ts) | 싱글톤 cross-test bleed 방지 |
| axe-core | 1A.4.d.3로 연기. `vitest-axe` 점찍어둠 | 1A.4.d.1 스코프 외 |
| 패키지 매니저 명령 | `pnpm test` / `pnpm test:watch` / `pnpm test:ui` / `pnpm test:coverage` | 표준 |

## 2. 1A.4.c 잔여 fix

### Issue A — OpenedUrl outcome 자동 done 차단

**문제**: `install.running` `onDone → '#onboarding.done'` 무조건 → OpenedUrl outcome 시 사용자가 "공식 사이트에서 끝내고 와주세요" 안내를 못 보고 마법사 종료.

**Fix**:
1. guard 추가: `isOpenedUrlOutcome: ({ event }) => event.output?.kind === "opened-url"`.
2. `running.onDone`을 guarded array로 교체:
   ```ts
   onDone: [
     { target: "openedUrl", guard: "isOpenedUrlOutcome", actions: "setOutcome" },
     { target: "#onboarding.done", actions: "setOutcome" },
   ]
   ```
3. 새 substate `openedUrl` 추가 (failed 형제):
   ```ts
   openedUrl: {
     on: {
       NEXT: "#onboarding.done",
       BACK: { target: "idle", actions: "clearInstallState" },
       SKIP: "#onboarding.done",
     },
   }
   ```
4. `useOnboardingInstallSub` 반환 type union에 `"openedUrl"` 추가.
5. Step3Install:
   - 분기 switch에 `case "openedUrl": return <OpenedUrlPanel outcome={outcome!} />` 추가.
   - InstallFailedPanel 안 OpenedUrl 조기-반환 제거 (이제 unreachable).
   - OpenedUrlPanel CTA를 `send({ type: "SKIP" })` → `send({ type: "NEXT" })`로 변경 (machine 명시 transition 매칭).

### Issue B — failed 상태에서 stale `installOutcome` 영향

**문제**: 이전 시도의 outcome이 context에 남아있어 RETRY 후에도 잔재. 사용자에게 잘못된 정보 노출 가능.

**Fix**: `running` state에 `entry: "clearInstallState"` 추가. 첫 진입 + RETRY (`reenter: true`) 양쪽 모두에서 outcome/error/log/progress/retryAttempt 일괄 초기화.

테스트 추가: failed → RETRY → success path에서 stale 상태 확인.

## 3. 산출물 파일

```
apps/desktop/
  vitest.config.ts                                      (NEW, ~40 LOC)
  src/__tests__/setup.ts                                (NEW, ~15 LOC)
  src/onboarding/machine.test.ts                        (NEW, ~250 LOC, 15+ 케이스)
  src/onboarding/persistence.test.ts                    (NEW, ~50 LOC, 5 케이스)
  src/onboarding/install-bridge.test.ts                 (NEW, ~25 LOC, 3 케이스)
  src/components/command-palette/filter.test.ts        (NEW, ~70 LOC, 6 케이스)
  src/onboarding/machine.ts                             (MODIFY — Issue A/B fix)
  src/onboarding/context.tsx                            (MODIFY — InstallSub union 확장)
  src/onboarding/steps/Step3Install.tsx                 (MODIFY — openedUrl 분기)
  package.json                                          (MODIFY — test deps + scripts)
```

## 4. 의존성 추가

```json
"devDependencies": {
  "@testing-library/jest-dom": "^6.6.0",
  "@testing-library/react": "^16.3.0",
  "@testing-library/user-event": "^14.5.2",
  "@types/jsdom": "^21.1.7",
  "@vitest/coverage-v8": "^4.1.4",
  "@vitest/ui": "^4.1.4",
  "jsdom": "^25.0.0",
  "vitest": "^4.1.4"
}
```

## 5. 검증 체크리스트

- [ ] `pnpm install` 통과
- [ ] `pnpm exec tsc -b` 통과 (Issue A/B fix 후)
- [ ] `pnpm run build` 통과
- [ ] `pnpm test` — 머신 15+ + persistence 5 + bridge 3 + filter 6 = **30+건 테스트 통과**
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 영향 없음 (Rust 변경 0)
- [ ] `cargo test --workspace` 100건 유지

## 6. 비목표 (1A.4.d.1 외)

- 컴포넌트 테스트 (Step1~4 + CommandPalette) — 1A.4.d.2
- axe-core 접근성 자동 검증 — 1A.4.d.3
- 통합 e2e (Tauri dev 자동화) — 후순위
- IPC wrapper 단위 테스트 (`installApp`/`detectEnvironment` 자체 mock) — 필요 시 1A.4.d.2 합류
