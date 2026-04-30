# LMmaster 모델 큐레이션 가이드

> Phase 13'.e.5 인계. 신규 모델을 카탈로그에 추가하는 단계와 매니페스트 작성 정책.

## 1. 후보 발굴 — 어디서 보나

### 1.1 글로벌 (영어 / 멀티)
- **HF Open LLM Leaderboard v2** — 자동 평가 6종 종합
- **Chatbot Arena (LMSYS)** — 인간 선호도 Elo
- **HuggingFace Trending** — 주간 hot
- **Ollama Library Popular** — 우리 메인 런타임 인기
- **Artificial Analysis** — speed/cost/quality 3축

### 1.2 한국어 ⭐
- **Open Ko-LLM Leaderboard** — Upstage 호스팅
- **HAE-RAE Bench / KMMLU** — 한국 문화/학술
- **Kor-IFEval** — 한국어 instruction following

### 1.3 코딩
- **EvalPlus** — HumanEval+ / MBPP+
- **BigCodeBench**

### 1.4 임베딩
- **MTEB** — embedding 종합

## 2. 후보 필터 (이걸 통과해야 카탈로그 등재)

| 항목 | 기준 |
|---|---|
| **GGUF 존재** | `*-GGUF` repo 또는 Ollama Hub wrapper |
| **라이선스** | Open (Apache / MIT / Llama / Gemma TOU) — 라이선스 모호하면 제외 |
| **트래픽** | HF downloads ≥ 1,000/주 (1B 미만 모델은 ≥ 500/주) |
| **chat template** | GGUF에 임베드되어 있거나 검증된 wrapper(예: sam860/exaone-4.0) 존재 |
| **언어** | 한국어 자연스러움 ≥ 6점 OR 글로벌 Top 50 |

## 3. 매니페스트 작성 — 필드별 가이드

### 3.1 `id` / `display_name`
- `id`: lowercase + hyphen만. 예: `gemma-3-4b-it`. **유일성 + 안정성** — 한 번 정하면 변경 X.
- `display_name`: 사용자 향 한국어 표기. "Gemma 3 4B Instruct".

### 3.2 `category` (택1)
- `agent-general`: 일반 비서
- `roleplay`: 캐릭터/창작/롤플레이
- `coding`: 코드 생성/리뷰
- `slm`: 1-3B 경량 (CPU/엣지)
- `sound-stt`: STT (Whisper 등)
- `sound-tts`: TTS

### 3.3 `tier` (Phase 13'.e.1 신규)
- `new`: 90일 이내 등장 + 트래픽 검증된 신모델. 🔥 NEW 탭에 노출.
- `verified` (default): 큐레이터 검증 완료. 메인 카탈로그.
- `experimental`: chat template 위험, fine-tune 진행형.
- `deprecated`: 보안/품질 이슈로 비추천.

NEW에서 Verified 졸업: 60일+ 안정 + 큐레이터 확인.

### 3.4 `source`
- HuggingFace GGUF repo 우선. 예:
  ```json
  "source": {
    "type": "hugging-face",
    "repo": "bartowski/Llama-3.2-1B-Instruct-GGUF",
    "file": "Llama-3.2-1B-Instruct-Q4_K_M.gguf"
  }
  ```

### 3.5 `hub_id` (선택, 권장)
- Ollama Hub의 검증된 wrapper. chat template 위험 회피.
- 예: `"hub_id": "sam860/exaone-4.0:1.2b"` 또는 `"gemma3:4b"`.
- 없으면 frontend가 `hf.co/{repo}:{quant}` 자동 derivation (chat template 위험 가능).

### 3.6 `quantization_options`
- 최소 Q4_K_M 1개. 옵션으로 Q5_K_M / Q8_0 추가.
- `size_mb` 정확히 (사용자 디스크 공간 검증용).
- `sha256` 검증된 값 (현재 모두 `0...0` placeholder — Phase 14에서 SHA 검증 도입 시 채움).

### 3.7 VRAM/RAM
- `min_vram_mb`: 최소 VRAM. CPU만 가능하면 `null`.
- `rec_vram_mb`: 권장 VRAM (편한 응답 속도 기준).
- `rec_ram_mb`: 권장 RAM (모델 + OS + 다른 앱).

### 3.8 점수 (0-10)
- `language_strength`: 한국어 자연스러움 (한국어 leaderboard 점수 / 직접 테스트)
- `roleplay_strength`: 캐릭터 / 창작 적합도
- `coding_strength`: 코드 작성 능력
- `portable_suitability`: 포터블/이동 환경 (작을수록 ↑)
- `on_device_suitability`: 사용자 PC 적합도

### 3.9 `tool_support` / `vision_support` / `structured_output_support`
- chat template + 학습 데이터 기반. boolean.

### 3.10 `community_insights` (Phase 13'.e.1 신규) ⭐
4-section 한국어 작성 — drawer "?" 토글에 노출.

