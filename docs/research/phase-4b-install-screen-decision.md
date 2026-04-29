# Phase 4.b — install 화면 (마법사 InstallProgress 메인 통합) 결정 노트

> 작성일: 2026-04-27
> 상태: 확정 (초안 → 즉시 구현)
> 선행: Phase 4-screens-decision §1.1 install, Phase 1A.4.c (마법사 Step3Install + InstallProgress 컴포넌트), Phase 1A.4.b (detect_environment IPC), Phase 4.a (StatusPill 추출)
> 후행: Phase 4.c (runtimes 화면 — 같은 카드 패턴 + 모델 목록 추가), Phase 4.f (diagnostics — install history)

---

## 0. 결정 요약 (5가지)

1. **카드 2개 (Ollama + LM Studio) + 우측 drawer + InstallProgress compact + 빈 상태 4 컴포넌트만** — 마법사가 정착한 같은 골조를 메인 화면용으로 재배치, drawer는 카탈로그와 동일 패턴 유지.
2. **`InstallProgress`에 `compact?: boolean` prop 추가 (default false)** — 마법사 모드(전체 화면 onb-step 헤더 + onCancel 버튼)와 메인 inline 모드(40px 단일 라인 + progress bar 4px) 구분. 기존 마법사 사용처 깨짐 0.
3. **상태 매핑 = `running` / `installed` 양쪽 모두 "준비됐어요" 처리** — runtime-detector는 두 상태를 구분하지만 사용자에겐 모두 ready로 노출 (Re-install / Open folder 액션 동일).
4. **합산 StatusPill** = `running 1+ → listening` / `installed 1+ → stopping(준비됐어요)` / 모두 unknown → booting / 그 외 idle. Tailscale 패턴.
5. **OpenedUrl outcome (LM Studio 등)도 InstallProgress가 finished 이벤트로 처리** — 마법사처럼 별도 패널 안 만들고, 진행 패널의 close 버튼으로 정리. 메인 화면은 마법사보다 단순화.

---

## 1. 채택안

### 1.1 IA — single-pane

```
<topbar: "런타임 설치" + 합산 StatusPill (booting/listening/stopping/idle)>
<카드 그리드 — 2 카드 (Ollama / LM Studio)>
  카드: <header(name + license badge)> <status row(StatusPill + announcement)> <body(reason 1줄)> <footer(받을게요 / 자세히 2 액션)>
<진행 패널 — InstallProgress compact 모드>
  active.id == 카드 → 패널 노출, 종료 후 close 버튼으로 dismiss.
<빈 상태>
  둘 다 준비됨 + 진행 패널 없으면 카탈로그 이동 CTA.
<우측 drawer (선택 시)>
  manifest detail: license / install size / homepage.
```

차용한 글로벌 사례:
- **Tailscale macOS** — 합산 StatusPill 패턴.
- **Linear settings** — 카드 + drawer 슬라이드 패턴 (카탈로그가 이미 정착한 패턴 재사용).
- **VS Code Extensions** — 카드 footer "Install / Re-install / Details" 액션 매트릭스.

트레이드오프: drawer는 manifest detail이 짧아 modal로 충분하지만, 카탈로그와 패턴 분리하면 사용자 학습 비용 증가 — drawer 채택.

### 1.2 InstallProgress compact 모드

- prop 시그니처: `compact?: boolean` (default false). optional이므로 기존 마법사 호출부 깨짐 0.
- compact=true 시 `<div className="onb-install-compact">`로 렌더 — 단일 라인 row(40px) + 작은 progress bar(4px).
- 헤더 / 자세히 보기 details / 큰 onCancel 버튼은 hidden — 단일 라인 row 안에 title + phase + cancel 버튼만.
- ProgressBar 자체에도 compact prop 흘려서 `.onb-install-progress.is-compact` 변종 적용.

### 1.3 IPC 통합

- `detectEnvironment()` 1회 마운트 + 설치 종료 후 1회 재호출 — status 갱신.
- `installApp(id, { onEvent })` 호출 → onEvent로 받은 InstallEvent를 `applyEvent` reducer로 누적, 마지막 10건만 log에 보존.
- `cancelInstall(id)`는 cancel 버튼 클릭 시 호출. terminal 이벤트 도달 후 close 버튼으로 dismiss.

---

## 2. 기각안 + 이유 (Negative space — 의무 섹션)

### 2.1 단일 카드 list로 줄이는 안

- **시도 / 검토**: 런타임이 2개뿐이라 그리드 대신 stacked list (LM Studio 위, Ollama 아래) 구성 검토.
- **거부 이유**: (a) 카드 2개가 명확한 비교(MIT vs EULA, 자동 vs 수동) 제공 — 사용자가 한 눈에 트레이드오프 파악. (b) Phase 6'에서 vLLM / KoboldCpp 추가 시 그리드가 자연스럽게 확장. list로 시작하면 추가 시 layout shift.
- **재검토 트리거**: 런타임이 4개 이상으로 늘면 카드 그리드가 과해질 수 있음 — 그때 카드 + sidebar 카테고리 split 검토.

### 2.2 drawer 대신 modal

