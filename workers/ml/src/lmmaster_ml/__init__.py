"""LMmaster ML Workbench — Python sidecar (placeholder, v1 비활성).

정책 (ADR-0012):
- Rust supervisor가 spawn / monitor.
- 통신: stdio JSON-RPC.
- 실제 작업(SFT/LoRA/quantize/export/packaging)은 v1.x ~ v2 단계적 활성.
"""
__version__ = "0.0.1"
