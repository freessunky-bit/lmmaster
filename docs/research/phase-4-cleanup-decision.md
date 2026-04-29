# Phase 4 cleanup decision — StatusPill 마이그레이션 + ko.json voice audit

> 일자: 2026-04-27
> 범위: 인라인 pill markup → design-system StatusPill 마이그레이션, ko.json Toss UX writing 8원칙 audit.

## 1. 채택안 (adopted)

### 1.1 StatusPill 마이그레이션
인라인 `<div className="home-gateway-pill">` 와 `<div className="gateway-pill">` 두 곳을 `@lmmaster/design-system/react`의 `StatusPill` 컴포넌트로 통합.

- **Home.tsx의 `GatewayPillLarge`** — `size="lg"`, banner 메시지를 `label`로, 포트를 `detail`로.
- **App.tsx의 sidebar gateway-pill** — `size="sm"`, `className="sidebar-pill"`로 하단 정렬, `gw.error`는 `ariaLabel`로.
- **OnboardingApp.tsx** — `wizard-gateway-pill` markup 부재 확인 (skip).
- **main.tsx** — `@lmmaster/design-system/react/pill.css` import 추가.
- **base.css** — `.sidebar-pill { margin-top: auto; }` 한 줄 추가.

`PillStatus` 매핑은 1:1 (`booting/listening/failed/stopping`) + default `idle`. `home.css`의 `.home-gateway-pill` 룰 블록은 주석 처리 + `MIGRATED to StatusPill` 표식 (audit trail 유지).

`design-system/package.json`에 `@types/react` devDependency 추가 — desktop tsc가 design-system .tsx를 그래프에 포함해 typecheck하기 때문.

### 1.2 ko.json Toss 8원칙 audit
8원칙 (의문문 호명 / 공식체 / 영어 뱅크 / 외래어 남발 / 명령조 / 부정문 / 모호한 시간 / 사용자 책임 전가) 기준으로 manual 검토.

위반 7건 식별 + 수정. 발견 영역: `home.gateway-*`, `screens.runtimes.card.notInstalled`, `screens.runtimes.models.empty.title`, `onboarding.scan.subtitle.running`, `onboarding.install.etaPending`, `keys.modal.revealBody`. catalog/bench/keys/workspace.repair/screens(나머지) 영역은 이미 audit 통과로 그대로 유지.

## 2. 기각안 (rejected — 의무 항목)

### 2.1 home.css의 unused pill 룰 즉시 삭제
**기각.** `.home-gateway-pill` 관련 CSS 룰은 StatusPill로 마이그레이션 후 사용되지 않지만, 즉시 삭제 시 git diff에서 audit trail이 사라져 회귀 추적이 어려워져요. 주석 + `MIGRATED to StatusPill` 표식으로 남겨 v1.x cleanup 페이즈에서 정식 삭제 예정.

### 2.2 일괄 sed 스크립트로 ko.json audit
**기각.** `s/입니다/예요/g` 같은 일괄 치환은 의미 변형 위험이 커요 (예: "필요 입니다" → "필요 예요"는 어색). 8원칙은 문맥별 판단이 필요한 항목 — manual 검토만이 안전하다고 판단.

## 3. 영향 범위 (scope)

| 파일 | 변경 |
|---|---|
| `apps/desktop/src/main.tsx` | pill.css import +1 줄 |
| `apps/desktop/src/App.tsx` | sidebar pill markup → StatusPill (8 줄), `mapGatewayStatus` 헬퍼 추가 (15 줄) |
| `apps/desktop/src/pages/Home.tsx` | `GatewayPillLarge` markup → StatusPill, `mapStatus` 헬퍼 추가 |
| `apps/desktop/src/pages/home.css` | `.home-gateway-pill` 블록 주석 처리 |
| `apps/desktop/src/i18n/ko.json` | 7개 키 한국어 voice 수정 |
| `packages/design-system/src/base.css` | `.sidebar-pill` 룰 +1 줄 |
| `packages/design-system/package.json` | `@types/react` devDep +1 |

## 4. 검증 결과 (verification)

- `pnpm exec tsc -b` — 본 task 책임 영역 0 에러 (남은 1건 `Runtimes.test.tsx` Unused @ts-expect-error는 본 task 영역 밖 기존 이슈).
- vitest — 본 task가 새로 깨뜨린 테스트 0건. 14개 fail은 모두 기존 i18n init / canvas mock / IPC mock 관련 이슈로 사전 존재.

## 5. ko.json audit 통계

- 검사 라인: 약 280개 키 (en.json 제외).
- 위반 라인 (수정): **7건**.
- 검토 필요 (그대로 유지): **3건** — `model.maturity.{stable,beta,experimental,deprecated}` 영문 그대로 (모델 maturity 표준어 — 별도 디자인 합의 필요), `screens.workbench.hero.body`의 "1-click" (마케팅 표현으로 의도된 듯), `onboarding.install.error.installer-exit-nonzero` "정상 종료하지 않았어요" (긍정 변환 시 어색).
- 이미 통과 (수정 X): catalog/bench/keys (errors 제외)/workspace.repair/screens.diagnostics/screens.projects/screens.settings/screens.workbench/screens.install (모두 이전 페이즈에서 해요체 audit 통과).

## 6. 다음 페이즈로 인계

- `home.css`의 `.home-gateway-pill` 주석 블록 v1.x cleanup 페이즈에서 정식 삭제.
- `Runtimes.test.tsx`의 Unused @ts-expect-error 별도 sub-phase에서 정리.
- `model.maturity.*` 한국어화 여부 디자인 합의 후 결정 (Stable → 안정, Beta → 베타 등).
- en.json voice audit은 별도 페이즈 (영어는 본 audit 대상 아님).
