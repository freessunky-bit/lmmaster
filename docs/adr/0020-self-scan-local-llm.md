# ADR-0020: 자가스캔 + 로컬 LLM augmentation — 요약 only, 판단은 deterministic

- Status: Accepted
- Date: 2026-04-26

## Context
사용자 요구: "앱이 자가스캔으로 LM Studio·Ollama·대중적 신모델을 발견하고 가능한 항목은 자동 업그레이드 진행. 이때 스캔에 사용되는 AI를 로컬에 자체 구축 가능한지 검증."

리서치(Phase 1' §3)로 확인:
- 한국어 fluency native 모델 EXAONE-3.5-2.4B(LG)·HyperCLOVA-X-SEED-3B(Naver)가 GGUF Q4_K_M으로 ~1.5~1.9GB.
- RTX 3060 6GB에서 200토큰 출력 ~3초, M2 Mac ~5초, CPU-only ~20초 — 백그라운드 작업으로 적합 (interactive는 NG).
- Apple Intelligence·Microsoft Phi Silica·Pixel Gemini Nano가 on-device LLM augmentation을 표준화.
- 단, 3B 모델은 (a) 한국어 1줄 요약은 reliable (b) version major/minor 분류 unreliable (regex가 더 정확) (c) "이 모델이 카탈로그 fit인가" judgement unreliable.

ADR-0013(Gemini boundary)은 외부 LLM에 deterministic 결정을 위임하지 말라고 정함. 동일 원칙을 로컬 LLM에도 확장.

## Decision

### 1. 사용 범위 (LLM 가능 / LLM 금지)

| Use case | 결정 | 비고 |
|---|---|---|
| (A) 모델 카드 한국어 1줄 블러브 | **LLM 사용** | 짧은 생성, JSON 입력. EXAONE/HCX-SEED reliable. |
| (B) 신버전 changelog 한국어 요약(2~3줄) | **LLM 사용** | "큰 변화 / 보안 / 성능" 같은 카테고리만 요약. |
| (C) Toss-style 안내 멘트 변주 | **LLM 사용** | onboarding 멘트, fallback 정적 템플릿 항상 보유. |
| (D) version major/minor/patch 분류 | **deterministic semver regex** | LLM 오판 위험. |
| (E) 신모델이 카탈로그 fit인가 | **deterministic 룰 + human review queue** | 라이선스/사이즈/editorial. |
| (F) GPU 적합도 / 모델 추천 | **deterministic recommender** | ADR-0014 유지. |
| (G) 헬스체크 / 설치 성공 판정 | **deterministic** | ADR-0013 동일. |

LLM 출력은 **항상** UI에 `source: "local-llm"` 뱃지 표시. deterministic 출력은 `source: "deterministic"`. 사용자가 신뢰 수준을 한눈에 식별.

### 2. 모델 cascade

```
scanner.summarize(input):
  if user has Ollama running with EXAONE-3.5-2.4B  → use (1순위)
  elif user has Ollama with HyperCLOVA-X-SEED-3B   → use (2순위)
  elif user has Ollama with Qwen2.5-3B-Instruct    → use (3순위)
  elif user has LM Studio running with 호환 모델     → use (lms / REST)
  else                                              → deterministic 한국어 템플릿
```

자동 다운로드 강제 금지. 모델 미설치는 첫실행 마법사의 첫 모델 큐(small=EXAONE-2.4B 추천)에서 처리 — 사용자 명시 동의 후.

### 3. 라이프사이클
- **schedule**: `tokio-cron-scheduler` 인-프로세스. 트리거:
  - on-launch (60초 grace)
  - 6h interval (cron)
  - UI on-demand 버튼
- **모델 로드**: Ollama HTTP `/api/generate` 또는 `/api/chat`, `keep_alive: "30s"`, `stream: true`.
  - JIT 로드 → 30초 idle eviction. 다음 스캔까진 메모리 비움.
- **LM Studio 사용 시**: `lms load <model> --ttl 30` 또는 REST `ttl: 30`.
- **결과 emit**: `app.emit("scan:summary", { model_id, korean_blurb, confidence: 0.0~1.0, source })`.
- **타임아웃 가드**: 단일 LLM 호출 30초 cap. 초과 시 deterministic fallback.

### 4. 프롬프트 정책
- system prompt는 **한국어**, 단일 임무 명세 ("아래 JSON으로 1줄 한국어 블러브를 만들어주세요. 마케팅 톤 금지."). 5KB 이하.
- user prompt는 **JSON 입력** (모델 카드, 변경 로그). 5KB 이하.
- 출력 schema: `{ "blurb": "...", "tags": [...] }` (JSON mode 가능 시).
- 환각 가드: 입력에 없는 사실 inject 금지. 길이 200자 이하.
- 모든 prompt는 `manifests/prompts/<task>.json`에 버전 관리.

### 5. fallback 한국어 템플릿
LLM 미동작 시:
- "{model_name}: {category}용 모델 ({size_mb}MB, {license}). 권장 VRAM {rec_vram}GB."
- "LM Studio v{new_version} 출시 — 변경 로그 보기"
- 항상 deterministic하고 충분히 informative.

### 6. 프라이버시 보장
- 모든 스캔 데이터(GitHub release 페이로드, HF metadata, 로컬 모델 메타)는 **로컬에만** 처리.
- LLM 호출은 **로컬 호스트만** (`127.0.0.1`).
- 외부 SaaS LLM(Gemini/OpenAI)으로 송신 금지 — 사용자 명시 opt-in일 때만 ADR-0013 범위로.
- UI 카피: "AI 모델 카탈로그가 LMmaster를 떠나지 않아요. 추천을 만드는 AI도 당신의 컴퓨터에서 동작합니다."

### 7. 테스트 / 검증
- 단위 테스트: deterministic fallback이 LLM 미동작 시 항상 valid 한국어 템플릿 반환.
- 통합 테스트: mock Ollama 서버로 keep_alive=30s 호출 + 30s 후 idle 확인.
- 회귀 테스트: snapshot 비교로 deterministic 결과 변동 없음 확인.

## Consequences
- 백그라운드 LLM 호출 비용: RTX 3060 ~3초, M2 ~5초, CPU 20초 — 6h마다 1~2회 호출 → 일일 사용 시간 영향 미미.
- VRAM 1.5~2GB: EXAONE/HCX-SEED Q4_K_M. 다른 활성 모델과 GPU contention 시 대기 큐 + 직렬화.
- LLM 미설치 사용자도 100% 동작 (deterministic fallback).
- 외부 SaaS 송신 0 — 프라이버시 마케팅 가능.
- 라이선스: EXAONE/HCX-SEED는 commercial w/ conditions — 우리는 사용자 PC에서 그들이 다운로드/실행 → 우리 재배포 0이라 안전. 사용자 동의 화면에서 라이선스 표시.

## Alternatives considered
- **외부 SaaS LLM (Gemini/OpenAI)으로 스캔 요약**: 거부 — 오프라인 불가, 프라이버시 손실, 사용 패턴 외부 노출.
- **모든 스캔 deterministic only (LLM 0)**: 가능하지만 한국어 카피 품질 저하. 거부 — augmentation 가치 있음.
- **로컬 LLM에 judgement 위임**: 거부 — ADR-0013 동일 원칙. 비결정성과 환각 위험.
- **자체 SLM 번들**: 본체 크기 폭증 (~1.5GB). 거부 — 사용자가 선택해 다운로드.

## References
- `docs/research/phase-1-reinforcement.md` §3
- ADR-0013 (Gemini boundary) — 동일 논리 확장
- ADR-0014 (curated registry) — trending 데이터 소스
- ADR-0019 (Always-latest hybrid) — 스캔이 갱신할 데이터의 소스
- huggingface.co/{LGAI-EXAONE/EXAONE-3.5-2.4B-Instruct, naver-hyperclovax/HyperCLOVAX-SEED-Text-Instruct-3B}
- Ollama keep_alive: github.com/ollama/ollama/blob/main/docs/api.md#parameters
- LM Studio TTL: lmstudio.ai/docs/local-server
- Apple Intelligence on-device: machinelearning.apple.com/research/introducing-apple-foundation-models
- tokio-cron-scheduler: github.com/mvniekerk/tokio-cron-scheduler
