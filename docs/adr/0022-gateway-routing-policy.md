# ADR-0022: Gateway 라우팅 정책 + scoped key 모델

- Status: Accepted
- Date: 2026-04-27
- Related: ADR-0001 (gateway scaffolding), ADR-0006 (per-webapp key 초기 안), ADR-0009 (workspace), ADR-0016 (wrap-not-replace)
- 결정 노트: `docs/research/phase-3p-gateway-decision.md`

## Context

Phase 3' 진입 — 게이트웨이가 본격적으로 *외부 어댑터로 라우팅하는 OpenAI 호환 프록시*가 된다. 다음 5 영역이 동시에 결정 필요:

1. **OpenAI 호환 endpoint 표면** — files / images / audio / fine-tune 같은 광범위한 API 어디까지 노출?
2. **SSE pass-through 방식** — axum::Sse로 reformat할지, raw byte stream relay인지.
3. **GPU contention 직렬화** — single-PC single-user 컨텍스트에서 동시 inference OOM 방지.
4. **모델 → 어댑터 dispatch** — 정적 RoutingMap vs 매번 list_models 조회.
5. **per-webapp scoped key** — Origin 검증 + scope 차원 + 저장 보안.

기존 `crates/core-gateway` 스캐폴딩은 build_router + serve_with_shutdown + auth stub만 보유. Phase 3'에서 본격화한다.

## Decision

### 1. OpenAI 호환 endpoint 4종만 v1에 노출

```text
POST /v1/chat/completions   (stream 지원)
POST /v1/embeddings
GET  /v1/models
GET  /v1/models/:id
```

- files / images / audio / fine-tune / assistants는 v1.1+ 이월. 사용자 시나리오 비중 낮음.
- 관리 endpoint는 별도 prefix `/_admin/*` (admin scope 키 전용).
- `/health`, `/capabilities`는 무인증 (Phase 0 정책 유지).

### 2. SSE pass-through는 byte-perfect bytes_stream relay

업스트림(Ollama 0.4+ OpenAI compat / LM Studio)이 이미 OpenAI SSE 포맷이므로 `axum::response::Sse`로 재구성하지 않고 raw byte stream을 그대로 forward.

```rust
let upstream = reqwest_client.post(upstream_url).json(&body).send().await?;
let stream = upstream.bytes_stream();
Response::builder()
    .header("content-type", "text/event-stream")
    .header("cache-control", "no-cache")
    .header("x-accel-buffering", "no")
    .body(Body::from_stream(stream))
```

- **Backpressure / cancel propagation**: axum future drop → reqwest stream drop → upstream connection close → 업스트림 abort. 자연스러운 chain. reqwest 클라이언트에 `pool_idle_timeout(30s)` + `tcp_keepalive(10s)`.
- **Timeout 분리**: `/v1/chat/completions`는 600초 별도 TimeoutLayer (다른 라우트 30s).

### 3. GPU contention 직렬화 — `tokio::sync::Semaphore` (global, permits=1)

v1 single-user single-PC 컨텍스트에서 동시 inference는 OOM 위험이 너무 크다. 1-permit semaphore로 직렬화. 큐 대기 중 클라가 끊으면 acquire 자체가 future drop으로 취소.

응답에 `x-lmmaster-queue-wait-ms` 헤더로 대기 시간 노출 (디버깅).

**v1.1 확장**: per-runtime semaphore (Ollama=1, LM Studio=1로 분리하여 각자 1개씩 동시 실행). Phase 6'의 Pipelines와 결합 검토.

### 4. 모델 → 어댑터 dispatch — 5초 TTL `RoutingMap`

```rust
RoutingMap = HashMap<ModelId, (RuntimeKind, Arc<dyn RuntimeAdapter>)>
```

- 5초 TTL + 백그라운드 refresh task가 모든 어댑터에 `list_models()` 호출 후 merge.
- alias 충돌 정책:
  1. 사용자 카탈로그에서 "기본" 마킹된 어댑터 우선 (v1.1, 현재는 N/A).
  2. priority — Ollama=1, LM Studio=2, llama-cpp=3, kobold=4, vLLM=5.
- 미존재 모델 요청 → `404 model_not_found` (OpenAI 호환 envelope: `{"error":{"message":...,"type":"not_found_error","code":"model_not_found"}}`).

### 5. Per-webapp scoped key — 5차원 scope

