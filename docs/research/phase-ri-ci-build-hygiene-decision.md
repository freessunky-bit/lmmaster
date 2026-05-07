# Phase R-I — CI / Build Hygiene 결정 노트

> **상태**: 채택 (2026-05-08, Phase R-H 직후)
> **선행 의존성**: Phase R-F+R-G+R-H (ADR-0064 + §H) 머지 완료.
> **다음 페이즈**: Phase R-J (Invariant Tests).
> **결정 일자**: 2026-05-08

---

## 1. 결정 요약

검수 리포트 medium 4건 + Trending Watcher 문서 drift 1건을 CI/build hygiene sub-phase로 묶음. 모두 small effort polish — ADR-0064 §I로 흡수 (별도 ADR 없음).

| ID | 결정 | 영역 | Effort |
|---|---|---|---|
| **I1 (R-I.1)** | CI `cargo test --workspace --no-run` → 실 실행 + Node CI에 `pnpm exec vitest run` 추가 | release gate | 30m |
| **I2 (R-I.2)** | `tsconfig.json::noEmit = true` + 기존 `apps/desktop/src/**/*.js` 일괄 정리 + `.gitignore` 추가 | resolver shadowing 방어 | 1h |
| **I3 (R-I.3)** | Knowledge ingest에 file size cap (10MB) 추가 — chunk-level cancel은 v1.x deferred | hot path / 메모리 cap | 30m |
| **I4 (R-I.4)** | Trending Watcher decision note §7 갱신 + deprecated workflow 삭제 조건 objective gate | negative space 보존 | 30m |

---

## 2. 채택안

### 2.1 I1 — CI test execution

**핵심**: `cargo test --workspace --no-run`은 컴파일만 검증 — 실제 invariant regression 못 잡음. release.yml은 `--exclude lmmaster-desktop`이라 desktop crate 컴파일 + 다른 crate 실 실행으로 통과. CI도 동일 패턴 적용.

**변경 파일**:
- `.github/workflows/ci.yml`:
  - Rust 매트릭스에 `cargo test --workspace --exclude lmmaster-desktop` (no-run 제거).
  - Node CI에 `pnpm --filter @lmmaster/desktop test` (vitest run) 추가.

### 2.2 I2 — tsc noEmit + .js 정리

**핵심**: `apps/desktop/tsconfig.json`에 `noEmit: false` (default)라 `tsc -b` 호출 시 .ts → .js 산출. Vite resolver는 .ts/.tsx 우선이지만 동일 디렉터리에 stale .js 잔류는 build cache drift 위험.

**변경 파일**:
- `apps/desktop/tsconfig.json`: `"noEmit": true` 추가 (typecheck-only).
- `.gitignore` (또는 `apps/desktop/.gitignore`): `apps/desktop/src/**/*.js` 패턴 추가 — 실수 commit 방지.
- `apps/desktop/src/**/*.js` 일괄 삭제 (.ts/.tsx 대응 있는 것만 — orphan .js는 별도 검사 후).

**의도적 .js 검사**: `apps/desktop/src/i18n/init.js` 같은 파일은 `.ts` 동명 파일이 함께 존재 (`init.ts`) → `.js`는 산출물. 안전 삭제.

### 2.3 I3 — Knowledge ingest file size cap

**핵심**: 현재 `IngestService`는 `std::fs::read_to_string(file)`로 파일 전체 메모리 로드. 100MB+ 파일이면 사용자 PC 메모리 폭발. 10MB cap 추가 + 한국어 카피.

**변경 파일**:
- `crates/knowledge-stack/src/ingest.rs`:
  - `MAX_INGEST_FILE_BYTES = 10 * 1024 * 1024` 상수 신설.
  - `read_to_string` 직전 `file.metadata()?.len() > cap` 검사.
  - 초과 시 `KnowledgeError::FileTooLarge { path, size_mb, cap_mb }` variant + 한국어 해요체.

**chunk-level cancel은 v1.x deferred**: 현재 stage-level cancel (Reading/Chunking/Embedding/Writing 4 checkpoint)은 큰 파일 1개 read 도중 즉시 반응 안 됨. 단 file size cap이 있으면 max 10MB read 시간이 1-2초라 cancel 응답성 실용적 충분. 풀 chunk-level cancel은 spawn_blocking refactor와 함께 v1.x.

