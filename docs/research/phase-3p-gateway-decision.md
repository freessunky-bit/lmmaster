# Phase 3' — Gateway proxy + per-webapp scoped key + portable repair + OpenAI 호환 SDK 결정 노트

> 작성일: 2026-04-27
> 상태: 보강 리서치 완료 → 설계 확정 (구현 직전)
> 선행: Phase 0 (gateway 스캐폴딩), Phase 1' (runtime-manager + Ollama/LMStudio adapter), Phase 2'.a~c (카탈로그 + 벤치)
> 후행: Phase 4' (Workbench v1 — 이 게이트웨이를 첫 소비자로 사용), Phase 5' (Pipelines), Phase 6' (Team mode)
> 신설 ADR: **ADR-0022 — Gateway 라우팅 정책 + scoped key 모델**

---

## 0. 결정 요약 (10가지)

1. **OpenAI 호환 endpoint 4종만 v1에 노출** — `POST /v1/chat/completions` (stream 지원), `POST /v1/embeddings`, `GET /v1/models`, `GET /v1/models/:id`. files / images / audio / fine-tune / assistants는 v1.1+ 이월. 관리는 별도 prefix `/_admin/*` (admin scope 키 전용).
2. **SSE pass-through는 `reqwest::Response::bytes_stream()` → `axum::body::Body::from_stream`** — `axum::response::Sse`로 재포맷팅 안 함. 업스트림(Ollama OpenAI compat 모드 / LM Studio)이 이미 OpenAI SSE 형식이므로 byte-perfect relay. 단 `model` 필드 dispatch만 별도 처리.
3. **모델 → 어댑터 라우팅 테이블은 in-memory `RoutingMap`** — 키 = `model_id` (alias 포함), value = `(RuntimeKind, upstream_handle)`. 매 request시 RuntimeManager에서 최신 `list_models()` 결과를 5초 TTL로 캐시. 모호한 alias (양쪽 런타임에 같은 이름)는 Ollama 우선 (priority=1, 동률이면 LM Studio가 priority=2).
4. **GPU contention 직렬화 — `tokio::sync::Semaphore`(global, permits=1)** — v1 single-user single-PC 컨텍스트에서 동시 inference는 OOM 위험이 너무 크다. 단순 1-permit semaphore로 직렬화하고 큐 대기 중인 request는 pending 표시. v1.1에서 per-runtime semaphore (Ollama=1, LM Studio=1로 분리하여 각자 1개씩)로 확장 검토.
5. **Per-webapp scoped key — 5개 차원 scope** — `models: glob[]`, `endpoints: glob[]`, `allowed_origins: Vec<Origin>` (CORS+Origin 헤더 결합 검증), `expires_at: Option<DateTime>`, `project_id: Option<String>`. v1은 rate_limit / quota는 **schema에 필드만 두고 enforce는 v1.1로 이월**. project_id는 Phase 6'에서 활성.
6. **CORS + Origin 헤더 이중 검증** — 키에 `allowed_origins: ["https://my-webapp.com", "http://localhost:5173"]`을 박고, 매 request마다 `Origin` 헤더가 키의 whitelist에 정확히 매칭(scheme + host + port)되는지 검증. browser-side 호출 (jeff/openai SDK가 브라우저에서 `dangerouslyAllowBrowser: true`로 직접 호출) 시나리오를 안전하게 만든다. CORS 응답 헤더는 **요청 키의 origin whitelist에서만** echo back — `*`는 결코 사용하지 않음.
7. **키 저장 — SQLite + at-rest argon2id 해시 (SQLCipher는 v1 옵션 미사용)** — 평문은 1회 issue 응답에서만 표시 후 폐기. DB에는 `argon2id($plaintext)` (메모리 64 MB / iter 3 / parallelism 1) + `key_prefix` 8자만 저장. SQLCipher는 portable USB 시나리오에서 마스터 패스워드 UX 부담이 커서 v1 default off — `LMMASTER_ENCRYPT_DB=1` env로 opt-in (v1.1 ADR 후 GUI 토글).
8. **Portable workspace fingerprint repair는 3-tier — green / yellow / red** — green: os+arch 일치 → silent. yellow: os 같음 / GPU 계열 다름 → toast "이 PC에서 처음 실행하는 환경이에요. 벤치 캐시를 새로 측정할게요" + 자동 invalidate `cache/bench/*`, `cache/scan/*`. red: os/arch mismatch → modal "다른 운영체제에서 가져온 워크스페이스예요. 런타임을 다시 설치해야 동작해요" + 가이드 + `runtimes/` 보존 (cross-compile 가능성 대비) + `manifest.runtimes_installed[]` invalidate. 모델 파일 (`models/`)은 GGUF agnostic이라 모든 tier에서 보존.
9. **OpenAI 호환 TS SDK는 `openai` npm 패키지 wrapper (자체 구현 안 함)** — `@lmmaster/sdk`는 `OpenAI` 클라이언트를 `extends`하는 thin wrapper로, 생성자에 baseURL 자동 주입 + 키 발급/회수 helper만 추가. raw fetch 자체 구현은 streaming chunked 파싱 reinvent + breaking change tracking 부담이라 거부.
10. **API 키 발급 GUI는 "발급 시 1회 reveal + alias 필수 + reveal 후 자동 mask" 패턴** — JetBrains / Stripe / Cursor 공통 패턴. 발급 modal은 한국어 해요체 ("이 키는 지금만 보여드려요. 닫으면 다시 볼 수 없어요"), 클립보드 카피 버튼 + 8초 후 자동 mask + 5분 후 modal auto-close. 키 목록 화면에는 prefix 8자(`lm-abcd1234`)만 표시.

