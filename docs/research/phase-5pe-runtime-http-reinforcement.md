# Phase 5'.e — Real HTTP Runtime Wiring + Ollama Modelfile Shell-out 보강 리서치

> 작성일: 2026-04-28
> 상태: 보강 리서치 (구현 직전)
> 선행: Phase 5'(Workbench v1 core), Phase 5'.b(IPC), Phase 5'.c(UI), Phase 2'.c(bench-harness `BenchAdapter` HTTP 패턴), Phase 1A.3(installer downloader 재시도 정책)
> 후행: Phase 5'.f(LoRA + 양자화 실 CLI 연결, v1.x 진입), Phase 7'(릴리스 prep)
>
> 본 노트 범위:
> - `WorkbenchResponder` (mock) → 실 HTTP 호출 (Ollama `/api/generate` / LM Studio `/v1/chat/completions`)
> - `run_stage_register` (JSON 영속) → `ollama create -f Modelfile` shell-out (EULA + 사용자 동의 게이트)

---

## §0. 결정 요약 (8가지)

1. **Ollama는 `/api/generate` 1순위 + `stream: false`** — Workbench Validate 단계는 단일 prompt × 단일 응답이라 `/api/chat`의 messages 배열 오버헤드 불필요. `prompt`+`system` 필드로 충분. Ollama-rs / DeepWiki / 공식 docs 일관 권고.
2. **LM Studio는 `/v1/chat/completions` + `stream: false` + `messages: [{role:"user", content:"..."}]`** — OpenAI 호환이라 단일 user 메시지로 비교 등가. `max_tokens: 256` 기본 (Workbench eval 평균 응답 길이).
3. **요청 timeout 60s, connect_timeout 5s** — bench-harness 30s budget는 *measurement window*이고 Validate는 사용자 향 1회 실행이라 더 여유 필요. `reqwest::ClientBuilder::timeout(60s).connect_timeout(5s)` 명시.
4. **5xx / 429만 재시도, 4xx는 즉시 실패** — `backon` crate `ExponentialBuilder { jitter, factor:2.0, min:200ms, max:5s, retry_count:3 }`. 405/400은 사용자 입력 오류라 재시도 무의미. 503(LM Studio "no model loaded")은 retry 후 사용자 안내 ("LM Studio에 모델을 먼저 로드해 주세요").
5. **Cancel은 `tokio::select!` + `CancellationToken`** — bench-harness Phase 2'.c 패턴 그대로 차용. HTTP request future + `cancel.cancelled()` 동시 listen, cancel 시 reqwest future drop으로 connection close.
6. **Modelfile shell-out은 `tokio::process::Command` + `kill_on_drop(true)` + 60s timeout** — `ollama create <name> -f <Modelfile_path>` 형식. cwd는 임시 작업 디렉터리(`workspace/workbench/{run_id}/register/`). stdout/stderr `tokio::io::BufReader` 라인 단위 수집 후 `RegisterReport`에 저장.
7. **EULA 게이트는 외부 런타임이 아니라 *우리 본체*에서 표시** — Ollama는 MIT라 EULA 자체 없음. LM Studio EULA는 LM Studio 설치 시점에 LM Studio가 직접 표시(우리 책임 아님). 우리는 "ollama create 실행 동의 + 출력물 가시성 안내" 1회 dialog (Workbench Settings → "Modelfile 자동 등록 동의").
8. **테스트는 wiremock-rs + `tempfile::tempdir()` shell-out fixture binary** — 실 ollama 바이너리 의존 없이 fixture echo 스크립트로 stdout/stderr/exitcode round-trip 검증. CI 머신에 ollama 설치 강제 X.

---

## §1. Ollama HTTP 엘리트 사례 (`/api/generate`)

### 1.1 `/api/generate` 본문 형태 (공식 docs + ollama-rs 차용)

**채택 형식**:

```text
POST http://localhost:11434/api/generate
Content-Type: application/json

{
  "model": "llama3.1:8b",
  "prompt": "한국어로 짧게 답해 주세요. 질문: ...",
  "system": "You are a Korean-first assistant. Always respond in Korean.",
  "stream": false,
  "options": {
    "temperature": 0.7,
    "num_ctx": 4096,
    "num_predict": 256
  },
  "keep_alive": "5m"
}
```

