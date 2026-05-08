# Phase R-F.3 — IPC selected_path_token Registry 결정 노트

> **상태**: 채택 (2026-05-08, v0.3.2 직후)
> **선행 의존성**: Phase R-F+R-G+R-H+R-I+R-J 모두 종결 (`commit 2ea43ec` 직후 v0.3.2 release).
> **다음 페이즈**: v0.3.3 release tag (R-F.3 종결) → DEFERRED 일반.
> **결정 일자**: 2026-05-08
> **보강 리서치**: `agentId a22558a7df6a43361` (1500단어).

---

## 1. 결정 요약

GPT Pro 검수 리포트(2026-05-07) critical로 분류된 IPC raw filesystem path 표면을 *selected_path_token registry*로 전환. R-F+R-G hotfix에서 HIGH 재분류 + deferred. 이제 별도 sub-phase로 처리.

| 변경 | 영역 | Effort |
|---|---|---|
| `tauri-plugin-dialog 2.x` 도입 | plugin 등록 + capability scope `dialog:allow-open` | 30m |
| `path_tokens.rs` 신규 | `Arc<RwLock<HashMap<TokenId, PathBuf>>>` + UUID v4 + 24h soft TTL | 1h |
| `issue_path_token` IPC 신규 | dialog plugin 결과 → token 발급 + canonicalize | 30m |
| 영향 IPC 3개 변경 | `ingest_path` / `workbench_preview_jsonl` / `WorkbenchConfig.data_jsonl_path` | 2h |
| frontend ipc + UI | `ipc/path-tokens.ts` + Workbench/Workspace UI (input → 파일 선택 button) | 1.5h |
| i18n `screens.common.pathPicker.*` 7키 | ko/en parity | 30m |
| 테스트 invariant | Rust +9 + vitest +4 | 1h |

**총 effort 약 7h** — v0.3.3 release.

---

## 2. 채택안

### 2.1 Plugin + Registry 도입

**Cargo.toml**:
```toml
tauri-plugin-dialog = "2"
```

**package.json**:
```json
"@tauri-apps/plugin-dialog": "^2"
```

**capabilities/main.json** permissions:
```json
"dialog:allow-open",
"allow-issue-path-token"
```

**lib.rs**:
- `.plugin(tauri_plugin_dialog::init())` (shell plugin 다음).
- `mod path_tokens;` + `app.manage(PathTokenRegistry::new())`.
- `invoke_handler!`에 `path_tokens::issue_path_token` 추가.

### 2.2 PathTokenRegistry 설계

`apps/desktop/src-tauri/src/path_tokens.rs` 신규:

```rust
pub struct PathTokenRegistry {
    inner: RwLock<HashMap<String, TokenEntry>>,
}

pub struct TokenEntry {
    canonical_path: PathBuf,
    kind: PathTokenKind, // File / Directory
    issued_at: Instant,
}

const TOKEN_TTL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, thiserror::Error)]
pub enum PathTokenError {
    #[error("선택한 파일을 찾을 수 없어요. 다시 선택해 주세요.")]
    Unknown,
    #[error("파일 선택이 만료됐어요. 다시 선택해 주세요.")]
    Expired,
    #[error("선택한 경로가 허용 범위를 벗어났어요.")]
    OutOfScope,
}
```

**채택 근거**:
- **Tokio RwLock**: read >> write 비중 (발급 1 + 매 IPC lookup N).
- **UUID v4 36-char**: 122 bit entropy로 collision 사실상 0.
- **24h soft TTL**: process 수명 fallback. localStorage 캐시 금지.
- **canonical_path 저장**: dialog 선택 직후 canonicalize → token 발급. lookup은 그대로 PathBuf 반환.

### 2.3 issue_path_token IPC

```rust
#[tauri::command]
pub async fn issue_path_token(
    path: String,
    kind: String,
    registry: State<'_, Arc<PathTokenRegistry>>,
) -> Result<String, PathTokenError> {
    let canonical = std::path::PathBuf::from(&path).canonicalize()
        .map_err(|_| PathTokenError::OutOfScope)?;
    let kind = match kind.as_str() {
        "file" => PathTokenKind::File,
        "directory" => PathTokenKind::Directory,
        _ => return Err(PathTokenError::OutOfScope),
    };
    Ok(registry.issue(canonical, kind).await)
}
```

### 2.4 IPC 3개 변경

**knowledge.rs::IngestConfig** — `path: String` → `path_token: String`:
```rust
pub struct IngestConfig {
    pub workspace_id: String,
    pub path_token: String,  // 기존 path → path_token
    ...
}
```

`ingest_path` 진입 시 `path_tokens.resolve(&config.path_token)` → `PathBuf`. 그 후 기존 로직 그대로.

**workbench.rs::workbench_preview_jsonl(path)** → `workbench_preview_jsonl(path_token)`. 동일 패턴.

**WorkbenchConfig.data_jsonl_path** → `data_jsonl_path_token: String`.

### 2.5 Frontend

`apps/desktop/src/ipc/path-tokens.ts` 신규:
```typescript
export async function pickJsonlFile(): Promise<string | null> { ... }
export async function pickDirectory(): Promise<string | null> { ... }
```

**Workbench.tsx UI**: data_jsonl_path 입력 — `<input type="text">` → `<button>파일 선택할게요</button> <span>{selectedFileName}</span>`.

**Workspace.tsx (ingest UI)**: 동일 패턴.

### 2.6 i18n 카피 (해요체)

