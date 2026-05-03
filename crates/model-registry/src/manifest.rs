//! Manifest 스키마 — ADR-0014 + Phase 2'.a 보강.
//!
//! 정책:
//! - VerificationInfo: 2-tier governance (Verified/Community). 기본 Community.
//! - HfMeta: schema-now-data-later — v1 시드는 비움, v1.1에서 HF Hub API로 채움.
//! - use_case_examples: Workbench 프롬프트 시드용 한국어 자연어 예시.

use serde::{Deserialize, Serialize};
use shared_types::{is_registered_intent, IntentId, ModelCategory, RuntimeKind};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelManifest {
    pub schema_version: u32,
    pub generated_at: String, // RFC3339
    pub entries: Vec<ModelEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub display_name: String,
    pub category: ModelCategory,
    pub model_family: String,
    pub source: ModelSource,
    pub runner_compatibility: Vec<RuntimeKind>,
    pub quantization_options: Vec<QuantOption>,
    pub min_vram_mb: Option<u64>,
    pub rec_vram_mb: Option<u64>,
    pub min_ram_mb: u64,
    pub rec_ram_mb: u64,
    pub install_size_mb: u64,
    pub context_guidance: Option<String>,
    pub language_strength: Option<u8>,
    pub roleplay_strength: Option<u8>,
    pub coding_strength: Option<u8>,
    pub tool_support: bool,
    pub vision_support: bool,
    pub structured_output_support: bool,
    pub license: String,
    pub maturity: Maturity,
    pub portable_suitability: u8,
    pub on_device_suitability: u8,
    pub fine_tune_suitability: u8,

    /// 검증된 Ollama Hub 모델명 — `hf.co/{repo}` 직접 풀이 chat template 누락 위험이 있는
    /// 한국어 특수 architecture (EXAONE 4.0 등)는 큐레이터가 검증된 wrapper로 매핑.
    ///
    /// 정책 (2026-04-30 — Ollama HF 통합 정밀 리서치):
    /// - 있으면 풀/측정 모두 `hub_id` 그대로 (예: `sam860/exaone-4.0:1.2b`).
    /// - 없으면 source가 HuggingFace일 때 `hf.co/{repo}:{quant}` 자동 derivation으로 폴백.
    /// - 사용자 첫 인상에 출력 깨짐을 막아주는 큐레이션 layer.
    #[serde(default)]
    pub hub_id: Option<String>,

    /// 카탈로그 노출 분류 (Phase 13'.e.1) — UI 탭 + 필터 산출용.
    ///
    /// 정책 (`docs/research/phase-13pe1-schema-decision.md`):
    /// - **New**: 90일 이내 등장 + HF 트래픽 임계값 통과한 검증 진행 중 모델. 🔥 NEW 탭.
    /// - **Verified**: 큐레이터 검증 완료 + 60일+ 안정. 메인 카탈로그.
    /// - **Experimental**: chat template 위험 있거나 fine-tune 진행형. 사용자 위험 부담 hint.
    /// - **Deprecated**: 보안/품질 이슈로 비추천. UI는 회색 + warning.
    ///
    /// `Maturity`(model 자체 안정성)와 별개. Maturity=Stable이면서 Tier=New 가능 (예: Gemma 3 출시).
    /// 누락 시 Verified로 폴백 — 기존 큐레이션 entries는 schema bump 없이 유지.
    #[serde(default)]
    pub tier: ModelTier,

    /// 큐레이터가 작성한 커뮤니티 인사이트 — drawer "?" 토글에 노출 (Phase 13'.e.4).
    ///
    /// 정책: 외부 LLM 자동 요약 X — LMmaster "외부 통신 0" 정책 + hallucination 위험 회피.
    /// 큐레이터가 수동으로 4-section (강점/약점/사용 분야/코멘트) 작성 + 출처 명시.
    #[serde(default)]
    pub community_insights: Option<CommunityInsights>,

    /// 2-tier governance — 누락 시 Community로 폴백. v1은 cosmetic.
    #[serde(default)]
    pub verification: VerificationInfo,

    /// Hugging Face metadata — v1 시드는 None. v1.1에서 cache.rs로 채움.
    #[serde(default)]
    pub hf_meta: Option<HfMeta>,

    /// Workbench 프롬프트 시드용 한국어 자연어 예시.
    #[serde(default)]
    pub use_case_examples: Vec<String>,

    /// 의도 태그 — 사용자 입력(intent picker)과 매칭되는 자유 태그. (Phase 11'.a, ADR-0048)
    ///
    /// 정책:
    /// - 카테고리(`category`)와 별개 축. 한 모델이 여러 intent에 속할 수 있음 (N:N).
    /// - validator는 `shared_types::INTENT_VOCABULARY`에 등록된 ID만 통과시킴.
    /// - 누락 시 빈 vec — 기존 entries는 schema bump 없이 호환.
    #[serde(default)]
    pub intents: Vec<IntentId>,

    /// 도메인 벤치마크 점수 — `IntentId → 0.0..=100.0`. (Phase 11'.a, ADR-0048)
    ///
    /// 정책:
    /// - 누락된 intent는 점수 미보유로 처리(추천에서 가중 0). 큐레이터가 점진 백필.
    /// - 큐레이터가 공식 leaderboard 또는 benchmarks paper에서 인용 + 출처는
    ///   `community_insights.sources`에 누적.
    /// - validator는 모든 key가 `INTENT_VOCABULARY`에 등록되고 값이 0..=100임을 검증.
    #[serde(default)]
    pub domain_scores: BTreeMap<IntentId, f32>,

    /// 모델 사용 목적 — Phase 13'.f.2 (DEFERRED.md §13'.f.2 §1/§5).
    ///
    /// 정책:
    /// - `general-chat` (기본) — 일반 채팅 추천에 등장. 누락 시 폴백.
    /// - `fine-tune-base` — 베이스 모델, instruction-tuned 아님. chat target에서 자동 제외.
    /// - `retrieval` — 임베딩 모델 (RAG). chat target에서 자동 제외, Workspace에서만 노출.
    /// - `reranker` — 검색 결과 재정렬. chat target에서 자동 제외.
    /// - 자동 제외 정책은 recommender의 `evaluate()`에서 `PurposeMismatch` 분기로 구현.
    #[serde(default)]
    pub purpose: ModelPurpose,

    /// 상업 사용 가능 여부 — Phase 13'.f.2.2 (DEFERRED.md §13'.f.2 §4).
    ///
    /// 정책:
    /// - `true` (기본) — Apache-2 / MIT / BSD 등 상업 자유.
    /// - `false` — CC-BY-NC / Llama Community 700M+ 사용자 제약 / EXAONE Custom 등.
    /// - UI는 `false`일 때 ⚠ "비상업" chip 노출. 큐레이터가 license 약관 검토 후 결정.
    /// - 누락 시 `true` 폴백 (기존 entries 호환).
    #[serde(default = "default_commercial")]
    pub commercial: bool,

    /// 콘텐츠 경고 — Phase 13'.f.2.2 (DEFERRED.md §13'.f.2 §3).
    ///
    /// 정책:
    /// - `None` (기본) — 일반 모델.
    /// - `RpExplicit` — 성인 RP / NSFW 모델. 첫 화면 추천 제외 + "성인 콘텐츠 허용" 토글 시에만 노출.
    /// - 누락 시 `None` 폴백.
    #[serde(default)]
    pub content_warning: Option<ContentWarning>,

    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,

    /// 비전 모델의 mmproj projector 파일 정보 — Phase 13'.h.2.c (ADR-0051).
    ///
    /// 정책:
    /// - llama.cpp는 vision 모델이 GGUF 본체 + mmproj-*.gguf 두 파일로 구성됨.
    /// - `vision_support: true` + 본 필드 누락 시 → llama.cpp 어댑터에서 한국어 안내 노출.
    /// - Ollama / LM Studio는 내부 처리하므로 본 필드 무시.
    /// - 누락 시 `None` 폴백 — 기존 entries 호환.
    #[serde(default)]
    pub mmproj: Option<MmprojSpec>,
}

