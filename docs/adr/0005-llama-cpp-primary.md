# ADR-0005: Primary portable runtime으로 llama.cpp 채택

- Status: **Superseded by ADR-0016** (2026-04-26)
- Date: 2026-04-26
- Note: Pivot 결과 v1 primary backend는 LM Studio + Ollama 어댑터로 변경됨.
  llama.cpp 자식 프로세스는 v1.x의 zero-config 옵션 + 워크벤치 양자화(llama-quantize)에서 활용.
  본 ADR은 historical 기록으로 유지.

## Context
사용자가 portable 데스크톱 형태를 원한다. 다양한 GPU(NVIDIA/AMD/Intel) + CPU-only + Apple Silicon를 동시에 커버해야 하고, 모델 포맷 표준(GGUF) 호환이 필요하다.

## Decision
Primary portable runtime으로 **llama.cpp**(`server` 바이너리)를 채택한다. 모델 포맷은 **GGUF**. CUDA / Vulkan / Metal / ROCm 빌드를 OS·GPU별 prebuilt로 다운로드.

추가 어댑터들:
- **KoboldCpp**: 캐릭터/RP 카테고리 사용자 친화 옵션
- **Ollama**: 외부 설치형 어댑터 (사용자가 이미 설치한 경우 attach)
- **LM Studio**: 외부 설치형 어댑터
- **vLLM**: 고사양 서빙 옵션 (Linux + CUDA 우선, Windows는 후순위)

## Consequences
- GGUF 단일 포맷 우선 → 모델 레지스트리 메타데이터가 단순해진다.
- 양자화 옵션(Q4_K_M, Q5_K_M, Q8_0 등)을 운영자가 모델 카드에서 노출 가능.
- llama.cpp의 OpenAI-호환 server endpoint를 우리 gateway가 추가 가공.
- Windows의 Vulkan 빌드, macOS의 Metal 빌드, Linux의 ROCm 빌드를 각각 검증해야 함.
- 캐릭터/RP는 KoboldCpp가 더 좋은 sampling 옵션을 가짐 → adapter 분리 유지.

## Alternatives considered
- **Ollama 단일 종속**: 거부 — 사용자가 명시적으로 금지. ollama는 어댑터 중 하나로만.
- **vLLM 우선**: Windows portable에 부적합. 고사양 서빙 옵션으로 분리.

## References
- https://github.com/ggerganov/llama.cpp
- ADR-0004 (Adapter pattern)
