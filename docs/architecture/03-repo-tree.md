# 5. Repo 구조

> 산출물 #5. 모노레포 디렉터리 트리와 각 위치의 책임.

## 5.1 최상위 트리

```
LMmaster/
├─ apps/
│  └─ desktop/                       # Tauri 2 + React 데스크톱 앱
│     ├─ src/                         # React 프런트
│     │  ├─ pages/                    # 9 화면(home, catalog, install, runtimes, projects, keys, workbench, logs, settings)
│     │  ├─ components/               # 앱 전용 컴포넌트(공유는 design-system)
│     │  ├─ hooks/
│     │  ├─ ipc/                      # Tauri invoke 래퍼
│     │  ├─ i18n/ko.json              # 한국어 카피(기본)
│     │  └─ main.tsx
│     ├─ src-tauri/                   # Tauri Rust backend
│     │  ├─ src/
│     │  │  ├─ main.rs                # 앱 엔트리, supervisor 부트
│     │  │  ├─ commands/              # tauri::command 핸들러
│     │  │  └─ events.rs              # IPC event 채널
│     │  ├─ tauri.conf.json
│     │  └─ Cargo.toml
│     └─ package.json
│
├─ crates/
│  ├─ core-gateway/                   # Local HTTP Gateway (Axum)
│  │  ├─ src/
│  │  │  ├─ lib.rs
│  │  │  ├─ routes/
│  │  │  │  ├─ chat.rs                # /v1/chat/completions (+stream)
│  │  │  │  ├─ models.rs              # /v1/models
│  │  │  │  ├─ embeddings.rs          # 스키마 only(v1)
│  │  │  │  ├─ health.rs              # /health, /capabilities
│  │  │  │  └─ admin.rs               # /_admin/* (GUI 전용)
│  │  │  ├─ auth.rs                   # API key middleware
│  │  │  ├─ usage_log.rs
│  │  │  └─ router.rs                 # 모델→런타임 라우팅 + 폴백
│  │  └─ Cargo.toml
│  │
│  ├─ runtime-manager/                # 자식 프로세스 supervisor + 상태기계
│  │  ├─ src/
│  │  │  ├─ supervisor.rs
│  │  │  ├─ state.rs                  # Cold/Warming/Standby/Active
│  │  │  └─ adapter.rs                # RuntimeAdapter trait
│  │  └─ Cargo.toml
│  │
│  ├─ adapter-llama-cpp/              # llama.cpp server adapter
│  ├─ adapter-koboldcpp/
│  ├─ adapter-ollama/
│  ├─ adapter-lmstudio/
│  ├─ adapter-vllm/
│  │
│  ├─ hardware-probe/                 # OS/CPU/RAM/GPU/VRAM/디스크/capability
│  │  ├─ src/
│  │  │  ├─ os.rs
│  │  │  ├─ cpu.rs
│  │  │  ├─ memory.rs
│  │  │  ├─ gpu/                      # nvidia, amd, intel, apple
│  │  │  ├─ disk.rs
│  │  │  └─ capability.rs             # cuda/rocm/vulkan/metal/directml
│  │  └─ Cargo.toml
│  │
│  ├─ model-registry/                 # manifest sync + cache + recommender
│  │  ├─ src/
│  │  │  ├─ manifest.rs               # 스키마
│  │  │  ├─ sync.rs                   # 원격 fetch + cache
│  │  │  ├─ cache.rs
│  │  │  ├─ recommender.rs            # deterministic 점수 함수
│  │  │  └─ category.rs
│  │  └─ Cargo.toml
│  │
│  ├─ portable-workspace/             # workspace manifest + path 해석
│  │  ├─ src/
│  │  │  ├─ manifest.rs
│  │  │  ├─ paths.rs
│  │  │  └─ repair.rs                 # fingerprint mismatch 시 복구
│  │  └─ Cargo.toml
│  │
│  ├─ key-manager/                    # API 키 발급/검증/scope/사용 로그
│  │  ├─ src/
│  │  │  ├─ store.rs                  # SQLite/SQLCipher
│  │  │  ├─ scope.rs
│  │  │  └─ middleware.rs             # gateway에서 사용
│  │  └─ Cargo.toml
│  │
│  └─ shared-types/                   # crate 간 공통 타입(serde-able)
│     └─ Cargo.toml
│
├─ packages/
│  ├─ design-system/                  # 데스크톱 + 기존 웹앱 공유
│  │  ├─ src/
│  │  │  ├─ tokens.css
│  │  │  ├─ tokens.ts
│  │  │  ├─ base.css
│  │  │  ├─ components.css
│  │  │  ├─ layout.css
│  │  │  ├─ motion.css
│  │  │  ├─ react/                    # Button, Input, Card, Sidebar, ...
│  │  │  └─ voice.md                  # 한국어 voice & tone
│  │  └─ package.json
│  │
│  └─ js-sdk/                         # @lmmaster/sdk (npm)
│     ├─ src/
│     │  ├─ index.ts
│     │  ├─ client.ts
│     │  ├─ chat.ts                   # streamChat / chatCompletions
│     │  ├─ install.ts                # installRuntime / installModel / progress
│     │  ├─ keys.ts                   # issueApiKey / listApiKeys
│     │  ├─ projects.ts               # bindProject / getProjectBindings
│     │  ├─ discovery.ts              # pingHealth / autoFindGateway
│     │  └─ types.ts
│     └─ package.json
│
├─ workers/
│  └─ ml/                             # Python sidecar (v1 placeholder)
│     ├─ pyproject.toml
│     ├─ src/lmmaster_ml/
│     │  ├─ server.py                 # JSON-RPC over stdio
│     │  ├─ jobs/sft.py               # placeholder
│     │  ├─ jobs/quantize.py          # placeholder
│     │  └─ jobs/export.py            # placeholder
│     └─ README.md
│
├─ manifests/
│  └─ models/                         # curated model manifest seed
│     ├─ index.json                   # 카테고리 트리
│     ├─ agents/
│     ├─ roleplay/
│     ├─ coding/
│     ├─ sound-stt/
│     ├─ sound-tts/
│     └─ slm/
│
├─ examples/
│  └─ webapp-local-provider/          # 기존 웹앱 통합 예제(미니)
│     ├─ src/
│     │  ├─ providers/local-companion.ts
│     │  └─ App.tsx
│     └─ package.json
│
├─ docs/
│  ├─ architecture/                   # 산출물 1·2·4·5
│  │  ├─ 00-overview.md
│  │  ├─ 01-rationale-companion.md
│  │  ├─ 02-roadmap.md
│  │  └─ 03-repo-tree.md
│  ├─ adr/                            # 산출물 3
│  │  ├─ README.md
│  │  └─ 0001~0014-*.md
│  ├─ risks.md                        # 산출물 6
│  ├─ oss-dependencies.md             # 산출물 7
│  ├─ design-system-plan.md           # 산출물 9
│  ├─ guides-ko/                      # 한국어 사용자 문서
│  │  ├─ ui-ia.md                     # 산출물 8
│  │  ├─ getting-started.md
│  │  ├─ install-models.md
│  │  ├─ webapp-integration.md
│  │  ├─ api-keys.md
│  │  ├─ troubleshooting.md
│  │  └─ workbench-future.md
│  └─ guides-dev/                     # 개발자 문서
│     ├─ sdk-integration.md
│     ├─ adapter-authoring.md
│     └─ contributing.md
│
├─ .github/
│  └─ workflows/                      # CI: rust + js + tauri build matrix
│
├─ Cargo.toml                         # workspace
├─ package.json                       # pnpm/npm workspace
├─ pnpm-workspace.yaml
├─ rust-toolchain.toml
├─ .gitignore
├─ LICENSE
└─ README.md                          # 한국어 우선
```

