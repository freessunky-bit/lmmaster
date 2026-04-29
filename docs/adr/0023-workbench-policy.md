# ADR-0023: Workbench v1 경계 정책 (LoRA / Modelfile / 양자화)

- Status: Accepted
- Date: 2026-04-28
- Related: ADR-0018 (Workbench v1 core 의도), ADR-0014 (model registry), ADR-0017 (manifest installer), ADR-0012 (Workbench placeholder, modified by ADR-0018), ADR-0024 (knowledge-stack RAG), ADR-0025 (Pipelines), ADR-0026 (Auto-updater)
- 결정 노트: `docs/research/phase-5p-workbench-decision.md`

## Context

Phase 5' 진입 — Workbench v1을 production-grade scaffold로 구체화한다. ADR-0018이 OSS 선택(llama-quantize / LLaMA-Factory / MLX-LM)과 5-화면 UX를 정했지만, 다음 7 영역이 LoC 수준으로 결정 필요:

1. **crate 책임 경계** — `workbench-core`는 어디까지 담당? Tauri IPC와 Python sidecar는?
2. **양자화 구현 방식** — Rust 자체 양자화 vs `llama-quantize` CLI subprocess.
3. **LoRA 학습 stack** — LLaMA-Factory만 vs Unsloth / MLX-LM 동시 지원.
4. **Modelfile generator** — 단일 함수 vs multi-stage builder (ADAPTER / MESSAGE 블록 포함).
5. **Korean evals 채점 방식** — substring matching vs LLM-as-judge.
6. **JSONL 자동 변환 포맷 범위** — 어떤 4 포맷? 우선순위?
7. **state machine + 재실행 cache** — wizard form vs explicit state machine, cache 키 설계.

기존 `crates/bench-harness` 정책(30s budget + cooperative cancel + partial report)을 패턴으로 준용.

## Decision

### 1. workbench-core crate에서 trait + Mock impl만

`crates/workbench-core`는 **순수 도메인 로직**만 담는다:
- `WorkbenchStep` / `RunStatus` / `WorkbenchRun` state machine.
- `Quantizer` / `LoRATrainer` trait + `MockQuantizer` / `MockLoRATrainer` v1 mock.
- `ChatMessage` / `ChatExample` JSONL 정규화.
- `ModelfileSpec` / `render` Modelfile generator.
- `EvalCase` / `EvalResult` / `EvalReport` evals.

실 CLI subprocess wrapper(`LlamaQuantizer`, `LlamaFactoryTrainer`)는 sub-phase 5'.b (Tauri IPC) / 5'.c (UI) 진입 시 별도 작업으로 추가. mock impl로 IPC 구조 + UI flow가 먼저 검증된 후 실 동작 연결.

### 2. 양자화는 `llama-quantize` CLI subprocess wrapper. Rust 자체 quant 거부

GGUF spec은 llama.cpp가 reference이며 spec 변경이 잦다. 직접 라이브러리 유지보수는 reinventing wheel + 지속 비용 큼. `llama-quantize` exe가 표준이므로 CLI wrapper로 충분. LMmaster의 차별화는 "한국어 UI + 자동 quant_type 추천 + 진행률 시각화"에 둔다.

### 3. LoRA는 LLaMA-Factory CLI subprocess. Unsloth / MLX-LM은 v1.x optional

LLaMA-Factory가 한국어 locale 제공 + Apache-2.0 + 100+ 모델 커버. v1은 LLaMA-Factory만 통합. Unsloth 가속 옵션 + macOS MLX-LM 분기는 v1.1 ADR로 이월.

### 4. Modelfile은 단일 함수 generator. multi-stage ADAPTER/MESSAGE는 v1.x

v1 Modelfile은 `FROM + PARAMETER + SYSTEM + 옵션 TEMPLATE` 5 directive만 다룬다. ADAPTER (LoRA stack) / MESSAGE (system role override) / LICENSE / multi-stage `FROM ./model.gguf AS base` 같은 고급 directive는 v1.x에 검토. 단순성 우선.

### 5. Korean evals는 deterministic substring matching. LLM-as-judge 거부

substring matching은 신뢰성 / 재현성 / 외부 통신 0 정책에서 우수. judge LLM은 매번 다른 점수를 줄 수 있고, 로컬 LLM judge는 자기 평가 회로(self-judging loop) 발생 위험. judge LLM 옵션은 v1.x advanced 토글로 오픈하되 default는 deterministic.

