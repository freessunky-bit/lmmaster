# Phase 8' / 9' / 10' / 11' / 12' / Env' — 잔재 항목 작업 계획

> 작성: 2026-04-28, 갱신: 2026-04-29 (Phase 8'.0/8'.1/11'/12' 추가).
> 상태: 계획 수립 — 결정 노트 6-section 따름.
> 대상: `unimplemented_audit_2026_04_28.md`의 #4~#31 + 환경 이슈 (STATUS_ENTRYPOINT_NOT_FOUND).

## §0 클러스터링 원칙

기존 Phase 1A~7' 패턴 따라 **충돌 없는 단위로 sub-phase 분할**, **사용자 가치 ROI 순 정렬**, **외부 의존성(모델 파일·바이너리) 따라 그룹화**.

| 항목 | 외부 의존 | 작업 면적 | 위험 | 사용자 가치 |
|---|---|---|---|---|
| #14 get_document_path | 없음 | XS (~80 LOC) | 낮음 | 중 (검색 결과 UX) |
| #15 LRU cleanup | 없음 | XS (~50 LOC) | 낮음 | 낮음 |
| #16 last_checked | 없음 | XS (~40 LOC) | 낮음 | 낮음 |
| #22 dead key | 없음 | XS (~10 LOC) | 낮음 | 0 (정합성) |
| #8 listCustomModels | 없음 | S (~150 LOC) | 낮음 | 중 (Workbench → Catalog 흐름) |
| #13 tauri-plugin-shell | npm dep | S (~100 LOC) | 낮음 | 낮음 |
| #11 per-key Pipelines | KeyManager schema | M (~400 LOC) | 중 | 중 |
| #12 PromptSanitize | regex | S (~250 LOC) | 낮음 | 중 (PII 강화) |
| #4 Pipelines hot-reload | ArcSwap crate | M (~350 LOC) | 중 (race) | 높음 |
| #10 SSE chunk transform | bytes/SSE parser | L (~600 LOC) | 높음 (perf, byte-perfect 보존) | 중 |
| #5 Real Embedder | ONNX 모델파일 + ort crate | XL (~1500 LOC) | 높음 (모델 다운로드 + GPU) | **높음** (RAG 핵심) |
| #6 LlamaQuantizer / LLaMA-Factory | 외부 binary + Python venv | XL (~2000 LOC) | 높음 (subprocess + 환경) | **높음** (Workbench 핵심) |
| #9 multi-runtime adapter | 각 runtime HTTP 스펙 | L (~800 LOC) | 중 | 낮음 (사용자 layer 적음) |
| #7 GlitchTip submit | self-hosted endpoint | M (~300 LOC) | 낮음 | 낮음 (opt-in) |
| Env STATUS_ENTRYPOINT | Windows ApiSet | XS (코드 X, 문서) | 낮음 | 0 (개발 편의) |
| **#30 Portable export/import** | **zip + aes-gcm** | **L (~1300 LOC)** | **중 (cross-OS)** | **높음 (6 pillar 약속)** |
| **#31 Guide menu** | **react-i18next + 자체 markdown** | **M (~900 LOC)** | **낮음** | **높음 (사용자 친화)** |

## §1 페이즈 구조 (7 메이저 + 1 환경 + 1 출시 차단)

```
Phase 7'.a' — 출시 차단 사항 (사용자 결정)
└── 7'.a'.x — minisign pubkey 발급 (#25)

Phase 8'.0 — Security & Stability hardening (NEW, v1 ship 전 권장)
├── 8'.0.a — SQLCipher 활성 (#23)
├── 8'.0.b — Single-instance + panic hook + WAL (#26 + #27 + #28)
└── 8'.0.c — Workbench artifact retention (#29)

Phase 8'.1 — Multi-workspace (NEW)
├── 8'.1.a — Workspace 생성/전환 UX (#24)
└── 8'.1.b — workspace_id wire-up

Phase 11' — Portable Workspace export/import (NEW, v1 ship 전 권장 — 6 pillar 약속)
├── 11'.a — export pipeline (zip/tar + manifest + 무결성 sha256)
├── 11'.b — import pipeline (verify + unpack + repair tier 자동 진입)
└── 11'.c — Settings UI "이 PC로 가져오기" / "다른 PC로 옮기기"

Phase 12' — Guide / Help system (NEW, 사용자 친화 핵심)
├── 12'.a — Guide page (NAV "가이드" + 8 섹션 + 검색 + deep link)
├── 12'.b — In-page contextual help (헤더 ? 툴팁 + tour)
└── 12'.c — Keyboard shortcuts + first-run "둘러보기"

Phase 8' — Polish & Pipelines extension
├── 8'.a — Quick wins batch (#14, #15, #16, #22)
├── 8'.b — Workbench↔Catalog 흐름 (#8, #13)
└── 8'.c — Pipelines 확장 (#11, #12, #4, #10)

Phase 9' — Real ML wiring (1~2 세션)
├── 9'.a — Real Embedder (#5)
├── 9'.b — LlamaQuantizer/LLaMA-Factory CLI (#6)
└── 9'.c — Multi-runtime adapters (#9)

Phase 10' — Telemetry submit (#7)
                ↓ Phase 7'.b release 인프라 후 처리

Env' — 환경 이슈
└── Env'.a — STATUS_ENTRYPOINT_NOT_FOUND 우회 + CI 전략
```

> **v1 ship critical path**: 7'.a'.x → 8'.0 → 8'.1 → **11'** → **12'** → 출시 가능.
> Phase 11' / 12'는 6 pillar 약속(포터블) + 사용자 친화 핵심 — v1.x보다 우선.

---

## §1.5 Phase 7'.a' — 출시 차단 사항 (사용자 결정)

### 7'.a'.x — minisign pubkey 발급 (#25)

**시간**: 사용자 30분 + 메인 5분
**중요도**: ⛔ 출시 절대 차단 사항

**현 상태**: `tauri.conf.json` `plugins.updater.pubkey: "TODO_REPLACE_WITH_MINISIGN_PUBLIC_KEY"`. 이 placeholder로 출시 시 자동 업데이트 무결성 검증 불가 → supply-chain 공격 위험.

**해결 단계**:
1. 사용자 (관리자 PowerShell):
   ```powershell
   cd C:\Users\wind.WIND-PC\Desktop\VVCODE\LMmaster\apps\desktop
   pnpm exec tauri signer generate -w "$env:USERPROFILE\.tauri\lmmaster.key"
   # 비밀번호 강한 것으로 (vault 보관)
   # public key는 stdout에 출력됨
   ```
