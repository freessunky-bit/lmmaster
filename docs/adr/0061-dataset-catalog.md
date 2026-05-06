# ADR-0061 — Dataset Catalog (DatasetCategory enum + dataset-catalog crate)

* **상태**: Proposed (2026-05-07). v2.0 메이저 분기 (Phase 23'.a) — 사용자 명시 진입.
* **선행**:
  - ADR-0014 (Curated Model Registry) — 큐레이션 정체성 + manifest schema 패턴.
  - ADR-0044 (Live Catalog Refresh) — bundle JSON + jsdelivr propagate + minisign.
  - ADR-0048 (Intent Axis + domain_scores) — 자유 태그 + validator gate.
  - ADR-0059 (Trending Watcher) — 큐레이터 review queue 패턴.
  - ADR-0060 (Trend Report) — trends-bundle parallel schema.
* **결정 노트**: `docs/research/phase-23pa-dataset-catalog-decision.md` (다음 sub-phase에서 작성)
* **보강 리서치**: `phase-22p-trend-report-reinforcement.md` §1~6 (한국어 RP 데이터셋 + 라이선스 매트릭스 활용).

## 컨텍스트

사용자 요청 (2026-05-07): NSFW 한국어 RP 데이터셋 + Personas-Korea + RP fine-tune 시드를 LMmaster *카탈로그*에 직관적 GUI로 통합. 모델 카탈로그(`ModelCategory`)와 *별개 축*이지만 동일 라이브 갱신 인프라(jsdelivr + minisign + 4-tier fallback) 재사용.

핵심 충돌:
- ModelCategory enum 확장 (e.g. `Dataset` variant 추가)는 polymorphism 비용 큼 — `bench-harness` / `recommender` / `runner` 등이 *모델 전용*.
- 별도 `DatasetCategory` enum + 신규 crate가 정공.

## 결정

### 1. DatasetCategory enum (별도, parallel structure)

```rust
// crates/dataset-catalog/src/lib.rs
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DatasetCategory {
    /// SFT 시드 — instruction-tuning 직접 사용 가능.
    SftSeed,
    /// LoRA 시드 — 베이스 모델 hint 동반.
    LoraSeed,
    /// RAG corpus — 청크/임베딩 후 검색 시드.
    RagCorpus,
    /// Persona / character — 캐릭터 카드, narrative.
    PersonaSeed,
    /// 평가 / 벤치마크 — KMMLU / KoBEST 등.
    EvalBenchmark,
}
```

### 2. DatasetUseCase (tagged enum, fine 정책)

```rust
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DatasetUseCase {
    SftSeed { format: ChatFormat, language: Vec<Lang> },
    LoraSeed { base_model_hint: Option<String>, target_layers: Option<Vec<String>> },
    RagCorpus { chunk_strategy: ChunkStrategy, default_chunk_size: u32 },
    PersonaSeed { count: u64, narrative_field: String },
    EvalBenchmark { metric_keys: Vec<String> },
}
```

### 3. DatasetEntry — manifest schema (model entry parallel)

```rust
pub struct DatasetEntry {
    pub id: String,                       // 예: "huggingface-krew-korean-rp"
    pub display_name: String,             // "huggingface-KREW/korean-role-playing"
    pub category: DatasetCategory,
    pub source: DatasetSource,            // HuggingFace / DirectUrl / Bundled
    pub size_mb: u64,
    pub row_count: Option<u64>,
    pub languages: Vec<String>,           // ["ko"], ["en", "ko"], ...
    pub license: String,                  // "Apache-2.0" / "CC-BY-4.0" / "CC-BY-NC-4.0" / "openrail-m"
    pub commercial: bool,                 // CC-BY-NC false
    pub content_warning: Option<ContentWarning>,  // model entry와 같은 enum 재사용
    pub minor_safety_attestation: Option<MinorSafetyAttestation>, // ADR-0062
    pub use_case: DatasetUseCase,         // SFT / LoRA / RAG / Persona / Eval
    pub format: DatasetFormat,            // Parquet / JSONL / CSV / Arrow
    pub checksums: Option<DatasetChecksums>,
    pub community_insights: Option<CommunityInsights>,  // 모델과 동일 struct 재사용
    pub maturity: Maturity,
    pub tier: ModelTier,                  // model과 동일 enum 재사용 (new/verified/experimental/deprecated)
}
```

### 4. crates/dataset-catalog/ 신규 crate

별개 crate로 분리 — `model-registry`와 의존성 분리. workspace 멤버 추가:

```toml
# Cargo.toml workspace.members
"crates/dataset-catalog",
```

내부 의존성:
- `shared-types` (ContentWarning, ModelTier, Maturity, CommunityInsights 재사용)
- `serde` / `serde_json` / `thiserror`
- 향후: `polars-parquet` / `tokenizers` (Phase 23'.c RAG 시드 1-click)

### 5. manifest 위치 (parallel structure)

```
manifests/
├── apps/
│   ├── catalog.json              # 모델 카탈로그 (ADR-0044)
│   ├── trends-bundle.json        # 트렌드 (ADR-0060, Phase 22')
│   └── datasets-bundle.json      # 데이터셋 (본 ADR, Phase 23'.a)
├── snapshot/
│   ├── models/<cat>/<id>.json    # 개별 모델 매니페스트
│   └── datasets/<cat>/<id>.json  # 개별 데이터셋 매니페스트 (신규)
└── trends/
    └── ...
```

build script: `.claude/scripts/build-dataset-bundle.mjs` 신규 (catalog-bundle.mjs 패턴 미러).

### 6. registry-fetcher generic 활용 — 코드 변경 0

ADR-0044 + Phase 13'.g.3 인프라가 이미 generic. 호출 측만 추가:

```rust
// apps/desktop/src-tauri/src/lib.rs (또는 datasets IPC)
let datasets = fetcher.fetch("datasets-bundle").await?;
```

minisign 서명도 같은 keypair 재사용 — `sign-catalog.yml`에 paths trigger 추가만 (Phase 13'.g.3 패턴).

### 7. UX — Trends.tsx 데이터셋 섹션 (이미 placeholder 작성)

`apps/desktop/src/pages/Trends.tsx`의 데이터셋 카드 섹션이 *placeholder → 실 데이터*로 v0.2.0 진입 시 교체. 카드 grid + status chip (available / research / queued) + 한국어 해요체 hint.

## 근거

- **별도 enum**: model 카탈로그와 *직교 축*. 한 모델에 여러 데이터셋 매핑 / 한 데이터셋 다중 모델 fine-tune.
- **신규 crate**: `model-registry` 의존성 / 빌드 시간 영향 0. 향후 `dataset-catalog`만 의존하는 desktop IPC 분리 가능.
- **manifest 패턴 재사용**: ADR-0044 검증된 흐름 그대로. minisign 서명도 Phase 13'.g.3에 *paths trigger 추가만*.
- **Personas-Korea + KREW + LimaRP/rp-opus 1순위 시드**: Agent 보강 리서치 §1 결정.

## 거부된 대안

1. **`ModelCategory` enum 확장 (Dataset variant 추가)** — `bench-harness` / `recommender` polymorphism 비용 大. 거부.
2. **datasets를 `manifests/snapshot/models/datasets/` 하위에 통합** — 위와 동일 polymorphism. 거부.
3. **Python sidecar로 datasets lib 활용** — Tauri 페이로드 폭증 (Python runtime). Rust `polars-parquet` 직접 사용.
4. **CC-BY-NC 데이터셋 (rp-opus 등) 거부** — `commercial: false` 라벨로 카탈로그 등록 OK. 단 LMmaster 본 제품 *상업 dual-license* 시 별도 트랙. v2.x 검토.
5. **자동 다운로드 후 캐시** — 사용자 명시 클릭으로만 (큐레이션 정체성 + 디스크 사용 명시 동의).
6. **데이터셋별 임베딩 모델 강제** — RAG 시드 시점에 큐레이터 권장만. 사용자 자율.
7. **Open-Korean-Corpora (ko-nlp) 통째 임베드** — 라이선스 검증 미완. 후순위.
8. **AI Hub 데이터셋 (한국 정부 공식)** — 비상업 + verification 필수. 사용자가 직접 다운로드 후 LMmaster import 흐름이 정합. v2.x 가이드.

## 결과 / 영향

### 신규 산출물 (이번 sub-phase)
- `crates/dataset-catalog/` — Cargo crate (lib.rs + manifest.rs + format.rs + 단위 테스트)
- `manifests/snapshot/datasets/` — 디렉토리 생성 (실 entries는 Phase 23'.c)
- ADR-0062 (NSFW 데이터셋 정책 — 자매 ADR)

### 미래 산출물 (Phase 23'.c)
- `manifests/apps/datasets-bundle.json` — 합본 (Personas-Korea + KREW + LimaRP/rp-opus 4 entries)
- `crates/dataset-catalog/src/loader.rs` — manifest 로더 + validator
- `apps/desktop/src-tauri/src/datasets.rs` — IPC (list_datasets / download_dataset / import_to_rag)
- `apps/desktop/src/pages/Trends.tsx` — placeholder → 실 데이터 fetch
- `.claude/scripts/build-dataset-bundle.mjs` — 합본 빌드
- `.github/workflows/sign-catalog.yml` — paths trigger에 `manifests/apps/datasets-bundle.json` 추가

### 백워드 호환
- ModelCategory / model-registry 변경 0.
- 기존 사용자 카탈로그 / 추천기 / 워크벤치 동작 X.

### EULA 갱신 (Phase 23'.b ADR-0062 §)
- 데이터셋 fetch 정책 명시 — fair use + CC BY 출처 표기 + CC BY-NC 비상업 명시.
- minor_safety 정책 (ADR-0062 §).

## 다음 단계

1. **본 ADR 사용자 명시 승인 + Phase 23'.a 진입** — `crates/dataset-catalog/` 신설 + manifest schema + 단위 테스트.
2. **Phase 23'.b — ADR-0062 NSFW 데이터셋 정책**.
3. **Phase 23'.c — datasets-bundle.json 시드 entries 4개 (Personas-Korea + KREW + LimaRP + rp-opus) + IPC + UI 통합 + RAG 시드 1-click**.
4. **v0.2.0 ship** — Phase 22'.d (실 trends-bundle 연동) + Phase 23' 모두 종료 후.
