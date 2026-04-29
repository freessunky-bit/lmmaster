# ADR-0038: Multi-workspace UX + ActiveWorkspaceContext

- Status: Accepted
- Date: 2026-04-28
- Related: ADR-0024 (knowledge-stack RAG), ADR-0009 (portable workspace), ADR-0017 (manifest installer), ADR-0023 (workbench policy), ADR-0035 (SQLCipher activation)
- 결정 노트: `docs/research/phase-8p-9p-10p-residual-plan.md` §1.7
- 구현 phase: Phase 8'.1

## Context

ADR-0024 §1은 RAG / 사용자 모델 / 키 / 자료가 모두 **per-workspace 격리**되어야 한다고 약속했다. SQLite 스키마 (`workspaces` 테이블) + IngestService + KnowledgeStore는 모두 `workspace_id`를 강제하지만, **UI 레벨에서는 워크스페이스를 만들거나 전환할 수단이 없었다.** 모든 페이지가 `workspaceId = "default"`를 하드코딩해, ADR-0024가 약속한 격리가 실현되지 않은 상태였다 (`apps/desktop/src/pages/Workspace.tsx:170` 등).

다음 제약을 동시에 만족해야 한다:

1. **첫 실행 onboarding 무중단** — 사용자가 워크스페이스 개념을 모르고도 즉시 사용할 수 있어야 한다 (silent default 자동 생성).
2. **외부 통신 0** — 워크스페이스 메타는 `app_data_dir/workspaces/index.json`에 로컬 저장. 외부 sync 없음.
3. **deterministic 격리** — workspace A의 자료가 workspace B 검색에 절대 노출되지 않아야 한다 (knowledge-stack의 `WHERE workspace_id = ?` 필터를 그대로 활용).
4. **한국어 해요체** — UI / 에러 / confirmation 모두 한국어.
5. **a11y 일관** — dropdown / 모달 / Esc / focus trap을 기존 design-system 패턴으로.

## Decision

### 1. Backend `workspaces.rs` IPC + JSON index 영속

`apps/desktop/src-tauri/src/workspaces.rs`에 새 IPC 모듈 신설.

