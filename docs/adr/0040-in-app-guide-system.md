# ADR-0040: In-app guide system — NAV "가이드" + 8 섹션 + ? 도움말 + F1 단축키 + first-run toast

- Status: Accepted
- Date: 2026-04-29
- Phase: 12'

## Context

LMmaster 첫 사용자가 6 pillar 기능(자동 설치 + 한국어 + 포터블 + 큐레이션 + 워크벤치 + RAG)을 발견하지 못하면 가치 전달이 무너져요. v1 직전 검사에서 다음 사용자 친화 부재가 확인됐어요.

- NAV에 가이드 / 도움말 메뉴 없음 — 사용자가 어디서 시작할지 모름.
- 페이지 헤더에 contextual help 없음 — 워크벤치 5단계, RAG ingest, API 키 scope 등 발견 비용 높음.
- 단축키 안내 모달 없음 — `⌘K`(Phase 1A.4.e)만 사용자가 우연히 발견.
- 첫 실행 후 안내 없음 — 마법사 직후 곧바로 빈 홈으로 이동.

해당 갭을 메우는 동시에 **외부 통신 0** 원칙(ADR-0013) + **design-system 일관성**(ADR-0011) + **한국어 우선**(ADR-0010)을 모두 유지해야 해요.

## Decision

**1) NAV "가이드" 메뉴 + Guide page (12'.a)**

- `App.tsx::NAV_KEYS`에 `guide` 추가 (settings 위).
- `apps/desktop/src/pages/Guide.tsx` 신설:
  - **8 섹션** — `getting-started` / `catalog` / `workbench` / `knowledge` / `api-keys` / `portable` / `diagnostics` / `faq`.
  - 좌측 sidebar: 섹션 목록 + 검색 input (substring + 한국어 jamo cheat).
  - 우측 본문: 마크다운 렌더 + "이 기능 사용해 볼게요" CTA → `lmmaster:navigate` dispatch.
  - **deep link**: `?section=workbench` URL hash + `lmmaster:guide:open` custom event 양쪽 지원.

**2) 마크다운 콘텐츠 + 공유 renderer**

- `apps/desktop/src/i18n/guide-{ko,en}-v1.md` — 8 섹션 마크다운 (한국어 해요체 + 영어 casual).
- 섹션 분리는 `\n---\n` + `<!-- section: id -->` 마커.
- `apps/desktop/src/components/_render-markdown.ts` — EulaGate에서 추출한 minimal renderer를 공유 모듈로.
- 외부 의존성 추가 없음 (react-markdown 도입 X).

**3) HelpButton (12'.b)**

- `apps/desktop/src/components/HelpButton.tsx`: ? 트리거 버튼 + popover.
- popover: focus trap + Esc + role=dialog + aria-modal=true + 외부 클릭 닫기.
- "전체 가이드 보기" 버튼이 NAV → `guide` + `lmmaster:guide:open` 양쪽 dispatch.
- 5 페이지 헤더에 통합: Workspace / Workbench / Catalog / ApiKeysPanel / Settings(포터블 섹션).

**4) ShortcutsModal + 글로벌 hotkey (12'.c)**

- `apps/desktop/src/components/ShortcutsModal.tsx`: F1 / Shift+? 글로벌 hotkey로 표시.
- 표 형식 — Ctrl+K / F1 / Ctrl+1~9 / Esc + 13행. mac은 `⌘` 자동 표기.
- input/textarea/contenteditable focus 시 글로벌 hotkey 자동 비활성 (사용자 타이핑과 충돌 회피).
- `useShortcutsHotkey` hook을 App.tsx에서 마운트 — Ctrl+1~9 NAV 이동 콜백.

**5) TourWelcomeToast (12'.c)**

- `apps/desktop/src/components/TourWelcomeToast.tsx`: 마법사 끝나면 우측 하단 toast.
- "지금 볼게요" → Guide page `getting-started` 진입.
- "다음에 할게요" → `localStorage["lmmaster.tour.skipped"] = "true"`.
- `localStorage["lmmaster.tour.shown"] = "true"` 1회 영속 — 본 적 있으면 안 띄움.
- framer-motion slide-in (이미 의존성 있음). prefers-reduced-motion 자동 비활성.

**6) i18n (ko/en 동시)**

