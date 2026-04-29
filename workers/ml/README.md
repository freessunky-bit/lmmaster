# LMmaster ML Workbench Worker

Python sidecar (v1 placeholder).

ADR-0012에 따라:
- Rust core가 supervisor로 spawn / monitor.
- 통신: stdio JSON-RPC.
- v1에서는 비활성. UI/메뉴/인터페이스 자리만 둔다.
- v1.x ~ v2에 SFT/LoRA → quantization → GGUF export → 온디바이스 packaging 순으로 단계 활성.

## 활성 시 절차

1. 사용자가 워크벤치 활성화 옵션을 선택.
2. 데스크톱 앱이 가상환경(`workspace/runtimes/ml-worker-py-<ver>/`)을 만든다.
3. 사용자가 시작할 작업 종류에 따라 optional dependency를 설치 (`sft`, `quantize` 등).
4. supervisor가 `python -m lmmaster_ml.server`로 사이드카 기동.
