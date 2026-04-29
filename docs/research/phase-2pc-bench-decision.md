# Phase 2'.c — 30초 벤치마크 harness 결정 노트

> 작성일: 2026-04-27
> 상태: 보강 리서치 완료 → 설계 확정 (구현 직전)
> 선행: Phase 1A (런타임/하드웨어 probe), Phase 2'.a (카탈로그), Phase 2'.b (Catalog UI)
> 연계: Phase 5' (Workbench 첫 응답 미리보기), G4 갭 (하드웨어-인지 배지)

## 0. 결정 요약 (7가지)

1. **`crates/bench-harness` 신설, 새 HTTP layer 만들지 않음** — 기존 `OllamaAdapter` / `LmStudioAdapter`에 streaming 측정 메서드를 trait extension으로 추가한다 (`BenchAdapter` trait, 두 어댑터에 impl). 같은 reqwest 클라이언트 / no-proxy / connect_timeout 정책 재사용.
2. **TTFT는 streaming bytes-stream의 첫 chunk 도착 시각으로 측정** — `/api/generate { stream: true }` (Ollama) / `/v1/chat/completions { stream: true }` (LM Studio) 의 SSE 첫 token chunk가 들어오는 순간을 `Instant::now() - request_sent`로 기록. e2e latency는 마지막 chunk + done flag 수신까지.
3. **Ollama의 native metric을 1순위로 채택** — `eval_count` / `eval_duration` (ns) / `prompt_eval_count` / `prompt_eval_duration` / `load_duration` / `total_duration` 다섯 필드는 streaming 마지막 chunk의 `done: true` 응답에 포함된다. HTTP wall-clock 대비 훨씬 정확하므로 `throughput_tps = eval_count / (eval_duration as f64 / 1e9)`. LM Studio는 OpenAI 표준이라 native counter 없음 → wall-clock fallback.
4. **30초 절대 타임아웃 + cooperative cancel** — `tokio::select! { _ = harness_run() => …, _ = sleep(Duration::from_secs(30)) => Timeout, _ = cancel.cancelled() => UserCancelled }`. timeout이어도 그때까지 받은 토큰으로 partial report 생성 (사용자 향: "30초 안에 N토큰 — 약 X tps").
5. **Warmup 1회 + measure 2회 평균 (한국어 프롬프트 3종 × 2 패스 = 6 호출)** — 최초 1회는 `keep_alive: "30s"` warmup만 (TTFT 폐기), 그 뒤 2회 측정하여 산술평균. "공정한 비교"의 기준은 first-token-after-warm. cold-start 측정은 별도 필드 `cold_load_ms`로 분리.
6. **결과 캐시 키 = `(runtime_kind, model_id, quant_label, host_fingerprint_short)` + TTL 30일 + 모델 digest 일치 검사** — 캐시 위치는 portable-workspace 의 `cache/bench/{runtime}-{model_slug}-{date}.json`. invalidate 트리거: (a) 30일 경과, (b) host_fingerprint(GPU 모델/VRAM/RAM)가 바뀜, (c) Ollama digest mismatch. v1.x에서 모델 변경 자동 감지를 위해 `digest_at_bench`도 저장.
7. **카탈로그 카드에 한 줄 한국어 요약 — "이 PC에서 초당 N토큰" + 시각 chip** — primary metric은 `throughput_tps`(generation tokens/s), secondary는 `ttft_ms`. 카탈로그 카드 hint chip: "초당 12.4토큰 · 첫 응답 0.8초". 미측정 모델은 카드에 "측정 가능" 버튼만 표시. peak RAM/VRAM은 Drawer 상세 화면에서만 노출 (카드는 단순화).

## 1. 보강 리서치 — 5영역

### 1.1 llama.cpp `llama-bench` — 표준 측정 항목과 토큰 카운트 보정 (~290 단어)

`llama.cpp/examples/llama-bench`는 사실상 GGUF 생태계 표준이다. 측정 축 4가지를 분리해 명세한다:

- **prompt processing throughput (`pp512` 등)**: 512 토큰 prompt를 단발 batch로 evaluate한 tokens/s. attention cache가 비어있는 prefill 단계.
- **generation throughput (`tg128` 등)**: 128 토큰 단발 generate. autoregressive 단계로 우리가 사용자 향으로 보여주는 "초당 N토큰"의 본질.
- **model load time**: GGUF mmap 후 첫 forward까지의 wall-clock.
- **batch / threads / n_gpu_layers** 같은 환경 변수 동시 기록 — 재현성 확보용.