### 6. JSONL 4 포맷: Alpaca / ShareGPT / OpenAI messages / 한국어 Q&A

자동 감지 priority(line별 단일 형식):
1. **OpenAI messages** (`messages[]` 키) — 가장 명확. role 정규화: `human`/`gpt` → `user`/`assistant`.
2. **ShareGPT** (`conversations[]` 키, `from`/`value`) — `human` → `user`, `gpt`/`bot` → `assistant`.
3. **Alpaca** (`instruction` + `output` + 옵션 `input`) — `instruction\n\ninput`을 user, `output`을 assistant.
4. **한국어 Q&A** (`질문` + `답변`).

빈 line skip. 형식 오류 line은 `tracing::warn!` + skip (전체 파일 실패 회피 — bench-harness partial report 정책과 같은 결).

### 7. 5단계 state machine: Data → Quantize → LoRA → Validate → Register

```
WorkbenchStep::{Data, Quantize, Lora, Validate, Register}
RunStatus::{Pending, Running, Completed, Failed, Cancelled}
```

- `WorkbenchRun::new(config)` → Pending + Data step.
- `advance_to(step)` — 현재 step을 `completed_steps`에 push (중복 방지) + 다음 step + status Running.
- `next_step()` — 자동 다음 단계 계산. Register 다음은 None.
- `mark_completed/failed/cancelled` — terminal.
- 재실행: cache는 v1.b portable-workspace 통합 시 `workspace/workbench/{run_id}/{step_label}/`.

wizard form 거부 사유: 단계별 출력이 다음 단계 입력이라 cancel/retry/skip 시나리오가 wizard에서 복잡. state machine 명시화 → cache 재실행 + 단계별 진행률 + 단계 단위 cancel UX가 자연.

### 8. 재실행 cache: `workspace/workbench/{run_id}/{step}/`

각 단계 출력은 `workspace/workbench/{run_id}/{step_label}/output.json`. 동일 input hash로 재실행 시 cache hit → step skip. v1.b에서 portable-workspace crate와 통합. cache invalidate는 fingerprint repair yellow tier에서 발동(ADR-0022 §8 참조).

## Consequences

**긍정**:
- mock impl로 IPC + UI 먼저 통과 → 실 CLI subprocess는 검증된 인터페이스에 plug-in.
- substring matching evals → 외부 통신 0 + 재현성 + 한국어 bias 회피.
- 5단계 state machine → 단계별 cancel / retry / cache 재실행이 자연.
- JSONL 4 포맷 자동 변환 → 사용자가 데이터 포맷 신경 안 써도 됨.
- ADR-0018의 OSS 선택을 그대로 이어받되 crate 책임 경계 명확화.

**부정**:
- v1.b까지는 mock만 → 사용자 시연 시 실제 양자화/학습 불가능. UI 흐름 검증만.
- LLaMA-Factory만 v1 → 가속 옵션(Unsloth) / macOS(MLX-LM) 사용자는 v1.1 대기.
- substring matching evals → "맥락 정확성" 같은 정량화 어려운 항목 미지원. judge LLM 옵션은 v1.x 이월.
- multi-stage Modelfile 미지원 → Ollama advanced 사용자는 직접 작성 필요.

**감내한 트레이드오프**:
- 단순성 vs 완성도 — v1은 단순성. 5단계 + 4 포맷 + 단일 stack으로 좁힘.
- 재실행 cache의 v1.b 의존 — v1 scaffold에서는 path helper만, 실 I/O는 portable-workspace와 통합 후.
- mock의 5-step 고정 — 실 CLI 진행률 파싱 시 step 수가 다를 수 있음. progress shape는 동일하되 갯수는 implementation 기준.

## Alternatives considered (negative space — 결정 노트 §2 미러)

### a. Hugging Face Transformers Python 임베딩

거부. Python runtime 의존이 데스크톱 zero-knowledge 정책에 충돌. 사용자 PC에 Python+venv를 강제하면 "포터블 실행" 약속을 깨고, CLI subprocess wrapper는 Rust ↔ exe boundary만 두므로 Python 없이 동작 가능. LLaMA-Factory는 wheel exe로 묶을 수 있다.

### b. Unsloth GUI / MLX-LM GUI 통합

거부. GUI는 LMmaster의 "한 화면 5단계 워크벤치" 정체성과 중복. CLI wrapper로 충분하며, GUI 통합은 v1.x advanced 옵션. 우리 Workbench가 1차 UX를 책임진다.

