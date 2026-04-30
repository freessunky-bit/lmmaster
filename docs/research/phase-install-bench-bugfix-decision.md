# Phase Install/Bench Bugfix — 결정 노트 (2026-04-29)

> **문제**: 사용자가 모델 카탈로그에서 "이 모델 설치할게요" 버튼을 눌러도 무반응이고, 30초 측정도 즉시 "측정 호출이 모두 실패했어요" 에러로 끝남.
> **임팩트**: v1 ship 직전 사용자 첫 인상의 두 핵심 행동이 모두 작동 안 함. 출시 차단 수준.

## 1. 결정 요약

1. **이벤트 이름 mismatch 수정** — `Catalog.tsx`가 `lmmaster:nav` dispatch, `App.tsx`는 `lmmaster:navigate` listen. 한 단어 차이로 영구 fail.
2. **모델 설치 IPC 신설** — runtime 설치(`install_app`)와 분리된 모델 풀 전용 `start_model_pull` + `cancel_model_pull`. Ollama `/api/pull` NDJSON 스트림 직접 파싱.
3. **카탈로그 drawer에서 in-place 모델 설치** — 페이지 이동 대신 드로워 안에 진행률 패널 인라인 노출. 이동 후 모델 컨텍스트가 사라지는 문제를 원천 차단.
4. **벤치 preflight 4단계** — (a) `/api/version`, (b) `/api/tags`, (c) cold-load probe, (d) 본 측정. 모델 미존재 시 측정 호출 0회로 즉시 종료 + ModelNotLoaded 보존.
5. **bench-harness: last_error 보존** — `runner.rs::harness_loop`가 모든 호출 실패 시 마지막 어댑터 에러를 `BenchErrorReport`에 매핑. generic "측정 호출이 모두 실패했어요" 폐기.
6. **BenchChip: ModelNotLoaded → 인라인 CTA "이 모델 먼저 받을게요"** — 측정 실패 사유별 분기. 한국어 카피 §4.1 준수.

## 2. 채택안

### 2.1 이벤트 이름 통일

`Catalog.tsx:317` `dispatchEvent("lmmaster:nav", { detail: "install" })` → `"lmmaster:navigate"`로 1줄 수정. 이미 다른 cross-nav 사용처(Diagnostics → Catalog)와 일치. **수정 비용 1줄, 사용자 클릭 즉시 반응.**

### 2.2 `start_model_pull` Tauri command (신규)

위치: `apps/desktop/src-tauri/src/model_pull/mod.rs` (신규).

- `tauri::ipc::Channel<ModelPullEvent>` per-call 스트림. `Emitter::emit` broadcast 대신 typed + ordered.
- 신규 `ModelPullRegistry` (clone of `InstallRegistry` 패턴) — `Arc<Mutex<HashMap<String, CancellationToken>>>`.
- 어댑터: 기존 `crates/adapter-ollama` `OllamaAdapter::pull_model`을 *재작성*. 현재 `stream: false`로 진행률 0이라 실용 불가 → reqwest `bytes_stream()` + `tokio_util::codec::LinesCodec`로 NDJSON 한 줄씩 파싱.
- 진행률 정규화: `HashMap<digest, (total, completed)>` 누적 후 `sum(completed) / sum(total)`로 단일 % 노출. layer 단위 % 그대로 노출 시 0 → 100 → 0 점프 발생.
- EMA speed: `speed = 0.7 * prev + 0.3 * delta_bytes / delta_ms` — 5초 sliding window 효과.
- ETA: `(total - completed) / speed_ema` — speed가 0이면 표시 보류.
- 에러: status 필드 없는 첫 객체에 `error` 필드 → `ModelPullError::ModelNotFound { model_id }`. connection drop → `Network`. timeout → `Timeout`.
- LM Studio: 모델 풀 미지원. 호환 모델일 때는 `open_url`로 LM Studio 앱 + 한국어 안내 toast (silent install 금지 — EULA 정책).

### 2.3 카탈로그 drawer 인라인 진행

`ModelDetailDrawer`에 새 `pullState: ModelPullChipState` 추가:
- `idle` — "이 모델 설치할게요" 버튼.
- `pulling` — 진행률 bar + speed + ETA + cancel.
- `done` — "받기 완료, 이제 측정할 수 있어요" + 측정 버튼 강조.
- `error` — 한국어 사실 진술 + 다시 시도 / 다른 모델 보기.

페이지 이동 없이 drawer 안에서 완결 — 이동 시 모델 컨텍스트 손실 + 다시 찾는 마찰 제거.

### 2.4 Bench preflight

`apps/desktop/src-tauri/src/bench/commands.rs::start_bench`에 `preflight()` 추가:

```rust
async fn preflight(adapter: &dyn BenchAdapter, model_id: &str, runtime_kind: RuntimeKind, http: &reqwest::Client) -> PreflightResult {
    // 1. /api/version
    // 2. /api/tags → contains model_id?
    // 3. cold-load probe: POST /api/generate {prompt:" ", num_predict:1, stream:false} 5s timeout
}
```

