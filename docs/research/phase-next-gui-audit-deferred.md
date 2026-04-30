# Next phase 작업 계획 — GUI audit 잔여 + 신모델 자동 갱신

> **2026-04-30 audit 후속**: P0 항목(LoRA 트리오) + P1 1건(Diagnostics 활성 키)은 이번 세션에서 fix.
> 본 문서는 **다음 페이즈에 손대야 할** 잔여 14건을 우선순위별로 정리.

## 📋 잔여 작업 매트릭스

### 🔴 P1 elite (UX 완성도 — v1 ship 전 권장)

| # | 항목 | 현재 상태 | 작업 범위 | 추정 |
|---|---|---|---|---|
| 1 | **신모델 자동 갱신** | `registry-fetcher`가 app manifest만 fetch. 모델 카탈로그는 번들된 11개로 고정 | `registry-fetcher`에 model manifest source 추가 — GitHub Releases (LMmaster/lmmaster-catalog repo) + jsDelivr fallback. 6시간 cron이 자동 갱신. | 1 sub-phase (4-6h) |
| 2 | **`update_api_key_pipelines` UI** | wrapper 있으나 호출 0건. 발급된 키의 필터 편집 불가 | `ApiKeysPanel.tsx`의 키 row에 "필터 편집" 액션 + modal | 2-3h |
| 3 | **Diagnostics 나머지 4 MOCK 제거** | gateway latency / 최근 요청 / 벤치 batch / repair history 모두 가짜 | (a) gateway latency: 백엔드 ring buffer IPC 신규, (b) 최근 요청: pipelines audit_log 재사용, (c) 벤치 batch: `getLastBenchReport` × N 합산, (d) repair history: workspace fingerprint history IPC 신규 | 1 sub-phase (4-6h) |
| 4 | **Crash report 뷰어** | `panic_hook.rs`가 `%LOCALAPPDATA%/lmmaster/crash`에 파일 작성하지만 사용자 GUI 없음 | Diagnostics 페이지에 "최근 충돌" 섹션 + 파일 목록 + 텍스트 미리보기 + "지우기" + (옵션) telemetry 전송 | 2-3h |

### 🟡 P2 polish (있으면 좋음)

| # | 항목 | 작업 범위 | 추정 |
|---|---|---|---|
| 5 | **`bench:started` listener** | ModelDetailDrawer가 `onBenchStarted` 추가 등록 → IPC invoke 직후 spinner 즉시 표시 | 30분 |
| 6 | **`list_workbench_runs` 표시** | Workbench 페이지 헤더에 "활성 작업 N개" 배지 + 클릭 시 list panel | 1-2h |
| 7 | **`list_ingests` 표시** | Workspace knowledge 카드에 "현재 인덱싱 중 N개" 배지 | 1-2h |
| 8 | **`workbench_serialize_examples` 디버그** | Step 1 preview 옆에 "정규화된 JSONL 다운로드" 액션 | 1h |
| 9 | **`get_pipelines_config` 디버그 미러** | PipelinesPanel에 raw config JSON viewer (collapsed) | 1h |
| 10 | **`submit_telemetry_event` 디버그** | Settings → Telemetry 패널에 "테스트 이벤트 보내기" 버튼 (opt-in 검증용) | 1h |

### ⚪ P2 백엔드 미구현 (v1.x deferred)

| # | 항목 | 비고 |
|---|---|---|
| 11 | **Gateway latency ring buffer IPC** | core-gateway에 미들웨어 추가 + IPC 신규. Diagnostics #3에 종속. | ADR 후보 |
| 12 | **Gateway access log SQLite** | 위와 같은 페이즈에서 진행. 수동 retention 정책 필요. | |

### ✅ Audit false positive (조사 결과 이미 정상)

| 항목 | 실제 상태 |
|---|---|
| `cancel_workspace_export` 버튼 누락 | 이미 `PortableExportPanel.tsx:405-408`에 구현됨 |

## 🎯 다음 페이즈 진입 권고

**최우선 (Phase 9'.c.1 또는 13')** — *사용자 신뢰감 직접 영향*:
1. **#1 신모델 자동 갱신** — 사용자가 "Gemma 안 보여"라고 한 그 문제. 다음 모델이 나올 때마다 manifest를 직접 추가하는 것은 지속 가능 X.
2. **#3 Diagnostics MOCK 4건** — "진단" 메뉴인데 가짜 숫자만 있으면 신뢰도 박살.

**권장 (Phase 13'.a)**:
3. **#2 API 키 필터 편집** — 한 번 발급한 키를 평생 못 고치면 운영 불가.
4. **#4 Crash 뷰어** — 사용자 PC에서 panic 났을 때 진단 path가 핵심.

**후순위 (v1.x backlog)** — #5~#10 (P2 polish, 합쳐 1 sub-phase)

## 📐 작업 페이즈 계획 (제안)

```
Phase 13'.a — Catalog Live Refresh  (1 sub-phase, ~6h)
├── 보강 리서치: GitHub Releases catalog 소스 + jsDelivr fallback 패턴
├── ADR-0044 신설 (모델 카탈로그 4-tier source)
├── crates/registry-fetcher: model_manifest_source 추가
├── apps/desktop/src-tauri/src/lib.rs: catalog hot-swap
├── apps/desktop/src/pages/Catalog.tsx: "다시 불러오기" 동작 변경
└── 결정 노트 6-섹션 + tests

Phase 13'.b — Diagnostics 실 데이터  (1 sub-phase, ~6h)
├── 보강 리서치: gateway middleware metrics 패턴 + ring buffer
├── ADR-0045 (Gateway access log + latency ring buffer)
├── core-gateway: GatewayMetrics middleware
├── apps/desktop/src-tauri/src/diagnostics/: 신규 IPC 4개
├── Diagnostics.tsx: 5 MOCK 제거
└── 결정 노트 + tests

Phase 13'.c — API 키 운영 + Crash 뷰어  (1 sub-phase, ~5h)
├── ApiKeyEditModal — pipelines override 편집
├── CrashReportPanel — Diagnostics 안에 새 섹션
├── panic_hook 파일 형식 표준화 (JSON metadata + raw stack)
└── tests

Phase 13'.d — P2 polish 일괄  (선택, ~5h)
└── #5~#10 batched
```

## 📚 결정 노트 / ADR 후보

- **ADR-0044**: Model catalog live refresh — registry-fetcher에 모델 source 추가
- **ADR-0045**: Gateway metrics middleware — latency ring buffer + access log
- **결정 노트**: 매 sub-phase별 6-section (CLAUDE.md §4.5 표준)

## ⚠️ 위험 / 의존성

- #1: GitHub repo 분리 필요? (`lmmaster-catalog` 별도 레포 vs 메인 레포 `manifests/` 폴더 직접 published)
- #3: Gateway latency 측정은 미들웨어 추가가 hot-path 성능에 영향. 관찰자효과 최소화 필요.
- #11~#12: 미구현이라 Diagnostics #3에 dependency. Phase 13'.b 안에서 함께.

## 🔗 관련 문서

- 본 audit subagent 결과: `docs/research/2026-04-30-gui-audit.md` (TBD — agent 결과 그대로 별도 보존 권고)
- Phase 9'.b 결정 노트: `docs/research/phase-9pb-workbench-real-reinforcement.md`
- ACL drift script: `.claude/scripts/check-acl-drift.ps1`
