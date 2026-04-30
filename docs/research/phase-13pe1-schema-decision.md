# Phase 13'.e.1 — 카탈로그 schema 확장 (tier + community_insights)

> Phase 13'.e의 첫 단계. 후속 .2(HF cron) / .3(NEW 탭) / .4(? 토글) / .5(큐레이션 +30)이 본 schema를 사용.

## 1. 결정 요약

1. **`ModelTier` enum 신설** — 카탈로그 노출 분류 (`new` / `verified` / `experimental` / `deprecated`).
2. **`CommunityInsights` struct 신설** — 큐레이터 작성 4-section 인사이트.
3. **`ModelEntry`에 두 필드 추가** — `tier` (default `verified`) + `community_insights: Option<>`.
4. **`Maturity`(모델 안정성)와 별개 유지** — 직교 개념 (Stable+New 가능).
5. **외부 LLM 자동 요약 거부** — 큐레이터 수동 작성, "외부 통신 0" 정책 일관.
6. **HF metadata는 자동 갱신** — 기존 `hf_meta` 필드 (Phase 13'.e.2가 채움).

## 2. 채택안

### 2.1 ModelTier enum

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ModelTier {
    /// 90일 이내 등장 + 트래픽 검증. 🔥 NEW 탭.
    New,
    /// 큐레이터 검증 완료. 메인 카탈로그. (default)
    #[default]
    Verified,
    /// 사용자 위험 부담 큰 모델 (chat template 깨짐 등).
    Experimental,
    /// 보안/품질 이슈로 비추천.
    Deprecated,
}
```

**기준 — Phase 13'.e.3에서 자동/수동 결정**:

| Tier | 자동 결정 (cron) | 큐레이터 수동 결정 |
|---|---|---|
| New | HF first-commit ≤ 90d + downloads ≥ 5K/월 + GGUF 존재 + open license | NEW 진입 결정은 큐레이터 review |
| Verified | New에서 60일+ 안정 졸업 (자동) | 큐레이터가 "검증 완료" 표시 |
| Experimental | — | 큐레이터가 명시 (chat template 위험 등) |
| Deprecated | — | 큐레이터가 명시 (보안/품질) |

### 2.2 CommunityInsights struct

```rust
pub struct CommunityInsights {
    /// 짧은 bullet 4~6개. 한국어 해요체 ("한국어 일상 대화 자연스러워요").
    pub strengths_ko: Vec<String>,
    /// 솔직한 약점 — 사용자 mismatch 일찍 차단.
    pub weaknesses_ko: Vec<String>,
    /// 자주 쓰이는 분야.
    pub use_cases_ko: Vec<String>,
    /// 큐레이터 1-2 문장 코멘트.
    pub curator_note_ko: String,
    /// 출처 URL — UI footnote.
    pub sources: Vec<String>,
    /// RFC3339. 60일+ 지나면 재검토 hint.
    pub last_reviewed_at: Option<String>,
}
```

### 2.3 ModelEntry 통합

```rust
pub struct ModelEntry {
    // ... 기존 필드 (hub_id 포함)

    #[serde(default)]
    pub tier: ModelTier,

    #[serde(default)]
    pub community_insights: Option<CommunityInsights>,

    // ... 기존 필드 (verification 등)
}
```

**`#[serde(default)]`로 기존 manifest entries 호환** — schema bump 불필요. tier 누락은 verified 폴백, community_insights 누락은 None.

### 2.4 Frontend 미러

`apps/desktop/src/ipc/catalog.ts`:
- `ModelTier` type alias
- `CommunityInsights` interface  
- `ModelEntry`에 `tier?` + `community_insights?` 추가 — optional로 받음 (backend가 모두 default 채워 보내지만 미래 호환성).

## 3. 기각안 + 이유 (negative space)

**A. `Maturity`에 `New` 추가 (별도 enum 안 만듦)**
- ❌ 거부: Maturity는 *모델 자체 안정성* (저자 명시), Tier는 *카탈로그 노출 분류* (LMmaster 큐레이션). 직교 — Gemma 3 출시 시 Stable+New 가능. 한 enum에 합치면 의미 충돌.

**B. 외부 LLM (Reddit/HF API) 자동 요약**
- ❌ 거부:
  - 외부 통신 0 정책 위반 (Reddit/HF Comments API).
  - LLM hallucination — 사용자가 "큐레이터 검증된 인사이트"로 신뢰하는데 거짓 정보면 신뢰도 박살.
  - 비용 — Anthropic/OpenAI API 키 추가 필요.
  - 큐레이터 수동이 명시적 + 책임 추적 가능.

**C. `community_insights`를 별도 manifest 파일 분리**
- ❌ 거부 (단순화): 한 entry당 1 manifest 유지가 단순. 분리 시 sync 책임 + 발견 불편.

**D. tier에 `Pinned` (큐레이터 강조) 추가**
- ❌ 거부 (v1.x로 deferred): 추천 strip이 이미 best/balanced/lightweight pinning 역할. 별도 tier 중복.

**E. 사용자가 직접 community_insights 작성 가능 (UGC)**
- ❌ 거부 (v1):
  - moderation 인프라 없음.
  - Phase 8'.b custom-models는 사용자 본인 모델 한정 — UGC scope 다름.
  - 큐레이터 단일 source가 신뢰도 우선.
  - 이를 v1.x에 user comments / annotations로 분리 도입 가능.

**F. tier 자동 결정만, 큐레이터 수동 표시 X**
- ❌ 거부:
  - chat template 위험 같은 *문맥적 판단*은 자동화 어려움.
  - 큐레이터 수동 + 자동 임계값 hybrid가 정확도 우위.

**G. `last_reviewed_at`을 `last_modified`로 통합**
- ❌ 거부: HF의 `last_modified`는 모델 파일 변경, `last_reviewed_at`은 큐레이터 review 시점. 의미 다름. 둘 다 noeun 헷갈림 방지.

## 4. 미정 / v1.x 이월

- **자동 tier decay 임계값 튜닝** — 60일 경과? 90일? 사용 데이터 보고 조정 (Phase 13'.e.3 후 1개월 측정).
- **사용자 향 강점/약점 piechart 등 시각화** — 텍스트만으로 충분한지 사용성 테스트 후.
- **HF Community / Reddit fetch 도입** — Phase 14 이후 검토. 외부 통신 정책 ADR 변경 필요.
- **Curator audit log** — 누가 언제 무슨 community_insights 수정했는지 기록. 큐레이터 multi 시 필수.

## 5. 테스트 invariant

| 영역 | invariant |
|---|---|
| `ModelTier::default()` | `Verified` 반환 — 누락 entries 안전 폴백 |
| `ModelTier` serde | kebab-case (`"new"` / `"verified"` / `"experimental"` / `"deprecated"`) |
| `CommunityInsights::default()` | 빈 vec + 빈 string + None last_reviewed_at |
| `ModelEntry` 호환성 | `serde(default)` 덕에 기존 manifest (tier/community_insights 없음) 그대로 deserialize |
| `ModelTier` round-trip | JSON serialize → deserialize 동일 값 |
| TS `ModelEntry.tier` | optional — backend가 default 채우거나 omit 모두 처리 |

## 6. 다음 페이즈 인계

**13'.e.2 (HF cron)** 진입 조건:
- 본 schema commit ✓
- ModelEntry.hf_meta 필드 이미 존재 — cron이 채울 대상

**13'.e.3 (NEW 탭)** 진입 조건:
- tier enum 사용 가능 ✓
- 큐레이터가 적어도 1개 entry에 `tier: "new"` 명시 (테스트용)

**13'.e.4 (? 토글)** 진입 조건:
- CommunityInsights struct 사용 가능 ✓
- 큐레이터가 적어도 1개 entry에 `community_insights` 채움 (테스트용)

**13'.e.5 (큐레이션 +30)** — 신규 manifest 작성 시 tier + community_insights 모두 채우도록 큐레이션 가이드 작성.

## 7. ADR

본 결정 노트와 짝으로 `docs/adr/0045-model-tier-and-community-insights-schema.md` 신설.
