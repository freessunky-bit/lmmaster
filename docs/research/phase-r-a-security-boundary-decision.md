# Phase R-A — Security Boundary 결정 노트

> 2026-05-03. GPT Pro 정적 검수 30건 중 v0.0.1 ship-blocker 보안 카테고리(S1+R1+S2+T4)를 본 sub-phase에서 해소. S6(Knowledge IPC)은 영향 범위가 커서 #31로 분리.

## 1. 결정 요약

- **D1**: Tauri 2 CSP 헤더를 `tauri.conf.json#app.security.csp`에 9개 directive로 명시. 비어 있던 webview default 정책을 explicit allowlist로 치환.
- **D2**: `capabilities/main.json`의 `shell:allow-open` scope를 `https://**` + `http://**`(전체 인터넷)에서 4개 도메인(GitHub / HF / jsdelivr / lmstudio.ai)으로 좁힘.
- **D3**: portable import 경로를 `resolve_import_target()` pure function으로 canonicalize + boundary 검증. workspace 외부 임의 디렉터리 삭제 가능성 차단.
- **D4**: `default_conflict_policy()`를 `Overwrite`(자동 wipe) → `Rename`(자동 suffix)으로 전환. 사고 surface 0.
- **D5**: `PortableApiError::PathDenied { reason }` thiserror variant 추가, kebab-case `kind: "path-denied"`. 한국어 메시지 + i18n 친화 분리.
- **D6**: 회귀 가드 7건을 `cfg(test)` 영역에 추가 (None / 빈 / subdir / `..` / 절대 외부 / 제어 문자 / kebab serialization).
- **D7**: ADR-0052 단일 ADR로 4개 결정(CSP + shell + path + Rename) 묶음. 별개 ADR 분기 X.
- **D8**: S6(Knowledge IPC tokenized path)는 sub-phase #31로 *분리* — 본 R-A 범위 외.

## 2. 채택안

### D1 — CSP 9 directive 명시

```json
{
  "default-src": "'self'",
  "script-src": "'self' 'wasm-unsafe-eval'",
  "style-src": "'self' 'unsafe-inline'",
  "img-src": "'self' data: blob: asset: http://asset.localhost",
  "font-src": "'self' data:",
  "connect-src": "'self' ipc: http://ipc.localhost http://127.0.0.1:* ws://127.0.0.1:*",
  "frame-src": "'none'",
  "object-src": "'none'",
  "base-uri": "'self'",
  "form-action": "'none'"
}
```

- `'wasm-unsafe-eval'`은 dynamic-subset Pretendard 폰트 디코더(WOFF2 + WASM) 한정.
- `'unsafe-inline'`은 design-system tokens.css가 inject하는 `--*` CSS 변수 + body inline style 한정 (XSS surface는 `default-src 'self'` + IPC 분리로 차단).
- `connect-src`의 `127.0.0.1:*` + `ws://127.0.0.1:*`는 자체 게이트웨이(Axum) + WebSocket 한정 — *외부 호스트 connect 0*.

### D2 — shell:allow-open scope 화이트리스트

```json
"allow": [
  { "url": "https://github.com/**" },
  { "url": "https://huggingface.co/**" },
  { "url": "https://cdn.jsdelivr.net/**" },
  { "url": "https://lmstudio.ai/**" }
]
```

- 4개로 v1 충족: GitHub Issue 신고(ADR-0049) + HF 모델 카드 + Pretendard CDN + LM Studio 다운로드.
- 추가 도메인은 ADR 후속에서 명시적으로.

### D3 — `resolve_import_target()` pure function

```rust
pub(crate) fn resolve_import_target(
    workspace_base: &Path,
    requested: Option<&str>,
) -> Result<PathBuf, PortableApiError> {
    let base_canon = workspace_base.canonicalize().map_err(...)?;
    match requested {
        None | Some("") => return Ok(base_canon),
        Some(s) if s.chars().any(|c| c == '\0' || c.is_control()) => {
            return Err(PathDenied { reason: "..." });
        }
        ...
    }
    let final_path = parent.canonicalize()?.join(file_name);
    if !final_path.starts_with(&base_canon) {
        return Err(PathDenied { reason: "workspace 디렉터리 밖..." });
    }
    Ok(final_path)
}
```

- `start_workspace_import`이 raw `target_workspace_root`를 직접 사용하지 않음 — 항상 resolve를 거침.
- canonicalize는 symlink resolved path 기준 → `..` + symlink 우회 모두 차단.
- 부모 canonicalize는 *대상 디렉터리 미존재* 케이스(아카이브 import) 대응.

