# Phase — NVIDIA Nemotron 3 Nano 4B + Personas-Korea 통합 결정 노트

> **작성일**: 2026-05-06
> **트리거**: 사용자 요청 — "엔비디아에서 한국어 데이터를 학습한 모델 출시했다고 하니 디폴트 카탈로그에 추가하고 가상 한국인 100인 설문조사 시뮬레이션 셋업 가이드 보강"
> **스코프**: (A) 모델 1종 카탈로그 등록 + (B) 데이터셋 1종 가이드 통합. ADR 신설 없음 — 기존 카탈로그 schema + 가이드 시스템 활용.

---

## 1. 결정 요약

- **A1**: `nemotron-3-nano-4b`를 `manifests/snapshot/models/agents/`에 추가. Tier=`new`, language_strength=6, NVIDIA Open Model License (commercial true).
- **A2**: `nvidia/Nemotron-Personas-Korea` 데이터셋은 *모델 카탈로그가 아닌* `docs/guides/`로 통합 — 데이터셋은 LLM 런타임이 로드하는 자산이 아니라 **사용자 워크플로 입력**이기 때문.
- **A3**: 가상 한국인 100인 설문 시뮬레이션은 `docs/guides/personas-korea-survey-simulation.md` 신규 작성 — Nemotron + Personas-Korea + Workbench 3종 조합 step-by-step.
- **A4**: i18n `ko.json`/`en.json`에 가이드 진입점 키 1쌍 추가. 카탈로그 카드 자체 카피는 manifest의 `community_insights` + `warnings`로 노출되므로 i18n 추가 불필요.
- **A5**: 추천기 테스트는 *기존 invariant 유지 + 신규 회귀 가드 1종*. mid-host에서 nemotron이 `excluded`되지 않고 후보로 들어가는지만 확인 (best 자리 다툴 필요 없음 — EXAONE 7.8B 우선 정책 보존).

---

## 2. 채택안

### A1. 모델 카탈로그 entry — `nemotron-3-nano-4b`

| 필드 | 값 | 근거 |
|---|---|---|
| `tier` | `new` | 2026-04-12경 출시, 90일 이내 + HF 트래픽 검증 (수만 다운로드). 카탈로그 🔥 NEW 탭에 노출. |
| `maturity` | `stable` | NVIDIA 공식 release. 기술 분류 (모델 자체 안정성)와 노출 분류(`tier`)는 별개. |
| `language_strength` | **6** | EXAONE 7.8B(9), HCX-SEED 8B(10), Llama 3.1 8B(7), Aya 8B(8) 좌표계에서 *다국어 sprinkle 한국어*는 Llama 3.1 바로 아래. |
| `coding_strength` | 6 | general-purpose, 코딩 특화 X. Codestral/Qwen Coder 대비 명확히 낮춰 큐레이터가 분류. |
| `tool_support` | true | NVIDIA 모델 카드 명시. structured_output도 true. |
| `min_vram_mb` | 3072 | Q4_K_M 2.84GB + KV cache 약간. |
| `rec_vram_mb` | 6144 | 262K context 활용 시 KV cache가 메모리 점유. |
| `min_ram_mb` | 6144 | CPU-only 폴백 시 4B Q4 + 시스템 여유. |
| `intents` | `agent-tool-use`, `translation-multi`, `ko-rag` | 15개 언어 학습 + 262K context + tool support 매핑. `ko-conversation`은 점수만 부여, intents에는 미포함 (전용 모델 우선 보호). |
| `domain_scores` | translation-multi 76, agent-tool-use 72, ko-rag 58, ko-conversation 52 | EXAONE/HCX-SEED 영역(80+ 한국어)을 침범하지 않는 배치. |
| `commercial` | true | NVIDIA Open Model License 명시 (상업 사용 가능). UI 비상업 chip 미노출. |
| `warnings` | "한국어 주력 X" + "Mamba-2 양자화 변종 출력 깨짐 가능성" | 사용자가 EXAONE 기대로 골랐다 실망하는 케이스 차단. |

### A2. Personas-Korea 데이터셋 — *모델 카탈로그 X, 가이드 O*

데이터셋 vs 모델 분리 원칙:
- **모델** = 런타임이 로드 (GGUF, llama.cpp이 추론 실행).
- **데이터셋** = 사용자 입력 자산 (RAG 시드, fine-tune corpus, persona 시뮬레이션 입력).

`crates/model-registry`는 *추론 가능한 LLM* 카탈로그라 데이터셋 entry 추가는 schema 위반. RagSeedStep / Workbench가 데이터셋 import를 처리하는 정상 경로.

### A3. 가상 한국인 100인 설문 시뮬레이션 가이드

