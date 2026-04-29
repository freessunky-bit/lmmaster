# Phase 1A.4.e — Polish & Palette 결정 노트

> 보강 리서치 (2026-04-27) 종합. Aurora + Glass 디자인 폴리시 + Command Palette (⌘K).

## 1. 핵심 결정

| 항목 | 결정 | 근거 |
|---|---|---|
| Aurora 레이어 | radial 3장: 두 장은 `--primary-a-*`, 한 장은 `--accent` 6% (whisper) | 단일 accent 강박 회피 + 2018 dashboard 톤 회피 (Linear/Vercel 패턴) |
| Aurora 애니메이션 | **없음** (정적) | 데스크톱 앱은 응시 시간이 길어 drift = 멀미. 마케팅 페이지와 다름 (Vercel 정적, Linear 마케팅만 동적) |
| Aurora 마스크 | `mask-image: linear-gradient(to bottom, black 70%, transparent)` | 하단 chrome bleed 회피 |
| Aurora 적용 | 마법사 root + 향후 MainShell main 영역 | viewport-wide은 backdrop-filter 금지 — aurora만 |
| Glass blur | `backdrop-filter: blur(20px) saturate(160%)` + `-webkit-` prefix | Comeau 가이드. <12 cheap, >30 unrecognizable |
| Glass 추가 layer | 1px white-a-2 outer border + `inset 0 1px 0 white-a-3` (top highlight, Apple secret) + 2-layer drop shadow | inner highlight 없으면 평면. Linear/Apple 공통 |
| Glass 적용 범위 | **Command Palette dialog + 옵션 toggle로 카드** | viewport 전면 backdrop-filter는 M1 air에서 30fps drop (battery kill) |
| Glass 격리 | `isolation: isolate` + `overflow: hidden` | 둥근 모서리 halo 제거 |
| Spotlight 구현 | JS pointermove → CSS var(--mx, --my) → rAF 게이트 → `radial-gradient at var(--mx) var(--my)` | CSS-only는 mouse-following 불가 (Houdini 미정착). rAF 자동 ~16ms throttle |
| Spotlight 적용 | `<SpotlightCard>` 컴포넌트로 wrap → 마법사 카드 + 향후 추천 카드 | Linear 카드 패턴 |
| Premium 룩 추가 | 80~120ms transition base rule + `:active scale(0.98)` (button only) + inset 1px white-a-2 (옵션) | 공통 micro-interaction. Linear 시그니처 |
| Reduced-motion | aurora layer hide + spotlight flat tone + active-press none | 토큰 레벨 dur-* 0ms는 이미 처리 |
| Reduced-transparency | glass background → opaque surface-2 + backdrop-filter none | `prefers-reduced-transparency: reduce` 새 미디어 쿼리 |
| Palette 라이브러리 | **Ark UI Combobox 5.36** — open/onOpenChange 컨트롤드 + useListCollection + 화살표/Enter/Esc 빌트인 | cmdk는 28KB 추가 dep. 자체 구현은 200줄. Ark는 90% 커버 |
| 글로벌 hotkey | document keydown ⌘K/Ctrl+K → preventDefault → setOpen(!open) | OS-level은 시스템 충돌 (VS Code/Slack 등 ⌘K). 후순위 settings opt-in |
| Palette 위치 | top: 120px, width: 540px, max-height: 60vh, 가로 중앙 | Linear 스펙 그대로 |
| Group 헤딩 | non-sticky | 30 명령 미만이라 sticky 비용 불필요 (Raycast 패턴) |
| 한국어 검색 | 단순 substring on (label + keywords[]) — Hangul jamo decomp 안 함 | 4KB 라이브러리 회피. keywords에 jamo cheat ("ㅅㅊ") 수동 추가 가능 |
| Command 등록 | `useCommandRegistration` hook — 라우트별 useEffect cleanup으로 자동 unregister | 분산 등록 + 자동 해제 |
| Palette state | **별도 React Context** (CommandPaletteProvider). xstate 영향 0 | 도메인 vs UI 분리 |
| 명령 perform | sync void / async Promise. Disabled는 회색 + selectable 안 됨. async는 close on resolve | 표준 Raycast UX |
| Animation | backdrop fade 120ms + dialog scale 0.96→1 y -8→0 150ms (framer-motion) | 1A.4.a에서 도입한 Framer Motion 재사용 |
| 입력 IME | autoFocus + IME composition 자연 처리 (Combobox 빌트인) | 한글 IME 별도 처리 불필요 |
| Empty state | "찾는 명령이 없어요. 다른 단어로 검색해 주세요." | 해요체 |