**우리에게 시사하는 점**: pp와 tg를 **분리해서 보고해야** "큰 prompt에서도 빠른가"를 답할 수 있다. 우리는 HTTP 위 어댑터라 token 단위 직접 셈은 어렵지만 — Ollama의 `prompt_eval_count` / `eval_count`가 정확히 이 두 값을 넘겨준다. LM Studio는 OpenAI 호환이라 `usage.prompt_tokens` / `usage.completion_tokens`만 와서 시간 분리는 wall-clock 추정 (TTFT를 prompt eval 끝으로 간주, 그 이후는 generation으로 간주).

**보정 정책**: 우리 BenchReport에 `pp_tps` (prompt processing tokens/s)과 `tg_tps` (generation tokens/s)를 모두 두되, LM Studio는 `pp_tps = None`로 두고 UI에서도 dash 처리. 카드 한 줄 요약의 primary metric은 `tg_tps` (사용자가 체감하는 "타자기 속도").

**토큰 카운트 정확도**: 한국어는 BPE에서 영어 대비 2~3배 토큰을 쓴다 (subword fragmentation). 따라서 같은 한국어 프롬프트로 모델 간 비교하면 fair — 다국어 비교는 의미 없음. 우리는 한국어 시드만 1순위, 영어는 v1.1+ optional.

### 1.2 vLLM `benchmark_serving.py` — TTFT / ITL / e2e 패턴 (~280 단어)

vLLM은 production serving용 stress test 표준을 만들었다. 핵심 5개 metric:

- **TTFT (Time To First Token)**: 클라이언트가 request를 보낸 시각부터 **첫 token chunk**가 들어오는 시각까지. SSE 위에서는 첫 `data:` 라인 도착 순간.
- **ITL (Inter-Token Latency)**: 두 번째 chunk부터 마지막 chunk까지의 chunk-to-chunk 평균/p50/p99. tg_tps의 inverse.
- **e2e latency**: 처음 send → 마지막 chunk + `[DONE]` 마커.
- **request throughput (req/s)**: 동시 부하 측정. 우리는 single-user smoke test라 N/A.
- **goodput**: SLO 만족하는 부분만. 단일 사용자엔 무의미.

**HTTP streaming TTFT 정확 측정 패턴 (Rust/reqwest)**:

```text
let req_sent = Instant::now();
let resp = client.post(url).body(...).send().await?;
let mut stream = resp.bytes_stream();
let first_chunk_at = None;
while let Some(chunk) = stream.next().await {
    if first_chunk_at.is_none() { first_chunk_at = Some(Instant::now()); }
    // SSE parse, accumulate tokens
}
let ttft = first_chunk_at.unwrap() - req_sent;
```

`reqwest::Response::bytes_stream()`은 `futures::Stream<Item = Result<Bytes>>`를 반환. 첫 byte와 첫 token chunk는 Ollama 기준 동시 도착 (line-buffered NDJSON). LM Studio의 `data:` SSE는 첫 line이 빈 chunk거나 role assignment일 수 있어 `delta.content.is_empty()`는 skip하고 첫 non-empty content를 기준 삼아야 한다.

**우리 채택**: TTFT는 첫 non-empty token (Ollama: `response` 필드 non-empty, LM Studio: `delta.content` non-empty). e2e는 마지막 `done: true` (Ollama) / `[DONE]` (LM Studio).

### 1.3 Ollama 자체 bench — `eval_count` / `eval_duration` 활용 (~270 단어)

Ollama의 `/api/generate` 응답은 streaming이든 non-streaming이든 마지막 chunk에 다음 5개 native counter를 담는다 (단위 ns):

- `total_duration` — request → response 전체.
- `load_duration` — 모델을 GPU/RAM에 올리는 데 걸린 시간 (이미 warm이면 ~0).
- `prompt_eval_count` — prefill 토큰 수.
- `prompt_eval_duration` — prefill에 걸린 시간.
- `eval_count` — 생성 토큰 수.
- `eval_duration` — 생성에 걸린 시간.

