# 9. 디자인 시스템 적용 계획

> 산출물 #9. 기존 웹앱과 데스크톱 앱이 공유하는 디자인 시스템의 토큰·컴포넌트·도입 단계.
> 톤은 Linear / Figma / VS Code Dark / Raycast / Vercel / Higgsfield의 교집합.

## 9.1 원칙 (메모리 상시 적용)

- Dark-only. 라이트 테마 만들지 않음.
- 거의 검정에 가까운 배경(`bg`) 위에 4단 surface depth.
- 네온 그린 단일 primary accent. secondary semantic 4종(purple/blue/amber/red).
- 4px spacing scale. spacing은 토큰만 사용.
- Display / Body / Mono 3-font system. 숫자는 항상 mono + tabular-nums.
- Micro transition 기본 장착. primary glow는 제한적 사용.
- 클래스명에 `ad-`, `adv-`, `ads-`, `banner-`, `sponsor-` 사용 금지.
- 인라인 스타일 남발 금지.
- 정보 밀도 우선 — 마케팅 여백 금지.

## 9.2 토큰 (`tokens.css` / `tokens.ts`)

### 9.2.1 색상

```css
:root {
  /* Background depth — 거의 검정에서 한 단계씩 밝아짐 */
  --bg:          #07090b;
  --bg-subtle:   #0b0e12;
  --surface:     #11151a;
  --surface-2:   #161b21;
  --surface-3:   #1c2229;

  /* Text — 3단 */
  --text:           #e8edf2;
  --text-secondary: #9aa3ad;
  --text-muted:     #6b7480;

  /* Border / divider */
  --border:        #1f2630;
  --border-strong: #2a323d;

  /* Primary — 네온 그린 단일 hero */
  --primary:       #38ff7e;
  --primary-hover: #4cffa0;
  --primary-press: #1ee063;
  --primary-on:    #04140a;     /* primary 위 텍스트 */
  --primary-glow:  rgba(56, 255, 126, 0.35);

  /* Semantic secondary */
  --info:    #6aa3ff;     /* blue */
  --warn:    #ffb454;     /* amber */
  --error:   #ff5c6c;     /* red */
  --accent:  #b388ff;     /* purple — 부가 강조 */

  /* Focus */
  --focus-ring: var(--primary);
}
```

### 9.2.2 Spacing (4px grid)

```css
:root {
  --space-0:  0;
  --space-1:  4px;
  --space-2:  8px;
  --space-3:  12px;
  --space-4:  16px;
  --space-5:  20px;
  --space-6:  24px;
  --space-8:  32px;
  --space-10: 40px;
  --space-12: 48px;
  --space-16: 64px;
}
```

### 9.2.3 Radius / Elevation

```css
:root {
  --radius-1: 4px;
  --radius-2: 8px;
  --radius-3: 12px;
  --radius-pill: 9999px;

  /* shadow는 절제. 깊이는 surface 단계로 표현 */
  --shadow-sm: 0 1px 2px rgba(0,0,0,0.4);
  --shadow-md: 0 4px 12px rgba(0,0,0,0.45);
  --shadow-glow: 0 0 0 4px var(--primary-glow);
}
```

### 9.2.4 Typography

```css
:root {
  --font-display: "Inter", "Pretendard Variable", system-ui, sans-serif;
  --font-body:    "Inter", "Pretendard Variable", system-ui, sans-serif;
  --font-mono:    "JetBrains Mono", "D2Coding", "Naver D2Coding", ui-monospace, monospace;

  --fs-12: 12px;
  --fs-13: 13px;
  --fs-14: 14px;
  --fs-15: 15px;
  --fs-16: 16px;
  --fs-18: 18px;
  --fs-22: 22px;
  --fs-28: 28px;

  --lh-tight:  1.2;
  --lh-normal: 1.45;
  --lh-loose:  1.6;

  --weight-regular: 400;
  --weight-medium:  500;
  --weight-semi:    600;
}
```

숫자는 항상 mono + `font-variant-numeric: tabular-nums;` (유틸 클래스 `.num`).

### 9.2.5 Motion

```css
:root {
  --ease-standard: cubic-bezier(0.2, 0.8, 0.2, 1);
  --ease-in:       cubic-bezier(0.4, 0, 1, 1);
  --ease-out:      cubic-bezier(0, 0, 0.2, 1);

  --dur-fast:   120ms;
  --dur-base:   180ms;
  --dur-slow:   280ms;
}
```

micro transition: 모든 인터랙티브 요소에 `transition: color, background, border-color, box-shadow var(--dur-fast) var(--ease-standard);` 기본.

