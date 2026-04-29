# ADR-0012: ML Workbench는 Python sidecar, v1 placeholder

- Status: **Modified by ADR-0018** (2026-04-26 — 워크벤치는 v1 핵심으로 격상)
- Date: 2026-04-26
- Note: 본 ADR의 "Python sidecar로 ML 작업 분리" 결정은 유지. 하지만
  "v1은 placeholder" 항목은 ADR-0018에 의해 폐기됨. v1에 LLaMA-Factory CLI 통합 + llama-quantize 양자화 경로가 핵심으로 들어간다.

## Context
사용자 요구사항에 파인튜닝/LoRA/PEFT/양자화/GGUF·ONNX export/온디바이스 패키징 등이 미래 확장으로 포함된다. 이 작업들의 사실상 표준 도구는 Python 생태계(transformers, peft, trl, axolotl, llama-factory, unsloth, bitsandbytes 등). Rust로 재구현하는 것은 비현실적.

## Decision
ML Workbench는 **Python sidecar** 프로세스로 실행한다(`workers/ml`). Rust core가 supervisor로서 spawn/monitor한다.

- 통신: stdio JSON-RPC 또는 local Unix socket / Named Pipe.
- 작업 큐는 Rust 측에서 관리, 실행만 Python에 위임.
- v1: **UI/메뉴/인터페이스 자리만** 만든다(disabled, "곧 제공" placeholder).
- 후속 v1.x ~ v2: 단계적으로 SFT/LoRA → 양자화 → GGUF export → 온디바이스 패키징 순.

Python 환경:
- 별도 가상환경(`workspace/runtimes/ml-worker-py-<ver>/`) 또는 사용자 시스템 Python attach 옵션.
- Windows portable에서는 embedded Python 옵션도 검토.

## Consequences
- 데스크톱 본체 바이너리는 Python 의존성 0. ML 활성화 시점에만 Python 환경 설치.
- ML 라이브러리 업데이트가 본체와 디커플링.
- v1 산출물 부담이 크게 줄어든다.

## Alternatives considered
- **Rust ML 스택(burn, candle)**: 가능하지만 transformers/peft 생태계 호환성 부족. 거부.
- **본체에 Python 임베드 강제**: 사용자 진입 마찰. 거부 — 옵션화.

## References
- 미래 ADR로 SFT/quantization 도입 시점 별도 작성 예정