실패 시 `BenchReport`에 `error: Some(BenchErrorReport::{RuntimeUnreachable | ModelNotLoaded | ColdLoadTimeout})` 채우고 즉시 반환. `run_bench` 호출 자체를 skip — 30초 budget 낭비 방지.

### 2.5 last_error 보존

`crates/bench-harness/src/runner.rs::harness_loop`에 `last_error: Option<BenchError>` 추적:

```rust
for pass in ... {
    for seed in ... {
        match adapter.run_prompt(...) {
            Ok(s) => samples.push(s),
            Err(BenchError::Cancelled) => return ...,
            Err(e) => { last_error = Some(e.clone()); /* warn + continue */ }
        }
    }
}
```

`aggregate`에 `last_error` 인자 추가 후 samples empty일 때 매핑:

```rust
let error = match (cancelled, timeout_hit, last_error) {
    (true, _, _) => Some(BenchErrorReport::Cancelled),
    (_, true, _) => Some(BenchErrorReport::Timeout),
    (_, _, Some(BenchError::RuntimeUnreachable(m))) => Some(BenchErrorReport::RuntimeUnreachable { message: m }),
    (_, _, Some(BenchError::ModelNotLoaded(m))) => Some(BenchErrorReport::ModelNotLoaded { model_id: m }),
    (_, _, Some(BenchError::InsufficientVram { need_mb, have_mb })) => Some(BenchErrorReport::InsufficientVram { need_mb, have_mb }),
    _ => Some(BenchErrorReport::Other { message: "측정 호출이 모두 실패했어요" })
};
```

기존 generic 메시지는 `BenchError::Internal`이 마지막 에러였을 때만 남음.

### 2.6 BenchChip 업그레이드

- `model-not-loaded` → "이 모델을 먼저 받아주세요" + "받을게요" 버튼 (drawer의 모델 풀 상태로 위임).
- `runtime-unreachable` → "Ollama가 실행 중이 아니에요" + "Ollama 켜기" 버튼 (런타임 페이지로 이동).
- `cold-load-timeout` → "모델을 불러오는 데 오래 걸려요. 다시 시도할까요?" + retry.

## 3. 기각안 + 이유

**A. 이벤트 이름 양쪽 수정 (양방향 호환)** — `nav` 이벤트도 추가 listen 등록.
- ❌ 거부: 같은 의미의 이벤트가 두 이름으로 공존하면 미래 이슈 누적. 단일 source of truth가 옳다.

**B. ollama-rs 크레이트 사용 (`pull_model_stream`)**
- ❌ 거부: ollama-rs Issue #89(abort 미지원) + #2876(stream drop이 server abort 아님) 미해결. Polyglot-Ko 12.8B 같은 7GB+ 모델에서 cancel 해도 백그라운드에서 계속 받는다 → 디스크 차서 사용자 신뢰 박살. 직접 reqwest + LinesCodec로 cancel 통제.

**C. 페이지 이동(InstallPage) + InstallPage에 모델 풀 추가**
- ❌ 거부: InstallPage는 *런타임 설치* 페이지(Ollama/LM Studio). 모델 풀을 거기 끼우면 UI 의미가 흐려짐. 추가로 catalog → install 이동 시 모델 카드 컨텍스트 사라짐 → 사용자가 다시 찾아야 함.

**D. layer-단위 progress 노출 (digest별 % 별도)**
- ❌ 거부: 7-9 layers 모델은 0→100 점프가 7회. 사용자 "왜 진행률이 거꾸로 가요?" 컴플레인 패턴(open-webui v0.1대 검증). aggregate가 단일 source of truth.

**E. 측정 시작 전 사전 검증 없이 30초 그대로 budget 소진**
- ❌ 거부: 모델 없으면 매 호출이 즉시 fail (HTTP 404). 30초 6회 호출 = 즉시 실패 6회 + 24초 대기 = 사용자 불쾌. preflight 1초로 30초 절약.

**F. last_error를 BenchError → BenchErrorReport `Other { message }` raw 매핑**
- ❌ 거부: 한국어 메시지는 backend에서 fmt!된 임의 string. UI가 분기할 수 없어 i18n + CTA 분화 불가. tagged enum variant로 매핑이 옳다.

**G. cold-load probe `num_predict: 0`** (가장 짧은 prompt)
- ❌ 거부: Ollama는 `num_predict: 0`을 model load only로 해석 — 측정값 0 반환. probe로 적합. 그러나 OOM 검출엔 부족 — 1 token 생성까지 가야 OOM이 드러남. `num_predict: 1`이 정답.

**H. cancel 시 `/api/delete` 자동 호출로 부분 다운로드 청소**
- ❌ 거부 (v1.x로 이월): destructive action. 사용자가 cancel만 한 의도에 delete까지 끼면 expectation 어긋남. 별도 "받다 만 모델 지울래요?" toast로 v1.x에 추가.

