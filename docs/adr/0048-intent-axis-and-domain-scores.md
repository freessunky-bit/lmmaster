# ADR-0048 — Intent 축 + domain_scores 스키마

* **상태**: 채택 (2026-04-30) — Phase 11'.a 머지 완료. 후속 11'.b/c는 본 ADR을 전제로 진행.
* **컨텍스트**: 사용자 비전 = "비전/영상/번역에 유리한 AI를 *의도(intent)* 기준으로 찾고, 도메인 벤치마크로 우열을 판단해 모델을 선택, 도메인 특화로 이어진다." 기존 카테고리 enum(coding/roleplay/agent-general/slm/sound-stt/sound-tts)은 1:1 분류라 한 모델이 여러 도메인에 걸치는 현실(Qwen2-VL = vision + 한국어 + tool-use)을 표현 못함.
* **결정 노트**: `docs/research/phase-11p-12p-v1x-domain-axis-decision.md`

## 결정

1. `ModelEntry`에 `intents: Vec<IntentId>` 추가 (자유 태그, N:N 매핑).
2. `ModelEntry`에 `domain_scores: BTreeMap<IntentId, f32>` 추가 (0..100 범위, 큐레이터 수동 인용).
3. `IntentId = String`, `INTENT_VOCABULARY` (v1.x 시드 11종)을 `shared_types/intents.rs`에 등록 — 등록된 ID만 manifest validator 통과.
4. 두 필드 모두 `#[serde(default)]` — schema_version bump 없음 (legacy 24 entries 호환).
5. Recommender `compute(...)` 시그니처에 `intent: Option<&IntentId>` 추가 — `None`이면 기존 로직 그대로(backward compat).

## 근거

- **카테고리 enum 확장 거부** — 1:1 분류는 큐레이션 마찰 + 사용자 멘탈 충돌. Intent 자유 태그가 N:N으로 더 자연스러움.
- **자동 벤치마크 실행 거부** — dataset 라이선스 + 시간 비용. 큐레이터 leaderboard 인용 + `community_insights.sources` 명시가 외부 통신 0 정책과 정합.
- **자유 텍스트 → Gemini 매핑 거부** — v1.x는 미리 정의된 11종 칩으로 시작. 사용자 신호 누적 후 v2.

## 거부된 대안

- **vision/translation/video 카테고리 enum 신설** — 1:1 강요 → 멀티 도메인 모델 분류 불가.
- **schema_version 2로 bump** — `serde(default)` 호환으로 불필요.
- **Recommender 전면 재작성 (sklearn ranking)** — deterministic 위반 + 학습 데이터 부재.
- **자유 텍스트 의도 입력 (v1.x)** — Gemini 의존 + 외부 통신 위반.
- **Intent 사전 자동 확장 (사용자가 추가)** — moderation 부재.

## 결과 / 영향

- 기존 24 entries는 빈 `intents: []` + `domain_scores: {}` 폴백 → 점진 백필.
- Recommender `intent=None` 호출 경로 0 변동 — 16 invariant 테스트 그대로 통과.
- Catalog UI는 IntentBoard 컴포넌트 신설 (sidebar 카테고리 위), ModelCard 도메인 점수 바 조건부 렌더.
- Workbench는 의도 컨텍스트를 URL hash로 전달받아 Stage 0 프롬프트 템플릿에 매핑.

## References

- 결정 노트: `docs/research/phase-11p-12p-v1x-domain-axis-decision.md`
- 후속 페이즈: 11'.b (Catalog 의도 보드 + Recommender 가중), 11'.c (HF 하이브리드), 12'.a/b (Workbench 사다리)
- 관련 ADR: ADR-0014 (manifest 초기), ADR-0045 (tier+insights), ADR-0049 (HF 하이브리드), ADR-0050 (Workbench 사다리)
- 코드:
  - `crates/model-registry/src/manifest.rs` (intents, domain_scores 필드 추가 위치)
  - `crates/model-registry/src/recommender.rs` (compute 시그니처 확장 위치)
  - `crates/shared-types/src/intents.rs` (신규 — INTENT_VOCABULARY)
  - `apps/desktop/src/pages/Catalog.tsx` (IntentBoard 삽입 위치 — RecommendationStrip 위)
