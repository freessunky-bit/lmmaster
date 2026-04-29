# Phase 1A.4.a — React 첫실행 마법사 골격 결정 노트

> 보강 리서치 (2026-04-26) 종합. xstate v5 + @ark-ui/react Steps + react-error-boundary + 해요체.

## 1. 핵심 결정

| 항목 | 결정 | 근거 |
|---|---|---|
| 상태 기계 | `xstate v5` `setup({...}).createMachine()` + `createActorContext()` | v5 정식 API, TS 추론 강화. context provider 패턴이 step 간 re-render 격리 |
| 컴포넌트 hook | parent provider + `useSelector` slice 읽기 + `useActorRef().send` 쓰기 | `useMachine`은 전체 트리 re-render |
| 비동기 actor | `fromPromise` + `signal.abort()` → `cancelInstall(id)` | 머신 종료 시 자동 cancel |
| 영속화 | xstate snapshot ↔ `localStorage["lmmaster.onboarding.v1"]` 동기 hydrate | zustand persist는 race 위험 + 진실원 중복 |
| 영속화 제외 | `install.running` 서브상태는 저장 안 함 — 충돌/재시작 시 idle로 깨어남 | 절반-끝난 install 재개 위험 회피 |
| 완료 플래그 | 별도 key `lmmaster.onboarding.completed = "1"` | 미래 마이그레이션 시 onboarding 재트리거 안 되게 |
| Steps UI | `@ark-ui/react` `Steps` v5.36+ | 헤드리스 + ARIA + 키보드 무료. 컨트롤드(`step={machine.value}`)로 xstate를 진실원으로 |
| 에러 경계 | `react-error-boundary` v4 — **per-step** + `resetKeys={[snapshot.value]}` | Step 2 실패가 Step 1을 죽이지 않게. 머신 transition 시 자동 reset |
| 비동기 에러 → boundary | `useErrorBoundary().showBoundary(error)` (callback/event handler) | render 외 에러는 자동 catch 안 됨 |
| 머신 vs 경계 분기 | 알려진 IPC 실패 → 머신 `onError` transition. 예측 못한 throw → boundary | clean split, 둘 다 같은 fallback 컴포넌트 노출 |
| 레이아웃 | full-page (App body 통째 교체) — 모달 안 함 | 마법사는 dismissible 아니어야 함 |
| 모션 | `framer-motion` `<AnimatePresence mode="wait">` 200ms transform+opacity, `<MotionConfig reducedMotion="user">` | Linear/Raycast 스타일. CSS만으로는 enter/exit 동시 처리 어려움 |
| 한국어 voice | 해요체 (Toss 8원칙) — 계속할게요 / 이전으로 / 나중에 할게요 / 닫기 / 문제가 생겼어요 다시 시도해 볼까요? | toss.tech 공식 |
| 로안워드 | 런타임 / 모델 / GPU 가속 — 그대로 사용. 첫 등장 시 한 번 풀어쓰기 ("로컬 모델(인공지능 두뇌)") | 한국 개발 커뮤니티 관행 |
| 테스트 | vitest + `@tauri-apps/api/mocks` `mockIPC` + 모듈-레벨 `vi.mock('./ipc/install')` 둘 다 가능 — 모듈 mock 우선 | 본 sub-phase에서는 vitest 도입은 후순위 (1A.4.d) |

## 2. 의존성 (apps/desktop/package.json 추가)

```json
"xstate": "^5.19.0",
"@xstate/react": "^5.0.0",
"@ark-ui/react": "^5.36.2",
"react-error-boundary": "^4.1.2",
"framer-motion": "^11.11.0"
```

## 3. 머신 상태도 (1A.4.a scope)

```
language --SET_LANG--> language (assign)
         --NEXT-----> scan
scan     --BACK-----> language
         --NEXT-----> install
install  --BACK-----> scan
         --SKIP-----> done
         --SELECT_MODEL-> install.running
   running --invoke install actor: onDone -> done, onError -> install.idle (assign error)
done (final)
```

1A.4.a에선 Step 2/3은 placeholder UI ("다음 sub-phase에서 만나요"). 머신은 4-state 전체를 갖되, `install.running` invoke의 src는 `'install'` actor (실제 호출은 1A.4.c).

## 4. Step별 i18n 키

