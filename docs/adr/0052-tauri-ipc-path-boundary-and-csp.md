# ADR-0052 — Tauri IPC 경로 경계(workspace boundary) + CSP 강화

* **상태**: Accepted (2026-05-03). Phase R-A 머지와 함께 적용.
* **선행**: ADR-0009 (portable workspace 정책 — workspace_root 단일 source). ADR-0039 (export/import 정책 — `target_workspace_root: PathBuf` 직접 노출). ADR-0036 (single-instance + WAL — 부팅 안전 가드).
* **컨텍스트**: 2026-05-02 GPT Pro 정적 검수에서 v0.0.1 ship-blocker 4건이 보안 카테고리에서 식별됐다.
  1. **S1**: `apps/desktop/src-tauri/tauri.conf.json`의 `app.security.csp` 키가 비어 있음 — `default-src` / `script-src` / `connect-src` 등 CSP 헤더가 webview에 주입되지 않음 (XSS 시 IPC 임의 호출 가능).
  2. **R1**: `capabilities/main.json`의 `shell:allow-open` scope가 `https://**` + `http://**`로 *전체 인터넷* — open(`file:///c:/Windows/System32/cmd.exe`) 같은 케이스를 허용해 RCE surface가 광범위.
  3. **S2**: `start_workspace_import(req.target_workspace_root)`이 `Option<String>`을 그대로 `PathBuf`로 변환 → ImportSink가 `remove_dir_all` 호출 시 *workspace 외부 임의 디렉터리 삭제* 가능. ConflictPolicy 기본이 `Overwrite`라 일치 시 자동 wipe.
  4. **S6**: Knowledge IPC가 raw `path: String`을 받아 같은 형태의 임의 경로 노출. (별개 sub-phase #31로 분리 — 본 ADR 범위 외)
* **결정 노트**: `docs/research/phase-r-a-security-boundary-decision.md`

## 결정

1. **CSP 헤더 명시 주입** — `tauri.conf.json#app.security.csp`에 9개 directive 명시:
   ```
   default-src 'self'
   script-src 'self' 'wasm-unsafe-eval'
   style-src 'self' 'unsafe-inline'
   img-src 'self' data: blob: asset: http://asset.localhost
   font-src 'self' data:
   connect-src 'self' ipc: http://ipc.localhost http://127.0.0.1:* ws://127.0.0.1:*
   frame-src 'none'
   object-src 'none'
   base-uri 'self'
   form-action 'none'
   ```
   `'wasm-unsafe-eval'`은 동적 폰트 서브셋(Pretendard) + WASM 디코더용. `'unsafe-inline'`은 styled-components/emotion이 아닌 네이티브 CSS 변수 토큰 system 한정. `connect-src`의 `127.0.0.1:*` + `ws://127.0.0.1:*`는 자체 게이트웨이 + WebSocket 한정.
2. **shell:allow-open scope 화이트리스트화** — `capabilities/main.json`을 4개 도메인으로 좁힘:
   ```json
   "allow": [
     { "url": "https://github.com/**" },
     { "url": "https://huggingface.co/**" },
     { "url": "https://cdn.jsdelivr.net/**" },
     { "url": "https://lmstudio.ai/**" }
   ]
   ```
   GitHub Issue 신고 / HF 모델 카드 / Pretendard CDN / LM Studio 다운로드 4개로 v1 충족. 추가는 ADR 후속.
3. **portable import 경로 경계 강제** — 신규 `resolve_import_target(workspace_base, requested) -> Result<PathBuf, PathDenied>` 헬퍼:
   - None / 빈 문자열 → `workspace_base.canonicalize()` 그대로 (active workspace 복원 케이스)
   - 제어 문자(`\0`, `\n` 등) → `PathDenied`
   - 그 외 → `workspace_base.join(requested)` 후 부모 canonicalize → `final_path.starts_with(&base_canon)` 검증
   - 절대 경로는 `PathBuf::join`이 RHS로 대체하지만 boundary 검증에서 거부됨
   - `..` segment는 canonicalize 후 prefix 검증으로 거부됨
4. **ConflictPolicy 기본 = Rename** — 기존 `Overwrite`(존재 시 디렉터리 wipe)에서 `Rename`(타임스탬프 suffix)으로. 사용자가 명시적으로 Overwrite 보낼 때만 wipe. `default_conflict_policy()` 함수에 ADR-0052 주석 명시.
5. **`PortableApiError::PathDenied { reason: String }` 신규 variant** — thiserror + serde tagged enum(`kind: "path-denied"`). 한국어 메시지 "workspace 밖 경로에는 가져올 수 없어요". frontend는 `kind: "path-denied"` switch로 한국어 상세 안내.
6. **테스트 invariant 7건** — `cfg(test)` 영역에 path boundary 회귀 가드:
   - `default_conflict_policy_is_rename` (기본 Rename)
   - `resolve_import_target_none_returns_workspace_base` (None 케이스)
   - `resolve_import_target_empty_string_returns_workspace_base` (whitespace trim)
   - `resolve_import_target_accepts_subdir` (정상 subdir)
   - `resolve_import_target_rejects_parent_traversal` (`..` 거부)
   - `resolve_import_target_rejects_absolute_outside` (절대 경로 외부 거부)
   - `resolve_import_target_rejects_control_chars` (`\0` / `\n` 거부)
   - `portable_api_error_path_denied_kebab_serialization` (kebab-case `kind`)

## 근거

- **Tauri 2 CSP 정책**: Tauri 2 docs는 CSP를 *설정에서 명시*해야만 webview에 주입함을 명시. v1처럼 자동 주입 X. 비어 있으면 webview default(매우 느슨)이 적용 — XSS 통한 IPC 임의 호출 가능.
- **shell scope ACL**: capability v2에서 `shell:allow-open`은 URL pattern allowlist. `https://**` 같은 와일드카드는 *전체 https 인터넷* 의미. 디바이스 로컬 정책(외부 통신 0)과 모순.
- **canonicalize 기반 path boundary**: `starts_with` 단독은 symlink 우회 가능. `canonicalize()` 후 비교 = OS-level resolved path 동일성 보장. 부모 canonicalize는 *대상 디렉터리 미존재* 케이스 대응(아카이브 import는 새 디렉터리 생성).
- **ConflictPolicy Rename default**: 사용자가 archive 받자마자 클릭 → 기본값으로 워크스페이스 wipe 사고 0건 보장. Overwrite는 명시 의도 시에만.
- **테스트 invariant**: `resolve_import_target`은 pure function (Tauri runtime X) → 단위 테스트로 100% 커버 가능. lmmaster-desktop 크레이트의 Windows DLL 한계는 *테스트 *컴파일* 시 검증되며, 동일 로직이 portable-workspace 통합 테스트(38 passing)와 함께 회귀 보호.

## 거부된 대안

1. **CSP를 `default-src 'self'` 단일 directive로 최소화**: data URL 인라인 이미지(아바타) + WASM 폰트 디코더 + IPC custom protocol이 모두 막힘. 9개 directive로 explicit 정책이 audit 친화도 더 높음.
2. **shell:allow-open scope 빈 배열**: GitHub Issue 신고/HF 모델 카드 등 사용자향 외부 링크 다수 깨짐. UX 회귀.
3. **path validation을 frontend에서 처리**: Tauri IPC는 frontend trust X 원칙(웹에서 서버 입력 검증과 동일). backend resolve_import_target = single source of truth.
4. **canonicalize 없이 `starts_with(workspace_base)` 만 사용**: symlink로 외부 디렉터리 가리키는 케이스 우회 가능. canonicalize는 OS-level resolved path로 정규화 → symlink 우회 불가.
5. **ConflictPolicy 기본을 Skip**: 진행 멈춤 → 사용자 혼란("왜 import 안 됐지?"). Rename은 *항상 진행* + 자동 suffix → 학습 비용 0.
6. **PathDenied를 `Disk` 또는 `ImportFailed`에 통합**: kebab-case `kind`로 frontend switch 안 됨. 명시 variant + `kind: "path-denied"` = audit 친화 + i18n 메시지 분리.
7. **resolve_import_target을 portable-workspace crate로 이동**: workspace_root는 Tauri State이므로 desktop crate 종속. 단위 테스트는 desktop crate `cfg(test)`로 충분.
8. **Knowledge IPC 동시 처리(S6 → 본 ADR 범위)**: 별개 sub-phase #31로 분리. IngestConfig.store_path 제거 + workspace_data_dir 자동 도출 + selected_path_token registry = 8+ IPC + frontend 전반 변경. R-A 범위 좁히고 ship-blocker만 빠르게 해소.

## 결과 / 영향

- **`apps/desktop/src-tauri/tauri.conf.json`**: `app.security.csp` 9개 directive 명시. webview default → explicit 화이트리스트.
- **`apps/desktop/src-tauri/capabilities/main.json`**: `shell:allow-open` scope 4개 도메인으로 축소.
- **`apps/desktop/src-tauri/src/workspace/portable.rs`**:
  - `PortableApiError::PathDenied { reason }` variant 추가 (kebab `kind: "path-denied"`).
  - `resolve_import_target()` 헬퍼 추가 (pure function, 60 LOC).
  - `default_conflict_policy()` Overwrite → Rename.
  - `start_workspace_import` workspace_base + resolve_import_target 통합.
  - `cfg(test)` 영역에 path boundary invariant 7건 + 기존 4건.
- **외부 통신 0 정책**: 변경 없음 — 4개 화이트리스트는 사용자 *명시 클릭 시* `tauri-plugin-shell::open`로만, *백그라운드 outbound* 0 유지.
- **ACL drift**: capabilities/main.json 1회 좁힘 (확장 X).
- **백워드 호환**: `target_workspace_root: None` 케이스(active workspace 복원)는 그대로 작동. Overwrite 명시 사용자만 영향, frontend Drawer는 기본 select가 Rename이라 UX 회귀 0.
- **i18n**: `errors.path-denied` 키 frontend switch 시 추가 (별개 frontend sub-phase). 본 ADR은 backend 정책만.

## References

- 결정 노트: `docs/research/phase-r-a-security-boundary-decision.md`
- GPT Pro 검수: 2026-05-02 30-issue static review (S1/R1/S2/T4 4건 본 ADR로 해소)
- 코드:
  - `apps/desktop/src-tauri/tauri.conf.json` (CSP)
  - `apps/desktop/src-tauri/capabilities/main.json` (shell scope)
  - `apps/desktop/src-tauri/src/workspace/portable.rs` (PathDenied + resolve_import_target + Rename default)
- 관련 ADR: 0009 (portable workspace), 0036 (single-instance), 0039 (export/import 정책), 0047 (minisign — R-B에서 재참조)
- 후속 ADR 후보: 0053 (R-B 카탈로그 trust pipeline + SQLCipher feature gate), 0054 (R-C network policy + reqwest no_proxy)