/// 멀티모달 projector 파일 사양 — Phase 13'.h.2.c (ADR-0051, 보강 리서치 §1.2).
///
/// 정책:
/// - HF resolve 직링 (외부 통신 화이트리스트 `huggingface.co`).
/// - sha256 32-byte hex. None이면 사용자 경고 노출 (ADR-0042 정책).
/// - precision은 F16 표준 권장. BF16/F32 옵션.
/// - source 큐레이터 출처 기록 (`bartowski` / `ggml-org` / `unsloth` / `lmstudio-community`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MmprojSpec {
    pub url: String,
    #[serde(default)]
    pub sha256: Option<String>,
    pub size_mb: u64,
    /// "f16" / "bf16" / "f32". F16이 표준.
    #[serde(default)]
    pub precision: Option<String>,
    /// "bartowski" / "ggml-org" / "unsloth" / "lmstudio-community".
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ModelSource {
    HuggingFace { repo: String, file: Option<String> },
    DirectUrl { url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantOption {
    pub label: String, // 예: "Q4_K_M"
    pub size_mb: u64,
    pub sha256: String,
    pub file_path: Option<String>, // HF repo 내 path
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Maturity {
    Experimental,
    Beta,
    Stable,
    Deprecated,
}

/// `commercial` 필드 default — Apache-2/MIT 등 대부분 모델이 상업 자유라 true 폴백.
fn default_commercial() -> bool {
    true
}

/// 콘텐츠 경고 — Phase 13'.f.2.2.
///
/// kebab-case serde. v1.x 시드는 `RpExplicit` 1종. v2 확장(violence, self-harm 등) 가능.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ContentWarning {
    /// 성인 RP / NSFW 모델. 첫 화면 추천 제외 + 사용자 명시 활성화 토글 필요.
    RpExplicit,
}

/// 모델 사용 목적 — Phase 13'.f.2.
///
/// `ModelCategory`(노출 분류)와 별개:
/// - `ModelCategory` = UI 사이드바 탭 분류 (coding / roleplay / embeddings 등).
/// - `ModelPurpose` = chat 추천 가능 여부 (chat에 적합한 일반 모델 vs 임베딩/재정렬/베이스).
///
/// `general-chat` 외 purpose는 chat target 추천에서 자동 제외 (recommender의 PurposeMismatch).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ModelPurpose {
    /// 일반 채팅 — 기본. 누락 시 폴백.
    #[default]
    GeneralChat,
    /// 베이스 모델, instruction-tuned 아님 — Workbench LoRA 시드용.
    FineTuneBase,
    /// 임베딩 모델 (RAG) — Workspace > 임베딩 모델에서만 노출.
    Retrieval,
    /// 검색 결과 재정렬 — RAG 후처리.
    Reranker,
}

/// 카탈로그 노출 분류 — Phase 13'.e.1.
///
/// `Maturity`와 별개:
/// - `Maturity` = 모델 자체 안정성 (저자가 매긴 알파/베타/스테이블).
/// - `ModelTier` = LMmaster 카탈로그 노출 분류 (큐레이터 + 자동 임계값으로 결정).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ModelTier {
    /// 90일 이내 등장 + 트래픽 검증된 신모델. 🔥 NEW 탭에 노출.
    New,
    /// 큐레이터 검증 완료 — 메인 카탈로그 1순위. 누락 시 폴백 default.
    #[default]
    Verified,
    /// chat template 위험이 있거나 사용자 책임 부담이 큰 모델. UI에 ⚠ hint.
    Experimental,
    /// 보안/품질 이슈로 비추천. UI 회색 처리 + warnings 강조.
    Deprecated,
}