### D4 — ConflictPolicy 기본 Rename

```rust
fn default_conflict_policy() -> ConflictPolicy {
    ConflictPolicy::Rename
}
```

- 사용자가 명시적으로 Overwrite 보낼 때만 wipe. 기본은 항상 진행 + 자동 suffix.
- frontend Drawer는 기본 select가 Rename이라 UX 회귀 0.

### D5 — `PathDenied` variant + kebab-case

```rust
#[error("workspace 밖 경로에는 가져올 수 없어요: {reason}")]
PathDenied { reason: String },
```

- `serde(tag = "kind", rename_all = "kebab-case")` → JSON `{ "kind": "path-denied", "reason": "..." }`.
- frontend는 `kind: "path-denied"` switch + i18n 키 `errors.path-denied`로 한국어 노출(Phase R-D에서 마무리).

### D6 — 회귀 invariant 7건

`portable.rs` `cfg(test)` mod에 추가:

| 테스트 | 의미 |
|---|---|
| `default_conflict_policy_is_rename` | D4 회귀 가드 |
| `resolve_import_target_none_returns_workspace_base` | None → active workspace |
| `resolve_import_target_empty_string_returns_workspace_base` | whitespace trim |
| `resolve_import_target_accepts_subdir` | 정상 subdir |
| `resolve_import_target_rejects_parent_traversal` | `..` 우회 거부 |
| `resolve_import_target_rejects_absolute_outside` | 절대 경로 외부 거부 |
| `resolve_import_target_rejects_control_chars` | `\0` / `\n` 거부 |
| `portable_api_error_path_denied_kebab_serialization` | kebab `kind` round-trip |

### D7 — ADR-0052 단일 결정 노트

CSP + shell + path 3개 경계는 *동일 본질(Tauri IPC 경계)*이라 ADR 분기 X.

### D8 — S6 분리

Knowledge IPC `IngestConfig.store_path` 제거 + `workspace_data_dir` 자동 도출 + `selected_path_token` registry는:

- 8+ IPC 영향 (ingest / list / search / delete / update).
- frontend Drawer + Settings 변경 동반.
- Phase R-A 범위(ship-blocker 신속 해소)와 어긋남.
- → sub-phase #31로 분리. R-A 후속에서 진행.

## 3. 기각안 + 이유

| # | 기각안 | 이유 |
|---|---|---|
| 1 | CSP를 `default-src 'self'` 단일 directive로 최소화 | data URL 인라인 이미지(아바타) + WASM 폰트 디코더 + IPC custom protocol 모두 막힘. 9개 directive로 explicit가 audit 친화도 더 높음 |
| 2 | shell:allow-open scope 빈 배열 | GitHub Issue 신고 / HF 모델 카드 등 사용자향 외부 링크 다수 깨짐. UX 회귀 |
| 3 | path validation을 frontend에서 처리 | Tauri IPC frontend trust X 원칙. backend resolve_import_target = single source of truth |
| 4 | canonicalize 없이 `starts_with(workspace_base)` 만 사용 | symlink로 외부 디렉터리 가리키는 케이스 우회 가능. canonicalize는 OS-level resolved path = symlink 우회 불가 |
| 5 | ConflictPolicy 기본을 Skip | 진행 멈춤 → 사용자 혼란("왜 import 안 됐지?"). Rename은 *항상 진행* + 자동 suffix → 학습 비용 0 |
| 6 | PathDenied를 `Disk` 또는 `ImportFailed`에 통합 | kebab-case `kind`로 frontend switch 안 됨. 명시 variant + `kind: "path-denied"` = audit 친화 + i18n 메시지 분리 |
| 7 | resolve_import_target을 portable-workspace crate로 이동 | workspace_root는 Tauri State이므로 desktop crate 종속. 단위 테스트는 desktop crate `cfg(test)`로 충분 |
| 8 | S6(Knowledge IPC) 동시 처리 | 8+ IPC + frontend 전반 변경 = 별개 sub-phase. R-A는 ship-blocker만 빠르게 해소 |
| 9 | CSP를 `meta` 태그로 frontend index.html 주입 | webview load 후 적용 → race condition. tauri.conf.json native header가 신뢰성 높음 |
| 10 | shell scope를 capability v1 형태(`scope.allowed`)로 유지 | v2 ACL 표준 위반. 4개 도메인 union을 v2 `allow: [{ url }]` 배열로 |

## 4. 미정 / 후순위 이월