### c. LoRA 자체 구현 (Rust 네이티브, burn/candle)

거부. LLaMA-Factory / Unsloth가 검증된 학습 stack 제공. fine-tuning 알고리즘 자체는 reinventing wheel. 가치는 "한국어 데이터로 5분 안에 LoRA를 굽는 한국어 UX"이며, 학습 엔진은 검증된 CLI에 위임. 결과 adapter를 GGUF + Modelfile로 자동 정제하는 부분이 차별화.

### d. evals를 LLM-as-judge로 구현

거부. deterministic answer 매칭이 신뢰성 / 재현성 / 외부 통신 0 정책에서 우수. judge LLM은 매번 다른 점수를 줄 수 있고, 로컬 judge는 self-judging loop. v1.x에서 advanced 옵션 (외부 통신 0 유지 위해 로컬 모델 only).

### e. Modelfile multi-stage ADAPTER / MESSAGE 블록 v1 지원

거부. v1은 단일 함수 generator. ADAPTER / MESSAGE / multi-stage FROM은 v1.x. 단순성 우선 — 사용자 99%는 FROM + PARAMETER + SYSTEM + TEMPLATE이면 충분.

### f. 양자화 GGUF 직접 라이브러리 구현

거부. GGUF spec은 llama.cpp가 reference. spec 변경이 잦아 직접 lib 유지보수 시 지속 비용 큼. `llama-quantize` exe가 표준이며 CLI wrapper로 충분.

### g. 5단계를 한 단계 wizard form으로 합치기

거부. 단계별 출력이 다른 단계의 입력이라 wizard form은 cancel/retry/skip 시나리오 처리가 복잡. state machine 명시화 → cache 재실행 + 단계별 진행률 + 단계 단위 cancel UX 자연.

## 검증 invariant

- **JSONL 변환**: 4 포맷 round-trip + mixed-format 한 파일 + 빈 line skip + 잘못된 line warn-skip + role 정규화 (human → user, gpt → assistant) + tool / unknown role 보존.
- **Modelfile generator**: 한국어 system prompt 큰따옴표 / 줄바꿈 / 백슬래시 escape 정확. stop sequences 여러 개 라인. template optional. quote-in-stop escape.
- **5단계 state machine**: `new` → Pending + Data, `advance_to`로 completed_steps 누적 (중복 방지), cancel/failed terminal, `next_step` Data → Quantize → Lora → Validate → Register → None 정확 순서, kebab-case serde.
- **양자화 mock**: 5-step progress emit, percent 0/25/50/75/100 단조 증가, cancel 시 즉시 Cancelled, stage 라벨 (loading/quantizing/writing).
- **LoRA mock**: 5-step progress emit, korean_preset 메시지 (alpaca-ko 키워드), cancel 협력.
- **evals**: expected_substring 매칭 (case-insensitive) / forbidden 검출 / 빈 fixture pass / by_category 정확 / baseline 10 case 3 카테고리 (factuality 4 + instruction-following 3 + tone-korean 3) / case_id 유일.
- **integration**: 5단계 mock 흐름 end-to-end + cancel 시나리오 2건 (quantize / lora) + serde round-trip.

## References

- 결정 노트: `docs/research/phase-5p-workbench-decision.md`
- ADR-0018 (Workbench v1 core 의도, OSS 선택)
- ADR-0014 (model registry — 등록 단계 통합 대상)
- ADR-0017 (manifest installer)
- ADR-0022 (gateway routing — fingerprint repair yellow tier에서 cache invalidate 트리거)
- LLaMA-Factory: <https://github.com/hiyouga/LLaMA-Factory>
- Unsloth: <https://github.com/unslothai/unsloth>
- mlx-lm: <https://github.com/ml-explore/mlx-examples>
- Ollama Modelfile spec: <https://github.com/ollama/ollama/blob/main/docs/modelfile.md>
- llama.cpp `llama-quantize`: <https://github.com/ggerganov/llama.cpp/tree/master/examples/quantize>
- AI Toolkit eval (VS Code): <https://learn.microsoft.com/windows/ai/toolkit/toolkit-evaluation>
- Foundry Toolkit eval: <https://github.com/microsoft/foundry-local>
- KoAlpaca: <https://github.com/Beomi/KoAlpaca>
- Alpaca-ko: <https://huggingface.co/datasets/beomi/KoAlpaca-v1.1a>
