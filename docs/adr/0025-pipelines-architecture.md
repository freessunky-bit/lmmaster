# ADR-0025: Pipelines — gateway-side filter modules

- Status: Accepted
- Date: 2026-04-28
- Related: ADR-0013 (외부 통신 0), ADR-0016 (wrap-not-replace), ADR-0022 (gateway routing — scope/token_budget), ADR-0023 (workbench 정책), ADR-0024 (knowledge-stack RAG)
- 결정 노트: `docs/research/phase-6p-updater-pipelines-decision.md`

## Context

Phase 6'.a 진입 — Open WebUI는 Pipelines를 *UI*에 적용하지만, LMmaster의 차별화 thesis #7은 **gateway layer에 Pipelines를 적용**하는 것. 웹앱은 그대로 OpenAI 호환 호출만 하는데, gateway가 PII redact / token quota / observability를 wire-level filter로 끼움 → 웹앱 코드 무수정.

다음 4 영역이 동시에 결정 필요:

1. **filter 실행 위치** — 외부 LLM filter / sidecar 프로세스 / gateway 내부 어디?
2. **filter 인터페이스** — chain pattern? middleware stack? per-request handler?
3. **per-route activation** — 모든 라우트에 강제? 키 단위 토글?
4. **audit / observability** — 매 filter 실행을 어떻게 trace할지.

기존 `crates/core-gateway`는 Phase 0 supervisor + Phase 3' routing scaffold만 보유. 본 ADR에서 Pipelines surface를 추가한다.

## Decision

### 1. Pipelines = gateway 내부 Rust trait + ordered chain

```rust
#[async_trait]
pub trait Pipeline: Send + Sync {
    fn id(&self) -> &str;
    async fn pre_request(&self, ctx: &mut PipelineContext, body: &mut serde_json::Value)
        -> Result<(), PipelineError>;
    async fn post_response(&self, ctx: &mut PipelineContext, body: &mut serde_json::Value)
        -> Result<(), PipelineError>;
}

pub struct PipelineContext {
    pub request_id: String,
    pub api_key_id: String,
    pub route: String,
    pub model: String,
    pub tokens_consumed: u64,
}

pub struct PipelineChain {
    pub pipelines: Vec<Arc<dyn Pipeline>>,
}
```

- Pipeline은 *gateway 프로세스 내부*에서 동작. sidecar 프로세스 없음 → IPC 비용 0.
- chain은 `Vec<Arc<dyn Pipeline>>` 순서대로 `pre_request`. `post_response`는 LIFO (middleware standard).
- v1은 *full response*에만 적용. streaming chunk transformation은 v1.x (byte-perfect SSE relay 보존을 우선).

### 2. v1 3종 Pipeline 시드

