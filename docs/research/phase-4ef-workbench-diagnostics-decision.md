# Phase 4.e + 4.f — Workbench placeholder + Diagnostics 4-grid 결정 노트

> 작성일: 2026-04-27
> 상태: sub-phase 구현 완료 — 보강 리서치 + 산출 + 테스트 동시 진행
> 선행: Phase 4 결정 노트(`phase-4-screens-decision.md`) §1.1 workbench / diagnostics, Phase 2'.c 30초 벤치, Phase 3' 게이트웨이 + 워크스페이스 fingerprint
> 후행: Phase 5' 워크벤치 v1 (현재 placeholder를 본 페이지로 승격), Phase 4.e/.f 통합 시 App.tsx에 nav 분기 + custom event listen 추가

---

## 0. 요약

- **Workbench (4.e)**: "곧 만나요" placeholder + 5단계 미리보기 (모두 disabled) + "준비되면 알려드릴게요" 토글 (localStorage 영속). hero illustration은 inline SVG + CSS gradient (외부 자원 0).
- **Diagnostics (4.f)**: 4-section grid 2x2 (자가스캔 / 게이트웨이 / 벤치 / 워크스페이스). 종합 health = 4 섹션 tier deterministic 합산 (red > yellow > green). bench / latency / 활성 키 / repair history는 mock 데이터 (v1.x에서 IPC로 교체).

---

## 1. 채택안

### 1.1 Workbench placeholder

- **단일 hero + 5단계 grid + 알림 토글** 구성. 부분 노출(예: 양자화만 풀어놓기) 거부 — 5단계 통합이 Phase 5'의 가치라 일부만 노출 시 사용자 신뢰 하락.
- **hero illustration**: anvil + spark + 5-step bar 시각 단서를 inline SVG로. role="img" + aria-label로 textual fallback. 외부 자원 0 정책 준수.
- **알림 토글**: `role="switch"` + `aria-checked` + localStorage `lmmaster.workbench.notify_on_release: "true"` 저장. Phase 5' 출시 toast가 이 키를 read해서 자동 알림 트리거.
- **5단계 카드**: `aria-disabled="true"` + lock glyph + hover 시 `screens.workbench.comingSoon` tooltip. 카드가 button이 아니라 `<div>` — disabled-button focus 충돌 회피.

### 1.2 Diagnostics 4-grid

- **2x2 grid 단일 페이지**. modal로 띄우는 안 거부 — diagnostics는 "한 번에 점검"이 가치라 페이지로 영구 접근 필요.
- **종합 health = 4 tier 합산 (deterministic)**.
  - 좌상 자가스캔: severity error → red, warn → yellow, 그 외 green. scan null → yellow.
  - 우상 게이트웨이: status failed → red, booting/stopping → yellow, listening → green.
  - 좌하 벤치: 항상 green (mock — v1.x에서 실 데이터 도착 시 평가 추가).
  - 우하 워크스페이스: `WorkspaceStatus.tier`를 그대로.
  - 합산: red > yellow > green priority. role="status" aria-live="polite".
  - 서버 측 계산 거부 — UI에 들어온 IPC 결과로 충분.
- **mock 데이터**: gateway latency sparkline (30 sample), 활성 키 카운트(3), 최근 5 요청 log, 벤치 막대(5 모델 token/sec), repair history 2 row. 모두 컴포넌트 내부 상수 — `// MOCK 마커`로 v1.x 교체 영역 명시.
- **차트 a11y**: sparkline `role="img"` + 평균/최대 textual aria-label. 막대 차트도 `role="img"` + 모델별 tps textual list aria-label.
- **새 측정 시작 버튼**: `window.dispatchEvent(new CustomEvent("lmmaster:navigate", { detail: "catalog" }))` — App.tsx가 통합 시 listen해서 nav 전환. 통합 전엔 자체 noop이지만 테스트 가능한 행위.

---

## 2. 기각안 (의무 — 결정 사유 명시)

### 2.1 Workbench 일부 기능 미리 노출 (양자화만)