## 5.2 워크스페이스 매니페스트

`Cargo.toml` (workspace):
```toml
[workspace]
resolver = "2"
members = [
  "apps/desktop/src-tauri",
  "crates/core-gateway",
  "crates/runtime-manager",
  "crates/adapter-llama-cpp",
  "crates/adapter-koboldcpp",
  "crates/adapter-ollama",
  "crates/adapter-lmstudio",
  "crates/adapter-vllm",
  "crates/hardware-probe",
  "crates/model-registry",
  "crates/portable-workspace",
  "crates/key-manager",
  "crates/shared-types",
]
```

`pnpm-workspace.yaml`:
```yaml
packages:
  - "apps/desktop"
  - "packages/*"
  - "examples/*"
```

## 5.3 빌드 산출물

- 데스크톱 앱: `apps/desktop/src-tauri/target/release/bundle/<os>/...`
- SDK: `packages/js-sdk/dist/` → npm publish.
- 디자인 시스템: `packages/design-system/dist/` → npm publish.
- Manifest: `manifests/models/` → 정적 호스팅(별도 CDN/repo).

## 5.4 명명 규칙

- Rust crate: `lmmaster-<name>` (단, 워크스페이스 디렉터리는 `<name>` 으로 짧게).
- npm package: `@lmmaster/<name>`.
- Python: `lmmaster-ml`.
- 환경 변수 prefix: `LMMASTER_`.
- DB 파일: `metadata.sqlite`, `secrets.sqlcipher`, `cache.sqlite`.
- 로그 파일: `logs/{component}-YYYYMMDD.log` (회전).

## 5.5 의존 방향(아키텍처 보존을 위한 규칙)

- `apps/desktop` → `crates/*` 직접 참조 가능(같은 프로세스).
- `crates/core-gateway` → `crates/runtime-manager` → `crates/adapter-*` (단방향).
- `crates/key-manager`, `crates/portable-workspace`, `crates/hardware-probe`, `crates/model-registry`는 leaf-ish — 다른 crate에 의존하지 않는다(공통 타입은 `shared-types`).
- `packages/js-sdk`는 **gateway HTTP API에만** 의존 — Rust crate를 import하지 않음.
- `workers/ml`은 stdio JSON-RPC 경계만 — Rust crate import 없음.

이 방향이 깨지는 PR은 거부 대상.