- **PromptSanitize** — 한국어 NFC 정규화 + 공백 normalize + 제어 문자 strip. RAG ingest pipeline (Phase 4.5' `chunker::normalize_korean`)과 동일 결.
- **ResponseTrim** — leading/trailing whitespace strip. 빈 응답이면 `PipelineError::EmptyResponse` → 422 reject.
- **TokenQuotaCheck** — `scope.token_budget` 초과 시 즉시 reject. `tokens_consumed`를 ctx에 누적해 다음 Pipeline이 읽을 수 있게 함.

### 3. Per-route + per-key activation

`Pipeline::id()`를 키로 매 ApiKey에 `enabled_pipelines: Vec<String>` 추가. 라우트 단위로도 activation 매트릭스 (`/v1/chat/*`, `/v1/embeddings`, `/v1/models` 각자 독립).

```rust
ApiKeyScope {
    enabled_pipelines: Vec<String>,  // ["prompt-sanitize", "response-trim", "token-quota"]
    ...  // ADR-0022 기존 5 dimension + pipelines
}
```

### 4. Audit log — 매 Pipeline 실행 1줄 tracing::info!

```text
INFO request_id=abc123 pipeline=prompt-sanitize stage=pre_request duration_us=240
INFO request_id=abc123 pipeline=token-quota stage=pre_request duration_us=18
INFO request_id=abc123 pipeline=response-trim stage=post_response duration_us=92
```

- `tracing::info_span!` 1 span per Pipeline × (pre_request + post_response) — 2 events per Pipeline.
- request_id는 `tower-http::request-id` 미들웨어와 propagate.
- Phase 6'.c에서 `/diagnostics export`로 사용자 동의 후 JSON 노출.

## Consequences

**긍정**:
- **외부 통신 0 유지** — Pipeline은 모두 Rust 내부 로직. Guardrails AI / NeMo Guardrails 같은 외부 LLM call 없음.
- **검증 가능** — 매 Pipeline은 deterministic 함수 + 단위 테스트 가능. RAG / Workbench와 같은 test invariant 패턴 (idempotency / 빈 입력 / 한글 정규화 round-trip).
- **웹앱 코드 무수정** — 기존 OpenAI 호환 client는 base URL만 바꿔서 그대로. PII redact / quota는 gateway side에서 자동 적용.
- **Per-key 토글** — 사내 시범 vs 개인 개발자 다른 정책 적용 가능 (Phase 6'.c Settings UI).

**부정**:
- **filter 로직은 모두 Rust** — Python 생태계 사용자 정의 filter 불가능. Phase 2+에서 sidecar marketplace 검토.
- **streaming chunk transformation 미지원** — full response에만 적용. SSE chunk 단위 PII redact는 v1.x.
- **schema migration** — `ApiKeyScope.enabled_pipelines` 필드가 기존 키에 추가됨. v1 출시 후 schema 변경 시 마이그레이션 필요.

**감내한 트레이드오프**:
- chain order의 부작용 — Pipeline A의 mutate 결과를 Pipeline B가 입력으로 받음. cycle/conflict 발생 시 audit log로 추적. v1은 3종 Pipeline 모두 독립 → cycle 없음.
- `tokens_consumed` 측정 정확도 — full response에서만 측정 가능. streaming 중 실시간 토큰 누적은 v1.x.
- LIFO post_response — 사용자 직관(filter 같은 순서 적용)과 다를 수 있음. middleware 패턴이라 docstring + 예시로 명시.

## Alternatives considered (negative space — 결정 노트 §2.5–§2.7 미러)

### a. 외부 LLM filter (Guardrails AI / NeMo Guardrails)

거부. Guardrails AI는 OpenAI moderation API call이 default → 외부 통신 0 정책 (ADR-0013) 위반. 한국어 필터 약함. Python 의존성 부담.

### b. Gateway 외부 sidecar 프로세스

거부. IPC latency가 byte-perfect SSE relay (ADR-0022 §2) 정신과 충돌. lifecycle 복잡도 (sidecar crash → supervisor 재시작) 증가. Tauri 단일 프로세스 모델의 단순함을 잃음.

### c. 모델 fine-tune으로 안전성 보장

거부. 카탈로그 5개 모델 × 분기 신규 출시 → 매번 재학습. deterministic 보장 안 됨 (확률적 출력). 한국어 능력 + 도메인 정확도 손상 위험.

### d. UI plugin (Open WebUI 패턴 그대로)

거부. *UI*에 적용하면 LMmaster 사용자만 보호되고 *우리 SDK 통해 들어오는 외부 웹앱*은 무방비. gateway-level이 thesis #7의 본질.

### e. Streaming chunk transformation v1 지원

거부. byte-perfect SSE relay는 chunk 단위 transformation 어려움. v1은 *full response* 적용 + streaming은 v1.x. 사용자 99%에게 full-mode가 충분.

### f. Per-route 강제 activation (모든 라우트에 모든 Pipeline)

거부. `/health`나 `/capabilities`에 PromptSanitize 적용은 무의미. per-route + per-key 토글이 정확.

## 검증 invariant

- **order preservation** — `pipelines[0].pre_request` → `pipelines[1].pre_request` 순. `post_response`는 역순 (LIFO).
- **early termination** — `pre_request` Err → chain 중단 + upstream 호출 안 함.
- **idempotency** — 같은 ctx + body로 두 번 호출해도 동일 결과 (NFC × 2 = NFC × 1).
- **per-route activation** — `/v1/chat/completions`에는 적용, `/health`에는 적용 안 함.
- **per-key activation** — ApiKey A에는 token-quota 적용, B에는 미적용.
- **audit log** — 매 Pipeline 실행은 `tracing::info!` 1줄 (request_id + pipeline_id + duration_us).
- **3종 Pipeline 단위 테스트** — PromptSanitize NFC + 빈 입력 graceful, ResponseTrim 빈 응답 422, TokenQuotaCheck budget 초과 reject.
- **PipelineError 한국어** — `EmptyResponse("응답이 비어 있어요")` / `QuotaExceeded("토큰 한도를 초과했어요: {used}/{budget}")`.

## References

- 결정 노트: `docs/research/phase-6p-updater-pipelines-decision.md`
- ADR-0013 (외부 통신 0)
- ADR-0016 (wrap-not-replace — 외부 런타임 lifecycle 존중과 같은 결로 웹앱 lifecycle 존중)
- ADR-0022 (scope.token_budget — TokenQuotaCheck Pipeline의 입력)
- Open WebUI Pipelines: <https://github.com/open-webui/pipelines>
- Guardrails AI: <https://github.com/guardrails-ai/guardrails>
- NeMo Guardrails: <https://github.com/NVIDIA/NeMo-Guardrails>
- OWASP filter chain: <https://cheatsheetseries.owasp.org/cheatsheets/Input_Validation_Cheat_Sheet.html>
- Llama Guard (Meta): <https://github.com/facebookresearch/PurpleLlama>