**검증**: Ollama 0.4.x 기준 streaming `done: true` chunk에 위 모든 필드 포함됨 (공식 문서 `api/generate` 섹션). 따라서:

```text
prompt_processing_tps = prompt_eval_count / (prompt_eval_duration as f64 / 1e9)
generation_tps        = eval_count        / (eval_duration        as f64 / 1e9)
```

이것은 llama-bench의 `pp_tps` / `tg_tps`와 동일한 의미. **HTTP wall-clock보다 훨씬 정확** — JSON parse / network round-trip 잡음 제외.

**우리 채택**: Ollama는 native counter 1순위, wall-clock TTFT는 단지 "사용자가 체감하는 첫 응답"용으로 별도 보존. LM Studio는 native counter 없으므로 `usage.prompt_tokens` / `usage.completion_tokens` + wall-clock 분할로 추정 (정확도 떨어진다는 caveat를 BenchReport에 `metrics_source: "native" | "wallclock-est"` 필드로 명시 — UI에서 LM Studio는 "추정" 배지).

**load_duration의 의미**: Ollama는 첫 호출 시 모델 로드를 같은 응답 안에 합산. warmup 호출에서 이 값을 따로 보존해 `cold_load_ms`로 BenchReport에 노출. 두 번째 호출부터는 `load_duration` ~ 0이어야 정상.

### 1.4 Foundry / Azure ML — 캐시 키 정책 (~250 단어)

Microsoft Foundry Local과 Azure ML real-time inference 둘 다 모델 카드에 "이 머신에서의 latency"를 표시한다. 두 시스템의 캐시 invalidate 정책을 보면:

- **Foundry Local**: `(model_sha, device_class, sdk_version)` 튜플로 키. SDK 업데이트 시 전부 리벤치.
- **Azure ML**: SKU별 prebuilt result + 사용자 endpoint별 dynamic. dynamic은 7일 TTL.
- **Hugging Face Inference Endpoints**: 모델 push / config 변경 시 자동 invalidate.

**우리 채택 키**:

```text
BenchKey = (
    runtime_kind: RuntimeKind,         // Ollama | LmStudio
    model_id: String,                  // catalog id
    quant_label: Option<String>,       // "Q4_K_M" 등 — null 가능 (LM Studio MLX)
    host_fingerprint_short: String,    // sha256(GPU model + vram_mb + ram_mb + os)[:16]
)
TTL = 30 days
```

**Invalidate 트리거**:
1. 30일 경과 — 디스크에 `bench_at` 타임스탬프 저장, lazy invalidate.
2. Host fingerprint 바뀜 — GPU 교체/RAM 추가/OS major upgrade 등.
3. Ollama digest 불일치 — 같은 `model_id`라도 사용자가 `ollama pull`로 교체했을 수 있음. `digest_at_bench` 저장 + 비교.
4. 사용자 명시 "다시 측정".

**저장 위치**: `portable-workspace`의 `cache/bench/` 디렉토리. 파일명 `{runtime}-{model_slug}-{host_short}.json`. 한 사용자/한 PC에서 모델당 1 파일.

**왜 30일?**: 모델 weight는 안 바뀌고 OS/드라이버 변동도 30일 정도면 합리적인 cadence. 7일은 너무 자주 (사용자 짜증), 90일은 GPU 드라이버 메이저 업데이트 후 stale.

### 1.5 Peak RAM/VRAM 측정 — NVML 1초 polling (~270 단어)

GPU peak VRAM 측정은 polling이 표준이다 (push notification 미지원). 3 OS 패턴:

- **Windows + NVIDIA**: NVML `Device::memory_info().used`를 1Hz로 polling. 우리는 이미 `crates/hardware-probe/src/gpu.rs`에 `nvml_wrapper` 통합돼 있어 그대로 재사용.
- **Windows non-NVIDIA**: DXGI `IDXGIAdapter3::QueryVideoMemoryInfo` (Adapter3 cast 필요) — `CurrentUsage` 필드. v1.1로 미룸 (Intel/AMD bench는 추후).
- **Linux + NVIDIA**: NVML 동일.
- **Linux + AMD**: `/sys/class/drm/cardN/device/mem_info_vram_used` (sysfs). v1.1.
- **macOS**: Metal `MTLDevice::currentAllocatedSize` (objc2-metal). v1.1.
- **System RAM (모든 OS)**: `sysinfo` crate의 `Process::memory()` — 단, Ollama/LM Studio가 별도 프로세스라 PID를 잡아야 함. **우리는 process-level 대신 system-level RSS 변동분**으로 추정 (bench 시작 직전 baseline → bench 동안 max 차이). 정확도는 낮지만 다른 프로세스 영향 받음을 caveat로 표시.