- 6 commands: `list_workspaces` / `get_active_workspace` / `create_workspace` / `rename_workspace` / `delete_workspace` / `set_active_workspace`.
- `WorkspaceInfo { id, name, description?, created_at_iso, last_used_iso? }`.
- 영속: `app_data_dir/workspaces/index.json` — atomic rename (`.tmp` → final, Phase 8'.0 패턴).
- schema_version 컬럼으로 향후 마이그레이션 대비.
- 첫 실행 시 default workspace (UUID v4 + 이름 "기본 워크스페이스") 자동 생성. 사용자에게 silent.
- 동일 이름 중복 거부 (`DuplicateName` 에러). 사용자가 의식적으로 이름을 다르게 하도록 유도.
- 마지막 1개는 삭제 거부 (`CannotDeleteOnlyWorkspace`).
- `set_active_workspace` / `create_workspace` / `rename_workspace` / `delete_workspace` 모두 `workspaces://changed` 이벤트 emit — frontend가 즉시 재구독.

### 2. Frontend `ActiveWorkspaceContext` + `WorkspaceSwitcher`

- `apps/desktop/src/contexts/ActiveWorkspaceContext.tsx` 신설:
  - `useActiveWorkspace()` hook — Provider 안에서만 사용. throw if missing.
  - `useActiveWorkspaceOptional()` hook — 테스트/스토리북 대응 (Provider 없이 null 반환).
  - 마운트 시 `list_workspaces` + `get_active_workspace` 동시 호출.
  - `workspaces://changed` 이벤트 listen → 자동 갱신.
  - `localStorage` hydration (`lmmaster.active_workspace_id`) — 앱 재시작 시 깜빡임 회피. backend가 source of truth.
- `apps/desktop/src/components/WorkspaceSwitcher.tsx` 신설 — 사이드바 상단 dropdown.
  - Display: 현재 active workspace 이름 + ▾.
  - Dropdown: 항목별 ✓ 표시 + 이름 바꾸기 / 지우기 버튼 + "+ 새 워크스페이스 만들기".
  - 모달: 생성 / 이름변경 / 삭제 confirmation. role="dialog" + aria-modal=true + focus trap.
- `App.tsx`: `<ActiveWorkspaceProvider>` 래핑 (EulaGate 안쪽). 사이드바에 `<WorkspaceSwitcher />`.
- `Workspace.tsx`: `workspaceId = "default"` 하드코딩 제거 → `useActiveWorkspaceOptional()`. prop은 테스트 override 전용.

### 3. v1 cascade 정책 — 메타데이터만 정리

`delete_workspace`는 `index.json`의 entry만 정리. Knowledge SQLite (.db) 파일과 custom-models는 디스크에 보존.

- 사용자에게 confirmation dialog로 명확히 알림: "이 워크스페이스의 자료와 사용자 모델은 모두 정리할 예정이에요. 디스크 파일은 보존되니, 다시 쓰고 싶으시면 백업해 두세요."
- v1.x에서 cascade DB 정리 + 사용자 모델 cleanup 추가.
- 이렇게 보수적으로 가는 이유: workspace 삭제 후 "실수였다, 복원해 주세요"라는 요구를 v1에서도 응대 가능하게.

### 4. 기존 `default` 데이터 자동 마이그레이션

- 첫 실행 시 default workspace 자동 생성 — 새 UUID. 기존 사용자 (이전 빌드)의 knowledge SQLite는 그대로 사용 가능 (사용자가 이전 ingest를 새 workspace로 옮기려면 파일을 직접 복사 — v1.x export/import로 자동화 예정).

### 5. workbench / model-registry는 v1에서 global 유지

- model-registry::register는 workspace_id 인자를 받지 않음 — Phase 5'.d 시점 결정. v1.x에서 per-workspace로 마이그레이션.
- Workbench Run config에도 workspace_id는 미포함 — 5단계 작업의 산출물(modelfile / lora adapter)은 디스크에 그대로 떨어지고, 사용자 모델 등록은 global custom-models.json에 누적.
- 이는 ADR-0024 약속과 *부분적으로* 어긋나지만, v1에서는 사용자 모델이 흔하지 않고 한 사용자가 여러 workspace에서 같은 사용자 모델을 재사용하는 시나리오가 있음을 고려. v1.x에서 per-workspace 격리로 마이그레이션 (memory + RESUME에 deferral 명시).

## Consequences

### 긍정

- ADR-0024 약속이 UI 레벨에서 실현 — 사용자가 workspace를 만들고 전환 가능.
- 기존 사용자 데이터 무중단 — default workspace 자동 시드.
- `workspaces://changed` 이벤트로 다중 윈도우 동기화 가능 (v1.x).
- atomic rename 영속으로 부분 쓰기 손상 방지.

### 부정 / Trade-off

- v1에서 cascade DB 정리 미구현 — 사용자가 디스크 정리 의무. 하지만 confirmation copy로 명확히 안내.
- workbench / custom-models가 global인 채 — v1.x에서 per-workspace 격리 작업 필요.
- 기존 Workspace 페이지 테스트 (`Workspace.test.tsx`)가 모두 `workspaceId="default"` prop을 명시하도록 갱신.

## Alternatives rejected

### A1. Workbench / Knowledge 페이지 각자 자체 workspace 선택

각 페이지에 별도의 workspace dropdown을 두는 안. 거부 이유:

- **fragmentation** — 사용자가 페이지마다 다른 workspace를 보고 있어도 인지하기 어려움. 멘탈 모델 깨짐.
- **상태 동기화 코스트** — 한 페이지에서 workspace 변경 시 다른 페이지도 갱신해야 함. 결국 글로벌 상태 필요.
- **사이드바 글로벌 switcher가 더 자연스러움** — Notion / Linear / Slack 등 multi-workspace SaaS의 표준 패턴.

### A2. URL 경로로 workspace 분리

`/workspaces/<id>/knowledge` 같은 URL 라우팅. 거부 이유:

- **Tauri는 SPA** — URL 의존성이 약함. 메뉴 클릭 = SetState 패턴이 일관.
- **딥링크 시나리오 부재** — Tauri 데스크톱 앱은 외부에서 URL로 진입하지 않음. 가치 < 비용.
- **router 의존성** — react-router 등 추가 의존성. design-system 단순함과 상충.

### A3. 단일 workspace 유지 (status quo)

ADR-0024 약속 위반. 거부.

### A4. 첫 실행 시 사용자에게 workspace 이름 명시 입력 요구

거부 이유:

- onboarding 흐름에 한 단계 추가 — 사용자 마찰 증가.
- 대부분 사용자는 단일 workspace로 시작. "기본 워크스페이스" silent default가 적합.
- 사용자가 명시 이름이 필요해지면 나중에 rename 가능.

### A5. delete 시 즉시 cascade DB 정리

거부 이유:

- v1에서는 "실수였다" 복구가 어려움. 사용자가 디스크 파일을 백업할 시간을 확보 (confirmation copy로 안내).
- 디스크 cleanup 로직은 다른 phase (portable workspace import/export, Phase 11')와 함께 설계하는 편이 일관.
- v1.x ADR addendum에서 cascade 정책을 정식화.

### A6. SQLite에 workspaces 테이블을 별도 DB로

`workspaces.db` 같은 SQLite 파일. 거부 이유:

- 워크스페이스 메타는 매우 작고 (수십 row 미만) IO도 드물어 — JSON file이 단순 + 검사 용이.
- atomic rename 패턴이 SQLite WAL보다 코드 면적 작음.
- 향후 schema_version 마이그레이션은 단순 JSON 형태 변환이 적합.

## References

- `apps/desktop/src-tauri/src/workspaces.rs` — IPC 모듈.
- `apps/desktop/src/contexts/ActiveWorkspaceContext.tsx` — Provider/hook.
- `apps/desktop/src/components/WorkspaceSwitcher.tsx` — UI.
- `apps/desktop/src/ipc/workspaces.ts` — TS wrapper.
- `crates/knowledge-stack/src/store.rs` — `workspaces` 테이블 (이미 존재).
- ADR-0024 — per-workspace 격리 약속.
