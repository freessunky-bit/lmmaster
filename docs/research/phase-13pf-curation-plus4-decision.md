# Phase 13'.f — 큐레이션 +4 (HCX-Seed 1.5B / Codestral 22B / Qwen 2.5 1.5B / Llama 3.3 70B)

* **상태**: 채택 (2026-04-30)
* **컨텍스트**: Phase 13'.e의 Curation +8 (12→20)에 이어 사용자가 명시 요청한 +22 큐레이션. 토큰 예산상 본 sub-phase는 *영향이 가장 큰 4개*에 집중하고 나머지 18개는 인계. 카탈로그 20→24 entries.

## 1. 결정 요약

1. **HCX-Seed 1.5B** (네이버, slm/, NEW tier) — 한국어 SLM 신상.
2. **Codestral 22B** (Mistral, coding/, verified tier) — 시스템 언어 코드 + FIM 강자. *비상업 라이선스 경고*.
3. **Qwen 2.5 1.5B** (Alibaba, slm/, verified tier) — 멀티링구얼 + Apache-2 자유 라이선스.
4. **Llama 3.3 70B** (Meta, agents/, verified tier) — 워크스테이션급 플래그십.
5. **나머지 18개** — `docs/CURATION_GUIDE.md` 또는 본 노트 §4에 인계 (deferred to Phase 13'.f.2).

## 2. 채택안

각 manifest는 기존 12 모델과 동일 패턴:
- top-level `tier`: `new` (HCX-Seed) / `verified` (나머지 3).
- `verification.tier`: `community` (HCX-Seed, 네이버 자체 검증) / `verified` (3개).
- `community_insights` 4 섹션 (strengths / weaknesses / use_cases / curator_note) + sources + last_reviewed_at 모두 한국어.
- `warnings` 필드에 라이선스 / VRAM / 라이선스 제약 등 명시.
- `hub_id`: Ollama 공식 등록 모델만 채움 (HCX-Seed는 `null` — HF 직접 풀).

선정 우선순위 (리서치 보고서 §"Korean-first 추천 우선순위"에 일부 일치):

| 모델 | 선정 이유 |
|---|---|
| HCX-Seed 1.5B | 네이버 신상 + 한국어 SLM 1순위. NEW 탭 첫 입주자. |
| Codestral 22B | Qwen 2.5 Coder가 약한 시스템 언어 (C++/Haskell/OCaml) 보완. |
| Qwen 2.5 1.5B | EXAONE/HCX-Seed가 특화 한국어라면 Qwen은 *멀티링구얼 + Apache-2*. |
| Llama 3.3 70B | 워크스테이션급 사용자 향 플래그십. 카탈로그 위계의 최상단. |

## 3. 기각안 + 이유 (negative space)

| 옵션 | 거부 이유 |
|---|---|
| **22개 모두 본 sub-phase에 작성** | 토큰 예산 4× 초과. Phase 13'.f.2로 분할이 합리적. |
| **KULLM3 (CC-BY-NC)** | 비상업 라이선스 + LMmaster의 일반 상업 사용 시나리오와 마찰. v1.x에 별도 NC-카테고리 도입 후 추가. |
| **Synatra 7B (Mistral-Ko-RP)** | RP + 한국어 카테고리 미정. roleplay/ 디렉터리에 별도 sub-phase에서. |
| **Yi-Ko 6B** | Yi-1.5 9B가 더 신선 + 안정. Yi-Ko는 v1.x 한국어 fine-tune 베이스로 별도 분류. |
| **Phi-3.5 mini** | 한국어 약함. 영어 reasoning 특화는 EXAONE / Qwen이 이미 충분. |
| **Phi-3.5 MoE** | llama.cpp 미지원 — 카탈로그 자체에 포함 불가. 영구 거부. |
| **Aya Expanse 8B/32B** | CC-BY-NC. Phase 13'.f.2에서 NC-카테고리와 함께. |
| **CodeLlama 7B/13B/34B** | 2023.08 모델 — Codestral / Qwen Coder / DeepSeek Coder가 모든 차원에서 우위. 카탈로그에 추가하면 사용자에게 *오래된 옵션* 인상. 거부. |
| **StarCoder2 3B/7B/15B** | 코드 base completion에 강하지만 instruct 모드는 Codestral / Qwen Coder가 우위. 후순위. |
| **MythoMax / Stheno / Nous-Hermes-2** | RP 카테고리 별도 sub-phase. Stheno는 NSFW 라벨 정책 정의 필요. |
| **TinyLlama 1.1B** | Llama 3.2 1B + Qwen 2.5 0.5B가 동급/하위 사이즈에서 더 강함. 거부. |
| **Qwen 2.5 0.5B** | 1.5B로 충분. 0.5B는 너무 약함. v1.x 모바일 임베드 시 재검토. |
| **SmolLM2** | 영어 only. Llama 3.2 1B와 정확히 같은 niche. 후순위. |
| **bge-m3 / KURE-v1 / multilingual-e5** | `category: "embedding"` 신규 enum 필요 — 스키마 변경 + RAG UI 분리 (워크벤치 RAG 설정에서 별도 셀렉터). Phase 13'.f.2 또는 9'.a 후속. |
| **Mixtral 8x7B Instruct** | 26GB+ VRAM에 한국어 약함. Llama 3.3 70B / EXAONE 32B가 같은 VRAM 대역대에서 더 좋음. 거부. |
| **Llama 3.1 8B / 70B** | Llama 3.3이 후속이라 3.1은 obsolete. 단 사용자 fine-tune 베이스로는 가치 — v1.x 별도 베이스 카테고리. |

## 4. 미정 / 후순위 이월 (Phase 13'.f.2 후보)

* **임베딩 카테고리** (bge-m3 / KURE-v1 / multilingual-e5) — 스키마에 `category: "embedding"` 추가 + RAG UI 분리. ~6h.
* **RP 카테고리** (Nous-Hermes-2 / MythoMax) — roleplay/ 디렉터리 활용 + content warning 라벨 정책 결정. ~3h.
* **NSFW 라벨 정책** (Stheno) — `content_warning: rp-explicit` 필드 추가 후 첫 화면 추천 제외 + "성인 콘텐츠 허용" 토글 시에만 노출. v1.x.
* **NC-라이선스 카테고리** (KULLM3 / Synatra / Aya Expanse / Codestral 자체도) — `commercial: false` 라벨 + UI에서 비상업 chip 표시. v1.x.
* **fine-tune 베이스 카테고리** (Yi-Ko / Llama 3.1 8B 등) — `purpose: "fine-tune-base"` 분류. v1.x.

## 5. 테스트 invariant

본 sub-phase 종료 시점에 깨면 안 되는 항목:

1. `model-registry::recommender_test` 16/16 통과 — 4개 추가 후에도 모든 host bucket 시나리오 유지.
2. `snapshot_loads_seed_entries` — `len() >= 8` lower bound 만족 (현재 24).
3. build-catalog-bundle.mjs 중복 id 검사 0건.
4. 모든 신규 manifest는 `maturity` enum (`experimental` / `beta` / `stable` / `deprecated`) + `verification.tier` enum (`verified` / `community`) 준수.
5. `community_insights` 4 섹션 + sources + last_reviewed_at 모두 채움.
6. 라이선스 제약 모델 (Codestral)은 `warnings`에 명시.

## 6. 다음 페이즈 인계

* Phase 13'.f.2 — 위 §4의 18개 잔여. 카테고리 schema 변경이 동반.
* ADR-0047 (이번 세션 후속) — 카탈로그 흐름과 무관하므로 13'.f 변경과 충돌 X.
* Diagnostics가 NEW tier 모델을 5초 polling 안에 인지하려면 Phase 13'.a의 6h refresh interval 의존 — 변경 X.
* 24 → 22 expansion 완성 시점은 Phase 13'.f.2 종료 시. 본 노트는 +4까지의 산출.

## References

- 결정 노트: 본 파일.
- 보강 리서치 (이번 세션): 22 모델 외부 메타데이터 + 라이선스 + VRAM + 한국어 강도 보고서.
- 코드:
  - `manifests/snapshot/models/slm/hcx-seed-1.5b.json`
  - `manifests/snapshot/models/coding/codestral-22b.json`
  - `manifests/snapshot/models/slm/qwen-2.5-1.5b.json`
  - `manifests/snapshot/models/agents/llama-3.3-70b.json`
  - `manifests/apps/catalog.json` (24 entries, build-catalog-bundle.mjs 재실행).
- 관련 ADR: ADR-0044 (live catalog refresh), ADR-0045 (tier + community_insights 스키마).
