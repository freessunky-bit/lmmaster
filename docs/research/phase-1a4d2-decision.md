# Phase 1A.4.d.2 — 컴포넌트 테스트 결정 노트

> 1A.4.d.1 리서치 재활용. 새 보안/성능 결정점 없음 — 패턴 적용만.

## 1. 핵심 결정

| 항목 | 결정 | 근거 |
|---|---|---|
| 환경 | 모든 컴포넌트 테스트 파일 상단 `@vitest-environment jsdom` pragma | DOM 필요 |
| Context 의존 hook mock 전략 | `vi.mock("../context", () => ({ useOnboardingX: vi.fn(), ... }))` + per-test `mockReturnValue` | OnboardingProvider 마운트 회피 → IPC actor 미발동 |
| i18n 처리 | 실 `i18next` init 안 시키고 `vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (k) => k, i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" } }) }))` | t() = key 반환 → 테스트가 i18n key로 assert |
| IPC mock | `vi.mock("../../ipc/install")` + `vi.mock("../../ipc/environment")` (필요 시) | 컴포넌트 렌더만 검증. CommandPalette는 invoke 안 함 |
| user-event | 모든 클릭/키보드는 `userEvent.setup()` + `await user.click(...)` | RTL 16+ 권장 |
| portal 렌더 | CommandPalette는 `createPortal(document.body)` — `screen.getByRole(...)`로 자연 접근 | RTL이 portal 추적 |
| `Channel<T>` 타입 mock | `vi.mock("@tauri-apps/api/core")`로 invoke + Channel 둘 다 vi.fn | 실제 사용 안 하지만 import resolution용 |
| framer-motion | 테스트 환경에서도 정상 동작 — mock 불필요 (내부적으로 `IntersectionObserver` 등 polyfill 필요 시 `setup.ts`에 추가) | RTL + jsdom 호환 |
| 검증 우선순위 | 렌더 케이스 + 주요 인터랙션 1개씩 — deep coverage는 다음 sub-phase | 토큰 효율 |

## 2. 산출 파일 (6 files, ~500 LOC)

```
apps/desktop/src/__tests__/
  test-helpers.tsx                     (NEW, ~40 LOC) — render-with-providers + mock factory
apps/desktop/src/onboarding/steps/
  Step1Language.test.tsx               (NEW, ~70 LOC, 4 케이스)
  Step2Scan.test.tsx                   (NEW, ~80 LOC, 4 케이스)
  Step3Install.test.tsx                (NEW, ~100 LOC, 5 케이스)
  Step4Done.test.tsx                   (NEW, ~30 LOC, 2 케이스)
apps/desktop/src/components/command-palette/
  CommandPalette.test.tsx              (NEW, ~120 LOC, 6 케이스)
```

## 3. setup.ts 보강

`IntersectionObserver` polyfill (framer-motion이 안전망으로 사용). `matchMedia` polyfill (motion config의 reduced-motion 감지).

## 4. 검증

- `pnpm test` — 기존 42 + 신규 ~21 케이스 = **63 vitest 통과 예상**.
- Vite build / cargo clippy / cargo test 영향 0.
