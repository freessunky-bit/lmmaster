# Phase R-H — Boundary Polish 결정 노트

> **상태**: 채택 (2026-05-08, R-F+R-G hotfix 후속)
> **선행 의존성**: Phase R-F+R-G (ADR-0064) 머지 완료.
> **다음 페이즈**: Phase R-I (CI/Build Hygiene) → Phase R-J (Invariant Tests).
> **결정 일자**: 2026-05-08

---

## 1. 결정 요약

GPT Pro 검수 리포트의 medium-severity boundary 결함 3건을 boundary polish sub-phase로 묶어 처리한다. R-F+R-G와 동일한 ADR-0064 보호 우산 아래 진행 (별도 ADR 신설 없이 §H 항목으로 흡수).

| ID | 결정 | 영역 | Effort |
|---|---|---|---|
| **H1 (R-H.1)** | `install_app(id)` IPC entry에 alpha-num/`-`/`_` 검증 + `canonicalize()` prefix check | path traversal | 30m |
| **H2 (R-H.2)** | `registry-fetcher::fetch_one` + `try_bundled` 양쪽에 id allowlist 검증 + canonical prefix check | bundled bypass 차단 | 30m |
| **H3 (R-H.3)** | `installer::action.rs::run_open_url`에 host allowlist (capability scope과 동일 4 도메인) | shell ACL drift defense-in-depth | 1-2h |
| **H4 (R-H.4)** | `KeyStore::ATTACH DATABASE` single quote escape | R-G.2에 통합 처리 — 본 phase 범위 외 | 처리 완료 |

본 3건은 모두 small-effort polish — 단일 결정 노트 + 통합 검증으로 충분.

---

## 2. 채택안

### 2.1 H1 — install_app(id) regex + canonicalize prefix

**핵심**: IPC entry에서 `id`를 `is_ascii_alphanumeric() || c == '-' || c == '_'`로 검증 + `manifest_file.canonicalize()` + `manifests_dir.canonicalize()` prefix check. `exists()` 제거 후 read 단일 fail path.

**변경 파일**:
- `apps/desktop/src-tauri/src/install/mod.rs`:
  - `InstallApiError::InvalidId { id: String }` variant 신규.
  - `install_app(id)` 첫 단계: `is_valid_app_id(&id)?` 호출.
  - `manifest_file` canonicalize 후 manifests_dir prefix 검증.
  - `exists()` 분리 호출 제거 (canonicalize fail이 same role).

### 2.2 H2 — fetch_one + try_bundled id allowlist

**핵심**: `FetcherError::InvalidManifestId { id }` variant 신규. `fetch_one` entry에 `validate_manifest_id(id)?` 호출. `try_bundled`도 defense-in-depth로 동일 검증 + `bundled_dir.canonicalize()` prefix check.

**변경 파일**:
- `crates/registry-fetcher/src/error.rs`: `InvalidManifestId { id: String }` variant 추가.
- `crates/registry-fetcher/src/fetcher.rs`: `validate_manifest_id` private helper + `fetch_one` 첫 줄 + `try_bundled` 첫 줄 호출 + canonical prefix check.

### 2.3 H3 — open_url host allowlist (defense-in-depth)

**핵심**: `webbrowser::open`은 Tauri shell ACL을 우회하므로 Rust action layer에서 capability scope `shell:allow-open`과 동일 4 도메인 allowlist 검증. `url` crate 재사용 (R-F.2에서 이미 workspace에 추가됨).

**변경 파일**:
- `crates/installer/Cargo.toml`: `url = { workspace = true }` 추가.
- `crates/installer/src/action.rs::run_open_url`:
  - `OPEN_URL_HOST_ALLOWLIST` 상수: `["github.com", "huggingface.co", "cdn.jsdelivr.net", "lmstudio.ai"]`.
  - `Url::parse(spec.url)` + scheme `http`/`https` 한정 + `host_str()` 정확 매치 (`eq_ignore_ascii_case`).
  - 검증 실패 시 `ActionError::InvalidSpec` 한국어 메시지.

---

## 3. 기각안 + 이유 (negative space)

