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
