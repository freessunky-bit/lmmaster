# ADR-0018: Workbench는 v1 핵심 산출물 — llama-quantize + LLaMA-Factory CLI

- Status: Accepted
- Date: 2026-04-26
- Modifies: ADR-0012 (Workbench는 v1 placeholder)

## Context
Pivot에서 워크벤치(SLM 양자화 + 도메인 파인튜닝)가 6 pillar 중 하나로 격상되었다. 시장에는 한국어 + 로컬 + cross-platform 데스크톱 클릭-투-파인튜닝 앱이 빈자리(보강 리서치 §4 결론). LLaMA-Factory는 이미 한국어 locale 보유, Apache-2.0, 100+ 모델 커버. llama.cpp의 `llama-quantize` 바이너리는 Python 의존성 없이 Rust supervisor가 spawn 가능.

## Decision

### 1. v1 통합 OSS
- **양자화: llama-quantize** (llama.cpp 산하 단일 바이너리).
  - convert: `convert_hf_to_gguf.py` (Python 1회 실행 / 사용자 첫 양자화 시 venv 자동 부트스트랩).
  - quantize: `llama-quantize <input.gguf> <output.gguf> Q4_K_M` 등.
  - Rust crate `crates/quantizer`가 spawn + 진행률 파싱.
- **파인튜닝: LLaMA-Factory CLI** (Apache-2.0).
  - Python sidecar (`workers/ml/`)에서 `llamafactory-cli train config.yaml` 실행.
  - LLaMA-Factory의 ko 로케일 활용.
  - 옵션 가속기: **Unsloth** (Win/Linux + NVIDIA, `--use_unsloth`).
- **macOS 분기: MLX-LM** (`mlx_lm.lora`, `mlx_lm.convert -q`).

### 2. v1 MVP 플로우 (5 화면)
1. **베이스 모델 선택** — 큐레이션된 4~6개(예: Llama-3.2-3B, Qwen2.5-3B/7B, Gemma-2-2B). HF에서 자동 다운로드.
2. **데이터 드롭** — JSONL/CSV/마크다운 폴더 드래그 → chat JSONL 자동 변환 + 토큰 수·예상 시간·VRAM 추정.
3. **프리셋** — 빠름(QLoRA 3B 1ep) / 균형 / 품질. 고급 옵션은 접힘 (`Advanced` 토글).
4. **학습** — live loss 차트(mono+tabular), VRAM 게이지, N steps마다 sample completion preview, 1-click 취소.
5. **내보내기** — LoRA merge → GGUF Q4_K_M 양자화 → **Ollama/LM Studio에 1-click 등록**.

### 3. 데이터 포맷 정책
- 표준은 OpenAI/HF chat JSONL: `{"messages":[{"role":"user","content":"..."},{"role":"assistant","content":"..."}]}`.
- 자동 변환: CSV(`prompt,response`) → JSONL. 마크다운/텍스트 폴더 → chunk + self-instruct(LLM-assisted, opt-in).

### 4. 기대값 (RTX 4070 Super 12GB QLoRA)
- 3B: batch 2, seq 2048, 6~8GB, **15~30분/epoch (5k 샘플)**.
- 7B: batch 1+grad-accum, seq 2048, 10~11GB, **1.5~3시간**.
- 13B: 11.5GB tight, Unsloth 필수.
UI는 사용자 선택 시 사양·예상 시간을 항상 표시.

### 5. ADR-0012 변경분
- "v1 placeholder, 인터페이스만" 항목 폐기.
- Python sidecar 구조는 유지(LLaMA-Factory가 Python). 부트스트랩은 첫 워크벤치 진입 시 자동.
- 양자화 경로는 Python 의존성 없음 → 더 빠른 first-use.

## Consequences
- **시장 차별점 확보**: 한국어 + 로컬 + cross-platform 클릭-투-파인튜닝 데스크톱 앱이 v1에 존재.
- **첫 양자화 시 마찰**: Python venv 자동 부트스트랩 필요(사용자에게 한국어로 진행 안내).
- **Mac 분기 비용**: MLX-LM 별도 driver. 다행히 LLaMA-Factory와 5-화면 플로우는 동일 추상화 가능.
- **VRAM 부족 사용자 대응**: 프리셋이 자동으로 lighter quant + 작은 base model 추천.
- **데이터 프라이버시**: 모든 학습은 로컬에서 진행 — 마케팅 핵심 카피.

## Alternatives considered
- **Axolotl** (YAML CLI 우선): 고급 사용자에 좋으나 GUI 부재 → v2.
- **HF AutoTrain 로컬**: Spaces UI 의존, 데스크톱 친화도 ↓.
- **자체 Rust 학습 스택(burn/candle)**: 모델 커버리지/생태계 부족. 거부.
- **Kiln 내장**: MIT지만 외부 inference API 호출 가정 → 로컬 우선과 충돌.

## References
- `docs/research/pivot-reinforcement.md` §4
- hiyouga/LLaMA-Factory (Apache-2.0, ko locale)
- unslothai/unsloth (Apache-2.0)
- ml-explore/mlx-lm (MIT)
- ggml-org/llama.cpp (`llama-quantize`, `convert_hf_to_gguf.py`)
- ADR-0012 (Modified)
- ADR-0016 (Wrap-not-replace)
