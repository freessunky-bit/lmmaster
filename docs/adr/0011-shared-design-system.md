# ADR-0011: 데스크톱 앱과 기존 웹앱이 디자인 시스템 공유

- Status: Accepted
- Date: 2026-04-26

## Context
기존 웹앱과 LMmaster가 같은 제품군이라는 인식을 주려면 톤앤매너 동일성이 필수다. 사용자는 dark-only + 네온 그린 단일 accent + 4단 surface depth + 4px grid 등을 명시적으로 지정.

## Decision
`packages/design-system`을 **단일 source of truth**로 둔다. 이 패키지를 (1) `apps/desktop`이 import, (2) 기존 웹앱이 npm dependency로 import.

내부 구성:
- `tokens.css` — 색상/spacing/radius/elevation/motion CSS variables
- `base.css` — 리셋 + 타이포 baseline
- `components.css` — 공유 컴포넌트 클래스
- `layout.css` — 레이아웃 프리미티브(stack, cluster, grid)
- `motion.css` — easing, duration, micro-transition
- `react/` — 공유 React 컴포넌트(Button, Input, Card, Sidebar, CommandPalette, Toast 등)
- `tokens.ts` — JS 측에서 동일한 값 export
- `voice.md` — 한국어 voice & tone 가이드(카피 작성용)

배포:
- 모노레포 내부에서는 workspace package로 직접 참조.
- 기존 웹앱(별도 repo)에서는 npm publish 또는 git submodule. v1은 npm publish(우리 organization scope) 우선.

규칙:
- 클래스명에 `ad-`, `adv-`, `ads-`, `banner-`, `sponsor-` 절대 금지.
- 인라인 스타일 남발 금지.
- 라이트 테마 만들지 않음.
- shadow 과용 금지, primary glow는 제한적.
- 숫자는 mono + tabular-nums.

## Consequences
- 기존 웹앱의 PR 1개로 디자인 시스템을 import해 톤 정렬.
- 두 코드베이스가 같은 토큰을 보므로 디자인 변경이 양쪽에 동시 반영.
- 디자인 시스템 자체가 breaking change에 민감 — semver 엄격히.

## Alternatives considered
- **각자 별도 디자인 시스템**: 톤 drift 보장. 거부.
- **CSS-in-JS 라이브러리(styled-components 등)**: portable + 빌드 단순성 측면에서 plain CSS + tokens가 우월. 거부(필요 시 v2 재검토).

## References
- ADR-0010 (한국어 우선)