**I. tauri::Emitter::emit() 사용 (현 install_app 패턴 미러)**
- ❌ 거부: emit은 broadcast — 같은 모델을 여러 창/팝업에서 동시 풀 시 race. `Channel<T>`은 1:1 typed + ordered + close 감지. ADR-0042 임베딩 다운로드도 emit이라 확장 시 Channel로 마이그레이션 권장 (별도 페이즈).

## 4. 미정 / 후순위 이월 (v1.x)

- **부분 다운로드 cleanup**: cancel 시 `/api/delete` 호출 옵션 toast.
- **resume**: Ollama 0.1.40+가 자체 chunk-resume 지원 — 우리 측 추가 작업 없음. 사용자 향 메시지에 "이어 받기 가능" 명시 카피 추가는 v1.x.
- **LM Studio 모델 풀**: SDK가 silent install 미지원 → v1.x에서 LMS CLI(`lms get`) 옵션 평가.
- **VRAM 사전 검증**: cold-load probe로 OOM detection 가능하지만 *사전 추정*은 model-registry에서 산출. v1.x에서 카드 hint chip에 통합.
- **다중 모델 동시 풀**: `ModelPullRegistry`가 다중 entry 지원하지만 UI는 1개만 노출. v1.x에서 sidebar mini-progress.

## 5. 테스트 invariant

신규 모듈/변경에 다음 invariant 보장:

| 영역 | invariant |
|---|---|
| `model_pull/registry.rs` | try_start 중복 거부 / cancel idempotent / cancel_all 모든 토큰 / Drop으로 finish 보장 (InstallRegistry 미러) |
| Ollama pull NDJSON 파싱 | (a) `completed` 누락 객체 파싱 graceful (= 0), (b) `error` 필드 단일 객체 → `ModelNotFound`, (c) `success` status 받으면 종료, (d) connection drop → `Network` |
| layer aggregation | 7 layer × 50% 진행 → 단일 % = 50%, 0 → 100 점프 없음 (모든 chunk 후 진행률 monotonic) |
| EMA speed | 5회 update 후 평균이 raw 마지막 값보다 noise 작음 (variance test) |
| Bench preflight | (a) version unreachable → RuntimeUnreachable + run_bench 호출 0회, (b) tags에 model 없음 → ModelNotLoaded + run_bench 0회, (c) cold-load timeout 5s → ColdLoadTimeout |
| `runner.rs` last_error 보존 | RuntimeUnreachable adapter → samples empty + `error: ModelNotLoaded` 아님 (last_error가 RuntimeUnreachable이라 그대로 매핑) |
| BenchChip a11y | 각 error variant마다 `role="status"` + 한국어 카피 substring 단언, retry 버튼 focus-visible ring 토큰 적용 |
| 한국어 카피 §4.1 | "받고 있어요" / "받을게요" / "다시 시도할게요" 해요체 — 의문형/공식체 금지 grep test |
| BenchReport tagged enum round-trip | 새 variant `runtime-unreachable` / `model-not-loaded` / `cold-load-timeout` JSON serde 검증 |

## 6. 다음 페이즈 인계

**진입 조건 충족**:
- 본 결정 노트 + 결정안 §1~§3 commit.
- 보강 리서치 1건 (subagent) — 본 노트 §2.2/§2.4의 NDJSON·preflight 패턴은 모두 리서치 결과 반영.

**의존성**:
- `crates/adapter-ollama` — `pull_model` 재작성 시 기존 `BenchAdapter::run_prompt` 영향 없음 (메서드 분리).
- `crates/bench-harness` — `BenchErrorReport` enum에 새 variant 추가는 IPC frontend 측 type 동기화 필요. `apps/desktop/src/ipc/bench.ts` 수정 필수.
- `apps/desktop/src/components/catalog/ModelDetailDrawer.tsx` — pull state 추가는 prop 추가 / 컴포넌트 분할 후보.

**위험 노트**:
- **race**: `Channel<ModelPullEvent>` close 후 send 시도 → InstallSinkClosed처럼 변환 + cancel 트리거. 기존 install_app 패턴 그대로 차용.
- **NDJSON parse 무한대기**: Ollama가 status 없이 멈춤 → reqwest read timeout 60s + `tokio::time::timeout` per chunk.
- **진행률 monotonic 보장 실패**: layer 누적 시 sum(total)이 동적으로 늘어남 (manifest 단계엔 layer 수 모름) → "측정 시작 후 layer가 추가되면 % 일시적 감소" 가능. 카피로 "받을 파일을 확인하고 있어요" 명시 단계 추가로 마스킹.
- **bench-harness API 변경 영향**: `aggregate` 함수 signature 변경은 외부 caller에게 breaking change. 본 crate는 internal-only이므로 무영향 검증.

**v1 영향**:
- Ship 차단 버그 2건 해결 — v1 베타 즉시 가능.
- 사용자 첫 인상 핵심 행동(설치 + 측정) 정상 동작.
- 잔존 v1.x 항목은 §4 4건 — 모두 비차단.
