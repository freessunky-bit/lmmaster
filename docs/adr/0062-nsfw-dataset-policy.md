# ADR-0062 — NSFW 데이터셋 정책 (minor_safety + 키워드 거부 + EULA)

* **상태**: Proposed (2026-05-07). Phase 23'.b — ADR-0061 자매.
* **선행**:
  - ADR-0014 (Curated Model Registry) — 큐레이션 정체성.
  - ADR-0061 (Dataset Catalog) — DatasetEntry schema (`content_warning` + `minor_safety_attestation` 필드).
  - 기존 NSFW 모델 정책 — `ContentWarning::RpExplicit` + `useAdultContentMode` 3-state 토글.
* **결정 노트**: `docs/research/phase-23pb-nsfw-dataset-policy-decision.md` (다음 sub-phase)

## 컨텍스트

NSFW 데이터셋 카탈로그 통합 시 **미성년자 보호** + **법적/윤리 책임** 매트릭스 정의 필요. 큐레이터 검증 게이트 + 사용자 EULA 갱신 + 정책 위반 시 자동 거부.

리스크:
- 미성년 콘텐츠 (loli/shota/age-regression 등) 포함 데이터셋 → 한국 청소년보호법 §4 / 미국 PROTECT Act / EU Child Sexual Abuse 위반.
- CC-BY-NC 데이터셋 → LMmaster 본 제품 상업 dual-license 시 비상업 트랙 분리 필요.
- HF NFAA (Not-For-All-Audiences) 플래그 미설정 데이터셋 → 사용자 첫 인상 망가짐.

## 결정

### 1. minor_safety_attestation 필수 필드 (DatasetEntry)

NSFW 라벨 (`content_warning: rp-explicit`) 데이터셋은 *반드시* 다음 필드 포함:

```rust
pub struct MinorSafetyAttestation {
    /// 큐레이터가 데이터셋 본문을 검증한 timestamp (RFC3339).
    pub verified_at: String,
    /// 큐레이터 식별자 — v1은 "lmmaster-curator" 단일.
    pub verified_by: String,
    /// 미성년 키워드 정규식 hit 0건 검증 결과.
    pub keyword_scan_clean: bool,
    /// HF NFAA 플래그 보유 여부.
    pub hf_nfaa_flag: bool,
    /// 라이선스가 OpenRAIL-M / Apache-2 / MIT / CC-BY 화이트리스트에 포함.
    pub license_whitelist: bool,
    /// 큐레이터 메모 (한국어 해요체).
    pub curator_note_ko: String,
}
```

**NSFW 데이터셋 + minor_safety_attestation 누락 시 카탈로그 자동 거부** (validator).

### 2. 미성년 키워드 하드 거부 리스트 (deterministic)

```rust
// crates/dataset-catalog/src/safety.rs
pub const MINOR_KEYWORDS_REJECT: &[&str] = &[
    // 영문
    "loli", "lolicon", "shota", "shotacon",
    "age-regression", "ageplay", "underage",
    "minor", "child", "preteen", "teen",
    // 일본어 (로마자 + 가나)
    "ロリ", "ショタ",
    // 한국어
    "미성년", "어린이", "아동",
];

pub fn dataset_has_minor_keywords(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    MINOR_KEYWORDS_REJECT
        .iter()
        .any(|k| lower.contains(&k.to_ascii_lowercase()))
}
```

큐레이터 등록 시 자동 정규식 scan — 1건이라도 hit 시 PR 자동 거부 + 큐레이터 알림.

### 3. 라이선스 화이트리스트 / 블랙리스트

| 분류 | 라이선스 | LMmaster 데이터셋 카탈로그 등록 |
|---|---|---|
| 화이트리스트 | Apache-2.0, MIT, BSD, CC-BY-4.0, CC-BY-SA-4.0, OpenRAIL-M | ✅ |
| 비상업 (별도 트랙) | CC-BY-NC-4.0, CC-BY-NC-SA-4.0 | ⚠️ `commercial: false` 라벨, 사용자 명시 동의 필요 |
| 블랙리스트 | proprietary, all-rights-reserved, *unspecified* | ❌ 자동 거부 |

### 4. EULA 갱신 (한국어 + 영어 동시)

```markdown
## NSFW 데이터셋 정책

LMmaster의 NSFW 데이터셋 카탈로그는 다음 정책을 따라요:

1. **미성년 콘텐츠 금지** — 큐레이터가 자동 키워드 scan + 본문 검증으로 1차 거부합니다.
   사용자가 미성년 묘사 데이터를 발견하면 즉시 신고해 주세요.
2. **HF NFAA 플래그 필수** — Not-For-All-Audiences 플래그가 없는 데이터셋은
   카탈로그에 등록되지 않아요.
3. **라이선스 화이트리스트** — Apache-2.0 / MIT / OpenRAIL-M 등 검증된 오픈
   라이선스만. CC-BY-NC는 *비상업 사용자 동의* 후만 노출.
4. **사용자 책임** — 다운로드 후 사용은 사용자 PC에서만 일어나요.
   사용자는 본인 국가의 법(한국 청소년보호법 / 미국 PROTECT Act / EU CSAM 등)을
   준수할 의무가 있어요.
```

기존 EULA 버전 bump (eula-v1 → eula-v2) — 사용자 재동의 필요.

### 5. NSFW 토글 + 데이터셋 필터 통합

기존 `useAdultContentMode` 3-state 토글이 *모델 + 데이터셋 모두* 동시 게이트:
- `hide` (기본): NSFW 모델 + NSFW 데이터셋 모두 숨김.
- `mixed`: NSFW 포함 모두 노출 + ⚠ chip.
- `only`: NSFW 모델 / NSFW 데이터셋만 노출.