**Polling 패턴 (tokio)**:

```text
let monitor_task = tokio::spawn(async move {
    let mut peak_vram = 0u64;
    let mut interval = tokio::time::interval(Duration::from_millis(1000));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                if let Some(used) = sample_vram() {
                    peak_vram = peak_vram.max(used);
                }
            }
            _ = stop_signal.notified() => break,
        }
    }
    peak_vram
});
```

Bench 끝나면 `stop_signal.notify_one()` + `monitor_task.await`. 30초 동안 30번 sampling — 충분한 해상도, NVML 호출 부하는 ms 미만이라 무시 가능.

**v1 범위**: Windows + NVIDIA만 정확 측정. 그 외는 `peak_vram_mb: None`로 두고 RAM도 system-level 변동으로만. 카드 표시는 NVIDIA Windows 한정.

## 2. BenchReport 스키마

```text
struct BenchReport {
    // 키
    runtime_kind: RuntimeKind,       // Ollama | LmStudio
    model_id: String,
    quant_label: Option<String>,
    host_fingerprint_short: String,
    bench_at: DateTime<Utc>,

    // 모델 식별 (invalidate용)
    digest_at_bench: Option<String>, // Ollama digest, LM Studio는 None

    // 1차 metric (사용자 카드용)
    tg_tps: f64,                     // generation tokens/s — 카드 primary
    ttft_ms: u32,                    // 첫 응답 ms — 카드 secondary

    // 2차 metric (Drawer 상세)
    pp_tps: Option<f64>,             // prompt processing tps (Ollama only)
    e2e_ms: u32,                     // 전체 응답 wall-clock
    cold_load_ms: Option<u32>,       // warmup의 load_duration

    // 리소스 (Windows+NVIDIA만 정확)
    peak_vram_mb: Option<u32>,
    peak_ram_delta_mb: Option<u32>,  // baseline 대비 증가분

    // 메타
    metrics_source: BenchMetricsSource, // Native | WallclockEst
    sample_count: u8,                // warmup 후 측정 패스 수 (기본 2)
    prompts_used: Vec<String>,       // 한국어 시드 id
    timeout_hit: bool,                // 30s 절대 타임아웃 발동 여부
    sample_text_excerpt: Option<String>, // 첫 80자 응답 (UI 미리보기)

    // 진단
    error: Option<BenchError>,        // 실패 시 한국어 에러 코드
}

enum BenchMetricsSource { Native, WallclockEst }
enum BenchError {
    RuntimeUnreachable,
    ModelNotLoaded,
    InsufficientVram { need_mb: u32, have_mb: u32 },
    Cancelled,
    Timeout,
    Other(String),
}
```

**왜 이 필드 조합인가**:
- `tg_tps` + `ttft_ms`만으로 카드 1줄 요약 가능.
- `pp_tps` + `e2e_ms` + `cold_load_ms`는 power-user용 Drawer.
- `metrics_source`로 Ollama/LM Studio 정확도 차이를 UI에서 표시.
- `prompts_used`로 어떤 한국어 시드를 썼는지 재현성 보장.
- `error`로 실패 케이스도 캐시 (반복 시도 방지).

## 3. 한국어 프롬프트 시드 (`manifests/prompts/bench-ko.json`)

3 task type — 워크벤치 첫 응답 미리보기와도 호환되도록 설계:

1. **`bench-ko-chat`** (일상 대화, 짧은 prefill / 짧은 generation):
   - "안녕하세요. 오늘 점심은 뭘 추천해주실 수 있나요? 가볍게 먹을 수 있는 한식으로요."
   - 목표 generation: 30~60 토큰. 자연스러운 친절한 응답 유도.