2. 메인:
   - `tauri.conf.json` `plugins.updater.pubkey` placeholder → 실 public key 교체.
   - `.gitignore`에 `*.key` 확정 (private key 절대 커밋 금지).
   - GitHub Actions secret (`TAURI_SIGNING_PRIVATE_KEY` + `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) 안내 docs.

**negative space**:
- 자체 호스팅 PKI — 운영 부담. minisign이 Tauri 권장 + 가벼움.
- 미서명 출시 — supply-chain 공격에 무방비.

**테스트 invariant**:
- placeholder 그대로면 `pnpm tauri build`가 명확한 에러 (Tauri 2가 검증).
- 실 키 적용 후 build 성공 + `latest.json`에 signature 포함.

**DoD**:
- pubkey 교체 완료.
- `tauri build` 성공 + signature artifact 생성.
- 사용자가 private key 안전 보관 + 비밀번호 vault 등록.

---

## §1.6 Phase 8'.0 — Security & Stability hardening (NEW, v1 ship 전 권장)

> **시간**: 보강 리서치 1 + 구현 1~2 sub-agent / ~3~4시간
> **중요도**: 🔴 신뢰성 + 보안 — v1 사용자가 처음 인지하기 전에 처리.
> **충돌**: 8'.0.a / 8'.0.b / 8'.0.c 서로 다른 파일 → 병렬 가능. lib.rs 동시 편집 주의.

### 보강 리서치 (1건)

`docs/research/phase-8p0-security-stability-reinforcement.md` (~400 LOC, 12+ 인용):

1. **SQLCipher** — rusqlite `bundled-sqlcipher-vendored-openssl` feature, `PRAGMA key`, 마이그레이션 정책 (기존 평문 DB → 암호화 DB).
2. **Tauri 2 single-instance plugin** — `@tauri-apps/plugin-single-instance` + `tauri_plugin_single_instance::init`, 기존 인스턴스에 신호 전달 (창 포커싱).
3. **Rust panic hook** — `std::panic::set_hook` + 한국어 메시지 + Tauri 다이얼로그 + telemetry 큐 적재.
4. **SQLite WAL mode** — `PRAGMA journal_mode=WAL`, busy_timeout, sync mode trade-off.
5. **Workbench artifact retention** — TTL / size-based eviction, user-visible 정리 UI.

### 8'.0.a — SQLCipher 활성 (#23)

**설계**:
- `crates/key-manager/Cargo.toml`: `rusqlite = { workspace = true, features = ["bundled-sqlcipher-vendored-openssl"] }` (workspace dep는 features 추가).
- `crates/key-manager/src/store.rs`: `KeyManager::open(path, key_passphrase)` 시그니처 확장. `PRAGMA key = '<passphrase>'` 첫 connection statement.
- 키 passphrase는 **OS 키체인** (`keyring` crate, 이미 workspace dep) 저장. 첫 실행 시 random 32-byte secret 생성 → 키체인 저장 → 이후 재사용.
- 기존 평문 DB가 있으면: 마이그레이션 wizard. 사용자에게 "기존 데이터 암호화할게요" dialog → ATTACH + 복사 + 원본 삭제.
- Settings에 "키 저장소 암호화" 상태 표시 (활성/비활성).

**파일**:
- `crates/key-manager/src/store.rs` 수정 (~80 LOC) + 6 신규 tests.
- `apps/desktop/src-tauri/src/lib.rs`: KeyManager open 시 keyring 사용.
- `apps/desktop/src-tauri/src/keys/migrate.rs` 신설 (~120 LOC) — 평문→암호화 마이그레이션.
- ADR-0035 (SQLCipher activation) 신설.

**negative space**:
- 사용자 입력 비밀번호 — UX 마찰 (초기 설정 + 매 실행 입력?). OS 키체인 자동화가 우월.
- 평문 DB 유지 — 보안 약속 위반.
- 외부 KMS — 외부 통신 0 위반.

**테스트 invariant**:
- 새 DB 생성 시 SQLCipher 헤더 자동 적용 (`hexdump`로 검증 가능).
- 잘못된 passphrase로 open 시 명확한 에러.
- 평문 DB 마이그레이션 성공 + 원본 정리.
- 키체인 미접근 시 graceful degrade ("암호화 비활성" 모드 + 사용자 경고).

### 8'.0.b — Single-instance + panic hook + WAL (#26 + #27 + #28)

**설계**:

**Single-instance**:
- `apps/desktop/src-tauri/Cargo.toml`: `tauri-plugin-single-instance = "2"`.
- `apps/desktop/package.json`: `@tauri-apps/plugin-single-instance ^2.0.0`.
- `lib.rs`: `Builder::default().plugin(tauri_plugin_single_instance::init(...))` 등록. 이미 실행 중이면 기존 창 포커스 + 새 인스턴스 종료.
- `capabilities/main.json`: 권한 추가.

**Panic hook**:
- `apps/desktop/src-tauri/src/panic_hook.rs` 신설:
   ```rust
   pub fn install() {
       std::panic::set_hook(Box::new(|info| {
           // 1. tracing::error 기록.
           // 2. crash report 파일 작성 (app_data_dir/crash/).
           // 3. telemetry 큐 적재 (opt-in 시).
           // 4. Tauri runtime이 살아있으면 한국어 다이얼로그 표시.
       }));
   }
   ```
- `lib.rs::run()` 첫 줄에 `panic_hook::install()`.

**WAL mode**:
- `crates/key-manager/src/store.rs`: open 후 `PRAGMA journal_mode=WAL` + `PRAGMA busy_timeout=5000`.
- `crates/knowledge-stack/src/store.rs`: 동일.
- `crates/model-registry/src/register.rs`: 동일.
- `apps/desktop/src-tauri/src/commands.rs::CatalogState`: SQLite 사용처 동일.

**파일**:
- `apps/desktop/src-tauri/src/panic_hook.rs` 신설 (~150 LOC).
- 4 crate × `PRAGMA journal_mode=WAL` 1줄씩 추가 + 테스트 보강.
- `apps/desktop/src-tauri/src/lib.rs` plugin 등록 + panic_hook install.
- `i18n` `dialogs.crash.*` keys.
- ADR-0036 (Single-instance + panic hook + WAL) 신설.

**negative space**:
- panic hook가 panic 자체 → 무한 재귀. `set_hook`에서 자체 panic 방어 (`catch_unwind`).
- WAL은 NFS / 네트워크 드라이브에서 동작 안 함. 사용자 PC 로컬 가정 — 문제 X.

**테스트 invariant**:
- 두 번째 실행 시 첫 인스턴스 윈도우 활성 + 두 번째 즉시 종료.
- panic 시 crash 파일 생성 + 다이얼로그 (Tauri runtime up이면).
- WAL 활성 후 SQLite 성능 회귀 X (write 속도 측정).

### 8'.0.c — Workbench artifact retention (#29)

**설계**:
- `crates/workbench-core/src/artifact_retention.rs` 신설:
  - `RetentionPolicy { max_age_days: u32, max_total_size_bytes: u64 }`.
  - `cleanup(workspace_dir, policy) -> CleanupReport`.
  - LRU + TTL 둘 다 지원 (oldest first 삭제 + N일 이상은 자동 삭제).
- `apps/desktop/src-tauri/src/workbench.rs`: 매 run 끝에 `cleanup_old_artifacts` 호출.
- Settings에 "Workbench 임시 파일" 패널: 현재 사용량 + "지금 정리할게요" 버튼 + retention 정책 UI.

**파일**:
- `crates/workbench-core/src/artifact_retention.rs` 신설 (~250 LOC) + 8 unit tests.
- `apps/desktop/src-tauri/src/workbench.rs` 수정.
- `apps/desktop/src/components/WorkbenchArtifactPanel.tsx` 신설 + test.
- i18n `screens.settings.workbench.artifacts.*`.

**테스트 invariant**:
- 30일 이상 artifact 자동 삭제 (mock filesystem).
- max_total_size 초과 시 oldest부터 삭제.
- 진행 중 run의 artifact는 보존.

### 8'.0 전체 검증 (DoD)

- cargo test (전 crate) ✅
- vitest run ✅
- 보안 검증: `keyring`에 키 저장됨 + DB 파일 hex dump에 SQLCipher 헤더.
- 안정성 검증: 두 번째 실행 시 single-instance, panic 강제 발생 시 crash report 생성, SQLite WAL 활성.
- ADR-0035 / 0036 / 0037 (artifact retention) 신설.

---

## §1.7 Phase 8'.1 — Multi-workspace UX (NEW)

> **시간**: 보강 리서치 0.5 + 구현 1 sub-agent / ~3~4시간
> **중요도**: 🔴 ADR-0024 약속 실현. RAG / Workbench의 "per-workspace 격리"가 UI 레벨에서 의미 있게 동작하려면 필수.

### 보강 리서치 (간단)

기존 `phase-4p5-rag-decision.md` + `phase-5p-workbench-decision.md` 재참조. 별도 보강 리서치 노트 X.

### 8'.1.a — Workspace 생성/전환 UX (#24)

**설계**:
- 사이드바 상단 또는 별도 드롭다운에 "워크스페이스: <현재명>" + "▾" 클릭 시 list + "새 워크스페이스 만들기".
- 새 워크스페이스: 이름 + (옵션) 설명. UUID 발급. SQLite에 `workspaces` 테이블 (이미 knowledge-stack에 있음 — model-registry / settings에도 추가).
- 활성 워크스페이스는 localStorage (`lmmaster.active_workspace_id`). 모든 페이지에서 useContext.
- `apps/desktop/src/contexts/ActiveWorkspaceContext.tsx` 신설.
- Workspace.tsx, Workbench.tsx, Catalog.tsx (custom models) 모두 활성 workspace 사용.

**파일**:
- `apps/desktop/src-tauri/src/workspaces.rs` 신설 — IPC commands `list_workspaces / create_workspace / rename_workspace / delete_workspace`.
- `crates/workspaces` crate 신설 또는 model-registry / knowledge-stack에 통합 — 결정 필요.
- `apps/desktop/src/contexts/ActiveWorkspaceContext.tsx` 신설.
- `apps/desktop/src/components/WorkspaceSwitcher.tsx` 신설 (~200 LOC) + test.
- App.tsx에서 ActiveWorkspaceProvider 래핑.
- ADR-0038 (Multi-workspace UX) 신설.

**negative space**:
- 기존 `"default"` 하드코딩 유지 — ADR-0024 약속 미실현.
- 풀 multi-workspace SQL schema migration — 첫 실행 시 자동 마이그레이션 + 데이터 정리 필요.

### 8'.1.b — workspace_id wire-up

- Workspace.tsx, Workbench.tsx, Catalog 사용자 정의 모델 섹션 모두 `useActiveWorkspace()` hook 사용.
- 기존 "default" 사용처 모두 active workspace로 전환.

**테스트 invariant**:
- 새 워크스페이스 생성 후 Knowledge ingest → 새 workspace에 저장 + 다른 workspace에선 검색 안 됨.
- 워크스페이스 전환 시 Knowledge / Workbench / Catalog custom 모두 전환된 workspace 데이터 표시.
- 워크스페이스 삭제 시 confirmation dialog + cascade 정리.

### 8'.1 DoD

- cargo + vitest ✅.
- 사이드바에 워크스페이스 switcher 표시.
- 새 워크스페이스 생성 → ingest → 검색 시 격리 확인.

---

## §1.8 Phase 11' — Portable Workspace export/import (NEW, 6 pillar 약속)

> **시간**: 보강 리서치 1 + 구현 2 sub-agent / ~4~5시간
> **중요도**: 🔴 ADR-0009 "Portable workspace" 6 pillar 약속의 사용자 경험 절반 미실현 — v1 ship 전 권장.
> **현재 상태**: `crates/portable-workspace`에 fingerprint / repair / manifest / paths만. zip/tar 패킹 export + verify-and-unpack import 부재. 사용자는 폴더를 통째로 복사한 뒤 repair tier 자동 처리만 가능 (수동 절차).

### 보강 리서치

`docs/research/phase-11p-portable-export-reinforcement.md` (~500 LOC, 15+ 인용):

1. **Archive 포맷** — zip vs tar.zst — Windows 호환 + 압축률. zstd level 9 / multi-threaded.
2. **Selective archive 정책** — 모델 파일 포함(수GB) vs 메타데이터만(수MB). 사용자 선택 + ETA.
3. **무결성 검증** — sha256 archive-level + per-entry. 부분 다운로드 / 손상 감지.
4. **민감 정보 분리 정책**:
   - API keys: opt-in (별도 export 패스프레이즈 + AES-GCM 재암호화).
   - SQLCipher DB: OS 키체인은 PC마다 다름 → 패스프레이즈 wrap 후 archive 동봉.
   - Telemetry anon UUID: 새 PC에서 새로 발급 (UUID 중복 방지).
5. **Cross-platform 정책**: ADR-0009 "같은 OS·아키텍처 계열" 한정. 다른 OS (Win→mac) → fingerprint mismatch → red tier → 한국어 경고.
6. **사용자 경험 패턴**: Obsidian Sync / Notion export / 1Password vault export / Steam cloud sync 사례 분석.
7. **부분 export**: 모델만 / 키만 / 설정만 — 사용자 경량 옵션.

### 11'.a — Export pipeline

**설계**:
- `crates/portable-workspace/src/export.rs` 신설:
  - `ExportOptions { include_models: bool, include_keys: bool, key_passphrase: Option<String>, target_path: PathBuf }`.
  - `ExportEvent` (kebab-case tagged): Started / Counting / Compressing { processed, total } / Encrypting / Finalizing / Done { sha256, archive_size_bytes } / Failed.
  - `export_workspace(workspace, options, progress, cancel)` — async fn.
  - 처리 순서:
    1. fingerprint snapshot 추가 (현 PC 정보 — 받은 PC가 비교용).
    2. 메타데이터 (manifest / 사용자 설정 / 프리셋 / 모델 메타) 패킹.
    3. 모델 파일 (옵션) 추가 — 큰 파일은 streaming.
    4. 키 데이터 (옵션) — 별도 패스프레이즈로 AES-GCM wrap 후 archive 동봉.
    5. archive-level sha256 계산.
    6. atomic rename (`.tmp` → final).

**파일**:
- `crates/portable-workspace/src/export.rs` 신설 (~400 LOC, 8 unit tests).
- `crates/portable-workspace/Cargo.toml`: `zip` workspace dep + `aes-gcm` (keys 옵션 시).
- `apps/desktop/src-tauri/src/workspace/export.rs` 신설 — Channel<ExportEvent> + Registry + cancel pattern (Phase 5'.b 패턴).
- 2 신규 commands: `start_workspace_export` / `cancel_workspace_export`.

**negative space**:
- 7zip / RAR — 의존성 + 라이선스 부담. zip + zstd가 충분.
- 모델 포함 default ON — 첫 export 시 수십 GB 발생 가능. **사용자 명시 opt-in이 안전**.
- 단일 .zip만 (split archive X) — Windows 4GB FAT 한계 무시 (NTFS 가정).
- AES-GCM 외 다른 알고리즘 (ChaCha20-Poly1305 등) — 표준 따름.

**테스트 invariant**:
- 모델 미포함 export → 다른 PC import 후 catalog에서 다시 받아야 함 (manifest는 전달).
- 모델 포함 export → import 후 즉시 사용 가능.
- 키 미포함 export — 사용자가 새 PC에서 재발급.
- 키 포함 + 잘못된 패스프레이즈 → 명확 에러 (panic X).
- Archive sha256 mismatch → 손상 감지.
- Cancel mid-export → `.tmp` 정리.

### 11'.b — Import pipeline

**설계**:
- `crates/portable-workspace/src/import.rs` 신설:
  - `ImportOptions { source_path, key_passphrase, target_workspace_root, conflict_policy: Skip|Overwrite|Rename }`.
  - `ImportEvent`: Started / Verifying / Extracting { processed, total } / DecryptingKeys / RepairTier { tier: green|yellow|red } / Done / Failed.
  - 처리 순서:
    1. archive sha256 검증.
    2. dual zip-slip 방어 (Phase 1A.3.b.2 패턴 재활용).
    3. 임시 디렉터리에 unpack.
    4. fingerprint 비교 → repair tier 분류 (green=즉시 사용 / yellow=벤치 재측정 안내 / red=런타임 재설치 안내).
    5. 키 패스프레이즈 unwrap → SQLCipher 신 DB 작성 (현 PC 키체인 사용).
    6. 사용자 confirmation 후 target_workspace_root에 commit (atomic).

**파일**:
- `crates/portable-workspace/src/import.rs` 신설 (~450 LOC, 7 unit tests).
- `apps/desktop/src-tauri/src/workspace/import.rs` 신설 — Channel<ImportEvent>.
- 2 신규 commands: `start_workspace_import` / `cancel_workspace_import`.

**테스트 invariant**:
- 손상된 archive → 명확 에러 (사용자 행동 가능).
- 다른 OS archive → red tier + 한국어 fallback "이 PC와 OS 계열이 달라요. 모델을 다시 받아야 해요".
- conflict_policy 모든 분기 (Skip/Overwrite/Rename).
- 잘못된 패스프레이즈 + 키 포함 archive → 명확 에러.
- Cancel mid-import → 임시 디렉터리 정리, target은 unchanged.

### 11'.c — Settings UI "포터블 이동" 패널

**설계**:
- Settings에 "포터블 이동" 패널 신설 (또는 별도 사이드바 메뉴 "옮기기"):
  - **내보내기**: "이 워크스페이스 내보낼게요" 버튼 → 옵션 dialog (모델 포함 체크 / 키 포함 + 패스프레이즈 입력) → 진행률 + ETA + 사이즈 표시.
  - **가져오기**: "워크스페이스 가져올게요" 버튼 → 파일 선택 → archive 검증 → preview (manifest 요약 — 어떤 PC에서, 언제, 얼마나) → confirm → 진행률 + repair tier 안내.
  - 진행률은 SpotlightCard + StatusPill 패턴 재활용.
  - 한국어 카피 해요체: "다른 PC로 옮기실 거예요?" / "이 PC로 가져오실 거예요?" / "약 NN GB · 약 NN분".
- 사용자 친화 흐름: 첫 사용자 향 짧은 안내 (3-step illustration): "1. 내보내기" → "2. USB / 클라우드로 옮기기" → "3. 새 PC에서 가져오기".

**파일**:
- `apps/desktop/src/components/portable/PortableExportPanel.tsx` 신설 (~250 LOC) + test (~150 LOC).
- `apps/desktop/src/components/portable/PortableImportPanel.tsx` 신설 (~250 LOC) + test (~150 LOC).
- `apps/desktop/src/ipc/portable.ts` 신설 (~100 LOC).
- Settings.tsx에 두 panel 추가 (또는 별도 페이지 — 결정 노트에서).
- i18n `screens.settings.portable.{export,import}.*` (~30 keys per locale).

### 11' DoD

- export → 같은 PC 재import 라운드트립 성공.
- export → 다른 PC import: green/yellow/red tier 모두 case 검증.
- 키 포함 export + 패스프레이즈 unwrap 정상.
- 사용자가 캡처 가능한 단일 zip 파일 산출 (Obsidian sync 수준 UX).
- ADR-0039 (Portable workspace export/import policy) 신설.

---

## §1.9 Phase 12' — Guide / Help system (NEW)

> **시간**: 보강 리서치 0.5 + 구현 1~2 sub-agent / ~3~4시간
> **중요도**: 🟡 사용자 친화 핵심 — 첫 사용자가 6 pillar 기능을 발견하지 못하면 LMmaster 가치가 안 전달됨.
> **현재 상태**: NAV에 가이드 / 도움말 메뉴 없음. tooltip / FAQ / 단축키 안내 부재. Diagnostics는 시스템 점검용.

### 보강 리서치

`docs/research/phase-12p-guide-system-reinforcement.md` (~300 LOC, 10+ 인용):

1. **In-app guide pattern**: Linear / Notion / Stripe Dashboard / VSCode Walkthroughs / Obsidian Help 사례.
2. **Contextual help**: tooltip ? 아이콘 + popover, deep link to guide section.
3. **First-run tour**: Shepherd.js / Intro.js / 자체 구현 trade-off (의존성 vs maintainability — 자체 구현이 design-system 일관성 우월).
4. **Search 기능**: Fuse.js (이미 catalog 검색 패턴) 재활용 또는 자체 substring + jamo 매칭.
5. **단축키 catalog**: Command Palette (이미 ⌘K 구현됨)와 통합.
6. **Markdown rendering**: Phase 7'.a EulaGate minimal renderer 재활용 (react-markdown 의존성 회피).
7. **다국어 가이드 콘텐츠**: ko-first, en mirror.

### 12'.a — Guide page (NAV "가이드")

**설계**:
- `NAV_KEYS`에 `guide` 추가 (Settings 위 또는 별도 위치 — 결정 노트에서).
- `apps/desktop/src/pages/Guide.tsx` 신설:
  - **8 섹션**:
    1. **시작하기** — 첫 실행 + EULA + 마법사 흐름.
    2. **모델 카탈로그** — 추천 / 필터 / 30초 측정 / 다운로드.
    3. **워크벤치** — 5단계 (Data → Quantize → LoRA → Validate → Register).
    4. **자료 인덱싱 (RAG)** — Knowledge 탭 + ingest + 검색.
    5. **API 키 + 외부 웹앱 통합** — 키 발급 + scope + base URL.
    6. **포터블 이동** — Phase 11' export/import 사용법.
    7. **자가 점검 + 자동 갱신** — Diagnostics + Settings 자동 갱신.
    8. **자주 묻는 질문 (FAQ)** — 흔한 문제 + 해결 + 한국어 단축키.
  - 각 섹션은 짧은 마크다운 (5~15줄) + 스크린샷 placeholder + "이 기능 사용해 보기" CTA (페이지 deep link via `dispatchEvent('lmmaster:nav')`).
  - **좌측 sidebar** — 섹션 목록 + 검색 input.
  - **검색** — 키워드 입력 시 substring + jamo 매칭 (Command Palette 패턴 재활용).
  - **deep link**: `?section=workbench` 같은 URL hash로 ContextualHelp에서 직접 진입.

**파일**:
- `apps/desktop/src/pages/Guide.tsx` 신설 (~400 LOC) + test (~250 LOC).
- `apps/desktop/src/i18n/guide-{ko,en}-v1.md` — 8 섹션 마크다운 (총 ~600줄).
- App.tsx NAV_KEYS + render branch + i18n `nav.guide`.
- Catalog과 동일하게 sidebar 검색 + 콘텐츠 본문 패턴.

### 12'.b — In-page contextual help

**설계**:
- 모든 주요 페이지 헤더에 "?" 도움말 아이콘 버튼.
- 클릭 시 popover — 해당 페이지 1~2줄 설명 + "전체 가이드 보기" 링크 (Guide page 해당 섹션 deep link).
- popover는 design-system token 사용, focus trap + Esc 닫기.
- 5 페이지 적용: Workspace / Workbench / Catalog / ApiKeys / Settings.

**파일**:
- `apps/desktop/src/components/HelpButton.tsx` 신설 (~150 LOC) + test (~120 LOC).
- 5 페이지 header에 `<HelpButton sectionId="..." />` 추가.

### 12'.c — Keyboard shortcuts + first-run "둘러보기"

**설계**:
- **단축키 표 (확장)**:
  - ⌘K / Ctrl+K — Command Palette (이미 구현).
  - F1 / Shift+? — Guide 열기 (활성 페이지 섹션으로).
  - ⌘1~⌘9 / Ctrl+1~9 — NAV 이동 (홈 / 카탈로그 / 설치 / 런타임 / 워크스페이스 / 프로젝트 / 키 / 워크벤치 / 진단).
  - Esc — 모달 / drawer 닫기 (이미 일부 구현).
- `apps/desktop/src/components/ShortcutsModal.tsx` 신설 — F1 누르면 표시 (Guide page 진입과 별개 / 빠른 참조).
- **First-run "둘러보기" CTA**:
  - 온보딩 마법사 끝난 직후 Toast: "처음이세요? 가이드 둘러볼래요?".
  - 클릭 시 Guide page 진입 + "시작하기" 섹션.
  - "다음에 할게요" 옵션 — localStorage `lmmaster.tour.skipped`.
  - 1회만 표시 (Toast 닫으면 안 다시 띄움).

**파일**:
- `apps/desktop/src/components/ShortcutsModal.tsx` 신설 (~200 LOC) + test (~120 LOC).
- App.tsx에 F1 / Ctrl+1~9 글로벌 hotkey 등록.
- App.tsx 첫 마운트 시 첫 실행 toast 표시 로직.
- i18n `shortcuts.*` + `tour.welcome.*`.

### 12' DoD

- NAV에 "가이드" 메뉴 노출 + 클릭 시 Guide page.
- 5 페이지 헤더에 "?" 도움말 버튼 + popover 동작.
- F1 단축키 활성 + ShortcutsModal 표시.
- 첫 실행 끝난 후 "가이드 둘러보기" toast (1회만).
- 한국어 + 영어 가이드 콘텐츠 8 섹션.
- ADR-0040 (In-app guide system) 신설.

---



### 8'.a — Quick wins batch (#14, #15, #16, #22)

**시간**: 1 sub-agent / ~30~45분
**충돌**: 없음 (4 항목 다 다른 파일)
**보강 리서치 불필요**: 모두 small surgical fix.

#### 8'.a.1 — `KnowledgeStore::get_document_path` (#14)

**파일**:
- `crates/knowledge-stack/src/store.rs`: `get_document_path(workspace_id, document_id) -> Option<PathBuf>` 메소드 추가. 기존 `documents` 테이블에서 path 컬럼 조회.
- `apps/desktop/src-tauri/src/knowledge.rs`: `search_knowledge` 안에서 SearchHit 빌드 시 `get_document_path` 호출. 실 path 또는 Korean fallback "원본 경로 없음".
- `crates/knowledge-stack/src/store.rs::tests`: +3 unit (existing/missing doc/cross-workspace).

**테스트 invariant**:
- 동일 workspace에서 등록한 문서는 path 반환.
- 다른 workspace의 document_id 조회 시 None.
- DB에 없는 document_id 조회 시 None (panic X).

#### 8'.a.2 — `update.skipped.{version}` LRU cleanup (#15)

**파일**:
- `apps/desktop/src/components/ToastUpdate.tsx`: 새 버전 도착 시 이전 버전(들)의 `lmmaster.update.skipped.*` 키 정리. semver compare로 latest version보다 낮은 모든 키 삭제.
- `ToastUpdate.test.tsx`: +2 vitest (이전 skipped 키 정리 / 같은 버전 유지).

**테스트 invariant**:
- 1.0.0 skipped 후 1.1.0 도착 → 1.0.0 키 제거.
- 1.0.0 skipped 후 0.9.0 표시 시(downgrade) → 1.0.0 키 유지.
- 정확히 매칭되는 키만 정리 (`lmmaster.update.skipped.` prefix).

#### 8'.a.3 — Last-checked timestamp 일관성 (#16)

**파일**:
- `apps/desktop/src-tauri/src/updater.rs`: `Poller::run`이 outdated 콜백뿐 아니라 매 cycle마다 `last_check_iso` 갱신. `PollerStatus::last_check_iso`도 매 polling 시 update.
- `apps/desktop/src/pages/Settings.tsx::AutoUpdatePanel`: 표시 키 일관 사용.
- updater.rs unit tests: +2 (uptodate 시 last_checked 갱신 / 실패 시 갱신 X).

**테스트 invariant**:
- UpToDate 응답에서도 last_check_iso 갱신.
- Failed 응답 시 last_check_iso는 그대로 (실패는 "확인 못 함").

#### 8'.a.4 — onboarding placeholder dead key 제거 (#22)

**파일**:
- `apps/desktop/src/i18n/{ko,en}.json`: `onboarding.scan.placeholder` / `onboarding.install.placeholder` 키 삭제 (Phase 1A.4.b/c 완료로 미사용).
- 테스트는 i18n key 존재 가정 안 하므로 변경 없음.

**검증**: tsc + vitest 통과.

#### 8'.a 전체 검증 (DoD)

- cargo test -p knowledge-stack -p lmmaster-desktop ✅
- vitest run ✅
- 기존 1096 → 1100+ tests (4 항목 합 +6~8건)
- clippy / fmt 0
- 결정 노트: 본 문서 §8'.a 인용. 별도 보강 리서치 미작성 (XS 단위).

---

### 8'.b — Workbench↔Catalog 흐름 (#8, #13)

**시간**: 1 sub-agent / ~1시간
**충돌**: 8'.a와 다른 파일 → 8'.a 끝난 뒤 순차.
**보강 리서치**: tauri-plugin-shell v2 capability 패턴만 quick search.

#### 8'.b.1 — Catalog가 listCustomModels 노출 (#8)

**설계**:
- Catalog UI에 "사용자 정의 모델" 별도 섹션 (top, 추천 strip 위 또는 카테고리 탭에 추가).
- `commands::list_custom_models` (이미 Phase 5'.d 있음) 활용.
- Custom model 카드는 일반 카탈로그 카드와 다른 visual cue (border 토큰 또는 badge "내가 만든 모델").
- 클릭 시 ModelDetailDrawer 재사용 — 단, custom model은 quant_options / use_cases / context_guidance가 빈 경우 graceful fallback.
- empty 상태: "Workbench에서 모델을 만들면 여기 표시돼요" + Workbench 진입 링크.

**파일**:
- `apps/desktop/src/ipc/catalog.ts`: `getCustomModels()` wrapper 추가 (existing IPC 호출).
- `apps/desktop/src/components/catalog/CustomModelsSection.tsx` 신설 (~150 LOC) + test (~120 LOC).
- `apps/desktop/src/pages/Catalog.tsx`: section 통합.
- i18n `screens.catalog.custom.*` keys (~10).

#### 8'.b.2 — `tauri-plugin-shell` 정식 도입 (#13)

**설계**:
- `@tauri-apps/plugin-shell` + Rust `tauri-plugin-shell` 추가.
- `apps/desktop/src/components/ToastUpdate.tsx`: `window.open` → `import { open } from '@tauri-apps/plugin-shell'; open(url)`.
- `capabilities/main.json`: `shell:allow-open` permission + URL scope (`https://github.com/**` 등).
- Settings의 다른 외부 링크도 일괄 마이그레이션 (있으면).

**파일**:
- `apps/desktop/Cargo.toml`: `tauri-plugin-shell = "2"`.
- `apps/desktop/package.json`: `@tauri-apps/plugin-shell ^2.0.0`.
- `apps/desktop/src-tauri/src/lib.rs`: plugin 등록.
- `capabilities/main.json`: shell scope.

**negative space (기각된 대안)**:
- `webbrowser` crate 직접 — Tauri 권장 X (capability scope 무시).
- `window.open` 유지 — Tauri 2 보안 모델과 일관성 부족.

#### 8'.b 전체 검증 (DoD)

- cargo test -p lmmaster-desktop ✅
- vitest run ✅ (+12~15건)
- pnpm tauri:dev에서 ToastUpdate "업데이트 보기" 클릭 → 외부 브라우저 정상 오픈

---

### 8'.c — Pipelines 확장 (#11, #12, #4, #10)

**시간**: 보강 리서치 1 sub-agent + 구현 2~3 sub-agent / ~3~4시간
**충돌**: 8'.a, 8'.b 끝난 뒤. core-gateway / pipelines / lib.rs 공유.
**보강 리서치 필수**: ArcSwap pattern + SSE chunk parsing best practice + KeyManager schema migration.

#### 보강 리서치 항목

`docs/research/phase-8pc-pipelines-extension-reinforcement.md` (~400 LOC, 12+ 인용):

1. **ArcSwap vs Mutex<Arc<T>>** for hot-reload (Vector / Linkerd / Tonic 사례).
2. **SSE chunk parsing**: `eventsource-stream` crate vs 자체 buffer parser. byte-perfect는 *line-aware* parse + emit이 더 안전.
3. **OpenAI streaming spec** — chunk 형식, `[DONE]` sentinel, `data: ` prefix.
4. **Per-key matrix DB schema migration** — sqlx migrations / rusqlite manual / 백업 정책.
5. **PromptSanitize**: NFC + control-char strip 전형 패턴, false-positive 회피.

#### 8'.c.1 — PromptSanitize Pipeline (#12)

**설계** (보강 리서치 §5 기반):
- `crates/pipelines/src/prompt_sanitize.rs` 신설.
- `Pipeline::apply_request`에서 messages[].content NFC normalize + zero-width / RTL override 등 control char strip.
- `unicode-normalization` 이미 `knowledge-stack`이 사용 중 — 재사용.
- `PipelineDescriptor` ID `prompt-sanitize` 추가, Settings UI에 토글 자동 노출.
- v1 시드에 합류 → 4종이 됨 (PII / TokenQuota / Observability / PromptSanitize).

**파일**:
- `crates/pipelines/src/prompt_sanitize.rs` 신설 (~250 LOC) + 10 unit tests.
- `crates/pipelines/src/lib.rs`: pub use.
- `apps/desktop/src-tauri/src/pipelines.rs::PipelinesConfig`: `prompt_sanitize_enabled: bool` 필드 추가, default true.
- `apps/desktop/src-tauri/src/gateway.rs::build_chain_from_state`: prompt_sanitize 토글 처리.
- i18n: `screens.settings.pipelines.pipelines.promptSanitize.{name,desc}` 추가.

**테스트 invariant**:
- NFC: ㅎ + ㅏ + ㄴ → 한 (single codepoint).
- Zero-width: U+200B 제거.
- RTL override: U+202E 제거.
- Plain ASCII는 unchanged.
- system role content도 처리 (user만 X, all roles).

#### 8'.c.2 — Pipelines hot-reload (#4)

**설계** (보강 리서치 §1 기반):
- `crates/core-gateway/src/pipeline_layer.rs::PipelineLayer`: 내부 chain을 `Arc<ArcSwap<PipelineChain>>`으로.
- `with_pipelines_audited`가 `&Arc<ArcSwap<PipelineChain>>` 반환 또는 내부 등록 방식.
- `apps/desktop/src-tauri/src/pipelines.rs::PipelinesState::set_pipeline_enabled`이 호출 시 `gateway::build_chain_from_state` 재실행 → ArcSwap 교체.
- `gateway.rs::run`이 `chain_swap: Arc<ArcSwap<PipelineChain>>`을 PipelinesState에 저장 (혹은 AppHandle에 manage).
- 토글 시 다음 요청부터 새 chain 적용. 진행 중 요청은 옛 chain 그대로 (race 안전).

**negative space**:
- `Mutex<Arc<PipelineChain>>` — 토글 시 lock 경합 + 모든 in-flight 요청이 동일 인스턴스 봐야 → ArcSwap이 lock-free 우월.
- 매 요청마다 chain 재빌드 — 토글 빈도 낮은데 비용 큼.

**파일**:
- `crates/core-gateway/Cargo.toml`: `arc-swap = "1"`.
- `crates/core-gateway/src/pipeline_layer.rs`: 수정 (~100 LOC) + 4 신규 tests.
- `apps/desktop/src-tauri/src/pipelines.rs`: chain_swap 보유 + set_pipeline_enabled 시 swap (~80 LOC) + 5 신규 tests.
- `apps/desktop/src-tauri/src/gateway.rs`: chain_swap 주입 (~30 LOC).

**테스트 invariant**:
- 토글 변경 후 다음 요청부터 새 chain 적용.
- 진행 중 요청이 옛 chain 인스턴스로 안전하게 끝남.
- 동시 토글 race-free.

#### 8'.c.3 — ApiKeys per-key Pipelines matrix (#11)

**설계** (보강 리서치 §4 기반):
- `crates/key-manager/src/lib.rs::ApiKeyScope`: `enabled_pipelines: Option<Vec<String>>` 추가. `None` = 전역 토글 따름, `Some(vec)` = override.
- DB migration: `api_keys` 테이블에 `enabled_pipelines TEXT` (JSON array) 컬럼. 빈 컬럼은 None.
- Pipelines apply 시 PipelineContext에 `principal_key_id`가 있으면 그 key의 override 우선, 없으면 전역.
- Settings → ApiKeys 패널에 "이 키에만 적용할 필터" 멀티셀렉트.

**파일**:
- `crates/key-manager/src/lib.rs`: schema migration + 필드 추가 (~80 LOC) + 6 신규 tests.
- `crates/core-gateway/src/pipeline_layer.rs`: PipelineContext 빌드 시 key override 반영 (~30 LOC).
- `apps/desktop/src/pages/ApiKeysPanel.tsx`: 키 발급 모달 / 편집에 멀티셀렉트 UI (~150 LOC) + test (~120 LOC).
- i18n: `screens.keys.pipelines.*`.

**테스트 invariant**:
- 키에 `enabled_pipelines = ["pii-redact"]` 설정 시 token-quota / observability 미적용.
- `enabled_pipelines = None` 키는 전역 토글 따름.
- 마이그레이션: 기존 키는 모두 None.

#### 8'.c.4 — SSE chunk transformation (#10)

**설계** (보강 리서치 §2~§3 기반):
- `core_gateway`의 `/v1/chat/completions` route에 stream chunk parser 도입.
- Chunk format: `data: {json}\n\n` 단위. parser가 line-aware buffer 유지 (`\n\n` 분리자 기다림).
- 각 chunk 추출 → JSON parse → `chain.apply_response(&mut ctx, &mut chunk_value)` 호출 → re-serialize → emit.
- `[DONE]` sentinel은 그대로 통과.
- byte-perfect는 깨지지만 v1.x로 약속한 trade-off (`PII redact가 streaming 응답에도 적용`).
- ADR-0025 §"감내한 트레이드오프"에 streaming chunk transformation v1.x로 명시 → ADR-0030 신설로 전환 결정 기록.

**negative space**:
- 자체 buffer parsing — `eventsource-stream` crate 도입이 안전하지만 의존성 +1. 본 코드 small enough라 자체 구현.
- 모든 chunk 차단 옵션 — Pipelines 자체 실패 시 stream 끊을 것인지 정책. v1은 best-effort: 단일 chunk 실패 시 그 chunk만 unchanged 통과.

**파일**:
- `crates/core-gateway/src/sse_chunk.rs` 신설 (~250 LOC) + 12 unit tests.
- `crates/core-gateway/src/pipeline_layer.rs`: response가 SSE면 sse_chunk handler에 위임 (~50 LOC).
- `crates/core-gateway/Cargo.toml`: 추가 dep 없음 (bytes / serde_json 이미 보유).

**테스트 invariant**:
- 정상 chunk: data: {...}\n\n 단위 parsing.
- 다중 chunk in single TCP segment.
- chunk가 buffer 경계 가로지름 (split mid-JSON).
- `[DONE]` sentinel 통과.
- chunk JSON parse 실패 → 원본 chunk 통과 + audit warn.
- PII redact 적용된 chunk → emit.
- Performance: byte-for-byte 차이 측정 (NoOp pipeline에서 입출력 동일).

#### 8'.c 전체 검증 (DoD)

- cargo test -p pipelines -p core-gateway -p key-manager -p lmmaster-desktop ✅
- vitest run ✅ (+30~40건)
- ADR-0028 (Pipelines hot-reload + chain swap) 신설.
- ADR-0029 (Per-key Pipelines override) 신설 — 기존 ADR 번호 충돌 시 0030/0031로 조정.
- ADR-0030 (SSE chunk transformation) 신설 — ADR-0025 §"감내한 트레이드오프" supersede.

**위험 노트**:
- ArcSwap race: 토글 → swap → 다음 요청 사이 race window 짧지만 존재. 테스트로 invariant 명시.
- per-key matrix migration: 기존 키 손상 없도록 롤백 가능한 schema migration (rusqlite version pragma + 트랜잭션).
- SSE byte-perfect 깨짐: 사용자에게 noticeable한 latency 변화 측정 (`bench-harness`로 측정 권장).

---

## §3 Phase 9' — Real ML wiring

### 9'.a — Real Embedder (#5) — bge-m3 / KURE-v1 cascade

**시간**: 보강 리서치 1 sub-agent + 구현 1~2 sub-agent / ~5~7시간
**보강 리서치 필수**: 모델 다운로드 정책 + ort runtime + ONNX 모델 quantize + tokenizer 통합.

#### 보강 리서치 항목

`docs/research/phase-9pa-embedder-reinforcement.md` (~600 LOC, 20+ 인용):

1. **bge-m3 vs KURE-v1 vs multilingual-e5** — 한국어 RAG 벤치마크 (KoSimCSE, KLUE).
2. **ort crate** (Rust ONNX Runtime) — Tauri 통합 사례, GPU(CUDA/DirectML) 옵셔널.
3. **모델 다운로드 정책** — HuggingFace mirror / 자체 mirror, 무결성 검증 (sha256), 재시도.
4. **Tokenizer**: `tokenizers` crate (HuggingFace) + 한국어 BPE.
5. **양자화**: int8 / fp16 / fp32 trade-off, 768d / 1024d 차원.
6. **Cascade 전략**: 빠른 임베딩 → top-K → 정밀 임베딩 rerank.
7. **Cold start UX**: 첫 ingest 시 모델 다운로드 진행률 노출.

#### 9'.a 구현

**파일** (~1500 LOC 예상):
- `crates/knowledge-stack/Cargo.toml`: `ort = "2"`, `tokenizers = "0.20"`, `hf-hub`.
- `crates/knowledge-stack/src/embed.rs`: `OrtEmbedder` 신설 (BgeM3Embedder + KureV1Embedder + 카스케이드 wrapper).
- `crates/knowledge-stack/src/embed_download.rs` 신설 — 모델 파일 다운로드 + 검증 + 캐시 (`app_data_dir/models/embed/`).
- `apps/desktop/src-tauri/src/knowledge.rs`: 시작 시 OrtEmbedder 생성 (모델 미존재 시 사용자 동의 후 다운로드 wizard).
- `apps/desktop/src/pages/Workspace.tsx`: 첫 ingest 시 모델 다운로드 progress UI.
- ADR-0031 (Real Embedder cascade) 신설.

**negative space**:
- OpenAI embedding API — 외부 통신 0 위반.
- Sentence-Transformers Python sidecar — Tauri 사이드카 추가 의존성 + 콜드 스타트 느림.
- llama.cpp embeddings — 한국어 미세 조정 부족.

**테스트 invariant**:
- 모델 미존재 시 graceful "다운로드 필요" 에러 (panic X).
- 모델 다운로드 sha256 검증 실패 시 에러 + 부분 파일 정리.
- bge-m3 1024d 임베딩 차원 정확.
- 한국어 입력에 대한 cosine similarity sanity check (동의어 > 무관 텍스트).
- Cancel mid-download.

#### 9'.a DoD

- ONNX 모델 다운로드 + 검증 + ingest 흐름 e2e 동작.
- RAG 검색이 sha256-mock 대비 의미 있는 ranking.
- 다운로드 캐시 invalidation 가능.

---

### 9'.b — LlamaQuantizer / LLaMA-Factory CLI (#6)

**시간**: 보강 리서치 1 + 구현 2 sub-agent / ~6~8시간
**보강 리서치 필수**: 외부 binary 의존성 정책 + Python venv 부트스트랩.

#### 보강 리서치 항목

`docs/research/phase-9pb-workbench-real-reinforcement.md` (~700 LOC, 25+ 인용):

1. **llama.cpp llama-quantize binary**: 빌드/배포 정책. portable binary download vs ollama-bundled vs build-from-source.
2. **LLaMA-Factory CLI**: Python venv 자동 생성 (uv / pip), GPU 환경 detect, 한국어 데이터셋 어노테이션 패턴.
3. **Modelfile 작성**: ollama create 입력 형식, base_model + quantization layer.
4. **GPU 격리**: 양자화 + LoRA 동시 실행 차단 (Semaphore=1, ADR-0022 패턴 재활용).
5. **시간/디스크 budget UI**: 사용자에게 양자화 5~30분 / LoRA 1~10시간 budget 사전 동의.
6. **장기 작업 cancel**: kill_on_drop + 임시 파일 정리.

#### 9'.b 구현

**파일** (~2000 LOC):
- `crates/workbench-core/src/quantize.rs::LlamaQuantizer` 신설 — `MockQuantizer` 옆에. binary path detection + spawn + progress parsing.
- `crates/workbench-core/src/lora.rs::LlamaFactoryTrainer` 신설 — Python venv 자동 부트스트랩 + LLaMA-Factory CLI spawn + WandB-free progress.
- `apps/desktop/src-tauri/src/workbench.rs`: `WorkbenchConfig.use_real_quantizer` / `use_real_trainer` 토글 추가. 미선택 시 Mock.
- `apps/desktop/src/pages/Workbench.tsx`: 5단계 UI에 "실 양자화 / Mock" 토글 + 사전 동의 dialog (시간/디스크 budget).
- ADR-0032 (Real Workbench external binary) 신설.

**negative space**:
- Python sidecar 상시 띄움 — 콜드 스타트 + 메모리 비용. on-demand spawn이 우월.
- Rust-only LoRA — `candle` crate 가능하지만 LLaMA-Factory만큼 한국어 데이터 패턴 미성숙.

**테스트 invariant**:
- llama-quantize binary 미존재 → 친절한 다운로드 안내 (panic X).
- Python 미설치 → "uv 설치 안내" 한국어 fallback.
- 5단계 mock 흐름은 그대로 동작 (regression X).
- Cancel mid-quantize → 임시 파일 정리.

---

### 9'.c — Multi-runtime adapters (#9)

**시간**: 보강 리서치 1 + 구현 1 sub-agent / ~3~4시간
**보강 리서치 필수**: koboldcpp / vllm / llama-server HTTP 스펙.

#### 보강 리서치 항목

`docs/research/phase-9pc-multi-runtime-reinforcement.md` (~400 LOC, 12+ 인용):

1. **llama.cpp `llama-server`** OpenAI 호환 endpoint.
2. **koboldcpp**: 자체 API + OpenAI bridge.
3. **vllm**: production-grade, OpenAI-compatible.
4. **자동 detect**: 각 runtime의 `/v1/models` 또는 헬스체크 endpoint.

#### 9'.c 구현

**파일** (~800 LOC):
- 기존 `crates/adapter-{llama-cpp,koboldcpp,vllm}/src/lib.rs` 검토 후 mount 누락 부분 보강.
- `apps/desktop/src-tauri/src/registry_provider.rs`: `LiveRegistryProvider::from_environment` 확장 — 모든 detected runtime 등록.
- `crates/runtime-detector`: 4종 runtime detect rules 추가.
- ADR-0033 (Multi-runtime expansion) 신설.

---

## §4 Phase 10' — Telemetry submit (#7)

**시간**: 보강 리서치 1 + 구현 1 sub-agent / ~2~3시간
**전제 조건**: Phase 7'.b release 인프라 + GlitchTip self-hosted 인스턴스 운영 결정.
**보강 리서치 필수**: GlitchTip / Sentry SDK / 익명화 정책.

#### 보강 리서치 항목

`docs/research/phase-10p-telemetry-reinforcement.md` (~300 LOC, 10+ 인용):

1. **GlitchTip API endpoint** — Sentry 호환 SDK 사용 가능.
2. **Anonymous ID 정책** — uuid4 + per-PC 고정 + opt-out 시 즉시 폐기.
3. **PII 필터링** — 프롬프트 / 모델 출력 절대 미전송.
4. **이벤트 종류** — crash report only vs anonymous usage stats.

#### 10' 구현

**파일** (~300 LOC):
- `apps/desktop/src-tauri/src/telemetry.rs`: `submit_event(event)` 함수 추가. opt-in 게이트 + queue + retry.
- crash report hook (panic_hook + tauri error events).
- ADR-0034 (GlitchTip telemetry) 신설.

---

## §5 Env' — STATUS_ENTRYPOINT_NOT_FOUND 우회

### Env'.a — 환경 진단 + CI 전략

**시간**: 30분 (코드 변경 X, 문서 + CI config)

#### 진단 결론 (이번 세션 확인)

- Windows 11 25H2 build 26200.
- `api-ms-win-core-synch-l1-2-0.dll`이 `C:\Windows\System32\downlevel/`에는 있지만 active path resolution 실패.
- 테스트 exe가 import하는 `WaitOnAddress` / `WakeByAddressSingle` / `WakeByAddressAll` (parking_lot 사용)이 kernelbase.dll에서 미해결.
- 영향 범위: lmmaster-desktop --lib 단위 테스트만. 실 앱 (`cargo run`) 정상 동작. 다른 crate 테스트 모두 정상.

#### 해결 옵션

**A. 사용자 시스템 복구 권장 (root cause fix)**:
```powershell
# 관리자 PowerShell
sfc /scannow
DISM /Online /Cleanup-Image /RestoreHealth
# Windows Update 누적 패치 재설치
```

**B. CI에서 보장 (개발 편의)**:
- GitHub Actions Windows runner에서는 정상 동작 가정.
- 로컬에서만 `cargo test --workspace --exclude lmmaster-desktop` 사용. 통합 테스트는 `cargo test --test '*' -p lmmaster-desktop` (rlib 사용).
- 또는 lmmaster-desktop의 unit tests를 별도 integration test crate로 추출 (`apps/desktop/src-tauri/tests/`로 이동) — 이렇게 하면 cdylib 의존성 회피.

**C. 코드 분리 (장기)**:
- lmmaster-desktop을 두 crate로: `lmmaster-desktop-lib` (rlib only, 모든 테스트 가능) + `lmmaster-desktop-app` (cdylib, Tauri shell only).
- 비용 큼. v1.x 후속.

#### Env'.a 구현

**파일** (코드 변경 거의 없음):
- `docs/troubleshooting.md` 신설 — STATUS_ENTRYPOINT_NOT_FOUND 진단 + 해결 가이드.
- `run-tests.bat`: `--exclude lmmaster-desktop` 옵션 추가 + 별도 `cargo test --test '*' -p lmmaster-desktop` integration test 라인.
- `.github/workflows/ci.yml` (없으면 생성) — Windows / mac / Linux matrix.

---

## §6 의존성 그래프 + 추천 순서

```
   ⛔ Phase 7'.a'.x (사용자 minisign 발급) ─→ 출시 차단 해제
                                                     ↓
   Phase 8'.0 (security/stability) ─────┐
   (SQLCipher / single-instance /        │
    panic / WAL / artifact)              │
                                         ↓
   Phase 8'.1 (multi-workspace) ────────┤
                                         │
   Phase 8'.a (quick wins) ─────────────┤
                                         ├─→ Phase 8'.b (UI flow)
   Env'.a (test 복구) ──────────────────┤
                                         ├─→ Phase 8'.c (보강 리서치 → 구현)
                                         │
                                         ├─→ Phase 9'.a (Embedder 보강 → 구현)
                                         ├─→ Phase 9'.b (Workbench 보강 → 구현)
                                         └─→ Phase 9'.c (Multi-runtime 보강 → 구현)

   Phase 7'.b 출시 인프라 (사용자 결정) ──→ Phase 10' (telemetry)
```

**Critical path (v1 ship 직전 필수)**:
- 7'.a'.x → 8'.0 → 8'.1 → **11'** → **12'** = security + stability + UX 약속 + 6 pillar 포터블 + 사용자 친화 가이드.

병렬화 가능한 segment:
- 8'.0.a / 8'.0.b / 8'.0.c 병렬 (서로 다른 파일).
- 11'.a / 11'.b 직렬, 11'.c 그 후 (의존).
- 12'.a / 12'.b / 12'.c 병렬 (서로 다른 파일).
- 8'.a / 8'.b / Env'.a 병렬.
- 9'.a / 9'.b / 9'.c 병렬 — 보강 리서치만 직렬, 구현은 병렬.

추천 세션 분할:
- **Session V1-Critical-A** (~4시간): 7'.a'.x (사용자 30분) + 8'.0 보강 리서치 + 8'.0 구현 (3 sub-phase 병렬)
- **Session V1-Critical-B** (~4시간): 8'.1 구현 + 11' 보강 리서치 + 12' 보강 리서치
- **Session V1-Portable** (~4~5시간): 11' 구현 (a 직렬 → b 직렬 → c 병렬)
- **Session V1-Guide** (~3~4시간): 12' 구현 (a/b/c 병렬) + 8'.a + 8'.b + Env'.a 병렬
- **Session V1.x-A** (~4시간): 8'.c 보강 리서치 + 8'.c 구현 (3 sub-phase)
- **Session V1.x-B** (~5시간): 9' 보강 리서치 (3건) + 9'.a 구현
- **Session V1.x-C** (~5시간): 9'.b 구현
- **Session V1.x-D** (~3시간): 9'.c 구현 + 10' 보강 리서치
- **Session V1.x-E** (Phase 7'.b 후): 10' 구현

총 ~32~38 시간 추정 (sub-agent 병렬 활용 시 ~18~22시간으로 압축).

> **권장**: V1-Critical-A/B + V1-Portable + V1-Guide는 v1 ship 전 처리 (6 pillar 약속 + 사용자 친화). 나머지는 v1 출시 후 v1.x 점진 처리.

---

## §7 결정 노트 6-section 매핑

CLAUDE.md §4.5 의무. 각 페이즈 보강 리서치 + 결정 노트는 본 문서 §섹션 인용:

| 페이즈 | 결정 노트 |
|---|---|
| **7'.a'.x** | **본 문서 §1.5 + minisign signer docs** |
| **8'.0** | **`docs/research/phase-8p0-security-stability-reinforcement.md` (신설)** |
| **8'.1** | **본 문서 §1.7 — 기존 phase-4p5-rag / phase-5p-workbench 재참조** |
| **11'** | **`docs/research/phase-11p-portable-export-reinforcement.md` (신설)** |
| **12'** | **`docs/research/phase-12p-guide-system-reinforcement.md` (신설)** |
| 8'.a | 본 문서 §2.8'.a (small fix, 별도 노트 X) |
| 8'.b | 본 문서 §2.8'.b + plugin-shell quick search |
| 8'.c | `docs/research/phase-8pc-pipelines-extension-reinforcement.md` (신설) |
| 9'.a | `docs/research/phase-9pa-embedder-reinforcement.md` (신설) |
| 9'.b | `docs/research/phase-9pb-workbench-real-reinforcement.md` (신설) |
| 9'.c | `docs/research/phase-9pc-multi-runtime-reinforcement.md` (신설) |
| 10' | `docs/research/phase-10p-telemetry-reinforcement.md` (신설) |
| Env'.a | `docs/troubleshooting.md` (신설) |

각 결정 노트는 6-section (요약 / 채택안 / 기각안+이유 / 미정 / 테스트 invariant / 다음 페이즈 인계).

---

## §8 새 ADR 후보 (시간순)

| 번호 | 제목 | 페이즈 |
|---|---|---|
| ADR-0028 | Pipelines hot-reload via ArcSwap | 8'.c |
| ADR-0029 | Per-key Pipelines override matrix | 8'.c |
| ADR-0030 | SSE chunk transformation policy | 8'.c |
| ADR-0031 | Real Embedder cascade (bge-m3 + KURE-v1) | 9'.a |
| ADR-0032 | External binary policy (llama-quantize / LLaMA-Factory) | 9'.b |
| ADR-0033 | Multi-runtime adapter expansion | 9'.c |
| ADR-0034 | GlitchTip self-hosted telemetry | 10' |
| **ADR-0035** | **SQLCipher activation + OS keychain** | **8'.0.a** |
| **ADR-0036** | **Single-instance + panic hook + SQLite WAL** | **8'.0.b** |
| **ADR-0037** | **Workbench artifact retention policy** | **8'.0.c** |
| **ADR-0038** | **Multi-workspace UX + active workspace context** | **8'.1** |
| **ADR-0039** | **Portable workspace export/import policy** | **11'** |
| **ADR-0040** | **In-app guide system + contextual help + shortcuts** | **12'** |

(README의 ADR-0028/0029 후보(Gateway audit / GlitchTip endpoint)와 시간순 충돌 — 본 번호 체계로 통일. 충돌 시 페이즈 진입 시점에 메인이 renumber.)

---

## §9 위험 + 완화 전략

| 위험 | 페이즈 | 완화 |
|---|---|---|
| **SQLCipher 마이그레이션 실패 → 키 손실** | **8'.0.a** | **트랜잭션 + 마이그레이션 전 평문 DB 백업 (사용자 데이터 디렉터리에 `.bak` 보존) + 사용자 confirmation** |
| **OS 키체인 미접근 (Linux) → 암호화 비활성** | **8'.0.a** | **graceful fallback 한국어 경고 + Settings 명시 + 사용자 동의 후 평문 모드 유지** |
| **panic hook 자체 panic → 무한 재귀** | **8'.0.b** | **`set_hook` 안에서 `catch_unwind` 보호 + crash report 작성 실패 시 silent abort** |
| **single-instance plugin 보안 — 다른 프로세스가 신호 가로채기** | **8'.0.b** | **named pipe 권한 user-only + Tauri plugin 기본값 follow** |
| **WAL 모드 + 네트워크 드라이브 비호환** | **8'.0.b** | **app_data_dir 로컬만 보장 (Phase 0 검증) — 문서 명시** |
| **multi-workspace 마이그레이션 — 기존 default 데이터 보존** | **8'.1** | **첫 실행 시 default workspace 자동 생성 + 기존 데이터 그대로 매핑** |
| **active workspace localStorage 손상 → 진입 불가** | **8'.1** | **localStorage 손상 시 default로 자동 폴백 + 손상 키 자동 삭제** |
| ArcSwap race 누수 | 8'.c.2 | `Arc::strong_count` 모니터링 + chain drop 시점 명시 |
| per-key migration 데이터 손실 | 8'.c.3 | 트랜잭션 + 백업 + 롤백 (`PRAGMA user_version`) |
| SSE byte-perfect 깨짐 | 8'.c.4 | NoOp pipeline 통과 시 입출력 동일 단언 (golden test) |
| 임베딩 모델 다운로드 실패 | 9'.a | 사용자 진행률 + 재시도 + 부분 파일 정리 |
| LLaMA-Factory 설치 실패 (Python 환경) | 9'.b | 친절한 한국어 진단 + 사용자 대안 안내 |
| 외부 telemetry 서버 다운 | 10' | 큐 + 24h 보관 + drop 정책 |
| Env STATUS_ENTRYPOINT 재발 | Env'.a | troubleshooting.md + CI Windows runner 별도 검증 |
| **minisign private key 노출** | **7'.a'.x** | **`.gitignore` 검증 + 1Password / vault 보관 + GitHub Actions secret 사용** |
| **Portable archive 손상 → 사용자 데이터 손실** | **11'.a/.b** | **archive sha256 + atomic rename + 임시 디렉터리 + 첫 import 시 원본 보존 (자동 삭제 X)** |
| **Portable export에 키 포함 → 패스프레이즈 약함** | **11'.a** | **password strength meter + zxcvbn 권장 + 사용자 명시 confirmation** |
| **Cross-OS portable archive → 사용자가 강제 import** | **11'.b** | **fingerprint mismatch 시 red tier + 한국어 경고 dialog + "정말 진행" double confirm** |
| **Guide 콘텐츠 stale (코드 변경 시 가이드 갱신 누락)** | **12'.a** | **각 가이드 섹션 끝에 "최종 갱신 버전 vX.Y" 명시 + 매 sub-phase DoD에 가이드 갱신 체크리스트** |
| **Tour Toast가 첫 사용자 외에 노출 — 사용자 짜증** | **12'.c** | **localStorage `lmmaster.tour.skipped` 1회만 표시 + 사용자 명시 "다음에 할게요" 후 영구 silent** |

---

## §10 다음 세션 진입 가이드 (Standby 상태)

**현재 상태 (2026-04-29)**: 본 계획서가 **standby 모드**. 코드 작업 미실행. 사용자 명시 진행 신호 시 시작.

다음 세션 시작 시:
1. `CLAUDE.md` + `MEMORY.md` 자동 로드.
2. 본 문서 + `docs/RESUME.md` + `docs/PROGRESS.md` 참조.
3. `unimplemented_audit_2026_04_28.md`에서 처리할 # 선택.
4. 해당 페이즈 보강 리서치 → 결정 노트 → 구현 순서.
5. sub-agent 병렬 가능하면 활용 (Phase 4 / 5'.a / 본 세션 패턴).

### 권장 진입 순서 (v1 ship 준비)

```
┌─ Step 1 — 사용자 결정 (외부 절차)
│  └─ 7'.a'.x: minisign keypair 발급 + tauri.conf.json 갱신
│
├─ Step 2 — Security & Stability (Session V1-Critical-A)
│  ├─ 8'.0.a SQLCipher
│  ├─ 8'.0.b single-instance + panic + WAL
│  └─ 8'.0.c artifact retention
│
├─ Step 3 — UX & 6 Pillar 약속 (Session V1-Critical-B + V1-Portable + V1-Guide)
│  ├─ 8'.1 Multi-workspace
│  ├─ 11' Portable export/import     ← 6 pillar "Portable" 약속 실현
│  └─ 12' Guide system               ← 사용자 친화 핵심
│
├─ Step 4 — 출시 차단 해제 (사용자 결정 + Phase 7'.b)
│  ├─ Authenticode + Apple Dev 인증서
│  ├─ EULA 법무
│  └─ release.yml CI matrix
│
└─ Step 5 — v1 ship 🚢

(이후 v1.x — 8' / 9' / 10' / Env'.a 점진 처리)
```

본 계획서 `docs/research/phase-8p-9p-10p-residual-plan.md`는 이정표 — 페이즈 진입 시 세부 결정 노트로 전개.

### Standby 검증 — 이 plan으로 시작 가능한지

다음 사용자 신호 중 하나로 시작:
- "Phase 8'.0 진행" / "Phase 11' 시작" 등 명시 페이즈 호명.
- "v1 ship 준비 작업 시작" — Step 1 → Step 5 자동 chain.
- "다음 세션 이어서 진행" — RESUME → 본 계획서의 다음 unfinished 페이즈 자동 진입.

각 페이즈 진입 시 sub-agent 병렬 패턴 (Phase 4 / 5'.a / 6'.b 검증된 패턴) + 4-stage 흐름 (보강 리서치 → 결정 노트 6-section → 구현 → 검증) 따름.