- `screens.guide.*`, `screens.help.*`, `screens.shortcuts.*`, `screens.tour.*` 신규.
- `nav.guide` 추가.

## Consequences

### Positive

- 첫 사용자 진입 성공률 상승 — 마법사 직후 toast가 가이드 진입을 유도.
- 페이지별 contextual help — 발견 비용 감소.
- 단축키 표 — 파워 사용자가 명령 팔레트와 NAV 단축키를 빠르게 학습.
- 외부 통신 0 원칙 유지 — markdown은 빌드 타임 번들, 검색은 클라이언트.
- design-system 일관성 — 모든 신규 UI가 토큰만 사용.

### Negative

- guide 콘텐츠 유지보수 — 6 pillar 기능 변화 시 ko/en 동시 갱신 필요.
- 텍스트 위주 — 스크린샷 / 동영상은 v1.x로 보류 (캡처 자동화 + 다국어 캡션 부담).

### Risk mitigations

- 한국어 카피 톤 위반 — CLAUDE.md §4.1 기준으로 lint 가능한 단위 (검토 시 grep 가능).
- Markdown 렌더러 XSS — 입력 escape + 마커는 우리가 작성한 파일에만 사용. user-input 처리 X.
- F1 / Ctrl 단축키 충돌 — input/textarea focus 시 비활성 + browser 기본 행동은 preventDefault.

## Alternatives considered

### 1) Shepherd.js / Intro.js — **기각**

- 의존성 +1 (~30 KB minified).
- design-system token과 별도 스타일 — 일관성 깨짐.
- 한국어 i18n 통합 비용.
- 자체 구현이 maintainability 우월 (커스텀 popover 100줄 미만).

### 2) 외부 docs 사이트 (GitBook / Notion / Vercel) — **기각**

- 외부 통신 0 원칙(ADR-0013) 위반.
- 오프라인 사용 불가 — LMmaster의 핵심 약속 위반.
- 사용자가 LMmaster 안에서 이탈 — 가이드 흐름 끊김.

### 3) Tooltip만 + NAV 메뉴 X — **기각**

- 진입 성공률 낮음 — 사용자가 ? 아이콘을 누르지 않으면 가이드 미발견.
- 8 섹션 분량의 콘텐츠가 tooltip 안에 들어가지 않음.
- Linear / Notion / Stripe Dashboard / VSCode Walkthroughs 모두 별도 가이드 page 채택.

### 4) react-markdown — **기각**

- 의존성 +1 (~50 KB).
- Phase 7'.a EulaGate에서 minimal renderer로 충분 검증 — 동일 패턴 재활용.
- markdown 작성자가 우리 자신이라 신뢰 가능 (user-input 아님).

### 5) Guide page 안에 동영상 / 스크린샷 — **v1.x로 이월**

- 캡처 자동화 + ko/en 다국어 캡션 + 화면 변경 시 재캡처 부담.
- v1은 텍스트 + 단계 list 위주로 시작 — 사용자 피드백에 따라 v1.x에서 추가.

## References

- 본 ADR의 상세 설계: `docs/research/phase-8p-9p-10p-residual-plan.md` §1.9.
- Linear Onboarding (in-app guide step pattern): https://linear.app
- Notion Help Center (sidebar + section nav): https://notion.so/help
- Stripe Dashboard contextual help (? icon popover): https://stripe.com/docs
- VSCode Walkthroughs (first-run tour): https://code.visualstudio.com/api/extension-guides/walkthroughs
- Obsidian Help (offline-first markdown guide): https://help.obsidian.md
- ADR-0010: Korean-first principle.
- ADR-0011: Shared design system.
- ADR-0013: External-comm-zero policy.