- **`stream: false`** — 응답을 단일 JSON 객체로 받음. Workbench Validate는 첫 응답 미리보기만 필요하므로 SSE 스트리밍 불필요. 공식 문서 명시: "the response will be returned as a single response object".
- **`options.num_ctx: 4096`** — 한국어 BPE는 영어 대비 2~3배 토큰을 쓰므로 4K context가 안정 기본값.
- **`options.num_predict: 256`** — Validate eval 응답 평균 길이. 너무 길면 timeout 위험.
- **`keep_alive: "5m"`** — 첫 호출 후 모델을 5분 메모리 보존, 연속 Validate 시 cold load 비용 회피. Workbench는 한 run 내 multiple validate 가능.
- **`system`** — Modelfile의 SYSTEM과 별도 추가 지시. Workbench가 KoAlpaca preset 적용 시 매번 명시 (Modelfile만 의존하면 base 모델 변경 시 잃음).

### 1.2 `/api/generate` 응답 (stream: false)

`done:true` 단일 chunk: `{model, created_at, response, done, done_reason, total_duration, load_duration, prompt_eval_count, prompt_eval_duration, eval_count, eval_duration, context}` — duration ns 단위. `done_reason`: `stop`(정상) / `length`(num_predict 초과) / `unload`. Validate UI는 `length` 시 "토큰 한계로 잘렸어요" 배지. bench-harness Phase 2'.c와 동일 필드 → Workbench는 별도 측정 X, `eval_count` / `eval_duration`만 progress hint로 노출.

### 1.3 `/api/chat` vs `/api/generate` — 단일 prompt eval 적합성

**결론: `/api/generate` 채택**. 근거 3가지:

1. **`/api/chat`는 `messages: [...]` 배열 강제** — 단일 prompt를 `[{"role":"user","content":"..."}]`로 wrap 필수. Workbench Validate는 system + single user prompt 패턴이라 wrap이 불필요한 noise.
2. **`/api/chat`는 도구 호출(tools) 위해 설계** — DeepWiki "Generation and Chat API"는 "/api/chat is for multi-turn conversational interface with message history"로 명시. Workbench는 history 없음.
3. **응답 필드 차이** — `/api/chat`은 `message: {role, content}` 객체, `/api/generate`는 `response: string`. Workbench는 plain text만 필요.

