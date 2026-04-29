# Phase 4.h 잔여 — 99+ Korean preset + IPC 결정 노트

> 작성일: 2026-04-27
> 상태: 보강 리서치 완료 → 설계 확정 → 프로덕션 구현
> 선행: Phase 4.h(crates/preset-registry 로더), Phase 2'.a (카탈로그 8 시드 모델)
> 후행: Phase 5'(저장된 채팅 / preset usage analytics), Phase 6'(community preset 등록 / governance)
> 관련 ADR: ADR-0007 (preset 매니페스트 governance — verified/community 2-tier)

---

## 0. 결정 요약 (5가지)

1. **7 카테고리 × 13~16 = 103 presets** — 코딩(15) / 번역(14) / 법률(14) / 마케팅(16) / 의료(15) / 교육(15) / 리서치(14). 99+ 목표 충족 + 100 round number 초과.
2. **의료/법률 disclaimer 의무 강제** — 첫 줄 disclaimer + 마지막 줄 전문가 상담 권유. preset-registry crate가 build-time `ensure_disclaimer`로 검증.
3. **catalog drawer에 preset chooser 통합 — 별도 화면 거부** — 사용자가 모델 선택 시점에 같이 보는 게 가장 자연스러움. 별도 `/presets` 화면은 v1 사용량 데이터를 모은 후 v1.x에서 결정.
4. **IPC는 read-only 2개 — get_presets / get_preset** — 사용자 편집은 v2 (community contribution governance와 함께). v1은 verified preset만 노출.
5. **PresetCache lazy load + 평생 hold** — 7 × ~14 × ~3KB ≈ 300KB로 메모리 부담 미미. invoke마다 disk read 회피.

---

## 1. 채택안

### 1.1 카테고리 × preset 분포

| 카테고리 | 개수 | 대표 preset |
|---|---|---|
| coding | 15 | refactor-extract-method, code-review, generate-tests, sql-query-optimize, regex-explain, fastapi-endpoint, react-component, type-annotation, ci-yaml-write, dockerfile-write, git-commit-message, code-comment-ko, debug-error-message, json-schema-from-sample, performance-profile-tip |
| translation | 14 | ko-en-tech, ko-to-en-business, en-to-ko-business, ko-to-ja, ja-to-ko, subtitle-ko, paper-abstract-ko-en, marketing-copy-en-ko, technical-spec-ko-en, code-comment-translate, error-message-translate, legal-doc-careful, glossary-build, foreign-name-romanize |
| legal | 14 | contract-clause-review, terms-of-service-summary, nda-review, employment-contract-check, privacy-policy-write, cease-and-desist-template, copyright-claim-explain, korean-court-precedent-search-helper, lease-contract-clause, e-commerce-disclosure-check, ai-output-disclaimer-template, gdpr-vs-pipa-compare, debt-claim-letter, late-payment-letter |
| marketing | 16 | instagram-copy, naver-blog-seo, kakao-channel-message, b2b-cold-email, slogan-brainstorm, product-landing-hero, ad-headline-test, brand-tone-guide, customer-review-reply, push-notification-copy, story-ad-script, podcast-promo-script, value-proposition-1pager, abandoned-cart-email, holiday-greeting-card, conference-pitch-1min |
| medical | 15 | patient-explain-procedure, side-effect-explain, paper-abstract-summarize, drug-interaction-warn, lab-result-explain, diagnosis-symptom-clarify, prescription-translate-ko, mental-health-coach-disclaimer, child-vaccine-explain, emergency-symptom-checklist, medication-schedule-summary, surgery-pre-op-checklist, dietary-restriction-explain, post-discharge-care-explain, health-checkup-result |
| education | 15 | middleschool-math-tutor, elementary-vocab-quiz, highschool-english-essay-feedback, middleschool-science-explain, korean-grammar-explain, study-plan-2week, ielts-writing-task1, korean-history-summary, music-theory-basics, art-history-period-summary, programming-for-kids, foreign-language-flashcard-ko, classroom-activity-design, parent-teacher-letter, study-motivation-coach |
| research | 14 | paper-summarize-ko, meeting-minutes-from-transcript, korean-search-results-synthesize, citation-format-apa, literature-review-structure, hypothesis-from-data, survey-question-design, qualitative-coding-helper, statistics-explain-ko, dataset-description-write, abstract-rewrite-tighter, related-work-section-draft, conference-cfp-extract, grant-proposal-section |

**총 103 presets** (목표 99+ / 100 round number 초과).

### 1.2 preset JSON 스키마 (preset-registry 정의 일치)

