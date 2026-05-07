# Phase R-J — Invariant Tests 결정 노트

> **상태**: 채택 (2026-05-08, Phase R-I 직후)
> **선행 의존성**: Phase R-F+R-G+R-H+R-I (ADR-0064 + §H + §I) 머지 완료.
> **다음 페이즈**: Phase R-F.3 (Tauri dialog plugin 도입 후) 또는 v0.0.1 release tag push.
> **결정 일자**: 2026-05-08

---

## 1. 결정 요약

검수 리포트 INFO 4건을 invariant test로 보강 — 깨끗한 영역을 *깨지면 안 되는* 형태로 고정. ADR-0064 §J 흡수.

| ID | 결정 | 영역 | Effort |
|---|---|---|---|
| **J1 (R-J.1)** | `_render-markdown.ts::escapeHtml` + `renderInline` XSS 거부 invariant test | XSS surface | 30m |
| **J2 (R-J.2)** | `.claude/scripts/check-i18n-parity.mjs` 신규 + CI step 추가 | i18n ko/en parity | 30m |
| **J3 (R-J.3)** | CI에 `unsafe` / `tokio::spawn` grep gate (desktop crate에서 tokio::spawn 0건 강제) | Rust soundness | 30m |
| **J4 (R-J.4)** | a11y modal checklist는 CLAUDE.md §4.3 행동 규칙으로 이미 명시 — 코드 변경 0 | a11y | 0 (행동 규칙 보존) |

---

## 2. 채택안

### 2.1 J1 — XSS escape invariant tests

**핵심**: `_render-markdown.ts`는 EulaGate + Guide의 *내부 markdown* 전용. user-input 처리 안 하지만, 미래 remote guide manifest 도입 시 escape invariant 깨지면 즉시 XSS surface. 명시 invariant로 고정.

**변경 파일**:
- `apps/desktop/src/components/_render-markdown.test.ts` 신규 — `escapeHtml` 5종 entity 변환 + `<script>` 거부 + `<img onerror>` 거부 + `` `code` `` 안 HTML escape + `**bold**` 안 HTML escape.

### 2.2 J2 — i18n parity script CI

**핵심**: ko.json / en.json 키 카운트 1042/1042 일치를 매 PR/push 검증. flattened key path가 양쪽에 모두 존재하는지 확인.

**변경 파일**:
- `.claude/scripts/check-i18n-parity.mjs` 신규 — 두 JSON deep-flatten 후 key set 비교. ko-only / en-only 키 모두 보고 + 차이 있으면 exit 1.
- `.github/workflows/ci.yml` Node CI step 추가: `node .claude/scripts/check-i18n-parity.mjs`.

### 2.3 J3 — unsafe / tokio::spawn grep gate

**핵심**: 신규 `unsafe` 추가 시 review gate. desktop crate에서 `tokio::spawn` 직접 사용 금지 (Tauri는 `tauri::async_runtime::spawn` 사용 — 자체 runtime 소유).

**변경 파일**:
- `.github/workflows/ci.yml` 신규 step:
  - `unsafe` count 보고 (정보성 — 강제 fail 안 함).
  - desktop crate (`apps/desktop/src-tauri/src/`)에서 `tokio::spawn` 발견 시 fail.

### 2.4 J4 — a11y modal checklist

CLAUDE.md §4.3에 이미 명시:
- `role="dialog" aria-modal="true" aria-labelledby` 3종 세트
- `Esc` / 배경 클릭 닫기
- focus 첫 요소 auto-focus
- `prefers-reduced-motion` 토큰 차원 자동 비활성

신규 modal 작성 시 이 체크리스트 준수. 코드 변경 0 — 행동 규칙으로 보존.

---

## 3. 기각안 + 이유 (negative space)

