# Phase 4.c — runtimes 화면 결정 노트

> 작성일: 2026-04-27
> 상태: 보강 리서치 완료 → 설계 확정 → 프로덕션 구현
> 선행: Phase 4.a (StatusPill / VirtualList 공통 컴포넌트), Phase 1' (adapter-ollama / adapter-lmstudio detect/health/list_models)
> 후행: Phase 4.d (projects), Phase 4.f (diagnostics 우상 게이트웨이 헬스), v1.1 (start/stop/restart/logs 활성화)
> 관련 ADR: ADR-0004 (RuntimeAdapter), ADR-0005 (Ollama wrap-not-replace)

---

## 0. 결정 요약 (4가지)

1. **2-pane 레이아웃 — 좌측 카드 sidebar(320px) + 우측 모델 VirtualList** — 결정 노트 §1.1 runtimes 그대로. 어댑터별 카드 column이 좌측, 선택된 어댑터의 모델만 우측에 24px row로 노출.
2. **start/stop/restart/logs는 v1 disabled** — 외부 데몬이라 어댑터 자체가 no-op이고, UI에서 active로 노출하면 사용자가 "끄기"를 눌렀을 때 아무 동작 없는 게 의아하다. disabled + tooltip("다음 업데이트에서 만나요")로 자리만.
3. **manual fetch만 — auto-refresh polling 거부** — 1초 polling은 battery + 네트워크 부담이고, 어댑터 detect()는 경량(GET /api/version 1.5s timeout)이지만 LM Studio는 GUI 앱이라 빈번한 polling이 사용자에게 보일 수 있다.
4. **last_ping_at은 호출 시점 RFC3339** — IPC 호출 시점 timestamp를 그대로 사용해 시계 차이/시간대 변환을 frontend에서 일관 처리.

---

## 1. 채택안

### 1.1 2-pane IA — 좌측 카드 + 우측 VirtualList

```
<topbar: 어댑터 합산 status — running 수 / 전체>
<2-pane>
  좌측 (sm 320px) — 어댑터 카드 column:
    각 카드: <header(name + StatusPill + version + port)> <body(loaded models 수, last ping ago)> <footer(disabled stop / disabled restart / 자세히 — v1은 disabled로 표시)>
    선택된 카드는 active border (primary).
  우측 (main, flex) — 모델 목록 VirtualList:
    선택된 어댑터의 모델만 24px row.
    컬럼: name (mono, flex) | size (num) | digest 8자 prefix
    검색 input (display_name 부분 일치) + 정렬 select (name / size).
    빈 상태: "어떤 모델도 로드되지 않았어요" + "카탈로그에서 받아볼래요?" CTA.
```

`@lmmaster/design-system/react`의 `StatusPill` (booting/listening/failed/idle) + `VirtualList` 사용 — 이미 Phase 4.a에서 구현 완료.

### 1.2 IPC contract

```rust
#[tauri::command]
pub async fn list_runtime_statuses() -> Result<Vec<RuntimeStatus>, RuntimesApiError>;

#[tauri::command]
pub async fn list_runtime_models(runtime_kind: RuntimeKind) -> Result<Vec<RuntimeModelView>, RuntimesApiError>;
```

- `RuntimeStatus` = kind, installed, version, running, latency_ms, model_count, last_ping_at.
- `RuntimeModelView` = runtime_kind, id, size_bytes, digest.
- `RuntimesApiError` = `unreachable | internal` (kebab tag).

구현 — `adapter_ollama::OllamaAdapter::new()` + `adapter_lmstudio::LmStudioAdapter::new()` 직접 호출. detect + health + list_models 합산.

LM Studio는 list_models가 size를 0으로 리턴 — 그대로 표시. digest도 빈 문자열.

### 1.3 a11y

- 어댑터 카드: `<article role="region" aria-labelledby={cardTitleId}>`.
- VirtualList row: `role="listitem"` (VirtualList 컴포넌트가 이미 처리).
- 합산 summary는 `role="status" aria-live="polite"`.
- 빈 상태도 `role="status" aria-live="polite"`.

---

## 2. 기각안 + 이유 (negative space)

### 2.1 start/stop active 노출

- **시도 / 검토 내용**: 사용자가 자기 PC의 외부 데몬을 GUI에서 통제할 수 있으면 더 편할 수 있음.
- **거부 이유**: Ollama / LM Studio는 외부 설치형이라 어댑터 자체가 stop/restart no-op이다. UI에서 active로 노출하면 사용자가 "끄기"를 눌렀을 때 실제로는 아무 일도 안 일어나는 안전 위험. 또한 LM Studio는 GUI 앱이라 외부에서 강제 종료하면 그 앱의 unsaved 상태를 잃을 위험. process kill을 어댑터에 추가하는 안은 ADR-0005 wrap-not-replace 정책 위반.
- **재검토 트리거**: 어댑터가 supervisor 모드로 진입하는 Phase 5'+에서 재검토 (자식 프로세스 모드면 stop/restart가 자연스럽게 의미를 가짐).

### 2.2 단일 카드에 모델까지 합치기