## 2. 시드 명령 (1A.4.e)

마법사:
- `wizard.lang.ko` "언어를 한국어로" — keywords ["language","korean","ko","ㅎㄱ","ㅇㅇ"]
- `wizard.lang.en` "Switch to English" — keywords ["언어","영어","en"]
- `wizard.scan.retry` "환경 다시 점검" — keywords ["scan","environment","ㅎㄱㅈㄱ"]
- `wizard.restart` "마법사 처음부터" — keywords ["restart","reset","reset wizard"]

MainShell (마법사 외):
- `nav.home` "홈으로 가기" — keywords ["home","ㅎ"]
- `nav.diagnostics` "진단 보기" — keywords ["diagnostics","ㅈㄷ"]
- `system.gateway.copyUrl` "게이트웨이 URL 복사" — keywords ["copy","url","port"] — disabled when status !== "listening"
- `wizard.reopen` "마법사 다시 보기" — keywords ["onboarding","wizard","ㅁㅂㅅ"] — markCompleted 해제 + reload

## 3. 파일 추가/변경 (총 ~660 LOC)

### Step 1 — design-system 토큰 + 클래스 (135 LOC)
- `packages/design-system/src/tokens.css` +25 — `--aurora-1/2/3`, `--aurora-mask`, `--shadow-glass-1/2`, `--glass-blur`, `--glass-saturate`
- `packages/design-system/src/components.css` +90 — `.glass`, `.surface-aurora`, `.spotlight` + reduced-transparency degradation
- `packages/design-system/src/base.css` +20 — interactive transition rule + `:active` scale (button/role=button only) + `<kbd>` style

### Step 2 — 마법사 적용 + SpotlightCard (55 LOC)
- `apps/desktop/src/onboarding/onboarding.css` +15 — `.onb-root` use `.surface-aurora`, `.onb-runtime-card.spotlight` 옵션 적용
- `apps/desktop/src/components/SpotlightCard.tsx` +40 — pointermove + rAF + var

### --- Checkpoint here (시각만 적용. Palette 미통합) ---

### Step 3 — Command Palette 신설 (~440 LOC)
- `apps/desktop/src/components/command-palette/types.ts` +25
- `apps/desktop/src/components/command-palette/context.tsx` +90
- `apps/desktop/src/components/command-palette/filter.ts` +20
- `apps/desktop/src/components/command-palette/CommandPalette.tsx` +130
- `apps/desktop/src/components/command-palette/palette.css` +120
- `apps/desktop/src/hooks/useCommandPaletteHotkey.ts` +25
- `apps/desktop/src/i18n/{ko,en}.json` +30 — palette.* keys

### Step 4 — 마운트 + 시드 등록 (20 LOC)
- `apps/desktop/src/App.tsx` +8 — `<CommandPaletteProvider>` wrap + `<CommandPalette/>` 글로벌 포털 + MainShell 시드 명령 4건 등록
- `apps/desktop/src/onboarding/OnboardingApp.tsx` +12 — useCommandRegistration로 wizard 명령 4건

### main.tsx — components.css import 추가
```ts
import "@lmmaster/design-system/components.css";
```

## 4. 검증 체크리스트

- `pnpm exec tsc -b` 통과
- `pnpm run build` 통과
- `cargo clippy --workspace --all-targets -- -D warnings` 통과
- `cargo test --workspace` 통과 (Rust 변경 없음 — 100건 유지)
- (사용자) dev — ⌘K/Ctrl+K로 팔레트 open, 한국어/영어 mixed 검색, ↑↓ Enter 선택, Esc 닫기. 마법사 진입 시 aurora glow + spotlight on hover 카드.

## 5. 비목표 (1A.4.e 외)

- OS-level 글로벌 단축키 (`tauri-plugin-global-shortcut`) — 후순위 settings opt-in
- es-hangul 같은 jamo decomposition 라이브러리 — 사용자 요청 시 도입
- Storybook — 후순위
- 사운드 효과 — 후순위
- Tauri OS-native title bar custom — 후순위

## 6. 참고

- [Ark UI Combobox](https://ark-ui.com/react/docs/components/combobox)
- [Comeau — frosted glass](https://www.joshwcomeau.com/css/backdrop-filter/)
- [MDN prefers-reduced-transparency](https://developer.mozilla.org/en-US/docs/Web/CSS/@media/prefers-reduced-transparency)
- [Albert Walicki — Aurora UI with CSS](https://dev.to/albertwalicki/aurora-ui-how-to-create-with-css-4b6g)
- Linear / Vercel / Raycast UX 표준