/// 큐레이터가 손으로 정리한 커뮤니티 인사이트 — drawer "?" 토글에 4-section으로 노출.
///
/// 정책 (`phase-13pe1-schema-decision.md`):
/// - 외부 LLM 자동 요약 거부 — 정확도 + "외부 통신 0" 정책.
/// - HF Community + r/LocalLLaMA + Korean 커뮤니티 + leaderboard 기반.
/// - 큐레이터가 사실 진술 + 1-2 문장 코멘트 + 출처 URL 리스트 작성.
/// - last_reviewed_at으로 "검토 후 N일 지남" hint 가능.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommunityInsights {
    /// 짧은 bullet 4~6개. 한국어 해요체 간결 구조 ("한국어 일상 대화 자연스러워요").
    #[serde(default)]
    pub strengths_ko: Vec<String>,
    /// 사용자가 mismatch 일찍 알 수 있게 솔직한 약점.
    #[serde(default)]
    pub weaknesses_ko: Vec<String>,
    /// 자주 쓰이는 분야 — 사용자가 자기 use case와 매칭하기 위함.
    #[serde(default)]
    pub use_cases_ko: Vec<String>,
    /// 큐레이터 1-2 문장 한국어 메모. "이 모델은 ~할 때 권장, ~할 땐 X 모델이 더 좋아요" 식.
    #[serde(default)]
    pub curator_note_ko: String,
    /// 출처 URL — UI에 footnote로 노출. r/LocalLLaMA / HF Community / leaderboard 등.
    #[serde(default)]
    pub sources: Vec<String>,
    /// 큐레이터 마지막 review 시각 (RFC3339). "60일+ 지남" → 재검토 권장 hint.
    #[serde(default)]
    pub last_reviewed_at: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum VerificationTier {
    Verified,
    #[default]
    Community,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VerificationInfo {
    #[serde(default)]
    pub tier: VerificationTier,
    /// RFC3339 — verified 시 기록.
    #[serde(default)]
    pub verified_at: Option<String>,
    /// 큐레이터 식별자 — v1은 "lmmaster-curator" 단일.
    #[serde(default)]
    pub verified_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HfMeta {
    /// HF Hub `/api/models/{repo}` 응답의 downloads 필드.
    pub downloads: u64,
    /// likes (heart) 카운트.
    pub likes: u64,
    /// lastModified RFC3339.
    pub last_modified: String,
}

/// Manifest 항목 검증 에러 — Phase 11'.a (ADR-0048).
///
/// `validate_entry`가 반환. build script와 통합 테스트 양쪽에서 사용.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ManifestValidationError {
    #[error("의도 ID '{0}'은(는) 사전에 등록되지 않았어요")]
    UnknownIntent(String),
    #[error("도메인 점수 '{intent}'={score}는 0..=100 범위를 벗어나요")]
    ScoreOutOfRange { intent: String, score: f32 },
    #[error("의도 ID '{0}'이(가) 중복돼 있어요")]
    DuplicateIntent(String),
}

/// 한 entry의 intents + domain_scores 무결성 검증. (Phase 11'.a, ADR-0048)
///
/// 검증:
/// 1. `intents`의 모든 ID가 `INTENT_VOCABULARY`에 등록.
/// 2. `intents` 내 중복 없음.
/// 3. `domain_scores`의 모든 key가 `INTENT_VOCABULARY`에 등록.
/// 4. `domain_scores`의 모든 value가 `0.0..=100.0` 범위.
pub fn validate_entry(entry: &ModelEntry) -> Result<(), ManifestValidationError> {
    use std::collections::HashSet;
    let mut seen: HashSet<&str> = HashSet::new();
    for iid in &entry.intents {
        if !is_registered_intent(iid) {
            return Err(ManifestValidationError::UnknownIntent(iid.clone()));
        }
        if !seen.insert(iid.as_str()) {
            return Err(ManifestValidationError::DuplicateIntent(iid.clone()));
        }
    }
    for (iid, score) in &entry.domain_scores {
        if !is_registered_intent(iid) {
            return Err(ManifestValidationError::UnknownIntent(iid.clone()));
        }
        if !(0.0..=100.0).contains(score) {
            return Err(ManifestValidationError::ScoreOutOfRange {
                intent: iid.clone(),
                score: *score,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_info_defaults_to_community() {
        let v = VerificationInfo::default();
        assert_eq!(v.tier, VerificationTier::Community);
        assert!(v.verified_at.is_none());
        assert!(v.verified_by.is_none());
    }

    // ── Phase 13'.e.1 — ModelTier + CommunityInsights invariants ──────

    #[test]
    fn model_tier_default_is_verified() {
        let t = ModelTier::default();
        assert_eq!(t, ModelTier::Verified);
    }

    #[test]
    fn model_tier_round_trip_kebab_case() {
        for (tier, expected) in [
            (ModelTier::New, "new"),
            (ModelTier::Verified, "verified"),
            (ModelTier::Experimental, "experimental"),
            (ModelTier::Deprecated, "deprecated"),
        ] {
            let v = serde_json::to_value(tier).unwrap();
            assert_eq!(v.as_str(), Some(expected), "serialize {tier:?}");
            let parsed: ModelTier = serde_json::from_value(v).unwrap();
            assert_eq!(parsed, tier, "round-trip {tier:?}");
        }
    }

    #[test]
    fn community_insights_default_empty() {
        let ci = CommunityInsights::default();
        assert!(ci.strengths_ko.is_empty());
        assert!(ci.weaknesses_ko.is_empty());
        assert!(ci.use_cases_ko.is_empty());
        assert_eq!(ci.curator_note_ko, "");
        assert!(ci.sources.is_empty());
        assert!(ci.last_reviewed_at.is_none());
    }

    #[test]
    fn legacy_entry_without_tier_or_insights_still_parses() {
        // 기존 12 entries는 tier/community_insights 없이 작성됨 — schema bump 없이 호환.
        let json = r#"{
            "schema_version": 1,
            "generated_at": "2026-04-27T00:00:00Z",
            "entries": [{
                "id": "legacy",
                "display_name": "Legacy",
                "category": "agent-general",
                "model_family": "x",
                "source": {"type": "direct-url", "url": "https://x"},
                "runner_compatibility": ["llama-cpp"],
                "quantization_options": [],
                "min_vram_mb": null,
                "rec_vram_mb": null,
                "min_ram_mb": 1024,
                "rec_ram_mb": 2048,
                "install_size_mb": 100,
                "tool_support": false,
                "vision_support": false,
                "structured_output_support": false,
                "license": "MIT",
                "maturity": "stable",
                "portable_suitability": 5,
                "on_device_suitability": 5,
                "fine_tune_suitability": 5
            }]
        }"#;
        let m: ModelManifest = serde_json::from_str(json).unwrap();
        let e = &m.entries[0];
        // 누락된 필드는 default 폴백 — verified + None + 빈 컬렉션.
        assert_eq!(e.tier, ModelTier::Verified);
        assert!(e.community_insights.is_none());
        // Phase 11'.a — legacy entries는 intents/domain_scores 없이도 호환.
        assert!(e.intents.is_empty());
        assert!(e.domain_scores.is_empty());
    }

    #[test]
    fn manifest_round_trip_with_minimal_entry() {
        let m = ModelManifest {
            schema_version: 1,
            generated_at: "2026-04-27T00:00:00Z".into(),
            entries: vec![ModelEntry {
                id: "test-model".into(),
                display_name: "Test Model".into(),
                category: ModelCategory::AgentGeneral,
                model_family: "test".into(),
                source: ModelSource::HuggingFace {
                    repo: "test/model".into(),
                    file: None,
                },
                runner_compatibility: vec![RuntimeKind::LlamaCpp],
                quantization_options: vec![],
                min_vram_mb: None,
                rec_vram_mb: None,
                min_ram_mb: 4096,
                rec_ram_mb: 8192,
                install_size_mb: 100,
                context_guidance: None,
                language_strength: None,
                roleplay_strength: None,
                coding_strength: None,
                tool_support: false,
                vision_support: false,
                structured_output_support: false,
                license: "Apache-2.0".into(),
                maturity: Maturity::Stable,
                portable_suitability: 5,
                on_device_suitability: 5,
                fine_tune_suitability: 5,
                verification: VerificationInfo::default(),
                hf_meta: None,
                use_case_examples: vec![],
                notes: None,
                warnings: vec![],
                hub_id: None,
                tier: ModelTier::default(),
                community_insights: None,
                intents: vec![],
                domain_scores: BTreeMap::new(),
                purpose: ModelPurpose::default(),
                commercial: true,
                content_warning: None,
                mmproj: None,
            }],
        };
        let s = serde_json::to_string(&m).unwrap();
        let m2: ModelManifest = serde_json::from_str(&s).unwrap();
        assert_eq!(m2.entries.len(), 1);
        assert_eq!(m2.entries[0].id, "test-model");
        assert_eq!(m2.entries[0].verification.tier, VerificationTier::Community);
        assert!(m2.entries[0].intents.is_empty());
        assert!(m2.entries[0].domain_scores.is_empty());
        assert!(m2.entries[0].mmproj.is_none());
    }

    // ── Phase 13'.h.2.c — MmprojSpec invariants (ADR-0051) ─────────────

    #[test]
    fn legacy_entry_without_mmproj_still_parses() {
        // 기존 39 entries는 mmproj 없이 작성됨 — schema bump 없이 호환.
        let json = r#"{
            "schema_version": 1,
            "generated_at": "2026-05-03T00:00:00Z",
            "entries": [{
                "id": "legacy-vision",
                "display_name": "Legacy Vision",
                "category": "agent-general",
                "model_family": "x",
                "source": {"type": "direct-url", "url": "https://x"},
                "runner_compatibility": ["llama-cpp"],
                "quantization_options": [],
                "min_ram_mb": 4096,
                "rec_ram_mb": 8192,
                "install_size_mb": 2000,
                "tool_support": true,
                "vision_support": true,
                "structured_output_support": false,
                "license": "MIT",
                "maturity": "stable",
                "portable_suitability": 5,
                "on_device_suitability": 5,
                "fine_tune_suitability": 5
            }]
        }"#;
        let m: ModelManifest = serde_json::from_str(json).unwrap();
        // mmproj 누락 시 None 폴백.
        assert!(m.entries[0].mmproj.is_none());
        assert!(m.entries[0].vision_support);
    }

    #[test]
    fn mmproj_spec_round_trip_full_fields() {
        let spec = MmprojSpec {
            url: "https://huggingface.co/ggml-org/gemma-3-4b-it-GGUF/resolve/main/mmproj-model-f16.gguf".into(),
            sha256: Some("a".repeat(64)),
            size_mb: 851,
            precision: Some("f16".into()),
            source: Some("ggml-org".into()),
        };
        let s = serde_json::to_string(&spec).unwrap();
        let back: MmprojSpec = serde_json::from_str(&s).unwrap();
        assert_eq!(back.url, spec.url);
        assert_eq!(back.sha256, spec.sha256);
        assert_eq!(back.size_mb, spec.size_mb);
        assert_eq!(back.precision, spec.precision);
        assert_eq!(back.source, spec.source);
    }

    #[test]
    fn mmproj_spec_minimal_fields_parses() {
        // sha256/precision/source 누락 — None 폴백.
        let json = r#"{"url": "https://huggingface.co/x/mmproj.gguf", "size_mb": 500}"#;
        let spec: MmprojSpec = serde_json::from_str(json).unwrap();
        assert!(spec.sha256.is_none());
        assert!(spec.precision.is_none());
        assert!(spec.source.is_none());
        assert_eq!(spec.size_mb, 500);
    }

    #[test]
    fn entry_with_mmproj_round_trip() {
        let json = r#"{
            "id": "gemma-3-4b",
            "display_name": "Gemma 3 4B",
            "category": "agent-general",
            "model_family": "gemma-3",
            "source": {"type": "hugging-face", "repo": "google/gemma-3-4b-it"},
            "runner_compatibility": ["ollama", "lm-studio", "llama-cpp"],
            "quantization_options": [],
            "min_ram_mb": 8192,
            "rec_ram_mb": 16384,
            "install_size_mb": 2500,
            "tool_support": true,
            "vision_support": true,
            "structured_output_support": true,
            "license": "Gemma Terms of Use",
            "maturity": "stable",
            "portable_suitability": 7,
            "on_device_suitability": 8,
            "fine_tune_suitability": 6,
            "mmproj": {
                "url": "https://huggingface.co/ggml-org/gemma-3-4b-it-GGUF/resolve/main/mmproj-model-f16.gguf",
                "sha256": null,
                "size_mb": 851,
                "precision": "f16",
                "source": "ggml-org"
            }
        }"#;
        let entry: ModelEntry = serde_json::from_str(json).unwrap();
        let mmproj = entry.mmproj.expect("mmproj 필드 deserialize");
        assert_eq!(mmproj.size_mb, 851);
        assert_eq!(mmproj.precision.as_deref(), Some("f16"));
        assert_eq!(mmproj.source.as_deref(), Some("ggml-org"));
        assert!(mmproj.sha256.is_none());
        assert!(mmproj.url.starts_with("https://huggingface.co/"));
    }

    #[test]
    fn manifest_parses_legacy_entry_without_optional_fields() {
        // 기존 시드(없는 verification/hf_meta/use_case_examples) 호환성 — schema-now-data-later.
        let json = r#"{
            "schema_version": 1,
            "generated_at": "2026-04-26T00:00:00Z",
            "entries": [{
                "id": "legacy",
                "display_name": "Legacy",
                "category": "slm",
                "model_family": "x",
                "source": { "type": "direct-url", "url": "https://example.com/m.gguf" },
                "runner_compatibility": ["llama-cpp"],
                "quantization_options": [],
                "min_ram_mb": 2048,
                "rec_ram_mb": 4096,
                "install_size_mb": 500,
                "tool_support": false,
                "vision_support": false,
                "structured_output_support": false,
                "license": "MIT",
                "maturity": "beta",
                "portable_suitability": 5,
                "on_device_suitability": 5,
                "fine_tune_suitability": 5
            }]
        }"#;
        let m: ModelManifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.entries[0].verification.tier, VerificationTier::Community);
        assert!(m.entries[0].hf_meta.is_none());
        assert!(m.entries[0].use_case_examples.is_empty());
    }

    #[test]
    fn verification_tier_round_trip() {
        let v = VerificationInfo {
            tier: VerificationTier::Verified,
            verified_at: Some("2026-04-27".into()),
            verified_by: Some("lmmaster-curator".into()),
        };
        let s = serde_json::to_value(&v).unwrap();
        assert_eq!(s["tier"], "verified");
        let v2: VerificationInfo = serde_json::from_value(s).unwrap();
        assert_eq!(v2.tier, VerificationTier::Verified);
    }

    #[test]
    fn hf_meta_round_trip() {
        let h = HfMeta {
            downloads: 12345,
            likes: 67,
            last_modified: "2026-04-20T12:00:00Z".into(),
        };
        let s = serde_json::to_string(&h).unwrap();
        let h2: HfMeta = serde_json::from_str(&s).unwrap();
        assert_eq!(h2.downloads, 12345);
        assert_eq!(h2.likes, 67);
    }

    // ── Phase 11'.a — intents + domain_scores invariants (ADR-0048) ──────

    /// 테스트 헬퍼 — 기본 ModelEntry (검증을 위해 필요한 최소).
    fn sample_entry() -> ModelEntry {
        ModelEntry {
            id: "sample".into(),
            display_name: "Sample".into(),
            category: ModelCategory::AgentGeneral,
            model_family: "x".into(),
            source: ModelSource::HuggingFace {
                repo: "x/y".into(),
                file: None,
            },
            runner_compatibility: vec![RuntimeKind::LlamaCpp],
            quantization_options: vec![],
            min_vram_mb: None,
            rec_vram_mb: None,
            min_ram_mb: 1024,
            rec_ram_mb: 2048,
            install_size_mb: 100,
            context_guidance: None,
            language_strength: None,
            roleplay_strength: None,
            coding_strength: None,
            tool_support: false,
            vision_support: false,
            structured_output_support: false,
            license: "MIT".into(),
            maturity: Maturity::Stable,
            portable_suitability: 5,
            on_device_suitability: 5,
            fine_tune_suitability: 5,
            hub_id: None,
            tier: ModelTier::default(),
            community_insights: None,
            verification: VerificationInfo::default(),
            hf_meta: None,
            use_case_examples: vec![],
            notes: None,
            warnings: vec![],
            intents: vec![],
            domain_scores: BTreeMap::new(),
            purpose: ModelPurpose::default(),
            commercial: true,
            content_warning: None,
            mmproj: None,
        }
    }

    #[test]
    fn validate_accepts_empty_intents_and_scores() {
        // legacy/시드 미백필 entry — 빈 컬렉션은 항상 valid.
        assert_eq!(validate_entry(&sample_entry()), Ok(()));
    }

    #[test]
    fn validate_accepts_registered_intents() {
        let mut e = sample_entry();
        e.intents = vec!["vision-image".into(), "ko-conversation".into()];
        e.domain_scores.insert("vision-image".into(), 53.7);
        e.domain_scores.insert("ko-conversation".into(), 78.0);
        assert_eq!(validate_entry(&e), Ok(()));
    }

    #[test]
    fn validate_rejects_unknown_intent_in_intents() {
        let mut e = sample_entry();
        e.intents = vec!["vision-image".into(), "made-up".into()];
        assert_eq!(
            validate_entry(&e),
            Err(ManifestValidationError::UnknownIntent("made-up".into()))
        );
    }

    #[test]
    fn validate_rejects_unknown_intent_in_scores() {
        let mut e = sample_entry();
        e.domain_scores.insert("not-a-real-intent".into(), 50.0);
        assert_eq!(
            validate_entry(&e),
            Err(ManifestValidationError::UnknownIntent(
                "not-a-real-intent".into()
            ))
        );
    }

    #[test]
    fn validate_rejects_score_out_of_range() {
        let mut e = sample_entry();
        e.domain_scores.insert("vision-image".into(), 120.5);
        assert_eq!(
            validate_entry(&e),
            Err(ManifestValidationError::ScoreOutOfRange {
                intent: "vision-image".into(),
                score: 120.5,
            })
        );

        let mut e2 = sample_entry();
        e2.domain_scores.insert("vision-image".into(), -1.0);
        assert!(matches!(
            validate_entry(&e2),
            Err(ManifestValidationError::ScoreOutOfRange { .. })
        ));
    }

    #[test]
    fn validate_rejects_duplicate_intent() {
        let mut e = sample_entry();
        e.intents = vec!["vision-image".into(), "vision-image".into()];
        assert_eq!(
            validate_entry(&e),
            Err(ManifestValidationError::DuplicateIntent(
                "vision-image".into()
            ))
        );
    }

    #[test]
    fn validate_score_boundary_zero_and_hundred() {
        let mut e = sample_entry();
        e.domain_scores.insert("vision-image".into(), 0.0);
        e.domain_scores.insert("ko-rag".into(), 100.0);
        assert_eq!(validate_entry(&e), Ok(()));
    }

    #[test]
    fn round_trip_with_intents_and_scores() {
        let mut e = sample_entry();
        e.intents = vec!["vision-image".into(), "ko-rag".into()];
        e.domain_scores.insert("vision-image".into(), 53.7);
        e.domain_scores.insert("ko-rag".into(), 67.4);

        let json = serde_json::to_string(&e).unwrap();
        let parsed: ModelEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.intents, vec!["vision-image", "ko-rag"]);
        assert_eq!(parsed.domain_scores.get("vision-image"), Some(&53.7));
        assert_eq!(parsed.domain_scores.get("ko-rag"), Some(&67.4));
    }

    // ── Phase 13'.f.2 — ModelPurpose invariants ──────────────────────

    #[test]
    fn model_purpose_default_is_general_chat() {
        assert_eq!(ModelPurpose::default(), ModelPurpose::GeneralChat);
    }

    #[test]
    fn model_purpose_round_trip_kebab_case() {
        for (purpose, expected) in [
            (ModelPurpose::GeneralChat, "general-chat"),
            (ModelPurpose::FineTuneBase, "fine-tune-base"),
            (ModelPurpose::Retrieval, "retrieval"),
            (ModelPurpose::Reranker, "reranker"),
        ] {
            let v = serde_json::to_value(purpose).unwrap();
            assert_eq!(v.as_str(), Some(expected), "serialize {purpose:?}");
            let parsed: ModelPurpose = serde_json::from_value(v).unwrap();
            assert_eq!(parsed, purpose, "round-trip {purpose:?}");
        }
    }

    #[test]
    fn legacy_entry_without_purpose_falls_back_to_general_chat() {
        let json = r#"{
            "schema_version": 1,
            "generated_at": "2026-04-27T00:00:00Z",
            "entries": [{
                "id": "legacy",
                "display_name": "Legacy",
                "category": "agent-general",
                "model_family": "x",
                "source": {"type": "direct-url", "url": "https://x"},
                "runner_compatibility": ["llama-cpp"],
                "quantization_options": [],
                "min_vram_mb": null,
                "rec_vram_mb": null,
                "min_ram_mb": 1024,
                "rec_ram_mb": 2048,
                "install_size_mb": 100,
                "tool_support": false,
                "vision_support": false,
                "structured_output_support": false,
                "license": "MIT",
                "maturity": "stable",
                "portable_suitability": 5,
                "on_device_suitability": 5,
                "fine_tune_suitability": 5
            }]
        }"#;
        let m: ModelManifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.entries[0].purpose, ModelPurpose::GeneralChat);
    }

    #[test]
    fn validation_error_messages_are_korean() {
        let err = ManifestValidationError::UnknownIntent("x".into());
        let msg = err.to_string();
        assert!(msg.contains("의도 ID"), "message: {msg}");

        let err = ManifestValidationError::ScoreOutOfRange {
            intent: "vision-image".into(),
            score: 200.0,
        };
        let msg = err.to_string();
        assert!(msg.contains("도메인 점수"), "message: {msg}");
        assert!(msg.contains("범위"), "message: {msg}");
    }
}
