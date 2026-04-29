# Phase 1' — runtime-manager + 어댑터 보강 결정 노트

> 인라인 보강 (2026-04-27). Ollama API + LM Studio OpenAI-호환 패턴은 `crates/runtime-detector`의 probe 모듈에 baseline 있음.

## 1. 핵심 결정

| 항목 | 결정 | 근거 |
|---|---|---|
| Ollama 어댑터 method | detect/health/list_models/pull_model/remove_model/warmup 실제 구현. start/stop/restart는 no-op handle 반환 (외부 데몬). install/update는 bail (별도 installer crate가 책임) | ADR-0005 / wrap-not-replace |
| Ollama pull progress | non-stream (단순 POST). SSE streaming progress는 v1.x로 연기 — UI는 InstallProgress와 별개 (모델 카탈로그 단계) | 토큰/복잡도 |
| LM Studio 어댑터 method | detect/health/list_models/warmup 구현. pull/remove는 EULA 상 bail | EULA 준수 |
| OpenAI 호환 endpoint 처리 | LM Studio `:1234/v1/models` + `/v1/chat/completions` 사용 | OpenAI compat |
| RuntimeManager 신설 | trait object registry — `register(adapter)` + `get(kind)` + `list_kinds()` | ADR-0004 |
| 우선순위 라벨 | runtime-manager에 `priority(kind) -> u8` (Ollama=1, LM Studio=1, 나머지 후순위) | Phase 3' Gateway routing 시 사용 |
| HealthMonitor | 별도 sub-phase로 분리 (이번엔 health() one-shot만) | 토큰 효율 |
| 기존 stub 어댑터 (llama-cpp/kobold/vllm) | 그대로 unimplemented 유지 — Phase 5'+ | 우선순위 |
| HTTP client | reqwest 기본 client + 1.5s probe timeout (runtime-detector와 동일) | 일관성 |
| 에러 처리 | anyhow::Result 유지 (기존 trait 시그니처 그대로) | API 안정성 |
| 테스트 | wiremock + Ollama API mock (5건 어댑터별) | 표준 |

## 2. Ollama API 매핑

| RuntimeAdapter method | Ollama HTTP | 비고 |
|---|---|---|
| `detect()` | `GET /api/version` | `version` 필드 추출 → `DetectResult.version` |
| `health(handle)` | `GET /api/version` | latency 측정 + state |
| `list_models()` | `GET /api/tags` | `models[].name` → `LocalModel.file_rel_path` |
| `pull_model(ref, sink)` | `POST /api/pull { name, stream: false }` | non-stream. 완료 시 ProgressUpdate 1회 |
| `remove_model(ref)` | `DELETE /api/delete { name }` | |
| `warmup(handle, ref)` | `POST /api/generate { model, prompt: "", keep_alive: "5m" }` | 모델 로드 트리거 |
| `start(cfg)` | no-op | 외부 데몬 — handle만 반환 (instance_id="external") |
| `stop(handle)` | no-op | 외부 데몬 — bail 안 함 (호출 무해) |
| `restart(handle)` | no-op | |
| `install(opts)` | `bail` | installer crate가 책임 |
| `update()` | `bail` | Ollama 자체 업데이트 |

## 3. LM Studio API 매핑 (OpenAI 호환 :1234)

| method | LM Studio HTTP | 비고 |
|---|---|---|
| `detect()` | `GET /v1/models` | 200 = installed (version 미노출) |
| `health(handle)` | `GET /v1/models` | latency |
| `list_models()` | `GET /v1/models` | `data[].id` → LocalModel |
| `pull_model(ref)` | `bail` | EULA — 사용자 LM Studio UI 사용 안내 |
| `remove_model(ref)` | `bail` | 동일 |
| `warmup(handle, ref)` | `POST /v1/chat/completions { model, messages: [...], max_tokens: 1 }` | OpenAI 호환 |
| `start/stop/restart` | no-op | |
| `install(opts)` | `bail` | EULA — open_url만 |
| `update()` | `bail` | LM Studio 자체 업데이트 |

## 4. RuntimeManager (신설)

```rust
pub struct RuntimeManager {
    adapters: HashMap<RuntimeKind, Arc<dyn RuntimeAdapter>>,
}

impl RuntimeManager {
    pub fn new() -> Self;
    pub fn register(&mut self, adapter: Arc<dyn RuntimeAdapter>);
    pub fn get(&self, kind: RuntimeKind) -> Option<Arc<dyn RuntimeAdapter>>;
    pub fn list_kinds(&self) -> Vec<RuntimeKind>;
    pub fn priority(kind: RuntimeKind) -> u8; // Ollama/LMStudio=1, 나머지=2..N
}
```

## 5. 산출 파일

- `crates/runtime-manager/src/lib.rs` — `RuntimeManager` struct 추가 (기존 trait 위에).
- `crates/runtime-manager/src/manager.rs` (신설) — Manager 본체 + tests.
- `crates/adapter-ollama/Cargo.toml` — `time` workspace dep 추가 (latency 측정용 — 기존 `tokio::time::Instant` 가능).
- `crates/adapter-ollama/src/lib.rs` 재작성 (~280 LOC).
- `crates/adapter-lmstudio/src/lib.rs` 재작성 (~180 LOC).
- 어댑터 wiremock 통합 테스트 (각 ~150 LOC, 5+ cases).

## 6. 검증 체크리스트

- `cargo test -p runtime-manager` ✅ (manager registry tests)
- `cargo test -p adapter-ollama` ✅ (5 cases — detect/health/list/warmup/pull-error)
- `cargo test -p adapter-lmstudio` ✅ (4 cases — detect/health/list/warmup; pull bail)
- `cargo clippy --workspace -D warnings` ✅
- 누적 cargo 카운트 ≥ 165