```json
{
  "id": "{category}/{slug}",
  "version": "2026-04-27.1",
  "category": "{category}",
  "display_name_ko": "...",
  "subtitle_ko": "...",
  "system_prompt_ko": "...",
  "user_template_ko": "...",
  "example_user_message_ko": "...",
  "example_assistant_message_ko": "...",
  "recommended_models": ["..."],
  "fallback_models": ["..."],
  "min_context_tokens": 4096,
  "tags": ["..."],
  "verification": "verified",
  "license": "CC0-1.0"
}
```

Build-time invariants (preset-registry):
- `id` prefix == `category/`.
- 카테고리가 legal / medical면 `system_prompt_ko`에 disclaimer 키워드(`disclaimer` / `변호사` / `전문가 상담` / `정확한 진단`) 1개 이상 포함.
- `recommended_models[]`는 카탈로그 8 시드 모델 중 하나 (cross-link validation).

### 1.3 카피 톤 가이드 (Korean-first)

- 해요체 + 명사구 혼용.
- 페르소나 정의 → 작업 절차 1~4단계 → 원칙 → 응답 형식.
- system_prompt_ko 분량 — 200자 이상 (placeholder 1줄짜리 금지).
- 외래어는 자리잡은 것만, 풀어쓰기 의미 있는 경우만 1회.
- 의료/법률은 첫 줄 disclaimer + 마지막 줄 전문가 상담 권유 의무.

### 1.4 IPC 표면

```rust
#[tauri::command]
pub fn get_presets(
    cache: tauri::State<'_, Arc<PresetCache>>,
    app: AppHandle,
    category: Option<PresetCategory>,
) -> Result<Vec<Preset>, PresetApiError>;

#[tauri::command]
pub fn get_preset(
    cache: tauri::State<'_, Arc<PresetCache>>,
    app: AppHandle,
    id: String,
) -> Result<Option<Preset>, PresetApiError>;
```

- `PresetCache`: `Mutex<Option<Vec<Preset>>>` — lazy load + 평생 hold.
- 디렉터리 해석: resource_dir(prod) → CARGO_MANIFEST_DIR ancestors(dev) → cwd ancestors(추가 fallback).
- 결과는 id 알파벳 순 (preset-registry::load_all 정렬 보장).

### 1.5 Catalog Drawer 통합

`ModelDetailDrawer.tsx`에 새 섹션 추가:
- 제목: `drawer.section.presets` = "이 모델 추천 프리셋".
- mount 시 `getPresets()` 호출 → `recommended_models[]`에 model.id 포함된 preset 필터링.
- 카드 list — 각 카드: `display_name_ko` + `subtitle_ko` + 카테고리 chip(`categoryLabelKo`).
- 빈 결과: `drawer.section.presetsEmpty` = "추천 프리셋이 없어요".

---

## 2. 기각안

### 2.1 (a) Preset에 영어 fallback prompt 거부 — Korean-first 정책

- **검토**: 영어 system_prompt를 함께 두면 영문 사용자도 흡수.
- **이유**: LMmaster v1은 한국어 데스크톱 wrapper. 영어 fallback은 카피 톤이 일관되지 않고, 한국 모델(EXAONE / HCX)이 영어 prompt에서도 한국어로 응답하는 일관성을 깰 수 있음.
- **결론**: ko 단일 채택. 영어 사용자는 사용자가 직접 prompt를 영어로 바꾸거나, v1.x 다국어 지원 전까지 대기.

### 2.2 (b) Preset chooser 별도 화면 거부 — Catalog Drawer로 충분

- **검토**: `/presets` 별도 화면 + 검색 + 필터 + 즐겨찾기.
- **이유**: 사용자 사용 시나리오는 ① 카탈로그에서 모델 고르고 → ② preset 선택. 모델과 분리된 별도 화면은 "preset만 둘러보기"라는 use case가 v1에선 약함. 또한 9개 nav 항목이 이미 차 있어 추가 시 cognitive overhead.
- **결론**: Catalog Drawer 내 섹션으로 충분. Phase 6'에 사용 데이터 보고 별도 화면 결정.

### 2.3 (c) 카테고리 sub-category 거부 — 7 카테고리만

- **검토**: 코딩 → frontend / backend / DevOps 같은 sub-category.
- **이유**: 7 × 14 = 100 단위면 사용자가 한 번에 훑어 선택 가능. sub-category는 100+ × 5+ 단계가 됐을 때 의미 있음. v1은 평면 7 카테고리.
- **결론**: 7 카테고리 평면. 태그(tags 필드)로 보조 분류만.

### 2.4 (d) Preset 사용 통계 v1 거부 — Phase 6'