2. **`bench-ko-summary`** (요약, 긴 prefill / 짧은 generation):
   - 약 200자 한국어 뉴스 단락 + "위 내용을 두 문장으로 요약해 주세요."
   - 목표 generation: 40~80 토큰. prompt processing throughput을 가장 잘 드러내는 task.
3. **`bench-ko-reasoning`** (추론, 짧은 prefill / 긴 generation):
   - "1, 1, 2, 3, 5, 8 다음 두 숫자는 무엇이고 왜 그럴까요? 단계별로 설명해 주세요."
   - 목표 generation: 80~150 토큰. tg_tps를 가장 잘 측정.

**왜 이 3 task인가**:
- **chat** = 사용자가 매일 체감하는 평균 길이. 카드의 "초당 N토큰"이 이 케이스 평균을 반영.
- **summary** = pp_tps 차별화 (긴 입력 처리 능력).
- **reasoning** = tg_tps 차별화 (긴 출력 안정성).
- **코드 task는 v1에서 제외** — 한국어 코드 prompt가 robust하지 않고, coding 카테고리는 별도 evaluation suite 필요. v1.1 후보.

3개 시드 × 2 패스 = 6 호출. 30초 안에 못 끝내면 그때까지 평균.

## 4. 30초 timeout + cancel 패턴

```text
async fn run_bench(adapter, model, prompts, cancel: CancellationToken) -> BenchReport {
    let bench_fut = async { /* warmup + 6 호출 + monitor_task */ };
    let timeout_fut = sleep(Duration::from_secs(30));

    tokio::select! {
        result = bench_fut => result,
        _ = timeout_fut    => BenchReport::partial(timeout_hit: true, ...),
        _ = cancel.cancelled() => BenchReport::cancelled(),
    }
}
```

**Cancel 시 정리**:
- `tokio_util::sync::CancellationToken` 사용 (이미 installer/scanner에서 표준).
- HTTP request는 `reqwest::Response::bytes_stream()`을 drop하면 connection close → 서버에 abort 신호.
- Ollama는 stream drop 시 generation을 중단 (LM Studio도 동일).
- monitor_task에 `stop_signal.notify_one()` 보내고 `await` (2초 안에 종료 보장).

**Partial report 정책**: 30초 안에 1 패스만 끝났더라도 그 1 패스로 BenchReport 생성. `sample_count: 1`로 표기. 0 패스(warmup도 못 끝남)는 `error: Timeout`. 사용자 카피: "30초 안에 측정을 마치지 못했어요. 모델이 큰 편이라 충분한 GPU/RAM이 필요할 수 있어요."

## 5. Warmup 정책 (공정 비교)

**왜 warmup이 필수인가**:
- Ollama는 `keep_alive` 만료 시 모델을 unload. 첫 호출은 cold-load (수 초).
- "이 모델이 이 PC에서 얼마나 빠른가"를 답하려면 cold-load 영향을 빼야 fair.
- 단, cold-load 자체도 사용자에게 의미 있음 → `cold_load_ms`로 분리 보존.

**Bench 시퀀스**:
1. **Warmup pass**: `bench-ko-chat` 1회, `keep_alive: "5m"`. response의 `load_duration`을 `cold_load_ms`로 보존. TTFT/throughput은 폐기.
2. **Measure pass 1**: 3 prompts × `keep_alive: "5m"` × stream=true. native counter 수집.
3. **Measure pass 2**: 동일. native counter 수집.
4. **Aggregate**: 산술평균 (median 대신 평균 — 샘플 2개라 median 무의미). p50/p99는 sample_count >= 5에서만 의미.
5. **Cleanup**: `keep_alive: "0s"` 호출로 모델 unload (사용자 메모리 회수). 단, 사용자가 "워크벤치에서 바로 쓰겠다"는 의도가 명확하면 keep alive 유지.

**왜 평균 2회만**: 30초 budget 안에서 6 호출이 빠듯함 (큰 모델 + 추론 prompt 결합 시). 측정 sample 수보다 timeout 안 걸리는 게 더 중요. v1.1에서 budget 60초 옵션 추가 시 sample 수 증가.

## 6. 카탈로그 카드 표시 — 한 줄 요약

**디자인 토큰 사용**: `packages/design-system/src/tokens.css` 의 `--accent-neon-green` + `--surface-2`.