| # | 거부된 대안 | 사유 |
|---|---|---|
| 1 | **XSS test: react-testing-library full DOM render** | 단순 escape 함수라 unit test로 충분. DOM render는 over-engineering |
| 2 | **i18n parity: typescript 컴파일 시점 typed keys** | 큰 refactor (typed-i18n-keys는 v2.x). 런타임 script가 즉시 효과적 |
| 3 | **i18n parity: vitest로 테스트 추가** | vitest는 frontend test runner — CI 단독 script가 더 빠름 + decoupled |
| 4 | **unsafe grep: 강제 fail (`-D unsafe`)** | hardware-probe FFI는 정상 unsafe 사용. 정보성 + tokio::spawn만 강제 fail |
| 5 | **a11y modal lint rule** | 기존 vitest-axe로 component 단위 검증 중. 룰 추가는 false positive 부담 |
| 6 | **i18n parity: locale별 fallback 방치** | "en만 있는 키"는 fallback에 의지하면 한국어 화면에 영어 노출 — invariant로 차단 |

---

## 4. 미정 / 후순위 이월 (v1.x)

| 항목 | 이유 |
|---|---|
| **typed-i18n-keys** | 큰 refactor. vitest 또는 codegen — v2.x |
| **CSP `connect-src` 더 좁힘** | 현재 `127.0.0.1:*`는 Workbench base_url localhost-only validate (R-F.2)와 정합. 추가 범위 좁힘은 v1.x |
| **`unsafe` strict gate (강제 fail)** | hardware-probe FFI 의존 — 큰 refactor 후 |
| **modal a11y axe 자동 검증 CI** | 컴포넌트별 vitest-axe는 이미 적용. CI 통합은 v1.x |

---

## 5. 테스트 invariant

| invariant | 위치 | 카운트 |
|---|---|---|
| `escapeHtml` 5 entity 변환 | `apps/desktop/src/components/_render-markdown.test.ts` | +1 |
| `escapeHtml` `<script>` / `<img onerror>` 거부 | 위 | +2 |
| `renderInline` `<bold>` 안 HTML escape | 위 | +1 |
| `renderInline` `` `code` `` 안 HTML escape | 위 | +1 |
| `renderMarkdown` `<script>` 본문 escape | 위 | +1 |
| i18n ko/en flatten key 일치 | `.claude/scripts/check-i18n-parity.mjs` | CI script (test 0, exit code) |
| desktop crate `tokio::spawn` 0건 | `.github/workflows/ci.yml` grep step | CI step (test 0, exit code) |

**총 vitest +6**.

---

## 6. 다음 페이즈 인계

### 6.1 Phase R-F.3 (4-8h, deferred)
- IPC raw path → selected_path_token registry. Tauri dialog plugin 도입 (`@tauri-apps/plugin-dialog`) 후 별도 sub-phase.
- 영향 IPC: `ingest_path`, `workbench_preview_jsonl`, `workbench_run`.
- ADR-0052 S6 v0.3.x 승격 결정 필요.

### 6.2 v0.0.1 release tag push (사용자 결정)
- Phase R-F+R-G+R-H+R-I+R-J 모두 종결.
- `git tag v0.0.1 && git push origin v0.0.1` → release.yml 자동 트리거 → SQLCipher feature wiring 실 검증 + GitHub Releases 자동 생성.

### 6.3 v1.x DEFERRED
- Phase R-K (Updater 옵션 A 활성, 2-3h).
- Phase R-L (Ollama Linux download_and_extract 자동화, 6-8h).

### 6.4 위험
- i18n parity script가 ko/en 외 다른 locale 추가 시 깨질 수 있음 — 현재는 ko/en만 운영, v1.x 신 locale 추가 시 script 확장.
- `unsafe` grep은 keyword 기반이라 string literal 안 `unsafe` 단어도 카운트. 정보성이라 false positive 무해.
- `tokio::spawn` grep은 `tauri::async_runtime::spawn` import alias 우회 가능 (`use tokio::spawn as ts;` 같은 패턴). 단 일반적 코드는 직접 호출이라 충분.

---

**문서 버전**: v1.0 (2026-05-08, Phase R-J 1차 작성).
