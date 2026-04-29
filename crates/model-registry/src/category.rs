//! 카테고리 필터 헬퍼.
//!
//! 정책: ModelCategory 기준으로 ModelEntry 슬라이스를 필터링.
//! 빈 결과도 정상 — UI에서 "비어있어요" 처리.

use shared_types::ModelCategory;

use crate::manifest::ModelEntry;

/// `category`에 정확히 일치하는 엔트리만 반환.
pub fn filter_by_category(entries: &[ModelEntry], category: ModelCategory) -> Vec<&ModelEntry> {
    entries.iter().filter(|e| e.category == category).collect()
}

/// 모든 카테고리 별 엔트리 카운트 — UI 카운트 배지용.
pub fn count_by_category(
    entries: &[ModelEntry],
) -> std::collections::HashMap<ModelCategory, usize> {
    let mut map = std::collections::HashMap::new();
    for e in entries {
        *map.entry(e.category).or_insert(0) += 1;
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::*;
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
            verification: VerificationInfo::default(),
            hf_meta: None,
            use_case_examples: vec![],
            notes: None,
            warnings: vec![],
        }
    }

    #[test]
    fn filter_by_category_returns_only_matches() {
        let entries = vec![
            make_entry("a", ModelCategory::AgentGeneral),
            make_entry("b", ModelCategory::Coding),
            make_entry("c", ModelCategory::AgentGeneral),
        ];
        let agents = filter_by_category(&entries, ModelCategory::AgentGeneral);
        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0].id, "a");
        assert_eq!(agents[1].id, "c");
    }

    #[test]
    fn filter_empty_returns_empty() {
        let entries = vec![make_entry("a", ModelCategory::AgentGeneral)];
        assert!(filter_by_category(&entries, ModelCategory::Roleplay).is_empty());
    }

    #[test]
    fn count_by_category_groups_correctly() {
        let entries = vec![
            make_entry("a", ModelCategory::AgentGeneral),
            make_entry("b", ModelCategory::Coding),
            make_entry("c", ModelCategory::AgentGeneral),
        ];
        let counts = count_by_category(&entries);
        assert_eq!(counts.get(&ModelCategory::AgentGeneral), Some(&2));
        assert_eq!(counts.get(&ModelCategory::Coding), Some(&1));
    }
}