| # | 거부된 대안 | 사유 |
|---|---|---|
| 1 | **install_app: regex crate 사용** | regex 의존성 추가 부담. `is_ascii_alphanumeric` + char 매치로 충분 |
| 2 | **install_app: `.` 허용** | manifest id에 `.` 없음. path traversal 위험만 추가 |
| 3 | **fetch_one: SourceConfig::resolve_url의 검증 함수 직접 재사용** | resolve_url은 URL 치환 함수 — 검증 함수만 추출하는 게 단일 책임 |
| 4 | **try_bundled: canonical prefix check만 (id 검증 X)** | id 검증 없이도 prefix check가 path escape 차단하지만, defense-in-depth로 양쪽 |
| 5 | **open_url: capability scope만 신뢰** | `webbrowser::open`은 Tauri shell plugin 우회 — Rust layer 검증 필수 |
| 6 | **open_url: tauri-plugin-shell의 `open` 호출로 교체** | 변경 면적 큼 + UX 변경 가능. 정책 drift는 작은 fix로 우선 |
| 7 | **open_url: ollama.com 화이트리스트 추가** | 외부 통신 화이트리스트 정책 (ADR-0055) 일관 — github.com 우회로 충분 |
| 8 | **open_url: subdomain wildcard 허용** | suffix attack 위험. 현재 capability scope도 정확 매치 (ADR-0052) |

---

## 4. 미정 / 후순위 이월 (v1.x)

| 항목 | 이유 | 위치 |
|---|---|---|
| **`webbrowser::open` → `tauri-plugin-shell::open` 전환** | 변경 면적 + 회귀 위험. Phase R-K 후속 또는 v1.1 |
| **install_app id 충돌 시 race detection** | 현재는 InstallRegistry::try_start으로 동시성 보호. 추가 lock 불필요 |
| **manifest path symlink escape** | canonicalize가 symlink resolve — 충분 |

---

## 5. 테스트 invariant (sub-phase DoD)

| invariant | 위치 | 카운트 |
|---|---|---|
| `is_valid_app_id` 정확 매치 (alpha-num/dash/underscore) + 거부 (`../`, `foo/bar`, control char, empty) | `apps/desktop/src-tauri/src/install/mod.rs::tests` | +5 |
| `validate_manifest_id` 동일 검증 + try_bundled bypass 거부 | `crates/registry-fetcher/src/fetcher.rs::tests` | +3 |
| `run_open_url` host allowlist (4 도메인 OK + 비-allowlist host 거부 + http/https만 + invalid url 거부) | `crates/installer/src/action.rs` 또는 별도 모듈 | +6 |

**총 테스트 차분 +14**.

---

## 6. 다음 페이즈 인계

### 6.1 Phase R-I (CI/Build Hygiene, 4-5h)
- R-I.1: CI test no-run 제거 + Node vitest 추가.
- R-I.2: tsconfig noEmit + .js 정리.
- R-I.3: Knowledge ingest cancel responsiveness.
- R-I.4: Trending Watcher decision note 갱신.

### 6.2 Phase R-J (Invariant Tests, 2h)
- XSS escape / i18n parity script CI / unsafe grep gate / a11y modal checklist.

### 6.3 Phase R-F.3 (4-8h, deferred)
- IPC raw path → selected_path_token registry. Tauri dialog plugin 도입 후 별도.

### 6.4 위험 노트
- `install_app` canonicalize는 `manifest_file`이 존재해야 성공 — 미존재 manifest는 그대로 ManifestNotFound 분기 (기존 동작 보존).
- `try_bundled` canonicalize 실패 시 BundledMissing 분기 — 회귀 0.
- `run_open_url` 변경은 LM Studio open_url + Ollama Linux open_url 두 케이스에 영향 — capability scope과 정합 검증.

---

**문서 버전**: v1.0 (2026-05-08, Phase R-H 1차 작성).

**참조**:
- ADR-0064 §H (Phase R-H 항목으로 흡수)
- 검수 리포트: `c:/Users/wind.WIND-PC/Downloads/LMmaster-review-report.md`
- Phase R-F+R-G 결정 노트: `phase-rf-rg-critical-hotfix-decision.md`