### 2.4 I4 — Trending Watcher decision note 갱신

**핵심**: 결정 노트 §7 "본 repo 영향 0"은 ADR-0059 prototype exception 추가 후 stale. negative space 보존 위해 정확한 영향 범위 갱신.

**변경 파일**:
- `docs/research/phase-21p-trending-watcher-decision.md` §7: "본 repo 영향 0" → "desktop runtime 영향 0, workspace/CI 영향 있음 (prototype crate 잔류 — ADR-0059 v1.x exception)".
- `.github/workflows/trending-watcher.yml` 헤더 카피: "v0.4 release 시 삭제 권장" (vague) → "별도 repo가 6h cron 4회 이상 정상 실행 + Issue 생성 1건 이상 검증 후 본 file 삭제" (objective gate).

---

## 3. 기각안 + 이유 (negative space)

| # | 거부된 대안 | 사유 |
|---|---|---|
| 1 | **CI: full `cargo test --workspace`** (desktop 포함) | Windows cdylib test entrypoint 환경 문제 — release.yml과 동일 exclude 정책 일관 |
| 2 | **CI: 별도 sqlcipher matrix job** | release.yml 신규 verify step (R-G.1)으로 충분. CI는 baseline 검증만 |
| 3 | **tsc noEmit false 유지** | resolver shadowing risk. Vite는 typecheck 안 함 — tsc 단독 typecheck-only가 정합 |
| 4 | **.js 파일 보존 + .gitignore만** | stale .js가 commit돼 있으면 .gitignore가 무력화. 일괄 정리 + .gitignore가 정공 |
| 5 | **chunk-level cancel 전체 refactor** | spawn_blocking 도입 + KnowledgeStore lock 재설계 — v1.x. file size cap만으로 실용적 충분 |
| 6 | **file size cap 100MB** | 사용자 PC 메모리 부담 + UX 신호 약함. 10MB는 일반 markdown/text 파일 99% 커버 |
| 7 | **Trending Watcher prototype crate 즉시 제거** | 별도 repo 운영 안정화 검증 전이라 회귀 위험. objective gate 후 삭제 |

---

## 4. 미정 / 후순위 이월 (v1.x)

| 항목 | 이유 |
|---|---|
| **chunk-level cancel + spawn_blocking refactor** | 큰 변경 + KnowledgeStore lock 재설계. v1.x 별도 sub-phase |
| **eslint policy 도입** | 현재 `package.json::lint` TODO. 별도 sub-phase |
| **Node CI matrix (Linux/Windows/macOS)** | 현재 ubuntu-only. tauri-action도 linux. Windows-specific frontend 회귀는 vitest jsdom으로 충분 |
| **Trending Watcher prototype crate 제거** | objective gate 충족 후 (별도 repo 6h cron 4회 + Issue 1건 검증) |

---

## 5. 테스트 invariant

| invariant | 위치 | 카운트 |
|---|---|---|
| `MAX_INGEST_FILE_BYTES` 초과 시 `FileTooLarge` 반환 | `crates/knowledge-stack/src/ingest.rs::tests` | +1 |
| `KnowledgeError::FileTooLarge` 한국어 메시지 | 위 | +1 |

**기존 보존**: `cargo test --workspace --exclude lmmaster-desktop`이 CI에서 처음으로 실 실행 — 기존 1100+ 테스트가 매 PR/push에 검증.

---

## 6. 다음 페이즈 인계

### 6.1 Phase R-J (2h)
- R-J.1: XSS escape invariant tests.
- R-J.2: i18n parity script CI.
- R-J.3: `unsafe` / `tokio::spawn` grep gate.
- R-J.4: a11y modal checklist 강화.

### 6.2 Phase R-F.3 (4-8h, deferred)
- IPC raw path → selected_path_token registry. Tauri dialog plugin 도입 후 별도.

### 6.3 위험
- `.js` 일괄 삭제 시 의도적 .js (i18n init.js 등)도 삭제 — `.ts` 대응 있는 .js만 삭제. 검사 필수.
- `noEmit: true` 후 `pnpm build` (tsc -b && vite build)는 vite가 typecheck X — vite는 esbuild로 바로 변환. 별도 issue 없음.

---

**문서 버전**: v1.0 (2026-05-08, Phase R-I 1차 작성).
