# ADR-0045 — ModelTier + CommunityInsights schema 추가

* **상태**: 채택 (2026-04-30)
* **컨텍스트**: Phase 13'.e.1 — 큐레이션 확장 + NEW 탭 + 커뮤니티 인사이트 GUI를 위한 schema foundation.
* **결정 노트**: `docs/research/phase-13pe1-schema-decision.md`

## 결정

1. `ModelEntry`에 `tier: ModelTier` (default `verified`) 추가.
2. `ModelEntry`에 `community_insights: Option<CommunityInsights>` 추가.
3. 두 필드 모두 `#[serde(default)]` — 기존 manifest 호환.
4. Frontend TS 미러 — `ModelTier` type alias + `CommunityInsights` interface.

## 근거

- **tier vs maturity 분리** — Maturity는 모델 자체 안정성, Tier는 카탈로그 노출 분류. 직교.
- **외부 LLM 자동 요약 거부** — "외부 통신 0" 정책 + hallucination 위험.
- **schema 호환성** — `serde(default)`로 schema bump 없이 추가.

## 거부된 대안

- **Maturity에 `New` 추가** — 의미 충돌 (Stable+New 동시 가능).
- **외부 LLM/Reddit fetch** — 정책 위반 + 신뢰도 위험.
- **community_insights 별도 manifest 분리** — sync 부담.
- **사용자 UGC** — moderation 인프라 부재.
- **tier 완전 자동화** — chat template 위험 같은 문맥 판단 자동화 어려움.

## 결과 / 영향

- 기존 12 entries는 tier 누락 → verified 폴백 (자동).
- Phase 13'.e.5 큐레이션 +30은 모두 tier + community_insights 명시.
- HF metadata 자동 갱신은 기존 `hf_meta` 필드 (Phase 13'.e.2)에서.

## References

- 결정 노트: `docs/research/phase-13pe1-schema-decision.md`
- 후속 페이즈: 13'.e.2 (HF cron), 13'.e.3 (NEW 탭), 13'.e.4 (? 토글), 13'.e.5 (큐레이션)
- 관련 ADR: ADR-0014 (manifest schema 초기), ADR-0026 (외부 통신 0 정책)
- 코드:
  - `crates/model-registry/src/manifest.rs` (ModelTier, CommunityInsights)
  - `apps/desktop/src/ipc/catalog.ts` (TS 미러)