- **시도 / 검토 내용**: 어댑터가 2개뿐이라 카드 자체에 모델 목록을 인라인으로 펼치는 안.
- **거부 이유**: 카드 안에 모델 50+ 가능성이 있고(Ollama tag 무제한), 카드가 세로로 너무 길어지면서 정보 위계가 깨진다. 2-pane이 더 정직.
- **재검토 트리거**: 어댑터가 항상 모델 5개 이내로 제한된다는 데이터가 쌓이면 검토.

### 2.3 auto-refresh 1s polling

- **시도 / 검토 내용**: 사용자가 외부 앱에서 모델을 새로 로드하면 LMmaster가 자동으로 갱신.
- **거부 이유**: detect() + list_models()는 가벼운 HTTP지만 1초마다는 LM Studio GUI에 보이는 빈도가 높아 사용자에게 "왜 이렇게 자주 호출하지" 같은 의문을 줄 수 있다. 또한 battery / 네트워크 부담도 비례 증가. 사용자가 화면 진입 / 카드 클릭 시점에 fetch만 해도 충분.
- **재검토 트리거**: 사용자 telemetry에서 "수동 새로고침이 잦다"가 50%+ 사용자에서 발생하면 5s polling 검토.

---

## 3. 미정 / 후순위 이월

| 항목 | 이월 사유 | 진입 조건 / 페이즈 |
|---|---|---|
| start/stop/restart 활성화 | 외부 데몬 통제 위험 | Phase 5' supervisor 모드 |
| 로그 보기 (어댑터 stdout/stderr) | 어댑터 외부라 stream 채널 없음 | Phase 5' supervisor 모드 |
| auto-refresh polling | UX 부담 | 사용자 telemetry 트리거 시 |
| 모델 unload 버튼 | LM Studio API에 unload 없음 | LM Studio API 변경 시 |
| llama.cpp / kobold.cpp / vLLM 카드 | adapter는 있지만 외부 attach가 아니라 supervisor 자식 프로세스 | Phase 5' |

---

## 4. 테스트 invariant

- `RuntimesApiError` kebab tag 직렬화 (unreachable / internal 2건).
- `RuntimeStatus` snake_case 필드 (model_count / last_ping_at) 직렬화.
- `local_model_to_view` 필드 보존 (id ← file_rel_path, digest ← sha256).
- 페이지: `listRuntimeStatuses` 2건 fixture → 카드 2개 렌더.
- 페이지: 첫 카드 자동 선택 → 우측 모델 fetch + 표시.
- 페이지: 검색 입력 → 모델 필터링 (substring).
- 페이지: 빈 모델 → 빈 상태 + CTA.
- 페이지: 빈 상태 CTA → `lmmaster:navigate { detail: "catalog" }` 이벤트 발생.
- 페이지: stop/restart/logs 모두 disabled.
- 페이지: 정렬 select 변경 → 크기 큰 순.
- 페이지: axe violations 0.

---

## 5. 다음 페이즈 인계

### 5.1 메인 통합 시 해야 할 일

1. `apps/desktop/src-tauri/src/lib.rs`:
   - `pub mod runtimes;` 추가.
   - `invoke_handler` macro에 `runtimes::commands::list_runtime_statuses, runtimes::commands::list_runtime_models` 추가.

2. `apps/desktop/src-tauri/capabilities/main.json`:
   - `permissions` array에 `allow-list-runtime-statuses`, `allow-list-runtime-models` 추가.

3. `apps/desktop/src/App.tsx`:
   - `import { RuntimesPage } from "./pages/Runtimes";` 추가.
   - `MainShell`의 nav 분기에 `activeNav === "runtimes" ? <RuntimesPage /> : ...` 추가.
   - 페이지가 emit 하는 `lmmaster:navigate` 이벤트에 listener 추가해 nav 전환 (detail에 따라 setActiveNav 호출).

### 5.2 위험 노트

- **첫 진입 시 첫 카드 자동 선택의 race**: 비동기 fetch가 끝난 시점에 setSelectedKind를 호출. `useEffect` 의존성에 selectedKind를 넣으면 무한 루프 가능 — 현재는 `selectedKind == null`일 때만 set.
- **VirtualList ResizeObserver 의존**: jsdom에 ResizeObserver 미구현 — 테스트 setup에 stub 추가했지만 일반 setup.ts와 충돌하지 않도록 conditional polyfill.

---

## 6. 참고

### 글로벌 사례 / 패턴 출처

- **Tailscale** — windowed UI status pill 4 상태 + dot 색 + port 표시.
- **Linear** — 좌측 카드 sidebar + 우측 main 2-pane + 24px row 패턴.
- **Cherry Studio** — 외부 런타임 attach 패턴 (단, Korean-first 관점에서 변형).

### 관련 ADR

- ADR-0004 (RuntimeAdapter trait) — `detect/health/list_models` 일관 인터페이스.
- ADR-0005 (Ollama / LM Studio wrap-not-replace) — start/stop/restart no-op 근거.
- ADR-0006 (디자인 시스템) — StatusPill / VirtualList 토큰 사용.

---

**문서 버전**: v1.0 (2026-04-27 초안). Phase 4.c 구현 종료 후 RESUME.md에 산출물 + 검증 결과 기록.