```
onboarding.steps.language: "언어"
onboarding.steps.scan: "환경 점검"
onboarding.steps.install: "첫 모델"
onboarding.steps.done: "준비 완료"

onboarding.language.title: "언어를 선택해 주세요"
onboarding.language.subtitle: "언제든 설정에서 바꿀 수 있어요"
onboarding.language.option.ko: "한국어"
onboarding.language.option.en: "English"

onboarding.scan.placeholder: "환경 점검 단계는 다음 업데이트에서 만나요"
onboarding.install.placeholder: "첫 모델 설치 단계는 다음 업데이트에서 만나요"

onboarding.done.title: "준비됐어요"
onboarding.done.subtitle: "이제 LMmaster를 마음껏 써보세요"
onboarding.done.cta: "시작할게요"

onboarding.actions.next: "계속할게요"
onboarding.actions.back: "이전으로"
onboarding.actions.skip: "나중에 할게요"

onboarding.error.title: "문제가 생겼어요"
onboarding.error.body: "다시 시도해 볼까요?"
onboarding.error.retry: "다시 시도"
onboarding.error.close: "닫기"

onboarding.progress.aria: "{{current}}단계 중 {{step}}단계"
```

## 5. CSS 토큰 활용 (디자인 시스템 조작)

- 카드: `var(--surface)` + `var(--border)` + `var(--radius-3)`.
- 활성 step dot: `var(--primary)` + `var(--shadow-glow)` (focus 시).
- 텍스트 hierarchy: `--text` (body) / `--text-secondary` (subtitle) / `--text-muted` (caption).
- spacing: 4px grid (`--space-*`).
- typography: 제목 `--fs-22` semi-bold + 본문 `--fs-15` regular + 캡션 `--fs-13` muted.
- 모션: `--dur-base` (180ms) + `--ease-emphasized` (cubic-bezier(0.16, 1, 0.3, 1)).

## 6. 파일 트리 (1A.4.a 산출물)

```
apps/desktop/src/onboarding/
├── machine.ts           # xstate machine + setup
├── context.tsx          # createActorContext + Provider with hydration
├── persistence.ts       # localStorage save/load helpers + completed flag
├── OnboardingApp.tsx    # 마법사 root — Steps + AnimatePresence + ErrorBoundary
├── StepErrorFallback.tsx # 한국어 에러 fallback
├── onboarding.css       # 마법사 전용 토큰 활용 스타일
└── steps/
    ├── Step1Language.tsx
    ├── Step2Scan.tsx       # placeholder ("다음 업데이트")
    ├── Step3Install.tsx    # placeholder ("다음 업데이트")
    └── Step4Done.tsx
```

## 7. App.tsx 게이팅

```tsx
const completed = isOnboardingCompleted();
return completed ? <MainShell /> : <OnboardingApp onComplete={() => markCompleted()} />;
```

`MainShell`은 기존 `App.tsx` 본문을 추출. `OnboardingApp`은 머신이 `done` 상태에 도달하면 `onComplete()` 호출 → flag set → re-render → MainShell.

## 8. 비목표 (1A.4.a 외)

- Step 2 실제 환경 점검 (hardware-probe + runtime-detector IPC) — **1A.4.b**
- Step 3 실제 첫 모델 설치 (installApp 호출 + InstallProgress 컴포넌트) — **1A.4.c**
- vitest + @testing-library/react 통합 + 머신/컴포넌트 테스트 — **1A.4.d**
- Storybook 개별 step preview — 후순위
- WCAG 2.2 axe-core 자동 검증 — 1A.4.d
- 첫 실행 자동 감지 (LM Studio가 이미 설치돼 있으면 onboarding skip) — 1A.4.b의 scan 결과로 판단

## 9. 참고 구현체

- [xstate v5 docs (persistence/promise-actors/migration)](https://stately.ai/docs/persistence)
- [Ark UI Steps](https://ark-ui.com/react/docs/components/steps)
- [react-error-boundary](https://github.com/bvaughn/react-error-boundary)
- [Toss 8원칙 (한국어 UX writing)](https://toss.tech/article/8-writing-principles-of-toss)
- [Framer Motion react-transitions](https://motion.dev/docs/react-transitions)
- [Tauri 2 mocking](https://v2.tauri.app/develop/tests/mocking/)