---

## 1. 채택안

### 1.1 OpenAI-compatible proxy 라우팅 (Axum SSE pass-through)

**노출 endpoint** (v1):
- `POST /v1/chat/completions` — body의 `model` 필드 inspect → RoutingMap lookup → 선택된 어댑터로 forward. `stream: true`면 SSE pass-through, false면 단발 JSON.
- `POST /v1/embeddings` — 동일 dispatch. v1은 Ollama만 지원 (`/api/embeddings`), LM Studio가 embedding 모델 로드된 경우 OpenAI compat. 미지원 모델은 `400 unsupported_endpoint`.
- `GET /v1/models` — RuntimeManager의 모든 어댑터에서 `list_models()` 합쳐서 OpenAI `{ data: [{ id, object: "model", owned_by, ... }] }` 형식으로 반환. owned_by는 runtime kind 문자열 (예: `"ollama"` / `"lmstudio"`).
- `GET /v1/models/:id` — 단일 모델 메타 + (있으면) 벤치 결과 hint 포함.

**SSE pass-through Rust 패턴 (채택)**:
업스트림이 이미 OpenAI SSE 포맷 (Ollama 0.4+의 `/v1/chat/completions` OpenAI compat 엔드포인트, LM Studio 기본)이라 굳이 우리가 reparse 후 axum::Sse로 재구성할 필요 없다. 채택안:

```text
let upstream = reqwest_client.post(upstream_url).json(&body).send().await?;
let stream = upstream.bytes_stream();
Response::builder()
    .header("content-type", "text/event-stream")
    .header("cache-control", "no-cache")
    .header("x-accel-buffering", "no")
    .body(Body::from_stream(stream))
```

**Backpressure / cancel propagation**: `axum::extract::Request`의 connection이 닫히면 axum이 future를 drop하고, drop이 reqwest stream을 drop하고, reqwest가 connection 닫음 → 업스트림 (Ollama)이 자체적으로 abort. 이 chain이 자연스럽게 작동하려면 reqwest 클라이언트에 `pool_idle_timeout`을 짧게(30s) 두고 `tcp_keepalive(Duration::from_secs(10))`을 활성. axum 측에서는 `tower_http::timeout::TimeoutLayer`를 chat/completions 라우트에는 600초로 별도 적용 (다른 라우트 30s와 분리).

**GPU contention 직렬화 (semaphore)**: `Arc<Semaphore>`를 `AppState`에 두고 chat/completions과 embeddings 진입 시 `acquire_owned()`. permit이 acquire되면 그제야 upstream forward. 큐 대기 중 클라이언트가 끊기면 future drop으로 acquire 자체가 취소. 응답에는 `x-lmmaster-queue-wait-ms` 헤더로 대기 시간 노출 (디버깅용).

**모델 → 어댑터 dispatch (RoutingMap)**:
```text
RoutingMap = {
    "qwen2.5:7b" -> (Ollama, OllamaHandle),
    "lmstudio-community/Qwen2.5-7B-Instruct-GGUF" -> (LMStudio, LmHandle),
    ...
}
```
구축: 5초 TTL 캐시 + 매 5초 백그라운드 refresh task가 모든 어댑터에 `list_models()` 호출 후 merge. alias 충돌 정책: 사용자가 카탈로그에서 특정 모델을 "기본" 마킹한 경우 그 어댑터 우선, 미마킹 시 priority(Ollama=1, LM Studio=2). request body의 `model` 값이 RoutingMap에 없으면 `404 model_not_found` (OpenAI 호환 envelope).

**차용한 글로벌 사례**:
- **LiteLLM proxy** — `/v1/chat/completions` body의 `model` 필드를 routing key로 사용. LiteLLM은 외부 provider까지 다중화하지만 우리는 로컬 어댑터 2종만이라 `RoutingMap` 1단계로 충분.
- **Open WebUI** — Ollama + OpenAI 호환 backend 동시 노출 패턴. 우리는 외부 OpenAI 호출은 명시적으로 거부 (외부 통신 0).
- **Ollama 0.4+ OpenAI compat** — `/v1/chat/completions`을 Ollama가 직접 노출하므로 우리 gateway는 단순 forward만 — Ollama → OpenAI 변환 직접 안 함.
- **vLLM AsyncLLMEngine** — OpenAI 호환 서버는 결국 stream을 byte-pass-through하는 게 표준 (reformat 안 함).

**트레이드오프**: semaphore permit=1은 동시 사용자 거부와 같다. v1은 single-PC single-user라 OK. v1.1에서 per-runtime semaphore + queue-aware UI 표시로 확장.

### 1.2 Per-webapp scoped key (Origin 검증 + scope)

**ApiKey 스키마 (v1 확장)**:
```text
ApiKey {
    id: String,                 // uuid
    alias: String,              // "내 블로그", "기존 웹앱" 사용자 라벨
    key_prefix: String,         // "lm-abcd1234" 표시용
    key_hash: String,           // argon2id($plaintext)
    created_at: DateTime,
    last_used_at: Option<DateTime>,
    expires_at: Option<DateTime>,
    scope: Scope,
    project_id: Option<String>, // Phase 6'
}

Scope {
    models: Vec<String>,         // glob ("qwen-*", "*")
    endpoints: Vec<String>,      // glob ("/v1/chat/*", "/v1/*")
    allowed_origins: Vec<String>,// ["https://blog.com", "http://localhost:5173"]
    rate_limit_per_minute: Option<u32>,  // v1은 schema only
    quota_tokens_per_day: Option<u64>,   // v1은 schema only
}
```

