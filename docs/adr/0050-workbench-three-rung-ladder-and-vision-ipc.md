# ADR-0050 — Workbench 3단 사다리 + 비전 IPC

* **상태**: 부분 채택 (2026-04-30 → 2026-05-01) — Phase 12'.a + 12'.b + 13'.h.1 + 13'.h.2.a 머지 (3단 사다리 + Ollama vision + LM Studio vision OpenAI compat 어댑터). llama.cpp **server 모드 직접 spawn**(13'.h.2.b)과 **자동 sidecar**(13'.h.2.c)는 v2 마이그레이션 — Ollama + LM Studio 두 어댑터로 카탈로그 vision 모델 작동 가능.
* **컨텍스트**: 사용자 비전 = "모델 셀렉트 → 도메인 특화 물꼬 지원". 현재 Workbench는 LoRA 파인튜닝 5단계(data→quantize→lora→validate→register)에 특화 — *즉시 사용 가능한 프롬프트 템플릿*과 *RAG 시드* layer가 비어있음. 비전 모달리티(이미지 입력)도 미구현.
* **결정 노트**: `docs/research/phase-11p-12p-v1x-domain-axis-decision.md` §2.5-6

## 결정

1. **3단 사다리** — Stage 0 프롬프트 템플릿(즉시) → Stage 1 RAG 시드(중기) → Stage 2~6 LoRA 파인튜닝(현재 5단계 보존, 깊은 진입).
2. **컨텍스트 바** — Workbench 상단에 `WorkbenchContextBar` 추가, URL hash `#/workbench?model=X&intent=Y`로 의도+모델 전달.
3. **Stage 0 (PromptTemplateStep)** — `intent`에 매칭되는 `use_case_examples` 노출. "내 패턴 저장"은 로컬 파일(`~/.lmmaster/prompts/<intent>/<name>.json`).
4. **Stage 1 (RagSeedStep)** — knowledge-stack crate 재활용, EmbeddingModelPanel 통합. `ko-rag` 의도 → KURE-v1 자동 권장.
5. **비전 IPC** — `chat_with_image(model_id, prompt, image_base64) -> stream` 신설. llama.cpp/Ollama vision 어댑터 활용. Chat 페이지 paperclip 버튼 + Stage 0 이미지 업로드 통합.
6. **하위 호환** — URL hash 없으면 기존 5단계로 진입 (default). 컨텍스트 바는 hash 있을 때만 노출.

## 근거

- **단일 페이지 사다리 vs 3페이지 분리** — 3페이지는 컨텍스트(의도+모델) 전달 비용 큼 + 사용자 멘탈 모델 분산. 단일 페이지가 더 자연스러움.
- **base64 vs path 전달** — base64는 IPC 페이로드 크지만 file scope 확장 불필요. 클라이언트 max 4096px 리사이즈 + JPEG 90%로 완화. path 옵션은 v2 검토.
- **Stage 0 = 즉시 사용** — 사용자가 모델 셀렉트 직후 *5단계 LoRA*를 강제로 봐야 했던 ROI 낮은 흐름 해소.

## 거부된 대안

- **신규 페이지 3개 분리 (PromptWorkbench / RagWorkbench / FineTuneWorkbench)** — 컨텍스트 분산 + 라우팅 비용.
- **base64 대신 임시 파일 path 전달** — Tauri scope 확장 + 보안 검토 비용.
- **자동 모달리티 감지** — 사용자 의도 명시가 더 정확.
- **영상 분석 v1.x 포함** — VRAM 16-24GB+ 요구로 일반 PC 사양 초과, 입력 IPC 복잡도 큼. v2+.
- **자세 분석 (pose estimation) 통합** — ONNX/MediaPipe 런타임 stack 변경. v2 별개 thesis.

## 결과 / 영향

- Workbench `STEP_KEYS`가 7종으로 확장 (`prompt`, `rag` + 기존 5종). 기존 reducer/state machine 그대로 + 신규 stage 라우팅만 추가.
- Catalog "이 모델로 시작 →" 버튼 → Workbench로 의도+모델 전달.
- Chat 페이지에 `vision_support: true` + `vision-image` 의도 시 이미지 첨부 활성.
- 영상 분석은 v2+ deferred (DEFERRED.md에 별도 항목).

## References

- 결정 노트: `docs/research/phase-11p-12p-v1x-domain-axis-decision.md` §2.5-6 + §7.3 (UI 와이어)
- 관련 ADR: ADR-0042 (real embedder cascade), ADR-0043 (real workbench external binary), ADR-0048 (intent 축), ADR-0049 (HF 하이브리드)
- 코드:
  - `apps/desktop/src/pages/Workbench.tsx` (Stage 0/1 삽입 위치 — STEP_KEYS 앞에)
  - `apps/desktop/src/components/workbench/WorkbenchContextBar.tsx` (신규)
  - `apps/desktop/src/components/workbench/PromptTemplateStep.tsx` (신규, Stage 0)
  - `apps/desktop/src/components/workbench/RagSeedStep.tsx` (신규, Stage 1)
  - `apps/desktop/src-tauri/src/chat.rs` (chat_with_image IPC 추가)
  - `crates/runner-llama-cpp/`, `crates/adapter-ollama/` (vision 어댑터 활용)
  - `crates/knowledge-stack/` (RAG 연결)