`docs/guides/personas-korea-survey-simulation.md` — 사용자 워크플로:
1. Personas-Korea 다운로드 (HF datasets 라이브러리 또는 직접 Parquet).
2. 100인 샘플링 (uuid hash로 결정성, 또는 인구통계 stratified).
3. 설문지 정의 (JSON 또는 한국어 자연어).
4. Nemotron 3 Nano 4B 또는 HCX-SEED 8B 선택 (4B는 빠름, 8B는 자연스러움).
5. Workbench에서 batch 프롬프트 — 페르소나 narrative를 system prompt로 주입, 설문 1문항씩 답변 생성.
6. 결과 집계 (CSV/JSON export).

### A4. i18n 키

```json
"guides.personas-korea": {
  "title": "가상 한국인 설문 시뮬레이션",
  "description": "엔비디아 Personas-Korea 데이터셋 + 로컬 LLM으로 가상 한국인 100인의 응답을 만들어 봐요."
}
```

ko/en 동시 갱신 (CLAUDE.md §4.1 i18n 정책).

### A5. 추천기 회귀 가드

`recommender_test.rs`에 1개 추가:
- `host_mid` (16GB RAM, RTX 3060 12GB, AgentGeneral) → `nemotron-3-nano-4b`가 `excluded`되지 않고 후보 셋에 등장. **best는 EXAONE 7.8B 또는 HCX-SEED 8B 중 하나로 유지** (한국어 우선 invariant 보존).

---

## 3. 기각안 + 이유 (negative space — 다음 세션 보호)

| 기각안 | 거부 이유 |
|---|---|
| **language_strength=8 (Aya 동급)** | Aya는 23개 언어 *공식 지원*에 한국어 포함, Nemotron은 학습 corpus에 한국어 *섞임*. 카드는 영어로만 명시. 8을 주면 EXAONE 9 / Aya 8 / Nemotron 8 → 사용자가 한국어 강함으로 오해. **6**으로 Llama 3.1(7) 아래에 고정해 한국어 전용 모델 보호. |
| **language_strength=4 (Phi 4 수준)** | 너무 낮음 — Personas-Korea narrative 같은 한국어 문서를 system prompt로 받아 처리하는 정도는 충분히 가능. 4는 한국어 거의 못 쓰는 모델용. |
| **`tier=verified`** | 90일 이내 + LMmaster 큐레이터 실사용 검증 미완. `new`로 두고 v1.x에 verified 승격 후보. |
| **Personas-Korea를 model entry로 등록 (데이터셋 카테고리)** | `ModelCategory` enum에 `dataset` 분기 신설 = schema bump = ADR 1건. 가이드만으로 100% 같은 사용자 가치 달성 가능. **ROI 낮음 — 거부.** |
| **새 intent `synthetic-persona` 추가** | INTENT_VOCABULARY에 12번째 추가 = `vocabulary_seed_size_v1x` 테스트 변경 + 기존 11종에 백필 부담. *한 모델에만 해당하는 intent*는 큐레이터 선택지로 부적합. 기존 `ko-rag`로 충분 (페르소나 narrative를 RAG 컨텍스트로 사용하는 패턴). |
| **OCR v2 카탈로그 추가** | LLM이 아닌 OCR 모델 → `ModelCategory`에 `ocr` 신설 필요. v1 스코프 외 — 추후 Phase로 이월. |
| **Nemotron-Personas-Korea를 RagSeedStep에 1-click 노출** | 데이터셋 100만 row + 1.7B 토큰 = 컴팩트 RAG 시드로는 과대. 100인 샘플링 단계가 필수라 1-click 부적합. 가이드 문서로 stepper 안내가 정확. |
| **자동 다운로드 (앱 내 progress bar)** | HF datasets 라이브러리 = Python 의존. Tauri-only 앱이 Python runtime 번들 = 페이로드 폭증. 가이드에서 사용자가 직접 다운받게 안내가 합리적 (또는 향후 별도 페이즈에서 Rust 직접 Parquet reader). |
| **v0.0.1 release tag와 묶기** | release tag는 *현재 master 상태 freeze*가 목적. 새 카탈로그 entry는 사용자 PC에 자동 push되지 않으므로 별도 minor patch에 들어가도 무방. release 차단 X. |

---

## 4. 미정 / 후순위 이월