- **i18n 키 `errors.path-denied`** — frontend Drawer가 PortableApiError를 한국어 토스트로 노출하는 부분은 Phase R-D(K1+K2+K3 i18n 마무리)에서 처리. 현재 `error.toString()`만 표시(한국어 메시지 자체는 thiserror에 들어 있어 표시는 됨, 다만 i18n 키 미정).
- **Knowledge IPC tokenized path (S6)** — sub-phase #31로 분리. R-A 후속에서 진행.
- **shell scope 추가 도메인** — Stripe / Telegram / Discord 등 사용자 요청 시 ADR 후속.
- **CSP report-uri** — 위반 리포팅은 v1.x. 현재는 console만.
- **lmmaster-desktop crate 단위 테스트 Windows DLL 한계** — `STATUS_ENTRYPOINT_NOT_FOUND` (0xc0000139)은 Tauri 2 + Windows + plugin-shell DLL 의존성. resolve_import_target 자체는 pure function이므로 컴파일 검증 + portable-workspace 통합 테스트 38건이 회귀 보호. 별도 cargo-nextest 도입은 v1.x.

## 5. 테스트 invariant

본 sub-phase가 깨면 안 되는 invariant 목록:

1. **CSP `default-src 'self'`** — 외부 호스트 script/style 자동 차단. webview console에서 CSP violation 0.
2. **shell:allow-open 4 도메인** — `tauri-plugin-shell::open` 호출이 화이트리스트 외 URL이면 ACL 거부. capability 파일 union 외 추가 X.
3. **`resolve_import_target` boundary** — workspace_base 외부 어떤 경로도 통과 X. `..` / 절대 경로 / 제어 문자 / symlink 모두 거부.
4. **ConflictPolicy default = Rename** — `serde(default)` 호출 시 항상 Rename. Overwrite는 명시 시에만.
5. **PathDenied kebab `kind`** — `serde_json::to_value` 결과 `kind == "path-denied"`. frontend switch 호환.
6. **portable-workspace 통합 테스트 38건** — export/import round-trip + cancel + sha256 회귀 0 (별개 crate, 본 변경에서 영향 없음 확인됨).
7. **clippy `-D warnings` lmmaster-desktop** — 본 변경 후 0 warning.

## 6. 다음 페이즈 인계

### 진입 조건

- ✅ R-A.1 (CSP + shell scope) 완료
- ✅ R-A.2 (path boundary code) 완료
- ✅ R-A.4 (path boundary tests) 완료
- ✅ R-A.5 (ADR-0052 + 결정 노트) 완료
- ⏳ commit + push (사용자 승인 대기)

### 의존성

- **Phase R-B** (Catalog Trust Pipeline) — S3 SQLCipher feature gate + S4 cache poisoning 방지 + S5 catalog signed fetch + R4 release workflow + T2 minisign round-trip. ADR-0053 + ADR-0054 후보.
- **Phase R-C** (Network + Correctness) — S7 reqwest no_proxy + allowlist + C1 chat_stream EOF + R3 Client::new() fallback + C3 installer URL filename validation.
- **Phase R-D** (Frontend Polish) — K1+K2+K3 i18n emoji 제거 + Catalog hardcoded fallback + thiserror Korean + `errors.path-denied` 키 추가.
- **Phase R-E** (Architecture v1.x) — A1 chat protocol decoupling + A2 bench trait + C2 OpenAI compat 공통화 + P1 KnowledgeStorePool + P4 channel cancel + R2 cancellation token + T3 wiremock — POST v0.0.1 release.
- **#31 Knowledge IPC tokenized path** — R-A 분리분. workspace_data_dir 자동 도출 + token registry. 별개 sub-phase.

### 위험 노트

- **CSP `'unsafe-inline'` style** — design-system tokens.css가 `<style>` 인라인이라 필수. XSS surface는 `default-src 'self'` + IPC 분리로 차단되지만, 향후 nonce-based로 강화 가능.
- **canonicalize 디렉터리 미존재** — 아카이브 import는 `target_workspace_root`가 *새 디렉터리*일 수 있음. 부모 canonicalize fallback으로 처리 — 현재 구현 OK.
- **lmmaster-desktop unit test Windows DLL** — Tauri 2 plugin-shell이 webview2 + shell32.dll에 dynamic link → cargo test 시 entrypoint not found. 컴파일 + clippy + portable-workspace 통합 테스트로 회귀 보호. cargo-nextest 도입은 v1.x.

### 다음 standby

**Phase R-B.1** (S3 SQLCipher feature gate) — `crates/knowledge-stack/Cargo.toml` + `crates/key-manager/Cargo.toml`에 `sqlcipher = ["rusqlite/bundled-sqlcipher"]` feature 추가, 기본 OFF (CI에서만 ON), build.rs warning. ADR-0035 인용.
