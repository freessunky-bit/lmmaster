# Phase 5' — Workbench v1 core 결정 노트

> 작성일: 2026-04-27
> 상태: 보강 리서치 + scaffold 완료. CLI subprocess 실 동작은 v1.x 진입.
> 선행: `phase-4ef-workbench-diagnostics-decision.md`(placeholder), `phase-2pc-bench-decision.md`(30초 벤치),
>       `competitive_thesis`(Thesis #5 — workbench가 LM Studio/Ollama 격차의 핵심), Phase 4.h(Korean preset 100+).
> 후행: Phase 5'.b — Tauri IPC `start_workbench_run / cancel_workbench_run / list_workbench_runs`,
>       Phase 5'.c — React Workbench 화면 (placeholder → 5단계 UI 승격),
>       Phase 5'.d — bench-harness 연계 (양자화 직후 자동 측정).

---

## §0. 결정 요약

권장 8 가지:

1. **5단계 state machine** — Data → Quantize → LoRA → Validate → Register. 각 단계는 trait + v1 mock impl.
   재실행 가능, cancel 가능, 단계 cache는 `workspace/workbench/{run_id}/{step}/`에 저장.
2. **JSONL 자동 변환** — Alpaca / ShareGPT / OpenAI messages / 한국어 Q&A 4 포맷을 line 단위 자동 감지 →
   OpenAI messages 1차 정규화. 잘못된 line은 skip + warn (전체 실패 회피).
3. **Modelfile generator** — Ollama Modelfile 자동 생성. FROM + PARAMETER + SYSTEM + 옵션 TEMPLATE.
   한국어 system prompt는 `escape_system_prompt`로 큰따옴표 / 줄바꿈 / 백슬래시 안전 처리.
4. **양자화** — `llama-quantize` CLI subprocess wrapper trait + `MockQuantizer`. v1.x에서 실 CLI 연결.
   진행률은 stdout `%` 파싱 정책 — mock은 0/25/50/75/100 5-step emit으로 IPC 구조 검증.
5. **LoRA** — `LLaMA-Factory` CLI subprocess wrapper trait + `MockLoRATrainer`. 한국어 instruction-tuning
   preset(`alpaca-ko` / `KoAlpaca`) 옵션 — `korean_preset: bool` 플래그로 선택.
6. **Korean QA evals** — pytest-style fixtures + deterministic substring 매칭. AI Toolkit / Foundry Toolkit
   eval 패턴 차용. baseline 10 case = factuality / instruction-following / tone-korean 3 카테고리 커버.
7. **trait-first 재실행 가능 단계** — 각 단계는 cache key로 멱등 재실행. 실패 시 단계 단위 retry,
   cancel 시 즉시 `Cancelled` 상태 전이. bench-harness 30s budget 패턴을 그대로 차용.
8. **에러는 한국어 해요체** — 사용자 향 메시지 1차 한국어, 영어는 fallback. 이미 bench-harness /
   scanner 기조와 동일.

---

## §1. 채택안 (각 영역 구체)

### 1.1 5단계 state machine (`flow.rs`)

- `WorkbenchStep`: `Data` → `Quantize` → `Lora` → `Validate` → `Register` (kebab-case serde).
- `RunStatus`: `pending` / `running` / `completed` / `failed` / `cancelled`.
- `WorkbenchRun`: `id` (uuid v4), `created_at` (RFC3339), `current_step`, `completed_steps[]`, `config`, `status`.
- `advance_to(step)` — `current_step`을 `completed_steps`에 push 후 `step`으로 전이. 중복 push 방지.
- `next_step()` — 현재 step 다음 단계 계산. Register 다음은 `None`.
- `mark_failed()` / `mark_cancelled()` — terminal status 전이.
- 재실행: cache는 `workspace/workbench/{run_id}/{step}/output.json`로 저장 (이번 sub-phase는 trait/state만,
  cache 실 구현은 5'.b IPC 경로에서).

### 1.2 JSONL 자동 변환 (`jsonl.rs`)

차용: LLaMA-Factory dataset preprocessing, Unsloth chat template doc.

- `ChatMessage { role, content }` + `ChatExample { messages: Vec<ChatMessage> }`.
- `parse_line(&str) -> ChatExample` — 한 줄 JSON 자동 감지:
  1. `messages[]` 키가 있으면 → OpenAI messages 그대로 통과 (role 정규화: `human`/`gpt` → `user`/`assistant`).
  2. `conversations[]` 키가 있으면 → ShareGPT (from `human` → `user`, from `gpt`/`bot` → `assistant`).
  3. `instruction` + (옵션 `input`) + `output` 키가 있으면 → Alpaca (`instruction\n\ninput`을 user, `output`을 assistant).
  4. `질문` + `답변` 키가 있으면 → 한국어 Q&A.
  5. 그 외는 `UnsupportedDataFormat` 에러.
- `parse_jsonl(&str) -> Vec<ChatExample>` — 빈 line skip, 형식 오류 line은 `tracing::warn!` + skip
  (전체 파일 실패 회피 정책 — bench-harness 부분 보고서와 같은 결.).
- `to_jsonl_line(&ChatExample)` / `write_jsonl(&[ChatExample])` — 정규화된 JSONL 출력.

### 1.3 Modelfile generator (`modelfile.rs`)

차용: <https://github.com/ollama/ollama/blob/main/docs/modelfile.md>.

- `ModelfileSpec`: `gguf_path`, `temperature`, `num_ctx`, `system_prompt_ko`, `stop_sequences[]`, `template?`.
- `escape_system_prompt(&str) -> String` — `\` → `\\`, `"` → `\"`, raw 줄바꿈 유지 (triple-quoted block).
- `render(&ModelfileSpec) -> String` — 형식:
  ```
  FROM ./path/to/model.gguf
  PARAMETER temperature 0.7
  PARAMETER num_ctx 4096
  PARAMETER stop "..."
  SYSTEM """한국어 시스템 프롬프트..."""
  TEMPLATE """{{ .Prompt }}"""
  ```

### 1.4 양자화 CLI wrapper (`quantize.rs`)

차용: llama.cpp `llama-quantize` (이전 `quantize` 바이너리).

- `QuantizeJob { input_gguf, output_gguf, quant_type }` — `quant_type` 예: `Q4_K_M` / `Q5_K_M` / `Q8_0`.
- `QuantizeProgress { percent, stage, message }` — `stage` = `"loading"` / `"quantizing"` / `"writing"`.
- `Quantizer` trait — `async fn run(...)`, `CancellationToken` 협력. 결과는 `Vec<QuantizeProgress>`.
- `MockQuantizer` — 0/25/50/75/100% 5-step emit, 각 step에서 cancel 점검.
  실 CLI는 v1.x `LlamaQuantizer` (subprocess + stdout 라인 파싱)에서 진입.

### 1.5 LoRA CLI wrapper (`lora.rs`)

차용: LLaMA-Factory CLI (`llamafactory-cli train`), KoAlpaca / Alpaca-ko prompt template.

- `LoRAJob { base_model, dataset_jsonl, output_adapter, epochs, lr, korean_preset }`.
- `LoRATrainer` trait — `async fn run(...)`, `CancellationToken` 협력. 진행률은 `QuantizeProgress`와 동일 shape 재활용
  (UI도 동일 progress pill 재사용).
- `MockLoRATrainer` — 동일하게 mock progress emit. `korean_preset = true` 시 message에 `alpaca-ko` 명시.

### 1.6 Korean QA evals (`eval.rs`)

차용: AI Toolkit eval format, Foundry Toolkit substring match.

- `EvalCase { id, user, expected_substring?, forbidden_substrings[], category }`. category = `factuality` / `instruction-following` / `tone-korean`.
- `EvalResult { case_id, passed, failure_reason?, model_response }`.
- `EvalReport { model_id, passed_count, total, by_category, cases[] }`.
- `evaluate_response(&EvalCase, &str) -> EvalResult` — substring 매칭(case-insensitive ASCII), forbidden 검출.
  expected가 None + forbidden 미발견 → pass.
- `aggregate(&str, Vec<EvalResult>) -> EvalReport` — `by_category`는 `(passed, total)` 집계.
- `baseline_korean_eval_cases() -> Vec<EvalCase>` — 10 case (factuality 3 + instruction-following 4 + tone-korean 3).

### 1.7 5단계 cache & 재실행

- 각 단계 출력은 `workspace/workbench/{run_id}/{step}/output.json` (이번 scaffold에서는 path만 정의).
- 동일 input hash (config 해시)로 재실행 시 cache hit → step skip.
- 실 hashing & path I/O는 5'.b에서 portable-workspace crate와 통합.

### 1.8 한국어 에러 (`error.rs`)

- `WorkbenchError`:
  - `Io(#[from] std::io::Error)`
  - `Json(#[from] serde_json::Error)`
  - `UnsupportedDataFormat(String)` — "입력 데이터 형식을 알 수 없어요: ..."
  - `ToolMissing { tool: String }` — "CLI 도구가 설치되어 있지 않아요: ..."
  - `Cancelled` — "측정 단계가 취소됐어요"
  - `EvalFailed { message: String }` — "Korean QA evals 실패: ..."
  - `Internal { message: String }` — "내부 오류: ..."
- bench-harness `BenchError`와 동일 결.

---

## §2. 기각안 + 이유 (의무, 5+ 건)

### 2.1 Hugging Face Transformers Python 임베딩

- **거부 사유**: Python runtime 의존이 데스크톱 zero-knowledge 정책에 직접 충돌. 사용자 PC에 Python+venv를
  강제하면 "포터블 실행" 약속을 깬다. CLI subprocess wrapper는 Rust ↔ exe boundary만 두므로 Python 없이도
  동작 가능 (LLaMA-Factory는 wheel exe로 묶을 수 있음).

### 2.2 Unsloth GUI / mlx-lm GUI 통합

- **거부 사유**: GUI는 LMmaster의 "한 화면 5단계 워크벤치" 정체성과 중복. CLI wrapper로 충분하며,
  GUI 통합은 v1.x에서 옵션 (사용자 advanced 사용처용). 우리 Workbench가 1차 UX를 책임진다.

### 2.3 LoRA 자체 구현 (Rust 네이티브)

- **거부 사유**: LLaMA-Factory / Unsloth가 이미 검증된 학습 stack을 제공하고, fine-tuning 알고리즘 자체는
  reinventing wheel. 우리는 "사용자가 한국어 데이터로 5분 안에 LoRA를 굽는 한국어 UX"가 가치이며,
  실제 학습 엔진은 검증된 CLI에 위임한다. 결과 adapter를 GGUF + Modelfile로 자동 정제하는 부분이
  LMmaster의 차별화.

### 2.4 GGUF 양자화 직접 라이브러리 구현

- **거부 사유**: GGUF spec은 llama.cpp가 reference. spec 변경이 잦아 직접 lib 유지보수 시 지속 비용 큼.
  `llama-quantize` exe가 표준이며 CLI wrapper로 충분. 우리는 "한국어 UI + 자동 quant_type 추천 +
  진행률 시각화"가 차별화.

### 2.5 evals를 LLM-as-judge로 구현

- **거부 사유**: deterministic answer 매칭이 신뢰성 / 재현성 / 외부 통신 0 정책에서 우수. judge LLM은
  매번 다른 점수를 줄 수 있고, 외부 통신 없이 로컬 LLM judge를 쓰면 자기 자신을 평가하는 회로 발생.
  judge LLM은 v1.x advanced 옵션으로 오픈하되 default는 deterministic.

### 2.6 5단계를 한 단계 wizard form으로 합치기

- **거부 사유**: 단계별 출력이 다른 단계의 입력이라 wizard form은 cancel/retry/skip 시나리오 처리가
  복잡. state machine으로 명시화하면 cache 재실행 + 단계별 진행률 + 단계 단위 cancel UX가 자연.
  (예: validate 실패 시 LoRA만 재실행).

---

## §3. 미정 / 후순위 이월

- **멀티 GPU LoRA** — v1.x. 단일 GPU에서 KoAlpaca 7B 양자화 + LoRA가 v1 SLA.
- **DPO / RLHF** — v2. SFT(supervised fine-tuning)부터 먼저.
- **데이터 augmentation** — 한국어 instruction 다듬기 / 합성 — v1.x.
- **evals 결과 시각화** — Diagnostics 페이지 통합 후보 (Phase 4.f mock 영역에 evals slot 신설).
- **judge LLM advanced 옵션** — v1.x 토글. 외부 통신 0 유지 위해 로컬 모델 only.

---

## §4. 테스트 invariant

- **JSONL 변환**: 4 포맷 round-trip + mixed-format 한 파일 + 빈 line skip + 잘못된 line warn-skip.
- **Modelfile generator**: 한국어 system prompt 큰따옴표 / 줄바꿈 / 백슬래시 escape 정확. stop sequences
  여러 개 라인. template optional.
- **5단계 state machine**: `new` → Pending + Data, `advance_to`로 completed_steps 누적, cancel/failed
  terminal, `next_step` Data → Quantize → Lora → Validate → Register → None 정확 순서.
- **양자화 mock**: 5-step progress emit, cancel 시 즉시 Cancelled.
- **LoRA mock**: progress emit, korean_preset 메시지, cancel.
- **evals**: expected_substring 매칭 / forbidden 검출 / 빈 fixture / by_category 정확 / baseline 10 case
  3 카테고리 커버.

---

## §5. 다음 페이즈 인계

1. **Tauri IPC**: `start_workbench_run` (config) → run_id, `cancel_workbench_run(run_id)`,
   `list_workbench_runs() -> WorkbenchRun[]`. emit `lmmaster:workbench-progress` 이벤트.
2. **React Workbench 화면**: Phase 4.e placeholder의 5-step preview를 active로 승격. 각 단계 카드는
   `running` / `completed` / `failed` / `cancelled` 4 상태 시각.
3. **bench-harness 통합**: `Register` 단계 직후 새 모델로 30초 벤치를 자동 트리거 (선택 옵션).
4. **portable-workspace 통합**: `workspace/workbench/{run_id}/` cache 디렉토리, repair 시 invalidate.
5. **Korean preset 통합 (Phase 4.h)**: LoRA 단계에서 KoAlpaca / 한국어 명령형 데이터 preset을 pre-fill.

---

## §6. 참고

- LLaMA-Factory: <https://github.com/hiyouga/LLaMA-Factory>
- Unsloth: <https://github.com/unslothai/unsloth>
- mlx-lm: <https://github.com/ml-explore/mlx-examples>
- Ollama Modelfile spec: <https://github.com/ollama/ollama/blob/main/docs/modelfile.md>
- llama.cpp `llama-quantize`: <https://github.com/ggerganov/llama.cpp/tree/master/examples/quantize>
- AI Toolkit eval (VS Code): <https://learn.microsoft.com/windows/ai/toolkit/toolkit-evaluation>
- Foundry Toolkit eval: <https://github.com/microsoft/foundry-local>
- KoAlpaca: <https://github.com/Beomi/KoAlpaca>
- Alpaca-ko: <https://huggingface.co/datasets/beomi/KoAlpaca-v1.1a>

---

**검증 결과** (2026-04-27, 본 sub-phase 작업 시점):

- `cargo test -p workbench-core` — pass.
- `cargo clippy -p workbench-core --all-targets -- -D warnings` — 0 warning.
- `cargo fmt --all -- --check` — 0 diff.

**다음 sub-phase 진입 조건**:

- [x] `crates/workbench-core` scaffold (state machine + 4 wrapper trait + evals + JSONL + Modelfile).
- [x] integration test 5단계 mock 흐름.
- [x] 결정 노트 + 기각안 6 건.
- [ ] Tauri IPC `start_workbench_run` (5'.b 책임).
- [ ] React 5-step UI 승격 (5'.c 책임).