- **거부 사유**: 5단계 플로우의 가치는 "데이터 → 양자화 → LoRA → 검증 → 등록" 통합. 양자화만 풀어내면 사용자가 "이게 워크벤치의 전부"라고 오해할 위험. Phase 5' 출시 시 통합된 5단계가 더 큰 wow-factor를 줘야 함. 현재 placeholder가 그 기대를 정확히 빌드함.

### 2.2 Diagnostics를 sidebar nav에서 분리 → modal로 띄우기

- **거부 사유**: diagnostics는 사용자가 "지금 어떤 상태인지"를 자주 들여다볼 페이지. modal은 영구 접근성이 떨어지고 사이드바 카운트 표시가 어색함. 또한 sub-section(scan / gateway / bench / workspace)별로 깊이 있는 정보를 수반해 modal 스크린 부동산이 부족.

### 2.3 종합 health score를 server(Rust) 측에서 계산

- **거부 사유**: 4 섹션 tier를 client에서 합산하는 게 충분. 새 metric 추가 시 client 변경만으로 충분하고, server-side calculation 추가 시 IPC 명세 + serde + 테스트가 더 늘어남. deterministic 합산이라 client에서 결정해도 결정성이 동일.

---

## 3. 검증 결과

(이번 sub-phase에서 추가 검증):

- `pnpm exec tsc -b` apps/desktop — 0 errors.
- `pnpm exec vitest run src/pages/Workbench.test.tsx src/pages/Diagnostics.test.tsx` — Workbench 6 + Diagnostics 8 = 총 14 tests pass.

---

## 4. mock 데이터 → 실 데이터 교체 위치 (v1.x)

`apps/desktop/src/pages/Diagnostics.tsx` 안의 `// MOCK` 마커:

| mock 변수 | 교체 IPC | 비고 |
|---|---|---|
| `MOCK_GATEWAY_LATENCY_MS` | gateway latency rolling buffer IPC (Phase 6'+ 신설) | 60초 window 30 sample |
| `MOCK_ACTIVE_KEY_COUNT` | `listApiKeys()` filter `revoked_at == null` length | 즉시 교체 가능 (이미 IPC 있음) |
| `MOCK_RECENT_REQUESTS` | gateway access log SQLite IPC (Phase 6'+ 신설) | 최근 5 row |
| `MOCK_BENCH_ENTRIES` | `getLastBenchReport(...)` batch (모델 5개 순회) | tg_tps 그대로 |
| `MOCK_REPAIR_HISTORY` | workspace repair history IPC (신설 필요) | tier + invalidated cache count |

---

## 5. App.tsx 통합 가이드 (메인 작업자 인계)

1. import 추가:
   ```tsx
   import { Workbench } from "./pages/Workbench";
   import { Diagnostics } from "./pages/Diagnostics";
   ```
2. `MainShell` 분기 라우팅에 케이스 추가:
   ```tsx
   activeNav === "workbench" ? <Workbench /> :
   activeNav === "diagnostics" ? <Diagnostics /> :
   /* 기존 분기 */
   ```
3. (선택) `lmmaster:navigate` custom event listen 추가 — Diagnostics의 "새 측정 시작" 버튼이 catalog로 점프:
   ```tsx
   useEffect(() => {
     const onNav = (e: Event) => {
       const detail = (e as CustomEvent).detail;
       if (typeof detail === "string" && (NAV_KEYS as readonly string[]).includes(detail)) {
         setActiveNav(detail as NavKey);
       }
     };
     window.addEventListener("lmmaster:navigate", onNav);
     return () => window.removeEventListener("lmmaster:navigate", onNav);
   }, []);
   ```

---

## 6. 다음 sub-phase 진입 조건

- [x] Workbench placeholder + 알림 토글 + a11y 통과
- [x] Diagnostics 4-grid + IPC 연결 + custom event nav + a11y 통과
- [x] i18n ko/en `screens.workbench.*`, `screens.diagnostics.*` 추가
- [x] 결정 노트 작성 + 기각안 의무 명시
- [ ] App.tsx 통합 (이번 sub-phase 책임 외 — 메인 작업자가 §5 가이드대로 적용)

---

**다음 작업**: Phase 4.g (settings 화면) 또는 Phase 4.h (Korean preset 100+) — 메인 작업자 신호 대기.