### 9.2.6 Z-index

```css
:root {
  --z-base:    0;
  --z-sticky:  100;
  --z-overlay: 1000;
  --z-modal:   1100;
  --z-toast:   1200;
  --z-palette: 1300;
}
```

## 9.3 공유 컴포넌트 (`packages/design-system/react/`)

v1에서 반드시 제공:
- `<AppShell>` — 사이드바 + topbar + content 슬롯.
- `<Sidebar>`, `<SidebarItem>`, `<SidebarGroup>`.
- `<Topbar>`, `<TopbarStatus>`.
- `<CommandPalette>` (⌘K).
- `<Card>`, `<Panel>`, `<Stat>` (mono 숫자).
- `<Button>` (variants: primary/secondary/ghost/danger).
- `<IconButton>`, `<TextField>`, `<Select>`, `<Toggle>`, `<Checkbox>`, `<Radio>`.
- `<Badge>`, `<Tag>`, `<HealthDot>`.
- `<Toast>`, `<ConfirmDialog>`, `<LoadingOverlay>`.
- `<Tabs>`, `<Accordion>`, `<Tooltip>`.
- `<ProgressBar>` (mono 진행률 숫자), `<Stepper>`.
- `<EmptyState>`, `<ErrorState>`.
- `<KBD>` (⌘ K 같은 키 표시).

각 컴포넌트는:
- 토큰만 사용(하드코딩 hex 금지).
- 키보드 접근성 기본.
- 한국어 라벨 wrap에 안전한 max-width/줄바꿈.

## 9.4 voice.md (한국어 voice & tone)

`packages/design-system/voice.md`:

요약:
- 친절하지만 실무형. 마케팅 톤 금지.
- 한 문장 한 정보. 긴 문장은 분리.
- 사용자에게 행동 유도 시 동사로 시작 — "설치하기", "다시 시도", "로그 보기".
- 부정어보다 긍정어 우선. ("실패했어요" → 사실 + 다음 액션 함께)
- 외래어는 한국어 보편 용어 우선("엔드포인트"는 그대로, "런타임"도 그대로 — 한국어 개발자 보편).
- 숫자는 항상 mono(코드/UI 양쪽).
- 영문 약어는 풀어 쓰지 않아도 됨(API, GPU, RAM 등) — 다만 첫 등장 시 짧은 한국어 보조.

## 9.5 도입 단계

| 단계 | 시점 | 산출물 |
|---|---|---|
| **D1** | M0 | tokens.css/ts + base.css + voice.md 초안 + Button/Card/AppShell |
| **D2** | M1 | Stat/HealthDot/Badge/Sidebar 완성, 홈 화면 토큰 100% |
| **D3** | M2 | Toast/Modal/ProgressBar/Stepper, 설치 센터 적용 |
| **D4** | M3 | CommandPalette, EmptyState/ErrorState |
| **D5** | M4 | 9개 화면 100% 토큰 적용, visual regression 베이스라인 |
| **D6** | M5~M6 | 기존 웹앱에 npm publish 후 PR로 통합 |

## 9.6 기존 웹앱 통합 절차

1. 기존 웹앱이 `@lmmaster/design-system`을 dependency로 추가.
2. 기존 웹앱의 글로벌 CSS에서 `@lmmaster/design-system/tokens.css`를 import.
3. 기존 웹앱의 자체 색/폰트/spacing 토큰을 점진적으로 디자인 시스템 토큰으로 교체.
4. provider abstraction에 `LocalCompanionProvider` 추가(설계상 별도 PR).
5. 부팅 시 `sdk.pingHealth()` + 미연결 모달.

전환은 한 번에 강제하지 않는다. 토큰부터 시작 → 컴포넌트 점진 교체.

## 9.7 검증

- 시각 회귀: Playwright + storybook 스냅샷.
- 토큰 미준수 lint: stylelint 규칙으로 하드코딩 hex 금지.
- 클래스명 lint: `ad-/adv-/ads-/banner-/sponsor-` 금지(grep CI).
- 키보드 접근성: axe-core 통합.

## 9.8 기존 웹앱과의 차이 허용 범위

- **공유**: 토큰, 기본 컴포넌트, voice & tone, 키보드 인터랙션.
- **차별**: 데스크톱 셸(사이드바 폭, OS-native chrome 통합 등)은 데스크톱 전용.
- **금지**: 어느 한쪽에서만 사용하는 색을 토큰화하지 않고 hex로 하드코딩하는 것.