```rust
ApiKey {
    id: String,                 // uuid v4
    alias: String,              // 사용자 라벨 ("내 블로그")
    key_prefix: String,         // "lm-abcd1234" (8자 표시용)
    key_hash: String,           // argon2id($plaintext, mem=64MB, iter=3, par=1)
    scope: ApiKeyScope,
    created_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
}

ApiKeyScope {
    models: Vec<String>,           // glob 패턴 ("exaone-*", "qwen2.5:*")
    endpoints: Vec<String>,        // glob ("/v1/chat/*", "/v1/models")
    allowed_origins: Vec<String>,  // 정확 매칭 (scheme+host+port)
    expires_at: Option<DateTime<Utc>>,
    project_id: Option<String>,    // Phase 6' (v1은 None)
    rate_limit: Option<RateLimit>, // schema만, enforce는 v1.1
}
```

### 6. CORS + Origin 헤더 이중 검증

브라우저에서 직접 호출(예: `OpenAI({ baseURL, apiKey, dangerouslyAllowBrowser: true })`) 시나리오를 안전하게:

- 매 request마다 `Origin` 헤더가 키의 `allowed_origins` whitelist에 정확 매칭되는지 검증 (scheme + host + port 모두 일치).
- CORS 응답 헤더는 **요청 키의 origin whitelist에서만** echo back. `Access-Control-Allow-Origin: *` 결코 사용하지 않음.
- preflight (`OPTIONS`)는 키 없이도 통과 — 단 `Access-Control-Allow-Origin`은 origin이 *어떤* 키의 whitelist에 있을 때만 echo. 없으면 일반 200 무내용.

### 7. 키 저장 — SQLite + argon2id 해시 (SQLCipher default off)

- 평문은 **발급 응답에서 1회만** 노출 후 폐기. 메모리에서도 즉시 zeroize.
- DB에는 `argon2id($plaintext)` + `key_prefix` 8자만 저장.
- argon2 파라미터: memory 64MB, iterations 3, parallelism 1 (OWASP 2024 권장).
- SQLCipher는 v1 default off — `LMMASTER_ENCRYPT_DB=1` env로 opt-in. v1.1에서 GUI 토글 + 마스터 패스워드 ADR.

**검증 흐름**:
```text
incoming key "lm-abcd1234XXXX..."
↓
key_prefix("lm-abcd1234")로 DB 후보 lookup (인덱스 hit)
↓
후보 row의 key_hash = argon2id($incoming) verify
↓
revoked_at IS NULL && expires_at > now()
↓
Origin 헤더 ∈ scope.allowed_origins
↓
request path matches scope.endpoints (glob)
↓
request body의 model matches scope.models (glob)
↓
permit acquire → upstream forward
```

### 8. Portable workspace fingerprint repair — 3-tier

| Tier | 조건 | 동작 |
|---|---|---|
| green | os+arch 일치 (이전 fingerprint와) | silent — 아무 알림 없음 |
| yellow | os 같음 / GPU 계열 다름 (예: NVIDIA → AMD) | toast "이 PC에서 처음 실행하는 환경이에요. 벤치 캐시를 새로 측정할게요" + `cache/bench/*` `cache/scan/*` invalidate |
| red | os/arch mismatch (Windows ↔ macOS 등) | modal "다른 운영체제에서 가져온 워크스페이스예요. 런타임을 다시 설치해야 동작해요" + 가이드 링크 + `manifest.runtimes_installed[]` invalidate. `runtimes/` 디렉터리 자체는 보존(cross-compile 가능성). |

- **모델 파일 (`models/`)은 모든 tier에서 보존** — GGUF는 OS-agnostic.
- DB encryption key는 OS keyring (macOS Keychain / Windows Credential Manager / Linux Secret Service)에 fingerprint별로 저장. red tier에서 keyring entry도 새로 생성.

### 9. OpenAI 호환 TS SDK = `openai` npm wrapper

```ts
import OpenAI from "openai";

export class LmmasterClient extends OpenAI {
  constructor(opts: LmmasterClientOptions) {
    super({
      baseURL: opts.baseURL ?? "http://127.0.0.1:0",
      apiKey: opts.apiKey,
      ...opts,
    });
  }
  // 추가 helper: lmmaster.keys.create({...}), lmmaster.keys.revoke(id), etc.
}
```

- `openai` v4+ peer dependency lock. 자체 streaming reparse 안 함.
- 사용자는 endpoint URL만 바꿔서 기존 OpenAI client 코드 그대로 사용 가능.
- v5 breaking change 대비 — peer dep 범위 명시 + CI에서 v4/v5 매트릭스 테스트 (v1.1).