- **sha256 백필** — manifest의 quantization_options.sha256은 placeholder zeros (시드 정책). 큐레이터가 실제 GGUF 다운로드 후 검증 시점에 채움. `phase-13pf-curation-plus4-decision.md` §sha256 백필 정책 따름.
- **HF 메타 자동 갱신** — `hf_meta` 필드는 v1.1 cache.rs에서 채울 예정. 본 entry도 그때 함께 갱신.
- **OCR v2 / 4B FP8/BF16 변종** — Nemotron OCR v2 별도 카테고리 필요 + FP8/BF16은 GGUF 아닌 변종이라 별도 어댑터 필요. v1.x 이후 검토.
- **Personas-Korea Rust 직접 reader** — 현재는 사용자가 HF datasets (Python) 또는 Parquet reader로 직접. 향후 `crates/datasets`에 Parquet reader 추가 가능 (사용자 신호 누적 후).
- **Workbench batch 프롬프트 UI** — 100인 배치를 Workbench가 진행상태 + 재시도로 처리하는 UI는 v1.x feature. 현재 가이드는 수동 loop 안내.

---

## 5. 테스트 invariant

본 sub-phase가 깨면 안 되는 invariant:

1. **`snapshot_loads_seed_entries`** — 핵심 한국어 모델 6종 (EXAONE 1.2B/3.5/4.0-32B, HCX-SEED 8B, polyglot-ko 12.8B, whisper-large-v3-korean) 절대 빠지지 않음. nemotron 추가가 이 invariant 영향 X.
2. **`host_mid_picks_balanced_korean`** — 16GB+12GB 호스트의 best는 EXAONE 7.8B 또는 HCX-SEED 8B. **nemotron-3-nano-4b가 best를 차지하면 안 됨** (한국어 우선 정책).
3. **`determinism_invariant`** — 동일 입력 100회 동일 결과. nemotron entry 추가 후에도 보존.
4. **새 가드** — `nemotron_3_nano_4b_is_in_catalog_for_mid_host` (회귀 가드): host_mid + AgentGeneral에서 nemotron이 *excluded에 없음* + 후보 ranking에 포함. (이 모델이 제외되면 manifest 깨졌다는 신호).
5. **manifest validator** — `intents`의 `agent-tool-use`, `translation-multi`, `ko-rag` 모두 `INTENT_VOCABULARY`에 등록됐는지 + `domain_scores` 모든 key 등록 + 값 0..=100.
6. **i18n 키 동기** — `guides.personas-korea.{title,description}` ko/en 양쪽 존재.

---

## 6. 다음 페이즈 인계

### 의존성 / 진입 조건
- 본 sub-phase는 **독립적** — 다른 페이즈 차단 X / 받지 X. v0.0.1 ship과 병렬.
- `crates/model-registry` 빌드 + `apps/desktop` 빌드 모두 영향 X (manifest는 런타임 로드).

### 위험 노트
- **Mamba-2 GGUF 호환성** — llama.cpp 정식 지원이지만 마이너 버그 보고 있음. LMmaster 번들 llama.cpp 버전 확인 필요. 사용자가 첫 사용 시 출력 깨지면 `warnings` 카피로 안내됨.
- **Personas-Korea 라이선스** — CC BY 4.0 (저작자 표시 필수). 가이드에 *사용 시 NVIDIA 명시 권장* 카피 포함.
- **개인정보 우려** — Personas-Korea는 100% 합성, 실제 개인 아님. KOSIS/대법원 통계 분포만 모방. 가이드에 명시.

### 다음 standby 후보
- **OCR v2 카테고리 신설** — `ModelCategory::Ocr` 추가 + Tesseract 대안. Phase 별도 (v1.x).
- **Workbench batch persona 시뮬레이션 UI** — 가이드의 수동 loop을 1-click 배치로. v1.x feature.
- **Rust Parquet reader** — `crates/datasets` 신설, Personas-Korea 직접 import. 사용자 신호 누적 후.

### 검증 명령
```powershell
.\.claude\scripts\verify.ps1
```
또는 개별:
```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --package model-registry
cd apps/desktop; pnpm exec tsc -b; pnpm exec vitest run
```

---

## 출처 (References)

- [nvidia/NVIDIA-Nemotron-3-Nano-4B-GGUF · HuggingFace](https://huggingface.co/nvidia/NVIDIA-Nemotron-3-Nano-4B-GGUF)
- [Nemotron 3 Nano 4B blog post](https://huggingface.co/blog/nvidia/nemotron-3-nano-4b)
- [nvidia/Nemotron-Personas-Korea · HuggingFace Datasets](https://huggingface.co/datasets/nvidia/Nemotron-Personas-Korea)
- [How to Ground a Korean AI Agent in Real Demographics with Synthetic Personas](https://huggingface.co/blog/nvidia/build-korean-agents-with-nemotron-personas)
- [700만명 가상 한국인 탄생 — AI타임스](https://www.aitimes.com/news/articleView.html?idxno=209762)
