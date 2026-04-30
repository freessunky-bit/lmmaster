//! Manifest 스키마 — ADR-0014 + Phase 2'.a 보강.
//!
//! 정책:
//! - VerificationInfo: 2-tier governance (Verified/Community). 기본 Community.
//! - HfMeta: schema-now-data-later — v1 시드는 비움, v1.1에서 HF Hub API로 채움.
//! - use_case_examples: Workbench 프롬프트 시드용 한국어 자연어 예시.

use serde::{Deserialize, Serialize};
use shared_types::{ModelCategory, RuntimeKind};

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

    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
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
        // 누락된 필드는 default 폴백 — verified + None.
        assert_eq!(e.tier, ModelTier::Verified);
        assert!(e.community_insights.is_none());
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
            }],
        };
        let s = serde_json::to_string(&m).unwrap();
        let m2: ModelManifest = serde_json::from_str(&s).unwrap();
        assert_eq!(m2.entries.len(), 1);
        assert_eq!(m2.entries[0].id, "test-model");
        assert_eq!(m2.entries[0].verification.tier, VerificationTier::Community);
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
}
