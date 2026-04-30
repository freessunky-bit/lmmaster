//! crate: model-registry — curated manifest 로더 + Catalog + Recommender.
//!
//! 정책 (ADR-0014, Phase 2'.a):
//! - bundled snapshot이 1차 신뢰 소스 (manifests/snapshot/models/).
//! - 사용자 overlay (workspace/manifests/) — 같은 id면 덮어씀.
//! - HF Hub API는 메타 보강용 (downloads/likes/last_modified) — v1.1.
//! - Recommender는 deterministic — 같은 (PC, catalog) → 같은 추천.

pub mod cache;
pub mod category;
pub mod manifest;
pub mod recommender;
pub mod register;
pub mod sync;

use std::path::Path;

use serde::{Deserialize, Serialize};
use shared_types::{HostFingerprint, ModelCategory};

pub use cache::CacheError;
pub use manifest::{
    CommunityInsights, ContentWarning, HfMeta, Maturity, ModelEntry, ModelManifest, ModelPurpose,
    ModelSource, ModelTier, QuantOption, VerificationInfo, VerificationTier,
};
pub use recommender::{compute as compute_recommendation, ExclusionReason, Recommendation};
pub use register::{CustomModel, ModelRegistry, ModelRegistryError};

/// 카탈로그 전체 — entries는 snapshot + overlay 머지된 결과.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    entries: Vec<ModelEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogView {
    pub entries: Vec<ModelEntry>,
    pub recommendation: Option<Recommendation>,
}

impl Catalog {
    /// 단일 디렉터리에서 로드.
    pub fn load_from_dir(dir: &Path) -> Result<Self, CacheError> {
        Ok(Self {
            entries: cache::load_from_dir(dir)?,
        })
    }

    /// snapshot(번들) + overlay(사용자) 두 디렉터리에서 로드 후 머지.
    pub fn load_layered(snapshot_dir: &Path, overlay_dir: &Path) -> Result<Self, CacheError> {
        let snap = cache::load_from_dir(snapshot_dir)?;
        let over = cache::load_from_dir(overlay_dir)?;
        Ok(Self {
            entries: cache::load_layered(snap, over),
        })
    }

    /// 직접 entries로 생성 — 테스트용.
    pub fn from_entries(entries: Vec<ModelEntry>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &[ModelEntry] {
        &self.entries
    }

    /// 카테고리 필터 — `category=None`이면 전체.
    pub fn filter(&self, category: Option<ModelCategory>) -> Vec<&ModelEntry> {
        match category {
            Some(c) => category::filter_by_category(&self.entries, c),
            None => self.entries.iter().collect(),
        }
    }

    /// 추천 — Deterministic. 기존 caller 호환 wrapper (의도 없음).
    pub fn recommend(&self, host: &HostFingerprint, target: ModelCategory) -> Recommendation {
        self.recommend_with_intent(host, target, None)
    }

    /// 추천 (의도 가중). Deterministic. (Phase 11'.b, ADR-0048)
    ///
    /// `intent`는 의도(intent picker) 신호 — `None`이면 `recommend(...)`와 동일 (backward compat).
    pub fn recommend_with_intent(
        &self,
        host: &HostFingerprint,
        target: ModelCategory,
        intent: Option<&shared_types::IntentId>,
    ) -> Recommendation {
        recommender::compute_with_intent(host, target, &self.entries, intent)
    }

    /// 카테고리 별 카운트 — UI 카운트 배지용.
    pub fn category_counts(&self) -> std::collections::HashMap<ModelCategory, usize> {
        category::count_by_category(&self.entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Maturity, ModelSource, VerificationInfo};
    use shared_types::RuntimeKind;

    fn make_entry(id: &str, cat: ModelCategory) -> ModelEntry {
        ModelEntry {
            id: id.into(),
            display_name: id.into(),
            category: cat,
            model_family: "x".into(),
            source: ModelSource::DirectUrl {
                url: "https://x".into(),
            },
            runner_compatibility: vec![RuntimeKind::LlamaCpp],
            quantization_options: vec![],
            min_vram_mb: None,
            rec_vram_mb: None,
            min_ram_mb: 1024,
            rec_ram_mb: 2048,
            install_size_mb: 100,
            context_guidance: None,
            language_strength: Some(5),
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
            verification: VerificationInfo::default(),
            hf_meta: None,
            use_case_examples: vec![],
            notes: None,
            warnings: vec![],
            hub_id: None,
            tier: crate::manifest::ModelTier::default(),
            community_insights: None,
            intents: vec![],
            domain_scores: Default::default(),
            purpose: Default::default(),
            commercial: true,
            content_warning: None,
        }
    }

    #[test]
    fn from_entries_preserves_order() {
        let cat = Catalog::from_entries(vec![
            make_entry("a", ModelCategory::AgentGeneral),
            make_entry("b", ModelCategory::Coding),
        ]);
        assert_eq!(cat.entries().len(), 2);
    }

    #[test]
    fn filter_none_returns_all() {
        let cat = Catalog::from_entries(vec![
            make_entry("a", ModelCategory::AgentGeneral),
            make_entry("b", ModelCategory::Coding),
        ]);
        assert_eq!(cat.filter(None).len(), 2);
    }

    #[test]
    fn filter_by_category() {
        let cat = Catalog::from_entries(vec![
            make_entry("a", ModelCategory::AgentGeneral),
            make_entry("b", ModelCategory::Coding),
        ]);
        let agents = cat.filter(Some(ModelCategory::AgentGeneral));
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].id, "a");
    }

    #[test]
    fn category_counts_groups() {
        let cat = Catalog::from_entries(vec![
            make_entry("a", ModelCategory::AgentGeneral),
            make_entry("b", ModelCategory::AgentGeneral),
            make_entry("c", ModelCategory::Coding),
        ]);
        let counts = cat.category_counts();
        assert_eq!(counts.get(&ModelCategory::AgentGeneral), Some(&2));
    }

    #[test]
    fn recommend_returns_some_choice() {
        let cat = Catalog::from_entries(vec![make_entry("a", ModelCategory::AgentGeneral)]);
        let host = HostFingerprint {
            os: "windows".into(),
            arch: "x86_64".into(),
            cpu: "test".into(),
            ram_mb: 16384,
            gpu_vendor: None,
            gpu_model: None,
            vram_mb: None,
        };
        let r = cat.recommend(&host, ModelCategory::AgentGeneral);
        assert_eq!(r.best_choice.as_deref(), Some("a"));
    }
}