### 6. 큐레이터 등록 흐름 (NSFW 데이터셋)

```
큐레이터가 NSFW 데이터셋 후보 발견
  ↓
1. HF NFAA 플래그 확인
2. 라이선스 화이트리스트 확인
3. 미성년 키워드 scan 실행 (dataset_has_minor_keywords)
4. 본문 100 row 샘플 직접 검토 (큐레이터 책임)
5. minor_safety_attestation 작성 + 한국어 메모
  ↓
manifest PR (manifests/snapshot/datasets/<cat>/<id>.json)
  ↓
sign-catalog.yml 자동 minisign + jsdelivr propagate
  ↓
사용자 카탈로그 NSFW 토글 only/mixed에서 노출
```

### 7. 위반 시 자동 거부 + 큐레이터 알림

manifest validator (Rust) 검증 단계:
- `content_warning: rp-explicit` + `minor_safety_attestation: None` → `Err(MinorSafetyMissing)`
- `minor_safety_attestation.keyword_scan_clean: false` → `Err(MinorKeywordHit)`
- 라이선스 블랙리스트 hit → `Err(LicenseBlacklisted)`

CI 시점에 자동 검증 + GitHub Issue로 큐레이터 알림.

## 근거

- **HF NFAA + OpenRAIL-M**: 업계 표준. NFAA는 *Not-For-All-Audiences* 메타 플래그, OpenRAIL-M은 라이선스 자체에 *"exploit, harm or attempting to exploit or harm minors"* 금지 조항 + viral clause.
- **deterministic 키워드 리스트**: LLM judge 거부 (ADR-0048 정신). 코드 상수 + 단위 테스트 100x.
- **라이선스 화이트리스트**: 큐레이터가 *어떤 라이선스인지* 명시 + 사용자 EULA 동의 후 노출. 위변조 + 라이선스 함정 차단.
- **사용자 자율 + 책임**: LMmaster는 *기술 인프라* 제공만. 다운로드 후 사용은 사용자 책임. EULA 명시.
- **3-state 토글 통합**: 기존 NSFW 모델 토글 패턴 재사용 (사용자 학습 곡선 0).

## 거부된 대안

1. **NSFW 데이터셋 카탈로그 자체 거부** — 사용자 가치 손실. 정공으로 *큐레이션 + 정책*이 정합.
2. **자동 LLM 검토 (NSFW 적합성 판정)** — ADR-0048 거부. deterministic 룰만.
3. **HF API로 NFAA 플래그 자동 확인** — runtime 검증은 부담. 큐레이터 등록 시점에 1회 확인이 정합.
4. **사용자 자체 키워드 리스트 추가 옵션** — UX 복잡 + abuse 위험. 큐레이터가 한 곳에서 관리.
5. **별도 NSFW 카탈로그 repo** — Phase 23'.c 운영 1~2개월 후 분리 검토. 본 sub-phase는 통합.
6. **OpenRAIL-M 외 라이선스 거부** — Apache-2 / MIT 등 다른 오픈 라이선스도 NSFW 데이터셋 호환 (큐레이터 검증 후).
7. **자동 EULA 재동의** — 사용자 부담. v1 EULA → v2 bump 시 *NSFW 데이터셋 토글 첫 활성*에서만 재동의 prompt가 정합.
8. **AI Hub 한국 정부 데이터셋 자동 등록** — 비상업 + 사용자 verification 필수. 큐레이터 등록 X, 사용자 import 가이드만.

## 결과 / 영향

### 신규 산출물 (이번 sub-phase)
- `crates/dataset-catalog/src/safety.rs` — 키워드 리스트 + scan 함수 + 단위 테스트.
- `crates/dataset-catalog/src/manifest.rs::validate_dataset_entry` — validator (minor_safety + license).
- `apps/desktop/src/i18n/eula-{ko,en}-v2.md` — EULA bump (v1 → v2, NSFW 데이터셋 정책 추가).

### 미래 산출물 (Phase 23'.c)
- 큐레이터 등록 GHA workflow — `keyword_scan_clean` 자동 검증.
- Settings 토글 — "NSFW 데이터셋 표시" (모델 NSFW와 별개 또는 통합).
- 첫 활성 시 *EULA v2 재동의 prompt*.

### 백워드 호환
- 기존 NSFW 모델 (Cydonia 24B / stheno-l3-8b) — 영향 0 (`content_warning` 그대로).
- 기존 EULA v1 사용자 — NSFW 데이터셋 *접근 시*만 v2 재동의.

### 라이선스
- 본 ADR + 정책 자체는 LMmaster의 일부 (MIT/Apache-2 dual).
- 큐레이션 데이터셋은 각자 라이선스 보존.

## 테스트 invariant

1. **`dataset_has_minor_keywords`** — `MINOR_KEYWORDS_REJECT` 모든 키워드 hit 시 true. 단위 테스트.
2. **License whitelist enforcement** — `validate_dataset_entry`가 블랙리스트 라이선스 거부.
3. **minor_safety_attestation 필수** — `content_warning: rp-explicit` + 누락 시 `Err(MinorSafetyMissing)`.
4. **EULA v2 재동의** — 첫 NSFW 데이터셋 접근 시 prompt + 사용자 거부 시 토글 비활성.
5. **deterministic 키워드 scan** — 동일 입력 100회 동일 결과.
6. **3-state 토글 통합** — `useAdultContentMode` 모델 + 데이터셋 동시 게이트 검증.

## 다음 단계

Phase 23'.c 진입 시:
1. `safety.rs` 구현 + 단위 테스트
2. `validate_dataset_entry` 구현 + invariant 테스트
3. EULA v2 작성 + 재동의 흐름
4. GHA workflow에 `keyword_scan` 자동 step 추가