```json
"community_insights": {
  "strengths_ko": [
    "짧은 bullet 4-6개. 해요체 사실 진술.",
    "예: '한국어 일상 대화 자연스러워요'"
  ],
  "weaknesses_ko": [
    "솔직한 약점 — 사용자 mismatch 일찍 차단.",
    "예: '128K context 넘으면 hallucination 증가'"
  ],
  "use_cases_ko": [
    "자주 쓰이는 분야 — 사용자 매칭용.",
    "예: '한국어 캐릭터 롤플레이'"
  ],
  "curator_note_ko": "큐레이터 1-2 문장. '이 모델은 ~할 때, ~할 땐 X 권장'.",
  "sources": [
    "huggingface.co/...",
    "ollama.com/library/...",
    "github.com/..."
  ],
  "last_reviewed_at": "2026-04-30T00:00:00Z"
}
```

**작성 원칙**:
- 외부 LLM 자동 요약 X (정책 + 정확도).
- HF Community 탭 + r/LocalLLaMA + 한국 커뮤니티 + leaderboard 기반.
- 한국어 해요체 + 사실 진술 + 행동 유도.
- 60일+ 지나면 last_reviewed_at 갱신 권장.

## 4. 후보 풀 — 추가 권장 (Phase 13'.e.5 잔여 22)

다음 페이즈에 추가 권장하는 모델 — 큐레이션 작업 시간 부족으로 본 페이즈 미포함:

### 한국어 (Korean)
- **KULLM3** — `nlpai-lab/KULLM3` (SOLAR 기반 한국어)
- **Yi-Ko** — `beomi/Yi-Ko-6B` (커뮤니티 한국어 fine-tune)
- **Mistral-Ko** — `Korabbit/Mistral-7B-Korean` (한국어 Mistral)
- **HCX-Seed 1.5B** — 더 가벼운 변종

### 글로벌
- **Llama 3.3 70B** — `llama3.3:70b` (RTX 4090×2 환경 사용자용)
- **Yi-1.5 34B** — `01-ai/Yi-1.5-34B-Chat`
- **Mixtral 8x7B** — MoE 메모리 최적화
- **Phi-3.5-mini 3.8B** — Microsoft small
- **Aya Expanse 8B** — Cohere 다국어

### 코딩
- **Codestral 22B** — Mistral 코드 모델
- **CodeLlama 13B** — Meta
- **StarCoder2 15B** — BigCode

### 롤플레이
- **MythoMax-L2 13B** — 캐릭터 롤플레이
- **Nous-Hermes-2 11B** — 창작
- **Stheno** — 한국어 롤플레이 fine-tune

### SLM (소형)
- **TinyLlama 1.1B**
- **Qwen2.5 0.5B / 1.5B**
- **SmolLM2 1.7B** (HuggingFaceTB)

### 임베딩 (workspace knowledge에서 사용)
- **bge-m3** — 다국어 강력 (이미 EmbeddingModelPanel에 있지만 catalog에도 등재 권장)
- **KURE-v1** — 한국어 특화
- **multilingual-e5-small** — 가벼움

### 음성
- **Whisper-Korean Distil** — 가벼운 STT
- **Bark Korean** — 한국어 TTS

## 5. 큐레이션 워크플로

```
1. 후보 발굴 — leaderboard / trending / 커뮤니티
   ↓
2. 필터 적용 — GGUF / 라이선스 / 트래픽
   ↓
3. manifest 초안 작성 (`manifests/snapshot/models/<cat>/<id>.json`)
   - 본 가이드 §3 모든 필드
   - tier / community_insights 명시
   ↓
4. 직접 테스트 (선택) — Ollama로 받아서 30초 측정 + 한국어 응답 검증
   ↓
5. catalog bundle 재생성:
   node .claude/scripts/build-catalog-bundle.mjs
   ↓
6. cargo test -p model-registry — 중복 id 검증 + count test 통과
   ↓
7. commit + push
   ↓
8. jsDelivr propagate (24h 이내) + 사용자 자동 갱신 (6h cron)
```

## 6. 검토 갱신 정책

- `last_reviewed_at` 60일+ 지난 entry는 큐레이터 재검토.
- 모델 deprecated되거나 라이선스 변경 시 `tier: "deprecated"` 갱신.
- HF lastModified가 새 메이저 버전 신호면 family watchlist에 추가 (`gemma-3` → `gemma-4` 출시 시).

## 7. 관련 문서

- `docs/research/phase-13pe1-schema-decision.md` — schema 결정 (tier + insights)
- `docs/research/phase-13pa-live-catalog-decision.md` — 라이브 갱신 (jsDelivr)
- `docs/adr/0044-live-catalog-refresh.md`
- `docs/adr/0045-model-tier-and-community-insights-schema.md`