`screens.common.pathPicker.*`:
- `selectFile`: "파일 선택할게요" / "Select file"
- `selectDirectory`: "폴더 선택할게요" / "Select directory"
- `noFileSelected`: "아직 선택한 파일이 없어요" / "No file selected"
- `selectedFile`: "선택한 파일: {{name}}" / "Selected: {{name}}"
- `tokenExpired`: "파일 선택이 만료됐어요. 다시 선택해 주세요." / "Selection expired — please pick again."
- `tokenUnknown`: "선택한 파일을 찾을 수 없어요. 다시 선택해 주세요." / "Selection not found — please pick again."
- `dialogFailed`: "파일 선택 창을 열지 못했어요" / "Could not open file picker"

ko/en parity 1063 → 1070.

---

## 3. 기각안 + 이유 (negative space)

| # | 거부된 대안 | 사유 |
|---|---|---|
| 1 | **`std::sync::Mutex` 또는 `parking_lot::RwLock`** | tokio runtime 안에서 호출되니 `tokio::sync::RwLock`이 정합. async-aware lock |
| 2 | **8-byte nonce token** | 65k 발급 시 ~1% collision (birthday paradox). UUID v4 122 bit이 안전 |
| 3 | **process 영구 + revoke API 제공** | 24h TTL + lazy sweep으로 충분. revoke는 사용처 0 |
| 4 | **frontend localStorage token 캐시** | token 영구화 시 process restart 후 dangling pointer. React state-only |
| 5 | **raw path field와 path_token 동시 지원 (step migration)** | raw path 잔류 = 공격 surface 그대로. exclusive token-only가 정공. frontend 함께 변경 |
| 6 | **dialog 결과를 backend가 직접 token으로 변환 (frontend bypass)** | Tauri dialog plugin은 frontend에서 호출. backend는 결과 path만 받아 token 발급 |
| 7 | **Workbench text input 보존 (power user fallback)** | 보안 surface 그대로 + UX 분기 부담. v2.x 검토 |
| 8 | **Token TTL을 1h / 7일** | 1h: ingest 진행 중 만료 위험. 7일: registry 메모리 누적. 24h가 균형 |
| 9 | **dialog cancel 시 explicit "취소했어요" 토스트** | 과한 UX. `null` 반환 → frontend 그대로 (mutation 0)이 자연 |
| 10 | **Tauri Mobile 호환성 본 sub-phase** | iOS/Android는 photopicker URI scheme(`content://`) — 임의 fs path 미반환. desktop-only 한정, mobile은 v2.x ADR |

---

## 4. 미정 / 후순위 이월

| 항목 | 진입 조건 |
|---|---|
| **Tauri Mobile dialog 호환** | v2.x mobile 진입 시 별도 ADR |
| **Rust Mutex deadlock detector** | 본 sub-phase 머지 후 reinforce |
| **Token sweep cron** (lazy → eager) | 메모리 사용 누적 모니터링 후 |
| **`save` / `confirm` dialog** | 사용처 등장 시. 현재 0 |
| **token-aware ACL drift checker** | CI script 추가 — v0.4.x |

---

## 5. 테스트 invariant

### Rust (`path_tokens.rs::tests`)

| invariant | 카운트 |
|---|---|
| `issue` → `resolve` round-trip canonical path 일치 | +1 |
| 만료 token (issued_at mock back-date) → `Expired` | +1 |
| 미발급 token → `Unknown` | +1 |
| `revoke` 후 `resolve` → `Unknown` (idempotent) | +1 |
| concurrent issue + resolve (`tokio::join!` 100 tasks) | +1 |
| UUID v4 collision 0 (10k 발급) | +1 |
| `issue_path_token` IPC accept file kind | +1 |
| `issue_path_token` IPC accept directory kind | +1 |
| `issue_path_token` IPC reject unknown kind | +1 |

**총 Rust +9**.

### Vitest

| invariant | 카운트 |
|---|---|
| `pickJsonlFile` mock dialog string 반환 시 token 발급 IPC 호출 | +1 |
| `pickJsonlFile` mock null 반환 시 graceful (no IPC call) | +1 |
| Workbench step button 클릭 → selectedFileName 노출 | +1 |
| 만료 응답 시 한국어 카피 노출 | +1 |

**총 vitest +4**.

**누적 차분 +13** (v0.3.3 release notes).

---

## 6. 다음 페이즈 인계

### 6.1 v0.3.3 release 흐름
1. plugin 도입 + path_tokens.rs + lib.rs 등록.
2. IPC 3개 변경 + frontend ipc + UI.
3. i18n + 테스트.
4. 검증 + commit + tag v0.3.3 + push.

### 6.2 위험 + 함정

- **Linux GTK 의존**: ubuntu-22.04 runner에 `libgtk-3-dev` + `libwebkit2gtk-4.1-dev` 이미 설치(release.yml). 미설치 환경 빌드 시 친절 에러.
- **frontend localStorage token 캐시 금지**: React state-only.
- **24h TTL과 진행 중 ingest**: kickoff에 1회 resolve → in-memory PathBuf 보유 → TTL 영향 0.
- **dialog cancel**: `open()` `null` 반환 → frontend 그대로.
- **Tauri Mobile photopicker**: desktop-only, mobile은 v2.x.

### 6.3 DEFERRED.md 갱신
- §16-19 "selected_path_token registry v2.x → v0.3.x 승격" 항목 *완결*로 표시 (이번 sub-phase로 종결).
- v0.4.x deferred: token-aware ACL drift checker / `save`+`confirm` dialog 확장.

---

**문서 버전**: v1.0 (2026-05-08, Phase R-F.3 1차 작성).

**참조**:
- 보강 리서치: agent `a22558a7df6a43361` (1500단어, 8 영역).
- ADR-0052 §S6 (selected_path_token registry).
- ADR-0064 §F.3 (deferred 항목 종결).
- Phase R-F+R-G 결정 노트.