**카드 hint chip 카피 변형**:
- 측정 완료, GPU OK: `초당 12.4토큰 · 첫 응답 0.8초`
- 측정 완료, CPU only: `초당 3.2토큰 · 첫 응답 2.1초 · CPU`
- 측정 완료, partial (timeout): `약 8토큰/초 · 30초 부분측정`
- 측정 안 됨: `[측정하기] 버튼 — "이 PC에서 얼마나 빠른지 30초 측정"`
- 측정 실패 (RuntimeUnreachable): `런타임이 꺼져 있어요`
- 측정 실패 (InsufficientVram): `이 PC에는 무거워요 (need 12GB)`

**Drawer 상세 (워크벤치 진입 직전)**:
- 줄 1: `초당 12.4토큰 (생성) / 256토큰/초 (입력 처리)`
- 줄 2: `첫 응답 0.8초 · 전체 응답 4.2초`
- 줄 3: `최대 GPU 메모리 6.2GB · 시스템 메모리 +1.8GB`
- 줄 4: `2026-04-27 측정 · 30일 후 재측정 권장`
- 줄 5: 측정 출처 배지 — `Ollama 정확측정` 또는 `LM Studio 추정`.

## 7. 구현 산출물 (다음 sub-phase)

- `crates/bench-harness/` 신설:
  - `Cargo.toml` — workspace deps만 (reqwest 0.12 / tokio / tokio-util / serde / chrono / sha2 / tracing / async-trait).
  - `src/lib.rs` — `BenchHarness` 진입점, `run_bench(model_id, opts, cancel)`.
  - `src/adapter.rs` — `BenchAdapter` trait + Ollama/LM Studio impl.
  - `src/report.rs` — `BenchReport` 스키마 + serde.
  - `src/cache.rs` — `BenchCache::load/save/invalidate`, host_fingerprint 계산.
  - `src/monitor.rs` — peak VRAM polling task (NVML 재사용).
  - `src/prompts.rs` — `manifests/prompts/bench-ko.json` 로드.
  - `src/error.rs` — `BenchError` 한국어 에러 enum.
  - `tests/integration_test.rs` — wiremock으로 stream 시뮬레이션 + cancel + timeout + cache invalidate.
- `manifests/prompts/bench-ko.json` 신설 (3 시드).
- Tauri commands: `start_bench(model_id) -> BenchReport`, `cancel_bench(model_id)`, `get_last_bench_report(model_id) -> Option<BenchReport>`.
- Capability JSON 갱신 (3 command 허용).
- React: `apps/desktop/src/components/BenchChip.tsx` — 카드 hint chip + 측정 트리거.
- React: `apps/desktop/src/components/BenchProgress.tsx` — 진행 stepper (warmup → 측정 1/2 → 측정 2/2 → 완료) + 30s 카운트다운 + 사용자 cancel.
- i18n 키: `bench.tps` / `bench.ttft` / `bench.measure` / `bench.cancel` / `bench.timeout-partial` / `bench.runtime-off` / `bench.insufficient-vram`.

## 8. 검증 체크리스트

- [ ] `cargo fmt --all -- --check` ✅
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` ✅
- [ ] `cargo test --workspace` — bench-harness 통합 8건 + 어댑터 추가 4건 = 12건 추가
- [ ] `pnpm exec tsc -b` ✅
- [ ] `pnpm exec vitest run` — BenchChip + BenchProgress 6건 추가 (101건 누적)
- [ ] dev 실행 — 카탈로그 카드에서 측정 트리거 → 30초 안에 한국어 결과 표시
- [ ] cancel 테스트 — 측정 중 cancel 버튼 → 2초 안에 정리 + partial 보고서 디스크 미저장
- [ ] timeout 테스트 — Ollama keep_alive 끄고 큰 모델 측정 → 30초 hit + partial 보고서 표시

## 9. 잔여 / v1.1 이월

- macOS / Linux peak VRAM (objc2-metal / sysfs).
- AMD / Intel GPU 측정.
- 코드 task prompt seed.
- 60초 / 120초 budget 옵션 + sample 5회 + p50/p99.
- 멀티 모델 동시 측정 (현재는 single model lock).
- HF Hub leaderboard 비교 ("이 모델 평균은 초당 N토큰" reference).
- BenchReport export / import (사용자가 결과를 공유하고 다른 PC에서 비교).