- **검토**: `record_preset_used(id)` IPC + SQLite 누적.
- **이유**: 사용 데이터가 community preset governance(Phase 6')에서 trending / popular을 보여주는 핵심이지만, v1에선 verified만 있어 ranking 효과 미미. 또한 사용 데이터를 외부 전송할지 / 로컬만 둘지 정책이 미정 (개인정보 / zero-knowledge 원칙).
- **결론**: v1은 read-only. Phase 5'/6'에 정책 확정 후 도입.

---

## 3. 검증 / 운영

- `cargo test -p preset-registry` — 통합 테스트가 자동으로:
  - 99+ preset 로드 (snapshot_loads_full_korean_preset_library).
  - 7 카테고리 모두 channel 그룹화.
  - 의료/법률 disclaimer 키워드 검증.
  - cross-link 카탈로그 8 시드 매칭.
  - id 알파벳 정렬.

- `cargo test -p lmmaster-desktop` — `presets::commands` 모듈 테스트:
  - `PresetApiError` kebab-case 직렬화.
  - `From<PresetError> for PresetApiError` 메시지 보존.
  - `PresetCache` 초기 상태 빈 mutex.

---

## 4. 산출 보고

### 4.1 카테고리별 preset 카운트

| 카테고리 | 합계 | 검증 (disclaimer 의무) |
|---|---|---|
| coding | 15 | -- |
| translation | 14 | -- |
| **legal** | **14** | ✅ 모두 disclaimer 키워드 포함 |
| marketing | 16 | -- |
| **medical** | **15** | ✅ 모두 disclaimer 키워드 포함 |
| education | 15 | -- |
| research | 14 | -- |
| **총계** | **103** | -- |

### 4.2 cross-link 검증

모든 `recommended_models[]`이 카탈로그 8 시드 모델 중 하나:
- `exaone-4.0-1.2b-instruct`, `exaone-3.5-7.8b-instruct`, `exaone-4.0-32b-instruct`, `hcx-seed-8b`, `polyglot-ko-12.8b`, `qwen-2.5-coder-3b-instruct`, `llama-3.2-3b-instruct`, `whisper-large-v3-korean`.

### 4.3 생성 파일

**Manifests** (98 신규 + 5 기존 = 103):
- `manifests/presets/coding/*.json` 15개 (14 신규).
- `manifests/presets/translation/*.json` 14개 (13 신규).
- `manifests/presets/legal/*.json` 14개 (13 신규).
- `manifests/presets/marketing/*.json` 16개 (15 신규).
- `manifests/presets/medical/*.json` 15개 (15 신규 — 카테고리 자체가 새로 채워짐).
- `manifests/presets/education/*.json` 15개 (14 신규).
- `manifests/presets/research/*.json` 14개 (14 신규 — 카테고리 자체 새로 채워짐).

**Rust + React**:
- `apps/desktop/src-tauri/src/presets/mod.rs`
- `apps/desktop/src-tauri/src/presets/commands.rs`
- `apps/desktop/src-tauri/permissions/presets.toml`
- `apps/desktop/src/ipc/presets.ts`

**수정**:
- `apps/desktop/src/components/catalog/ModelDetailDrawer.tsx` — "이 모델 추천 프리셋" 섹션 추가.
- `apps/desktop/src/i18n/ko.json` — `screens.presets.category.*`, `drawer.section.presets`/`presetsEmpty` 추가.
- `apps/desktop/src/i18n/en.json` — 미러.
- `crates/preset-registry/tests/loader_test.rs` — `snapshot_loads_full_korean_preset_library` 확장 (>= 99) + 7 카테고리 검증.

### 4.4 메인 통합 시 필요한 작업

1. `apps/desktop/src-tauri/Cargo.toml` — `preset-registry.workspace = true` 추가.
2. `apps/desktop/src-tauri/src/lib.rs` — `presets` 모듈 등록 + `PresetCache` State + `get_presets`/`get_preset` invoke handler 등록.
3. `apps/desktop/src-tauri/capabilities/main.json` — `allow-get-presets`, `allow-get-preset` 추가.
4. `apps/desktop/src-tauri/tauri.conf.json` bundle.resources에 `manifests/presets/{category}/*.json` (또는 `manifests/presets/**`) 추가.

### 4.5 다음 단계 (Phase 5'/6')

- 사용 통계 — preset_used / preset_completion 이벤트 (zero-knowledge 정책 확정 후).
- Community preset governance — verified / community 2-tier 등록 워크플로.
- Preset 별도 화면 결정 — Catalog Drawer 통합으로 충분한지 사용 데이터 보고.
- 다국어 — 영어/일본어 preset (별도 카테고리 또는 multi-language 필드).
