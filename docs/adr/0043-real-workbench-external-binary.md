# ADR-0043: Real Workbench — llama-quantize binary + LLaMA-Factory CLI

- **Status**: Accepted
- **Date**: 2026-04-29
- **Phase**: 9'.b
- **Related**: ADR-0023 (Workbench v1 boundary), ADR-0018 (Workbench v1 core), ADR-0026 (Auto-Updater)

## Context

Phase 5'.a~5'.e에서 Workbench 5단계(Data → Quantize → LoRA → Validate → Register)는 `MockQuantizer`/`MockLoRATrainer`로 구동. 실제 양자화/파인튜닝은 미구현. 사용자가 Workbench로 실 모델을 양자화하거나 LoRA 학습하려면 외부 binary + Python venv 필요.

핵심 제약:
- 외부 통신 0 원칙 (ADR-0013) — 모델 학습 데이터/결과는 PC 로컬에서만.
- 사용자 PC 환경 다양성 — Python 미설치 / GPU 미보유 / 디스크 여유 부족 가능.
- 장기 작업 (양자화 5~30분, LoRA 1~10시간) — cancel 보장 + 임시 파일 정리 의무.

## Decision

**llama-quantize binary on-demand spawn + LLaMA-Factory CLI venv 자동 부트스트랩**.

### Quantize stage — `LlamaQuantizer`

- `tokio::process::Command::new("llama-quantize.exe").kill_on_drop(true)`.
- `LMMASTER_LLAMA_QUANTIZE_PATH` env override 지원 (CI / 테스트).
- PATH auto-detect (Ollama 번들 binary 또는 사용자 직접 다운로드).
- 미발견 시 graceful 한국어 안내 ("llama-quantize를 찾지 못했어요. ...").
- stdout/stderr 라인별 progress emit (5단계 매핑).
- 30분 timeout. cancel cooperative.
- stderr → 한국어 매핑 (4 시나리오: missing / disk full / format / 기타).

### LoRA stage — `LlamaFactoryTrainer`

- Python venv 자동 부트스트랩 (`<app_data_dir>/lora/venv/`).
- `uv` 우선, 없으면 `pip`. python 3.10+ 필수.
- 첫 부트스트랩: ~5GB (torch + llamafactory). 사용자 명시 동의 후만.
- 재실행 시 venv 재사용 (skip).
- `python -m llamafactory train --config <yaml>` subprocess.
- stdout 진행률 line parsing (epoch / loss / step).
- 4시간 timeout (LoRA 장기 작업).
- CancellationToken — child kill_on_drop + 임시 파일 정리.

### 사용자 동의 게이트

`use_real_quantizer` / `use_real_trainer` 토글 (default false). 사용자가 명시 활성 시 사전 동의 dialog:
- Quantize: "약 5~30분 소요. 디스크 ~500MB 필요. 진행할까요?"
- LoRA: "venv 5~10GB 다운로드 + 1~10시간 학습. 진행할까요?"

미선택 시 Mock 사용 (기존 5단계 흐름 unchanged).

## Consequences

### Positive
- 사용자가 실제 모델 양자화/학습 가능 (LMmaster의 Workbench 가치 실현).
- 외부 통신 0 원칙 유지 (모든 처리 로컬).
- Mock fallback으로 기존 동작 호환.
- `LMMASTER_*_PATH` env override로 CI/테스트 격리.

### Negative
- llama-quantize binary 사용자가 별도 받아야 (Ollama 번들 활용 또는 GitHub Releases 다운로드).
- LLaMA-Factory venv 5~10GB 디스크 부담.
- Python 미설치 / 잘못된 버전 시 친절한 진단 한국어 안내 의무 (panic X).
- GPU 없으면 LoRA가 매우 느림 (CPU fallback).

## Alternatives rejected

### 1. Python sidecar 상시 실행
- ❌ 콜드 스타트 + 메모리 비용. 사용자가 Workbench 안 쓸 때도 RAM 점유.
- ✅ 채택안: on-demand spawn (실제 사용 시에만).

### 2. Rust-only LoRA (`candle` crate)
- ❌ 한국어 데이터 패턴 + LLaMA-Factory ecosystem 미성숙.
- ❌ 사용자 정의 학습 루프 작성 부담.
- ✅ 채택안: 검증된 LLaMA-Factory CLI.

### 3. Bundle binary 동봉
- ❌ 수GB 추가 — installer 부담 + 라이선스 호환성 검토.
- ❌ Ollama가 같은 binary 번들하므로 사용자가 이미 보유 가능성.
- ✅ 채택안: PATH detect + env override + 사용자 안내.

### 4. Docker 기반
- ❌ 사용자가 Docker 설치 + WSL2 / Hyper-V 활성 부담.
- ❌ GPU 통과 (CUDA-Docker) 추가 복잡성.
- ✅ 채택안: 직접 subprocess.

## Test invariants

- llama-quantize binary 미존재 시 graceful 에러 (panic X).
- Mock 5단계 흐름 regression 0건.
- Cancel mid-quantize → child kill + 임시 파일 정리.
- LLaMA-Factory venv 부트스트랩 cancel 가능.
- Python 미설치 → "python 3.10+ 필요해요" 한국어 안내.
- LMMASTER_LLAMA_QUANTIZE_PATH env override → 테스트 binary 사용.

## References

- ADR-0023 (Workbench v1 boundary).
- ADR-0018 (Workbench v1 core — LLaMA-Factory 채택).
- Phase 5'.e (`bench-harness/src/workbench_responder.rs`) — kill_on_drop / cancel cooperative / 60s timeout / stderr 한국어 매핑 패턴 재활용.
- llama.cpp llama-quantize: https://github.com/ggerganov/llama.cpp/blob/master/examples/quantize/README.md
- LLaMA-Factory: https://github.com/hiyouga/LLaMA-Factory
- uv (Python venv manager): https://github.com/astral-sh/uv