- **시도 / 검토**: manifest detail이 짧으니 modal (center)로 띄우고 backdrop click 닫기.
- **거부 이유**: 카탈로그 화면이 이미 drawer 패턴으로 manifest detail을 노출 — 같은 "더 자세히" 의도를 modal과 drawer로 분기하면 사용자 인지 비용. 한 패턴 유지.
- **재검토 트리거**: drawer가 너무 비어 보이는 케이스 (LM Studio처럼 manifest 정보가 1줄)면 v1.1에서 inline expansion 검토.

### 2.3 drag-and-drop GGUF 모델 인입

- **시도 / 검토**: install 화면에 "여기에 GGUF 파일을 끌어다 놓으세요" 영역.
- **거부 이유**: Phase 5' 워크벤치 영역. install 화면은 *런타임* 설치만 담당 — 모델은 Phase 2' 카탈로그가 책임. 두 영역 혼합 시 사용자가 "왜 같은 화면에서 둘 다 안 되지?" 혼란.
- **재검토 트리거**: Phase 5' 워크벤치 본격 출시 후 — 그때도 워크벤치에서만 처리.

### 2.4 OpenedUrl outcome 별도 패널

- **시도 / 검토**: 마법사처럼 LM Studio가 외부 사이트로 열리면 "공식 사이트가 열렸어요" 별도 패널.
- **거부 이유**: 메인 화면은 마법사보다 컨텍스트가 풍부 (사용자가 이미 앱에 익숙). InstallProgress 의 finished 이벤트로 처리 후 close 버튼이면 충분. 별도 패널은 화면 전환 비용만 추가.
- **재검토 트리거**: 사용자 피드백에서 "LM Studio 설치가 끝났는지 모르겠어요" 다발 시.

---

## 3. 미정 / 후순위 이월

| 항목 | 이월 사유 | 진입 조건 / 페이즈 |
|---|---|---|
| "폴더 열기" 액션 | Tauri shell open command 필요 — 별도 IPC 추가 | Phase 4.f diagnostics에서 함께 처리 (워크스페이스 폴더 열기와 같은 감각) |
| 합산 StatusPill 라벨 세분화 | 현재 4 상태 매핑 단순 — 사용자 피드백 기반 | Phase 6' 자가 관찰 |
| GGUF drag-and-drop | Phase 5' 워크벤치 영역 | Phase 5' |
| Re-install 시 confirm 모달 | 현재 즉시 install 호출. 사용자가 실수로 1GB 재다운로드할 위험 | Phase 4.h polish |

---

## 4. 테스트 invariant

- **빈 상태**: 두 런타임 모두 running일 때 빈 상태 + 카탈로그 CTA 노출. 한 쪽이라도 not-installed면 빈 상태 hidden.
- **i18n**: ko/en 양쪽에 `screens.install.*` 14 키 모두 1:1 미러. fallback 누락 0.
- **Drawer**: 카드 클릭 → role="dialog" 노출, Esc로 닫힘.
- **InstallProgress compact**: 진행 중일 때 컴팩트 패널 + cancel 버튼 노출, cancel 클릭 시 cancelInstall(id) 호출.
- **a11y**: vitest-axe `violations.toEqual([])` (color-contrast / html-has-lang / region 비활성).
- **시그니처 보존**: 기존 InstallProgressProps 기본 호출(`<InstallProgress title=... data=... onCancel=... />`)은 compact 미전달이라도 기존 마법사 모드 유지.

---

## 5. 다음 페이즈 인계

- **선행 의존성**: Phase 4.a StatusPill (`@lmmaster/design-system/react`에서 이미 export). Phase 1A.4.c InstallProgress (compact prop 추가).
- **이 페이즈 산출물**:
  - `apps/desktop/src/pages/Install.tsx` (페이지)
  - `apps/desktop/src/pages/install.css` (스타일 + InstallProgress compact 변종)
  - `apps/desktop/src/pages/Install.test.tsx` (vitest jsdom — 6 케이스)
  - `apps/desktop/src/components/InstallProgress.tsx` (compact prop 추가, default false)
  - `apps/desktop/src/i18n/{ko,en}.json` (`screens.install.*` 14 키)
- **다음 sub-phase 진입 조건**: App.tsx 라우팅에 `<InstallPage onNavigate={setActiveNav} />`를 활성 nav가 install일 때 마운트. (메인 agent 책임 — 이 sub-phase 산출물 아님.)
- **위험 노트**:
  - Tauri runtime이 마운트 직후라면 `detectEnvironment()`가 잠깐 실패할 수 있음 — error 토스트는 v1에서 console.warn만, UI 상에서는 카드만 not-installed로 노출.
  - InstallProgress compact CSS가 install.css에 살고 있어 다른 화면이 InstallProgress(compact=true)를 쓰려면 install.css import 필요. 현재 다른 화면은 사용 안 함.

---

## 6. 참고

- 결정 노트 `docs/research/phase-4-screens-decision.md` §1.1 install (상위 IA + 컴포넌트 시그니처).
- ADR-0006 디자인 시스템 (StatusPill / Drawer 패턴).
- ADR-0017 런타임 manifest (Ollama / LM Studio metadata).
- 메모리: 갱신 항목 없음 — 본 결정은 결정 노트에서만 보존.