### 10. API 키 발급 GUI — "1회 reveal + alias 필수 + 자동 mask"

- 발급 modal: 한국어 해요체 ("이 키는 지금만 보여드려요. 닫으면 다시 볼 수 없어요").
- 클립보드 카피 버튼 + 8초 후 자동 mask + 5분 후 modal auto-close.
- 키 목록 화면: prefix 8자만 표시 (`lm-abcd1234·····`). reveal 버튼 없음 — 새로 발급 only.
- 회수(revoke)는 idempotent. UI는 즉시 disabled 표시 + 24h 후 hard delete.

## Consequences

**긍정**:
- OpenAI 호환 SDK가 그대로 돈다 → 기존 웹앱이 endpoint URL 변경만으로 통합.
- byte-perfect SSE relay → reformat 버그 가능성 0.
- semaphore 1-permit으로 OOM 위험 명확히 제거.
- 1회 reveal + argon2id로 키 누출 위험 최소화.

**부정**:
- semaphore=1은 동시 사용자 거부와 동등 (single-PC OK, 팀 모드 v1.1+ 이월).
- per-key rate limit / quota는 schema만 두고 enforce는 v1.1 (사용자 신뢰가 충분히 높은 v1 시나리오).
- SQLCipher off는 USB 분실 시 키 노출 위험 — 사용자에게 명시적으로 안내 + opt-in env 제공.
- openai v5 breaking change 시 SDK 영향 — peer dep 범위로 lock + CI 매트릭스로 완화.

**감내한 트레이드오프**:
- v1 단순성 vs v1.1 확장성 — 단순성 우선. semaphore / rate limit / SQLCipher 모두 v1.1에서 깊이.
- byte-perfect relay vs 우리 측 검증 — 검증은 진입(scope/origin)에서만. 응답 stream 자체는 신뢰.

## Alternatives considered (negative space — 결정 노트 §3 미러)

- **LiteLLM proxy 임베드** — Python 의존성 + 외부 provider 다중화는 외부 통신 0 정책 위반.
- **Open WebUI 대체** — UI까지 통째 재발명. 우리는 wrap-not-replace + Korean-first 아이덴티티가 본질.
- **SSE 자체 reparse + axum::Sse 재구성** — chunked OpenAI SSE 포맷이 모두 같으므로 reparse는 reinvent. byte-perfect가 단순 + 안전.
- **SQLCipher default ON + 마스터 패스워드** — portable USB 시나리오에서 패스워드 UX 부담 과도. v1은 opt-in.
- **per-runtime semaphore (v1)** — single-PC에서 Ollama+LMStudio 동시 GPU 사용은 contention 보장. v1.1에서 검토.
- **자체 SSE 파싱 SDK** — openai v4 wrapper면 기존 생태계 호환 + 유지보수 0.
- **Origin 검증 없이 키만** — 브라우저 직접 호출 시 키 leak 위험 (devtools에서 Auth 헤더 노출). origin whitelist가 필수.
- **infinite TTL key** — 만료 없는 키는 leak 시 영구 위험. expires_at 옵션 필수.

## 검증 invariant

- byte-perfect SSE relay — wiremock으로 chunk 순서/개행 정확 일치.
- cancel propagation — 클라이언트 abort 시 업스트림 connection close 1초 내.
- semaphore — 동시 2 request 제출 시 두 번째는 첫 번째 완료까지 block.
- Origin 매트릭스 — exact match / port mismatch / scheme mismatch / null origin / origin spoof 5종.
- scope glob — `exaone-*`이 `exaone-3.5-7.8b`는 매치, `qwen-7b`는 거부.
- error envelope — OpenAI 호환 `{ error: { message, type, code } }` 형식.
- argon2 결정성 — 같은 plaintext + salt면 같은 hash, 다른 plaintext면 verify=false.
- fingerprint 5 시나리오 — green/yellow/red 각 1개 + edge (vram null) + cross-os.

## References

- 결정 노트: `docs/research/phase-3p-gateway-decision.md`
- LiteLLM proxy / Open WebUI / Ollama 0.4 OpenAI compat / vLLM AsyncLLMEngine.
- OWASP Argon2id 권장 (2024).
- Tailscale device fingerprint repair 패턴.
- JetBrains / Stripe / Cursor / Anthropic Console 키 발급 UX.