**검증 흐름 (axum middleware)**:
1. `Authorization: Bearer <key>` 추출. 없으면 `401 missing_api_key`.
2. `key_prefix` (앞 11자) lookup → 후보 ApiKey 1건. argon2id verify. mismatch → `401 invalid_api_key`.
3. `expires_at` 체크 → 만료면 `401 expired_api_key`.
4. `Origin` 헤더 확인. `allowed_origins`가 비어있으면 origin-less 호출 (server-side curl 등) 허용. 비어있지 않은데 Origin 헤더 없거나 whitelist 미스매치면 `403 origin_not_allowed`.
5. 요청 path가 `scope.endpoints` glob 매칭하는지. fail → `403 endpoint_not_in_scope`.
6. Body의 `model` 필드(있는 경우)가 `scope.models` glob 매칭하는지. fail → `403 model_not_in_scope`.
7. principal (key_id, scope, project_id)을 `req.extensions_mut()`에 주입 → handler가 사용 로그 기록 시 사용.

**키 발급 UX (한국어 해요체)**:
- 위치: 설정 → "API 키" 탭 (사이드바). novice는 "API 키"가 어려울 수 있어 부제 "이 키로 다른 웹앱이 LMmaster에 안전하게 연결돼요"를 항상 함께 표시.
- 발급 modal: 1) 별명(필수, 예: "내 블로그"). 2) 모델 선택 (glob multi-select, 기본 `*` = 전부). 3) 허용 origin (최소 1개 필수, "주소 입력하세요" placeholder, 검증: scheme://host[:port] 형식). 4) 만료일 (옵션, 30일/90일/무기한 preset). 5) 발급 버튼.
- 발급 후 modal: 키 평문 1회 reveal + 클립보드 복사 + "이 창을 닫으면 다시 볼 수 없어요" 강한 경고 (네온 그린 단일 accent 외 다른 색 없음, 디자인 토큰 준수). 8초 후 자동 mask + 5분 후 auto-close.

**차용한 글로벌 사례**:
- **GitHub fine-grained PAT** — 발급 시 1회 reveal + scope (repo/issue/etc) + expiration. 우리도 동일 패턴.
- **Stripe restricted keys** — 별명 필수 + key prefix 표시 + 회수 가능. 우리 `key_prefix` 표시 동일.
- **Anthropic Console** — 키 한 번 보여주고 mask. 우리 동일.
- **JetBrains IDE settings sync token** — reveal 토글 1회 후 mask + 한국어 풀어쓰기 라벨 ("동기화 토큰(접근 권한)"). 우리 풀어쓰기 라벨 채택.

**트레이드오프**: rate limit / quota를 schema-only로 두고 enforce 안 하는 건 신뢰 비용. 그러나 v1 single-PC 시나리오에서 brute force 의심 정황 없음 + ko-novice 사용자에 GUI 추가 부담. v1.1에서 GUI 추가하며 enforce ON.

### 1.3 Portable workspace fingerprint repair

**3-tier 판정 (`crates/portable-workspace::repair`)**:
- **green** (os+arch+gpu_family 일치): manifest의 `last_repaired_at` 갱신만, 사용자 알림 없음.
- **yellow** (os+arch 일치, gpu_family 또는 vram tier 다름): toast (top-right, 6초) "이 PC에서 처음 실행하는 환경이에요. 벤치 캐시를 새로 측정할게요." + 자동 invalidate `cache/bench/*`, `cache/scan/*`. `models/`, `runtimes/`, `data/` (SQLite) 보존.
- **red** (os 또는 arch mismatch): modal blocking "다른 운영체제에서 가져온 워크스페이스예요" + 단계별 가이드 (1) 모델은 그대로 사용 가능 (2) 런타임은 다시 설치해야 해요 (3) 설정과 대화 기록은 안전해요. 사용자 클릭 시 `runtimes/` 안의 OS-specific 바이너리는 보존 (USB → 다시 USB 시 복구), 다만 `manifest.runtimes_installed[]`에서 현재 OS와 다른 항목은 invalidate.

**보존 vs invalidate 매트릭스**:

| 항목 | green | yellow | red |
|---|---|---|---|
| `data/` (SQLite — 키, 설정, 채팅 히스토리) | 보존 | 보존 | 보존 |
| `models/` (GGUF 파일) | 보존 | 보존 | 보존 (OS-agnostic) |
| `runtimes/` (런타임 바이너리) | 보존 | 보존 | 호환되는 것만 사용, 나머지는 manifest에서만 invalidate |
| `cache/bench/*` | 보존 | invalidate | invalidate |
| `cache/scan/*` | 보존 | invalidate | invalidate |
| `cache/registry/*` | 보존 | 보존 | 보존 (OS-agnostic) |
| API 키 (data/keys.db) | 그대로 | 그대로 | 그대로 — 단 사용자에게 "다른 PC에서 옮긴 키예요. 회수할까요?" 1회 확인 (보안 가드) |

**fingerprint 업데이트 시점**: repair flow 완료 직후 manifest의 `host_fingerprint`를 새 값으로 overwrite + `last_repaired_at = now`. 만약 사용자가 USB를 원래 PC로 다시 꽂으면 또 다시 repair flow가 뜨는데 — green tier라 silent 갱신만 발생.

**차용한 글로벌 사례**:
- **VSCode workspace trust** — workspace 옮길 때 "이 폴더를 신뢰하시나요?" 한 번만 묻는 패턴. 우리 red tier 모달이 동일 정신.
- **Tailscale device key migration** — device 옮겼을 때 fingerprint 새로 발급 + 이전 디바이스 인증 무효화. 우리는 디바이스 인증이 아니라 캐시 무효화라 단순화.
- **JetBrains IDE settings 동기화** — settings sync에서 OS 다를 때 일부 설정만 마이그레이트하는 selective restore. 우리 보존/invalidate 매트릭스가 이 정신.
- **Chrome profile migration** — 프로파일 폴더 다른 PC에 카피하면 일부 캐시는 invalidate, 즐겨찾기는 보존하는 selective 로직.

**API 키 회수 옵션**: red tier에서 사용자가 "API 키 회수"를 선택하면 SQLite의 `api_keys` 테이블 전체 truncate + 새 default 키 1개 자동 issue + 사용자에게 새 평문 1회 reveal. 이 옵션은 default off, 사용자 명시 클릭 시만.

### 1.4 OpenAI 호환 TS SDK shape

**`@lmmaster/sdk` 패키지 (npm 또는 workspace 패키지로 portable workspace에 포함)**:

```text
import { LMMaster } from "@lmmaster/sdk";

const client = new LMMaster({
    baseURL: "http://127.0.0.1:11436/v1", // 자동 감지 가능 (Tauri IPC가 알려줌)
    apiKey: "lm-abcd1234...",
});

// OpenAI SDK 그대로 사용 가능 (LMMaster extends OpenAI):
await client.chat.completions.create({
    model: "qwen2.5:7b",
    messages: [...],
    stream: true,
});

// LMMaster 전용 helper:
await client.lmmaster.listModels();  // 우리 owned_by 메타 포함
await client.lmmaster.health();      // /health 호출
```

**구현 전략**: `class LMMaster extends OpenAI`. 생성자에서 baseURL 기본값을 `http://127.0.0.1:<port>/v1` (port는 portable workspace의 manifest.json `ports.gateway`에서 lookup하는 helper `getDefaultBaseURL()` 함께 export), 추가로 `client.lmmaster` 네임스페이스로 우리 전용 메서드 (health, listKeys 등 — admin scope 필요한 건 별도 admin client).

**Bring-your-own-key vs gateway-managed key 정책**: 사용자는 SDK 초기화 시 키를 반드시 넘겨야 한다. localhost-only라도 키 없이 동작 시 동일 PC의 다른 사용자/악성 프로세스가 곧바로 호출 가능 (ADR-0007). 단 dev 편의를 위해 SDK에 `LMMaster.devClient()` factory를 두어 portable workspace의 `data/keys.db`에서 default 키를 자동 로드 (Tauri 환경에서만 동작, 브라우저에서는 throw).

**기존 웹앱 통합 (가장 중요한 USP)**: 기존 OpenAI SDK 사용 코드는 baseURL + apiKey 두 줄만 바꾸면 동작:

```text
- const openai = new OpenAI({ apiKey: process.env.OPENAI_API_KEY });
+ const openai = new OpenAI({
+     baseURL: "http://127.0.0.1:11436/v1",
+     apiKey: "lm-abcd1234...",
+     dangerouslyAllowBrowser: true,  // browser 직접 호출 시
+ });
```

이게 "wrap-not-replace"의 핵심. SDK 미사용 시도 동일하게 `fetch("http://127.0.0.1:11436/v1/chat/completions", { headers: { Authorization: "Bearer lm-..." } })`만으로 동작.

**차용한 글로벌 사례**:
- **`openai` npm 패키지** — `client.chat.completions.create({...})` API. 우리는 그대로 상속.
- **`@anthropic-ai/sdk`** — extends 가능한 client class 패턴. 우리 동일.
- **`@vercel/ai` SDK** — 다중 provider abstraction. 우리는 단일 provider라 abstraction 불필요, OpenAI SDK 직접 사용.
- **Ollama 공식 npm `ollama`** — Ollama 자체 API용. 우리는 OpenAI 호환 layer가 1차라 Ollama SDK 의존성 안 둠.

**트레이드오프**: `openai` npm 패키지에 의존 → openai breaking change에 노출. 그러나 OpenAI SDK는 v3→v4 마이그레이션 외엔 안정적이고, 자체 구현 시 streaming chunk 파싱 / retry / error envelope 모두 reinvent라 비용이 더 크다. peer dependency로 두어 사용자가 openai 버전 선택 가능.

### 1.5 API 키 발급 GUI UX

**발급 flow (3단계)**:
1. **목록 화면** (설정 → API 키): 발급된 키 카드 리스트 (alias, key_prefix, allowed_origins 칩, last_used_at). "새 키 발급" 버튼 (네온 그린 primary).
2. **발급 form modal**:
   - "별명을 알려주세요" (필수, novice 친화적 한국어 라벨, "내 블로그" placeholder)
   - "어떤 모델까지 허용할까요?" (multi-select, 기본 "전체 허용", glob input 가능)
   - "어떤 웹사이트에서 호출되나요?" (origin chips, 최소 1개 — 단, 고급 사용자는 "서버 호출 (origin 없음)" 토글로 비울 수 있음)
   - "만료일을 정할까요?" (옵션, 30일/90일/무기한)
   - "발급할게요" 버튼
3. **발급 직후 modal** (가장 중요): 평문 키 box + 클립보드 복사 버튼 + "이 키는 지금만 보여드려요. 닫으면 다시 볼 수 없어요" 강한 경고 (네온 그린 outline + 본문 mono tabular-nums) + 8초 카운트다운 후 자동 mask + 5분 후 modal auto-close + "닫기" 버튼.

**보안 가드**:
- 클립보드 카피 시 평문 → 30초 후 클립보드 자동 클리어 (Tauri `set_clipboard` + `setTimeout`).
- 화면 노출 후 자동 mask 8초 (스크린샷 대비).
- 풀 reveal 토글은 모달에 1회만 — 닫은 후 재 reveal 불가 (다시 발급 권유).
- 키 검색 화면에는 prefix만 — 평문 표시 어디에도 없음.

**차용한 글로벌 사례**:
- **GitHub fine-grained PAT 발급 UX** — alias + scope + expiration + 1회 reveal. 우리 동일.
- **Stripe restricted key UX** — restricted scope select + name. 우리 origin scope 추가.
- **Cursor settings → API keys** — minimal 1-form. 우리 한국어로 ko-novice 적합화.
- **JetBrains IDE token entry** — reveal 토글 + auto-mask. 우리 동일.

---

## 2. 기각안 + 이유 (Negative space — 의무 섹션)

### 2.1 LiteLLM proxy 임베드 (gateway 자체 대체)

- **시도 / 검토**: LiteLLM은 100+ provider routing + virtual key + spend tracking + rate limit이 완비된 production-grade Python 프록시. Gateway 본체를 LiteLLM 사이드카로 대체하면 v1에서 즉시 다중 provider + key scope를 얻을 수 있다.
- **거부 이유**: (1) Python 의존성 — Tauri portable USB 시나리오에서 Python runtime + venv 동봉이 디스크 100MB+ + 시작 latency. (2) ADR-0007에서 이미 "v1 자체 경량" 결정. (3) 외부 통신 0 정책상 LiteLLM의 강점인 다중 provider routing은 무용. (4) 키/사용 로그가 Python sqlite + 우리 SQLite 이중. (5) Korean-first error message 어렵다 (LiteLLM은 영어).
- **재검토 트리거**: Phase 6'에서 "team mode" (여러 사용자가 한 LMmaster에 붙는 시나리오)가 활성화되면 LiteLLM의 spend tracking + rate limit + admin UI 활용 가치 재검토.

### 2.2 Open WebUI를 frontend로 대체

- **시도 / 검토**: Open WebUI는 Ollama 호환 + OpenAI 호환 + 멀티 모델 채팅 GUI를 이미 갖추고 있다. 우리 Workbench를 Open WebUI로 대체하면 Phase 4' 부담 격감.
- **거부 이유**: (1) Open WebUI는 multi-user + auth 중심 — 우리 single-PC + companion 시나리오와 mental model 다름. (2) Korean-first localization 부분적 (한국어 번역 누락 영역 다수). (3) 디자인 토큰(neon green / 4px grid / dark-only) 일치 불가 — fork 비용이 자체 구현 비용 초과. (4) "기존 웹앱이 호출만 하는 wrapper" USP와 "Open WebUI를 켜고 거기 들어가서 쓴다" UX는 정반대 (USP 깨짐).
- **재검토 트리거**: 엔터프라이즈 SKU 신설 시 "Open WebUI 호환 모드" 별도 옵션으로 검토.

### 2.3 SSE 자체 reparse + axum::Sse 재구성

- **시도 / 검토**: 업스트림 SSE를 `data:` 라인 단위로 parse → JSON 검증 → axum::response::Sse로 재구성 → 클라이언트로 송신. error envelope 검증 / 누락 필드 보강이 가능.
- **거부 이유**: (1) Ollama / LM Studio 모두 OpenAI 호환 SSE를 이미 정확히 보내므로 reparse는 zero-value reinvent. (2) reparse 시 매 chunk마다 UTF-8 boundary / trailing newline / `[DONE]` 마커 처리 직접 — bug surface area 큼. (3) byte-perfect relay가 OpenAI SDK 호환성 가장 안전 (예상 못 한 필드 forwarding).
- **재검토 트리거**: 업스트림이 OpenAI 표준에서 벗어난 chunk 보내기 시작하면 (예: Ollama가 OpenAI compat 모드를 deprecate하면) 자체 reformat 필요.

### 2.4 SQLCipher default ON (encryption 기본 활성)

- **시도 / 검토**: ADR-0007이 "SQLCipher / OS keychain"을 옵션으로 명시. portable USB 환경에서 키 평문 노출 위험 대비 SQLCipher default ON.
- **거부 이유**: (1) SQLCipher는 마스터 패스워드 UX 필요 — novice 한국 사용자에 진입장벽. (2) portable USB 시나리오에서 사용자가 마스터 패스워드 분실 시 모든 데이터 손실 = 신뢰 사고. (3) 키 자체는 argon2id 해시로 저장 (DB 파일 카피해도 평문 키 복원 불가). (4) 사용 로그 + 채팅 히스토리도 민감하지만 v1 single-PC + OS 사용자 권한 격리에 의존. (5) v1.1에서 `LMMASTER_ENCRYPT_DB=1` 환경변수 + GUI 토글 추가하면 opt-in 가능.
- **재검토 트리거**: 첫 사용자 피드백에서 "내 USB를 누가 가져갈 수 있어요" 우려가 5건 이상 누적되면 v1.1 우선순위 상승.

### 2.5 Per-runtime semaphore (Ollama=1, LM Studio=1로 분리)

- **시도 / 검토**: Ollama와 LM Studio가 GPU 리소스를 공유한다고 해도, 동시에 다른 모델이면 가능. permit을 어댑터별로 둬서 throughput 두 배.
- **거부 이유**: (1) 같은 GPU 1장에서 두 모델 동시 로드는 OOM 위험 + thermal throttling. (2) 사용자 PC GPU 다양성 (RTX 3060 8GB부터 4090 24GB까지) — 안전한 default는 가장 작은 PC 기준. (3) v1 single-user에서 동시 호출 빈도 낮음 (체감 차이 적음). (4) 코드 복잡도 (어댑터 등록 시 permit 발급 + per-model VRAM 추적) 증가 대비 user value 한계.
- **재검토 트리거**: v1.1에서 multi-process workbench (한 사용자가 동시 여러 채팅 띄움) 시나리오 활성화 시. + Phase 5' Pipelines가 동시 N개 step에 다른 모델을 라우팅할 때.

### 2.6 자체 OpenAI 호환 TS SDK (raw fetch 기반 자체 구현)

- **시도 / 검토**: `openai` npm 패키지 의존 없이 raw `fetch` + ReadableStream으로 자체 구현. 의존성 0 + breaking change 자유.
- **거부 이유**: (1) streaming chunk 파싱 (`data:` 라인 + `[DONE]` 마커) 자체 구현 시 edge case (chunk가 라인 중간에 잘림 등) 모두 reinvent. (2) retry / timeout / error envelope 처리 reinvent. (3) `openai` SDK의 TypeScript 타입 정의 (chat.completions.create의 generic 타입) 직접 작성 필요 — 50+ method × 수백 필드. (4) 사용자 기존 코드 마이그레이션이 단순 baseURL 교체가 아니게 됨 (USP 약화).
- **재검토 트리거**: `openai` npm 패키지가 OpenAI 전용 기능 (tier-based pricing 검증 등)을 하드코딩해 우리 baseURL 우회 거부 시. + 패키지 크기가 v1.1에서 critical 이슈가 되면 (현재 ~150KB minified, 허용 범위).

### 2.7 키 발급 시 password 요구 (마스터 패스워드 + 키)

- **시도 / 검토**: 1Password / Bitwarden 패턴처럼 마스터 패스워드 + 발급 키 이중 인증.
- **거부 이유**: (1) novice 한국 사용자에 진입장벽 추가. (2) v1 single-PC 시나리오에서 OS 사용자 권한이 1차 가드 — 추가 password는 보안보다 UX 비용. (3) password 분실 시 키 복구 불가 → 사용자 신뢰 사고.
- **재검토 트리거**: Phase 6' team mode에서 multi-user가 한 LMmaster를 공유하면 마스터 패스워드 (admin) 필수가 됨.

### 2.8 모든 endpoint 무인증 (localhost 신뢰 가정)

- **시도 / 검토**: localhost-only 바인딩이라 외부 위협 없음 → 무인증으로 UX 단순화.
- **거부 이유**: ADR-0007에서 이미 거부. 동일 PC의 다른 사용자/악성 프로세스가 곧바로 호출 가능. 키 의무는 보안 + 사용 로그 attribution (어느 웹앱이 얼마나 썼는지) 양쪽 가치.
- **재검토 트리거**: 없음 (이 결정은 보안 고정).

### 2.9 fingerprint 영향 받는 모든 캐시 즉시 invalidate (yellow도 red 처리)

- **시도 / 검토**: 안전 우선 — fingerprint mismatch면 무조건 모든 캐시 invalidate.
- **거부 이유**: (1) 사용자 USB → 다른 PC → 원래 PC 시나리오에서 매번 캐시 재구성 = 분 단위 대기. (2) yellow tier (같은 OS, GPU 살짝 다름)는 모델 파일 + 등록 정보는 정상 동작. (3) novice에 "왜 또 다시 측정해요?" 의문.
- **재검토 트리거**: 잘못된 캐시 hit으로 인한 사고 (예: bench cache가 다른 GPU로 측정한 값을 보여줌) 발생 시.

### 2.10 키 prefix 4자 (짧게)

- **시도 / 검토**: prefix 4자 (`lm-1234`)면 사용자 화면에 더 깔끔.
- **거부 이유**: (1) 4자 prefix는 collision 확률 무시 못 함 — 사용자가 100개 발급 시 매 lookup마다 모든 후보 argon2 verify → 느림. (2) 8자(`lm-abcd1234`)면 collision 사실상 0 + lookup O(1) hash. (3) Stripe / GitHub 8~12자 prefix가 표준.
- **재검토 트리거**: 없음.

---

## 3. 미정 / 후순위 이월

| 항목 | 이월 사유 | 진입 조건 / 페이즈 |
|---|---|---|
| Rate limit / quota enforce | v1 schema-only — 단일 사용자 brute force 우려 적음, GUI 부담 큼 | v1.1 (사용 로그 통계 GUI 추가 시) |
| Anthropic-compatible shim (`/v1/messages`) | OpenAI 호환만으로 USP 충분 | Phase 6' 또는 v1.2 (Anthropic SDK 사용 웹앱이 5건 이상 보고되면) |
| `/v1/files`, `/v1/images`, `/v1/audio` endpoint | 로컬 모델 multimodal 지원이 우선 (Phase 4' Workbench 다음) | v1.x (multimodal 모델 카탈로그 진입 후) |
| `/v1/assistants`, `/v1/threads` | OpenAI 신 API, stateful — 우리 stateless 모델과 mental model 다름 | 후순위 v2+ |
| Per-runtime semaphore | v1 single-user 동시성 낮음 | v1.1 (Phase 5' Pipelines 동시 step 또는 multi-tab Workbench) |
| SQLCipher default ON | 마스터 패스워드 UX 부담 | v1.1 GUI 토글, env opt-in으로 v1에 stub |
| LiteLLM 통합 (team mode) | 외부 통신 0 + single-PC | Phase 6' team mode ADR 후 |
| Browser SDK CSP / Subresource Integrity | 발급 키가 노출되는 브라우저 환경 안전성 | v1.1 (사용자 보고 누적 시) |
| Anthropic Console 스타일 사용량 그래프 GUI | 사용 로그는 v1에 기록만, 시각화는 후순위 | v1.1 (사용 로그가 1MB 이상 누적되는 사용자 패턴 확인 후) |
| `LMMASTER_ENCRYPT_DB` 환경변수 → GUI 토글 | v1은 env로만 opt-in | v1.1 |
| Mac / Linux portable USB cross-OS 자동 마이그레이트 | red tier modal로 처리, 자동 마이그레이트는 GGUF model 외 어렵 | v1.x |

---

## 4. 테스트 invariant

> 본 페이즈가 깨면 안 되는 동작들. clippy + test에 박는다.

- **OpenAI byte-perfect SSE relay**: 업스트림(Ollama mock)이 보낸 raw SSE bytes가 클라이언트에 변형 없이 도착 (wiremock + bytes diff 1byte 단위 검증). chunk 경계, trailing newline, `data: [DONE]` 마커 모두 보존.
- **Cancel propagation**: 클라이언트 connection drop 시 업스트림 reqwest 호출도 cancel (mock 서버에서 connection close 수신 검증, 5초 이내).
- **Semaphore 직렬화**: 동시 2 request 시 두 번째는 첫 번째 완료까지 대기 (`x-lmmaster-queue-wait-ms` 헤더 ≥ 첫 번째 처리 시간).
- **Origin 검증 매트릭스 round-trip**:
  - allowed_origins=`["https://blog.com"]`, Origin=`https://blog.com` → 200
  - allowed_origins=`["https://blog.com"]`, Origin=`https://evil.com` → 403 origin_not_allowed
  - allowed_origins=`["https://blog.com"]`, Origin=`null` (file://) → 403
  - allowed_origins=`[]` (서버 호출), Origin 없음 → 200
  - allowed_origins=`[]`, Origin=`https://something.com` → 200 (서버 호출 모드는 origin 무시)
- **Scope glob 매칭**:
  - scope.models=`["qwen-*"]`, body.model=`"qwen2.5:7b"` → 200
  - scope.models=`["qwen-*"]`, body.model=`"llama3.2:3b"` → 403 model_not_in_scope
  - scope.endpoints=`["/v1/chat/*"]`, path=`/v1/embeddings` → 403 endpoint_not_in_scope
- **OpenAI 호환 error envelope**: 모든 4xx/5xx 응답이 `{"error":{"message","type","code"}}` 구조. openai-python의 `APIError.code`가 우리 `code`를 정확히 추출.
- **Argon2id verify 결정성**: 같은 plaintext + 같은 salt → 같은 hash. 재시작 후에도 verify 성공.
- **Fingerprint repair 보존/invalidate 매트릭스**: 위 표 5종 시나리오 모두 테스트 (green, yellow-gpu, yellow-vram, red-os, red-arch).
- **API 키 1회 reveal**: issue 응답 후 즉시 plaintext_once 폐기 검증 (메모리에서 zero-out + 두 번째 lookup 불가).
- **SSE timeout 600s vs 일반 30s**: chat/completions 라우트만 600s, 다른 라우트는 30s (TimeoutLayer route 단위 검증).
- **/v1/models OpenAI shape**: `data: [{ id, object: "model", created, owned_by }]` 정확히 (openai SDK `models.list()`로 round-trip 검증).
- **i18n**: 모든 error message에 ko/en 1:1 키 매핑, fallback 깨짐 없음.
- **a11y**: API 키 발급 modal vitest-axe `violations.toEqual([])`.

---

## 5. 다음 페이즈 인계

- **선행 의존성**:
  - Phase 1' RuntimeManager `list_models()` per-adapter 정확.
  - Phase 2'.b 카탈로그 UI에서 모델 alias / 기본 어댑터 마킹 사용자 입력 가능.
  - 디자인 시스템의 modal / form / chip 컴포넌트 준비.
- **이 페이즈 산출물**:
  - `crates/core-gateway/src/routes/chat.rs` — `/v1/chat/completions` proxy 본체 (semaphore + bytes_stream pass-through).
  - `crates/core-gateway/src/routes/models.rs` — `/v1/models`, `/v1/models/:id`.
  - `crates/core-gateway/src/routes/embeddings.rs` — `/v1/embeddings` (Ollama만).
  - `crates/core-gateway/src/routes/admin.rs` — `/_admin/keys/*` (CRUD).
  - `crates/core-gateway/src/routing.rs` (신설) — RoutingMap + 5초 TTL 캐시.
  - `crates/core-gateway/src/auth.rs` — middleware 본격 구현 (argon2id verify + Origin 검증).
  - `crates/key-manager/src/store.rs` — SQLite + argon2id.
  - `crates/key-manager/src/scope.rs` — glob 매칭 (`globset` crate).
  - `crates/key-manager/src/middleware.rs` — gateway에서 호출되는 verify entry point.
  - `crates/portable-workspace/src/repair.rs` — 3-tier 판정 + 보존/invalidate 매트릭스.
  - `crates/portable-workspace/src/fingerprint.rs` (신설) — host fingerprint 비교 로직.
  - `packages/sdk-ts/` (신설 — pnpm workspace) — `@lmmaster/sdk` thin wrapper of `openai`.
  - `apps/desktop/src/features/api-keys/` — 발급/회수 GUI (modal + list).
  - `docs/adr/0022-gateway-routing.md` (신설).
  - 통합 테스트 ≥ 25건 (gateway proxy 10 + scope 6 + repair 5 + sdk 4).
- **다음 sub-phase로 가는 진입 조건**:
  - `cargo test --workspace` 누적 ≥ 220.
  - `pnpm exec tsc -b` + `pnpm run build` (apps/desktop + packages/sdk-ts) 통과.
  - `verify.ps1` 풀 통과.
  - `docs/RESUME.md`에 Phase 3' 산출 기록.
- **위험 노트**:
  - **`openai` npm 패키지 v5 breaking change** — peer dep으로 두되 v4를 lock. v5 진입 시 마이그레이션 일정 별도 검토.
  - **Ollama OpenAI compat 모드 deprecation** — 현재 안정, 그러나 Ollama가 자체 API만 유지하기로 하면 우리 gateway에 OpenAI ↔ Ollama 변환 layer 필요 (현재 byte-pass-through 전략 무효).
  - **Origin 헤더 spoof** — server-side curl에서 임의 Origin 위조 가능. 그러나 이미 키 hash 검증 통과한 호출이라 방어선 1차는 키 자체. Origin은 browser 환경의 추가 가드.
  - **Argon2id 메모리 64MB × concurrent verify** — 동시 100건 중복 verify 시 6.4GB 일시 점유. 우리 single-user에서 비현실적이지만 v1.1에서 LRU verify cache (key_id → verified_at TTL 1분) 도입 여지.
  - **fingerprint hash 알고리즘 변경** — 현재 sha256(os+arch+gpu_family+vram_tier+ram_tier+cpu_brand). 알고리즘 바꾸면 모든 사용자가 매 실행마다 yellow tier — 변경은 schema_version bump 동반 필수.
  - **portable USB write protection** — 일부 USB는 read-only로 마운트 가능. manifest 갱신 실패 → 무한 repair flow. graceful fallback 필요 (RO 감지 시 메모리에만 fingerprint 갱신, 다음 실행 때 다시 mismatch 모달).

---

## 6. 참고

### 글로벌 사례 / 공식 문서

- **Ollama OpenAI compat (0.4+)** — `https://github.com/ollama/ollama/blob/main/docs/openai.md`. `/v1/chat/completions`, `/v1/embeddings`, `/v1/models` 노출. body 파라미터 매핑.
- **LM Studio OpenAI 호환** — `:1234/v1/*` 기본. 공식 docs.
- **LiteLLM proxy routing** — `https://docs.litellm.ai/docs/proxy/quick_start` (모델 → provider routing 패턴).
- **Open WebUI multi-backend** — `https://docs.openwebui.com/` (Ollama + OpenAI 동시 노출).
- **vLLM async OpenAI server** — `vllm/entrypoints/openai/api_server.py` (SSE pass-through 패턴).
- **GitHub fine-grained PAT** — `https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens` (alias + scope + expiration UX).
- **Stripe restricted keys** — `https://docs.stripe.com/keys` (key_prefix + restricted scope).
- **Anthropic Console keys** — `https://console.anthropic.com/` (1회 reveal + mask).
- **Argon2id RFC 9106** — `https://www.rfc-editor.org/rfc/rfc9106.html` (memory=64MB, iter=3 권장).
- **OpenAI Node SDK** — `https://github.com/openai/openai-node` (extends 가능한 client class).
- **`globset` Rust crate** — `https://docs.rs/globset/` (scope.models / scope.endpoints glob 매칭).
- **`tower::Semaphore` + axum** — axum book (semaphore을 AppState로 공유하는 패턴).

### 관련 ADR

- ADR-0001 (companion 아키텍처) — gateway가 companion의 1차 외부 인터페이스.
- ADR-0006 (gateway 프로토콜 — OpenAI compat REST + SSE) — 본 결정의 헌법.
- ADR-0007 (key-manager — LiteLLM 비고정) — scoped key 정신.
- ADR-0008 (SQLite 저장소) — keys.db / usage_log.db.
- ADR-0009 (portable workspace manifest) — fingerprint repair 정신.
- ADR-0010 (Korean-first) — 발급 GUI 한국어 해요체.
- ADR-0011 (디자인 시스템) — modal / form / chip 토큰 준수.
- ADR-0016 (wrap-not-replace) — SDK가 OpenAI SDK extends.
- ADR-0022 (신설 — Gateway 라우팅 정책) — 본 결정의 채택안 1.1, 1.2를 정식화.

### 메모리 항목 추가/갱신

- 신규: `gateway_proxy_v1` — "Gateway는 OpenAI 호환 4 endpoint + byte-perfect SSE pass-through + global semaphore=1. Per-webapp scoped key (5 차원: models, endpoints, allowed_origins, expires_at, project_id). v1은 rate limit / quota schema-only."
- 신규: `scoped_key_v1` — "argon2id($plaintext) + key_prefix 8자 + 1회 reveal. SQLCipher default off, env opt-in. CORS + Origin 헤더 이중 검증. 다른 origin은 403 origin_not_allowed."
- 신규: `portable_repair_v1` — "3-tier (green/yellow/red). 모델 항상 보존, 벤치/스캔 캐시는 yellow+ invalidate, 런타임은 red에서 manifest invalidate. API 키는 사용자 옵션."
- 신규: `sdk_strategy_v1` — "@lmmaster/sdk = openai npm peer dep + class extends OpenAI. 자체 fetch 구현 안 함. dangerouslyAllowBrowser는 사용자 책임."
- 갱신: `competitive_thesis` — Phase 3' 결정 반영 (G3 갭 — 키 매니저는 v1 자체 경량, LiteLLM은 Phase 6' 검토).