bench-harness(Phase 2'.c)도 `/api/generate`만 사용 (단일 한국어 prompt × 3개 시드 × 2회 측정). 일관성 유지.

### 1.4 인용

- 공식 docs: <https://docs.ollama.com/api/generate>
- 공식 docs api.md: <https://github.com/ollama/ollama/blob/main/docs/api.md>
- DeepWiki Generation/Chat API: <https://deepwiki.com/ollama/ollama/3.2-generation-and-chat-api>
- ollama-rs 0.3.4 (production wrapper): <https://github.com/pepperoni21/ollama-rs> — `GenerationRequest::new(model, prompt).options(ModelOptions::default().temperature(0.2).num_ctx(4096))`. 우리는 의존성 추가 없이 reqwest 직접 호출 (기존 `OllamaAdapter` 패턴 유지).
- llama-index Ollama integration: <https://docs.llamaindex.ai/en/stable/examples/llm/ollama/> — 동일하게 `/api/generate` + `stream: false` + `keep_alive` 권장.

---

## §2. LM Studio HTTP 엘리트 사례 (`/v1/chat/completions`)

### 2.1 `/v1/chat/completions` 본문 형태 (OpenAI 호환)

**채택 형식**:

```text
POST http://localhost:1234/v1/chat/completions
Content-Type: application/json

{
  "model": "loaded-model-id-or-empty",
  "messages": [
    {"role": "system", "content": "You are a Korean-first assistant. Always respond in Korean."},
    {"role": "user",   "content": "한국어로 짧게 답해 주세요. 질문: ..."}
  ],
  "stream": false,
  "temperature": 0.7,
  "top_p": 0.9,
  "max_tokens": 256
}
```

- **`stream: false`** — 단일 JSON 응답. OpenAI 표준이라 LM Studio도 동일 동작.
- **`max_tokens: 256`** — Ollama의 `num_predict`와 등가. LM Studio는 OpenAI 호환이라 `max_tokens` 명칭 유지.
- **`model`** — LM Studio는 *현재 로드된 모델*만 응답. 사용자가 LM Studio UI에서 로드한 모델 ID를 우리가 알 필요 있음. `GET /v1/models`로 사전 조회 → `data: []`(empty)면 "LM Studio에 모델을 먼저 로드해 주세요" 한국어 에러.
- **`temperature` / `top_p`** — Modelfile PARAMETER와 동일 의미. Workbench config에서 단일 source.

### 2.2 응답 형태 (stream: false)

OpenAI 표준 `{id, object, created, model, choices:[{index, message:{role, content}, finish_reason}], usage:{prompt_tokens, completion_tokens, total_tokens}}`. `finish_reason`: `stop` / `length` / `content_filter`. Validate UI는 `length` 시 "토큰 한계로 잘렸어요" 배지 (Ollama와 동일 처리). `usage.completion_tokens` = Ollama `eval_count` 등가 — token 수만 progress hint로 노출 ("응답 18토큰").

### 2.3 "no model loaded" 처리 — 사실은 503 아님

**리서치 결과**: LM Studio는 모델 미로딩 상태에서:

1. **`GET /v1/models` → 200 OK + `{"data": []}`** (빈 배열). 이게 1차 진단 패턴 (cline issue #8030, dify issue #14231).
2. **`POST /v1/chat/completions` 직접 호출 시** — 빈 응답 / 400 / 미정의 동작. 일관성 낮음.

**채택 정책**:
1. **사전 `GET /v1/models` 1회 호출** — Validate 시작 직전. `data: []`면 사용자에게 "LM Studio를 열고 모델을 로드해 주세요" 한국어 dialog + Validate 진입 차단.
2. **`model: ""`(빈 string) 허용** — LM Studio는 "현재 로드된 모델"을 자동 선택. 다중 모델 동시 로드는 LM Studio 0.3.x+ 일부 빌드만 지원하므로 single-model 가정.
3. **400/500 응답 시 `LmStudioRuntimeError::NotReady` enum** — 사용자 향 메시지 "LM Studio가 준비되지 않았어요. 모델 로드 후 다시 시도해 주세요".

### 2.4 인용

- 공식 docs OpenAI compat: <https://lmstudio.ai/docs/api/openai-api>
- Chat Completions: <https://lmstudio.ai/docs/developer/openai-compat/chat-completions>
- "no model loaded" 진단 (cline issue): <https://github.com/cline/cline/issues/8030>
- "no model loaded" 진단 (dify issue): <https://github.com/langgenius/dify/issues/14231>
- production wrapper 패턴: <https://github.com/cline/cline> (LM Studio adapter는 `GET /v1/models` 1회 사전조회 후 본 호출).

---

## §3. `ollama create -f Modelfile` shell-out

### 3.1 정확한 명령 형식

```text
ollama create <model_name> -f <Modelfile_path>
```

- **`<model_name>`**: 우리가 정한 식별자 (예: `lmmaster-koqa-20260428-1430`). 충돌 회피 위해 timestamp suffix.
- **`-f <Modelfile_path>`**: 절대 경로. Workbench가 사전에 `workspace/workbench/{run_id}/register/Modelfile`로 작성한 파일.
- **cwd**: Modelfile 안의 `FROM ./relative/path/to.gguf` 같은 상대 경로가 cwd 기준으로 해석되므로 **반드시 Modelfile 디렉터리를 cwd로 지정**. 우리는 GGUF 절대경로 권고 → cwd 영향 최소화.

### 3.2 stdout/stderr 처리

**관찰된 출력 패턴**: success는 "transferring model data" → "using existing layer" → "writing manifest" → "success". 실패는 "Error: open ./model.gguf: no such file" / "Error: command must be one of 'from', 'license', ..." / "Error: dial tcp 127.0.0.1:11434: connect: connection refused".

**채택 처리**: `tokio::process::Command::new("ollama").args(["create", &name, "-f", path]).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true).spawn()`. stdout/stderr `BufReader::lines()` async iterate → `Vec<String>` 수집 + `tracing::info!`. exit success → `RegisterStatus::Created`. exit fail → stderr 마지막 5라인을 `RegisterError::CreateFailed { stderr_tail }`에 담아 한국어 매핑 ("no such file" → "Modelfile에서 참조하는 모델 파일을 찾지 못했어요", "connection refused" → "Ollama 데몬이 실행 중이 아니에요", "command must be one of" → "Modelfile 형식이 잘못됐어요").

### 3.3 사전 점검 — Ollama daemon 상태

`ollama create`는 daemon(11434)에 연결한다. daemon 미실행 시 위 "connection refused" 발생.

**채택 패턴**: shell-out 직전 `GET http://localhost:11434/` 1회 ping (timeout 2s). 응답 200 / "Ollama is running" 텍스트 확인.

- daemon down → "Ollama 데몬이 꺼져 있어요. Ollama 앱을 실행한 다음 다시 시도해 주세요" + Validate 진입 차단 + 한국어 안내 카드 노출.
- daemon up but model 없음 → `ollama create`가 알아서 base를 pull (`FROM llama3.1:8b` 등)하므로 우리는 ping만으로 충분.

**Windows 특이사항**: Ollama Windows는 systray 앱으로 daemon 자동 시작. 사용자가 systray 종료한 경우 ping 실패 → 한국어 안내.

### 3.4 실패 시나리오 매트릭스

| 시나리오 | 감지 | 사용자 향 한국어 메시지 |
|---|---|---|
| daemon down | `GET /` ping 실패 | "Ollama 데몬이 꺼져 있어요. Ollama를 켠 뒤 다시 시도해 주세요." |
| Modelfile 경로 오류 | stderr "no such file" | "Modelfile에서 참조한 파일을 찾지 못했어요." |
| 잘못된 base | stderr "command must be one of" | "Modelfile 형식이 잘못됐어요. 자동 생성을 다시 실행해 보세요." |
| 디스크 부족 | stderr "no space left on device" | "디스크 공간이 부족해요. 공간을 확보한 뒤 다시 시도해 주세요." |
| 모델명 충돌 | stderr "model already exists" | "같은 이름의 모델이 있어요. 다시 등록할까요?" (사용자에게 overwrite 옵션 제공). |
| 네트워크 실패 (base pull) | stderr "failed to fetch" | "기본 모델을 받지 못했어요. 인터넷 연결을 확인해 주세요." |
| timeout (60s 초과) | tokio::time::timeout | "등록이 너무 오래 걸려서 멈췄어요. 큰 모델은 더 시간이 필요해요." |

### 3.5 인용

- 공식 docs Modelfile: <https://github.com/ollama/ollama/blob/main/docs/modelfile.md>
- Troubleshooting: <https://docs.ollama.com/troubleshooting>
- Modelfile create 가이드: <https://oneuptime.com/blog/post/2026-02-02-ollama-custom-modelfiles/view>
- 실패 사례 (issue #4666): <https://github.com/ollama/ollama/issues/4666> — `ollama create` CLI가 stock 템플릿에서도 "command must be one of" 에러 보고. 우리 escape_system_prompt가 안전 처리 (Phase 5' modelfile.rs 기존 구현).
- daemon 상태 진단: <https://fixdevs.com/blog/ollama-not-working/>
- Modelfile 가이드: <https://medium.com/@gabrielrodewald/running-models-with-ollama-step-by-step-60b6f6125807>

---

## §4. 타임아웃 / 재시도 / cancel

### 4.1 reqwest builder timeout vs request timeout

**채택**: `ClientBuilder` 차원에서 default timeout 설정 + 호출별 override.

```text
let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(60))             // 전체 request timeout
    .connect_timeout(Duration::from_secs(5))      // connection establish only
    .no_proxy()                                   // ADR-0013 외부 통신 0 + localhost 명시
    .build()?;
```

- **`timeout(60s)`** — request send → response complete까지 wall-clock cap. Validate는 1회 응답 256토큰이라 60s면 cold load 포함 충분.
- **`connect_timeout(5s)`** — TCP/TLS handshake만. 데몬 down 빠른 감지 (60s 기다리지 않음).
- **`no_proxy()`** — 시스템 환경변수 `HTTP_PROXY` 무시. ADR-0013 일관 + 사내 프록시 환경에서 localhost 강제.

bench-harness(Phase 2'.c)는 `connect_timeout(2s) + 30s wall-clock budget`. Validate는 더 여유 있는 60s budget — 사용자 향 단발 호출이라.

### 4.2 5xx / 429 재시도 (backon crate)

**채택 정책**: `backon` 0.4+ ExponentialBuilder. `with_min_delay(200ms).with_max_delay(5s).with_factor(2.0).with_max_times(3).with_jitter()`. 재시도 대상: 5xx + 429 + reqwest connect/read timeout. 즉시 실패: 4xx (400/401/403/404/405/422) — 사용자 입력 / 모델 ID 오류라 재시도 무의미. 200ms → 400ms → 800ms (5s cap), 총 1.4s 추가 wait → 사용자 인내 한계 내.

**Retry-After 헤더 존중**: 503/429 응답에 `Retry-After` 헤더 있으면 backon delay 대신 헤더 값 사용. reqwest는 헤더 자동 노출 (`response.headers().get(reqwest::header::RETRY_AFTER)`). bench-harness Phase 2'.c는 *measurement run*이라 retry 안 함 (정확도 우선). Workbench Validate는 *user-facing*이라 retry로 transient 실패 흡수.

### 4.3 cancel via CancellationToken + child kill

**HTTP cancel**: `tokio::select!`로 send future + `cancel.cancelled()` 동시 listen. cancel 시 reqwest future drop → connection close → 서버 측 abort (Ollama / LM Studio 동일).

**subprocess cancel** (`ollama create`): `Command::new("ollama").args([...]).kill_on_drop(true).spawn()`. `tokio::select!`로 `child.wait()` + `cancel.cancelled()` + `tokio::time::sleep(60s)` 동시 listen. cancel/timeout 시 `child.start_kill()` (SIGKILL Unix / TerminateProcess Windows) + `child.wait().await` (zombie 회피).

- **`kill_on_drop(true)`** — Child wrapper drop 시 자동 kill. panic 안전.
- **graceful shutdown 안 함** — Tokio issue #2504 (SIGTERM-then-SIGKILL built-in 미지원). `ollama create`는 idempotent하므로 SIGKILL 단순 채택.
- bench-harness Phase 2'.c와 동일 `kill_on_drop` 패턴 (NVML polling subprocess). 일관성.

### 4.4 인용

- backon crate: <https://github.com/Xuanwo/backon> + 설계 글: <https://rustmagazine.org/issue-2/how-i-designed-the-api-for-backon-a-user-friendly-retry-crate/>
- reqwest timeouts: <https://reintech.io/blog/reqwest-tutorial-http-client-best-practices-rust>
- tokio CancellationToken: <https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html>
- tokio Child kill issue (#2504): <https://github.com/tokio-rs/tokio/issues/2504>
- Retry-After 헤더 처리: <https://docs.rs/reqwest/latest/reqwest/header/constant.RETRY_AFTER.html>

---

## §5. EULA / 사용자 동의 게이트

### 5.1 외부 EULA 책임 분리

| 컴포넌트 | EULA 보유 | 우리 책임 |
|---|---|---|
| Ollama (MIT) | 없음 | 0 — 단순 shell-out |
| LM Studio | LM Studio 자체 EULA (closed source) | LM Studio가 *설치 시점에* 직접 표시. 우리가 우회 X. |
| LMmaster 본체 | 자체 EULA (Phase 7' 작성) | 첫 실행 시 한국어 EULA dialog (Phase 7'에서) |
| `ollama create` 등록 | N/A | "Modelfile을 시스템에 등록해도 될까요" 1회 동의 (Workbench Settings) |

### 5.2 동의 시점 — 처음 Register 단계 진입 시 1회

**채택**:
1. **첫 Register 단계 진입 시** modal dialog 노출:
   - 제목: "이 모델을 Ollama에 등록할까요?"
   - 본문: "Workbench가 Ollama에 새 모델을 등록할게요. Ollama가 실행 중이어야 해요. 등록한 모델은 다른 앱에서도 보여요."
   - checkbox: "다음부터 묻지 않을게요"
   - 버튼: "등록할게요" / "취소할래요" (해요체).
2. **"다음부터 묻지 않을게요" 체크** → workspace settings에 `workbench.register.consent: true` 영속. 다음번 Register는 dialog 없이 바로 실행.
3. **Settings → "Workbench" → "동의 다시 묻기"** 버튼으로 reset 가능.

### 5.3 LM Studio 모델 사용 시점 — 추가 동의 X

LM Studio에서 받은 모델로 Workbench를 돌리는 경우 — LM Studio EULA는 LM Studio 설치 시점에 이미 사용자가 동의. 우리가 추가 dialog 띄울 의무 없음. ADR-0016 wrap-not-replace 정신.

### 5.4 인용

- EULA clickwrap 패턴: <https://www.termsfeed.com/blog/eula-installation/>
- 첫 실행 onboarding 패턴: <https://www.appcues.com/blog/the-10-best-user-onboarding-experiences>

---

## §6. 테스트 invariant (sub-phase DoD)

### 6.1 wiremock-rs HTTP mock — 10 시나리오

`MockServer::start()` + `Mock::given(method("POST")).and(path("/api/generate")).respond_with(ResponseTemplate::new(200).set_body_json(json!({...})))` 패턴. `set_delay()`로 timeout 시뮬레이션, `respond_with`를 두 번 mount해 retry 시나리오.

| Invariant | 시나리오 |
|---|---|
| Ollama happy path | `/api/generate` 200 + `done:true` → `ValidateReport.text` 정상 |
| Ollama length limit | `done_reason:"length"` → `truncated:true` 배지 |
| Ollama 503 retry-then-success | 첫 호출 503, 두 번째 200 → backon 재시도 후 성공 |
| Ollama 4xx no retry | 400 → 즉시 실패, 1회 호출만 |
| LM Studio happy path | `/v1/chat/completions` 200 + `finish_reason:"stop"` → 정상 |
| LM Studio no model | `GET /v1/models` `data:[]` → `LmStudioError::NotReady` |
| LM Studio length limit | `finish_reason:"length"` → `truncated:true` |
| timeout | wiremock `set_delay(70s)` → 60s timeout fired → `WorkbenchError::Timeout` |
| cancel mid-flight | `CancellationToken::cancel()` 호출 → `WorkbenchError::Cancelled` |
| Retry-After 헤더 존중 | 429 + `Retry-After: 1` → 1s 기다린 뒤 재시도 |

### 6.2 shell-out fixture binary — 6 시나리오

CI에 ollama 의존 X — 가짜 `fake_ollama` Rust binary (`tests/fixtures/fake_ollama/main.rs`)가 `FAKE_OLLAMA_SCENARIO` env 분기로 stdout/stderr/exitcode emit. `success` → `exit 0` + "success", `no_file` → stderr "Error: open ./model.gguf: no such file" + `exit 1`, `no_daemon` → stderr "connection refused" + `exit 1`, `disk_full` → "no space left", `timeout` → `sleep(120s)`. binary path는 `std::env::var("CARGO_BIN_EXE_fake_ollama")` (Cargo가 자동 set).

| Invariant | 시나리오 |
|---|---|
| shell-out success | 파라미터: `success` → exit 0 → `RegisterReport.status: Created` |
| Modelfile 잘못된 경로 | 파라미터: `no_file` → 한국어 매핑 "Modelfile에서 참조한 파일을 찾지 못했어요" |
| daemon 미실행 | 파라미터: `no_daemon` → "Ollama 데몬이 꺼져 있어요" |
| 디스크 부족 | 파라미터: `disk_full` → "디스크 공간이 부족해요" |
| timeout 60s | 파라미터: `timeout` → 60s tokio::time::timeout fire → `WorkbenchError::Timeout` |
| cancel during shell-out | 5s 후 `CancellationToken::cancel()` → `kill_on_drop` SIGKILL → `Cancelled` |

### 6.3 한국어 메시지 단언

각 에러 variant `Display`가 정확한 해요체 한국어 포함 — `assert!(format!("{err}").contains("Ollama 데몬이 꺼져 있어요"))` 패턴. CLAUDE.md §4.1 카피 톤 매뉴얼 일관 — "...해요" / "...할게요" / "...했어요".

---

## §7. 위험 노트 (next session 함정)

### 7.1 SSE streaming은 Phase 5'.e 범위 X

Ollama `/api/generate`는 `stream: true`일 때 SSE-like NDJSON. Workbench Validate는 *단발 응답*이면 충분 → `stream: false`. 다음 세션이 "live token streaming UI 좋아 보이는데?" 추가하면:
- frontend Drawer + backend SSE 양쪽 변경 필요
- bench-harness가 이미 streaming TTFT 측정 (Phase 2'.c) — Workbench Validate는 그 결과를 *재활용*만, 자체 streaming X
- v1 출시 후 사용자 피드백으로 결정. v1.x 잠재 영역.

### 7.2 LM Studio 503 vs 다른 에러 — 명확하지 않음

LM Studio는 모델 미로딩 시 503을 *항상* 주지 않음. 빌드에 따라 200 + 빈 응답 / 400 / undefined. 채택:
- **사전 `GET /v1/models`** 1회 polling (data:[] 검사) — 가장 신뢰 가능.
- POST 응답 5xx → backon retry → 여전히 실패 시 "LM Studio가 준비되지 않았어요" + Validate 진입 차단.

다음 세션이 "왜 사전 polling? POST 응답으로 충분한데" 라며 제거하지 말 것 — 빌드 의존성을 회피하기 위함.

### 7.3 shell-out Tauri sandbox 권한

Tauri 2 `tauri-plugin-shell`은 *명시적 scope allowlist*. 우리는 `ollama` CLI를 *Rust backend*에서 직접 spawn하므로 (frontend → backend IPC → Rust `tokio::process::Command`) Tauri shell plugin 우회 — 권한 prompt 없음.

**주의**: frontend가 `invoke('open_shell', { cmd: 'ollama' })` 패턴을 사용하면 plugin scope 위반. 반드시 IPC 명령(`run_stage_register`)을 정의하고 Rust backend가 spawn — frontend는 cmd string 직접 전달 X. 이미 Phase 5' IPC 패턴 일관.

### 7.4 Ollama Windows daemon systray 이슈

Windows에서 사용자가 systray "Quit Ollama" 클릭 → daemon 즉시 종료. 우리의 `GET /` ping이 connection refused.

**채택**: 한국어 안내 + "Ollama 다시 켜기" 버튼 (외부 `open_url("ollama://launch")` 또는 사용자 매뉴얼 안내). silent restart 시도 X (ADR-0016 wrap-not-replace).

### 7.5 Modelfile escape 반복 검증

Phase 5' modelfile.rs `escape_system_prompt`는 한국어 + 줄바꿈 + 큰따옴표 안전 처리. 다음 세션이 "raw 한국어로 충분"이라 escape 제거하면 issue #4666 같은 "command must be one of" 재발생 위험. Phase 5'.e 테스트 케이스에 escape 라운드트립 invariant 추가:

```text
let spec = ModelfileSpec { system_prompt_ko: "안녕\"하세요\\n반갑\\습니다", ... };
let rendered = render(&spec);
// rendered가 SYSTEM """안녕\\"하세요\\\\n반갑\\\\습니다""" 같이 정확 escape
```

### 7.6 Connection 재사용 vs 새 client per call

reqwest::Client는 connection pool 보유. Workbench는 *short-lived process*가 아니라 long-running app이므로 single Arc<Client>를 재사용 (Phase 2'.c bench-harness와 동일 패턴). 다음 세션이 "매 호출마다 ClientBuilder::new()" 만들면 connection pool 무효화 + cold start latency.

---

## §8. 참고 (선행 페이즈 차용 + ADR)

### 8.1 Phase 2'.c bench-harness `BenchAdapter` HTTP 패턴 재활용

bench-harness는 이미 `OllamaAdapter` / `LmStudioAdapter`에 streaming HTTP 메서드 추가 (Phase 2'.c). Workbench `WorkbenchResponder`는 *단발(stream:false)* 변형이라 분리 trait이 좋음 — `WorkbenchResponder::validate_once(prompt, cfg, cancel) -> Result<ValidateReport, WorkbenchError>`. 구현체: `OllamaResponder` / `LmStudioResponder` / `MockResponder`(기존 유지). `reqwest::Client`는 `Arc::clone`으로 bench-harness와 공유 (앱 단일 client pool).

### 8.2 Phase 1A.3 installer downloader 재시도 정책

Phase 1A.3은 resumable download + 5xx 재시도 (backon 동일 crate). Workbench responder는 short request라 resumable 불필요 — 재시도 정책만 차용.

**차이**: installer는 *멱등 download*라 retry 안전. Workbench HTTP는 *generation request*라 retry 시 다른 응답 받음 (temperature > 0). 사용자 향 "재시도 시 응답이 다를 수 있어요" 문구는 *불필요* (단일 응답 미리보기라 사용자가 1회 응답에 만족하면 끝).

### 8.3 ADR 인용

- **ADR-0004** (runtime adapter pattern): `RuntimeAdapter` trait가 모든 어댑터의 base. Workbench는 한 단계 위 abstraction (`WorkbenchResponder`)이지만 동일 client pool 공유.
- **ADR-0013** (외부 통신 0): localhost-only + `no_proxy()`. Phase 5'.e의 모든 HTTP는 11434 / 1234 단일 호스트.
- **ADR-0016** (wrap-not-replace): `ollama create`는 외부 CLI 호출이고 silent X. 사용자 동의 게이트 §5.

### 8.4 다음 페이즈 인계 — Phase 5'.f (CLI subprocess 실 동작, v1.x)

- Phase 5'.f가 `LlamaQuantizer` (llama-quantize subprocess) + `LLaMAFactoryLoRATrainer` 실 연결.
- 본 Phase 5'.e의 `tokio::process::Command + kill_on_drop` 패턴 그대로 재사용.
- 진행률 stdout 라인 파싱 (Phase 5' 결정 노트 §1.4 `0/25/50/75/100` 5-step) — Phase 5'.f가 실 stdout 패턴 매칭.

**진입 조건**:
- Phase 5'.e 머지 + WorkbenchResponder (real HTTP) green.
- shell-out fixture binary 패턴이 Phase 5'.f 실 CLI subprocess에 그대로 transferable.
- llama-quantize / LLaMA-Factory 둘 다 별도 의존성 (사용자 PATH or bundled). 기본은 사용자 PATH; bundle은 v1.x.

### 8.5 Phase 7' (릴리스) 인계

- Workbench Register는 사용자 동의 게이트 (§5)와 LMmaster 본체 EULA (Phase 7') 모두 의존.
- 본 노트 §5의 동의 dialog는 Phase 5'.e에서 구현하고, 그 위에 Phase 7'이 첫 실행 EULA를 추가.

---

## §9. 결정 노트 6-섹션 매핑 (CLAUDE.md §4.5)

| 섹션 | 본 노트 위치 |
|---|---|
| §1 결정 요약 | §0 (8가지) |
| §2 채택안 | §1 / §2 / §3 / §4 / §5 |
| §3 기각안 + 이유 | §1.3 (`/api/chat` 거부), §4.2 (4xx no retry), §5.1 (LM Studio EULA dialog 안 띄움), §7.1 (SSE streaming 거부), §7.5 (escape 제거 거부) |
| §4 미정 / 후순위 | §7.1 (live token streaming v1.x), §8.4 (Phase 5'.f 실 CLI) |
| §5 테스트 invariant | §6 (wiremock 10 시나리오 + fixture binary 6 시나리오) |
| §6 다음 페이즈 인계 | §8.4 (Phase 5'.f) + §8.5 (Phase 7') |

---

**버전**: v1.0 (2026-04-28). Phase 5'.e 구현 직전. 다음 갱신: 구현 완료 시 검증 결과 + 차분 테스트 카운트 추가.
