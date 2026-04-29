//! crate: preset-registry — Korean preset 100+ 매니페스트 로더 (Phase 4.h).
//!
//! 정책 (phase-4-screens-decision.md §1.2):
//! - 7 카테고리 × 14~16 = 100+ presets.
//! - JSON 매니페스트 `manifests/presets/{category}/{slug}.json`.
//! - Verified / community 2-tier (Phase 2'.a과 동일 거버넌스).
//! - 의료/법률은 disclaimer 의무 — system_prompt에 disclaimer 텍스트 포함 검증.
//! - recommended_models[]는 카탈로그 entry id (build-time cross-link 검증 권장).

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum PresetCategory {
    Coding,
    Translation,
    Legal,
    Marketing,
    Medical,
    Education,
    Research,
}

impl PresetCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Coding => "coding",
            Self::Translation => "translation",
            Self::Legal => "legal",
            Self::Marketing => "marketing",
            Self::Medical => "medical",
            Self::Education => "education",
            Self::Research => "research",
        }
    }

    pub fn all() -> &'static [PresetCategory] {
        &[
            Self::Coding,
            Self::Translation,
            Self::Legal,
            Self::Marketing,
            Self::Medical,
            Self::Education,
            Self::Research,
        ]
    }

    /// disclaimer 의무 카테고리 (의료/법률).
    pub fn requires_disclaimer(&self) -> bool {
        matches!(self, Self::Legal | Self::Medical)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum VerificationTier {
    Verified,
    #[default]
    Community,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub version: String,
    pub category: PresetCategory,
    pub display_name_ko: String,
    pub subtitle_ko: String,
    pub system_prompt_ko: String,
    pub user_template_ko: String,
    pub example_user_message_ko: String,
    pub example_assistant_message_ko: String,
    pub recommended_models: Vec<String>,
    #[serde(default)]
    pub fallback_models: Vec<String>,
    pub min_context_tokens: u32,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub verification: VerificationTier,
    pub license: String,
}

#[derive(Debug, Error)]
pub enum PresetError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json parse {path}: {source}")]
    Json {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("disclaimer 의무 카테고리({category:?})인데 system_prompt에 disclaimer 누락: {id}")]
    MissingDisclaimer {
        id: String,
        category: PresetCategory,
    },
    #[error("preset id가 카테고리 prefix와 일치하지 않아요: {id} (category={category:?})")]
    IdMismatch {
        id: String,
        category: PresetCategory,
    },
}

const DISCLAIMER_KEYWORDS: &[&str] = &["disclaimer", "변호사", "전문가 상담", "정확한 진단"];

fn ensure_disclaimer(preset: &Preset) -> Result<(), PresetError> {
    if !preset.category.requires_disclaimer() {
        return Ok(());
    }
    let lower = preset.system_prompt_ko.to_lowercase();
    let has = DISCLAIMER_KEYWORDS
        .iter()
        .any(|kw| lower.contains(&kw.to_lowercase()));
    if !has {
        return Err(PresetError::MissingDisclaimer {
            id: preset.id.clone(),
            category: preset.category,
        });
    }
    Ok(())
}

fn ensure_id_prefix(preset: &Preset) -> Result<(), PresetError> {
    let prefix = format!("{}/", preset.category.as_str());
    if !preset.id.starts_with(&prefix) {
        return Err(PresetError::IdMismatch {
            id: preset.id.clone(),
            category: preset.category,
        });
    }
    Ok(())
}

/// 디렉터리에서 모든 `.json` preset을 로드. 검증 실패 시 첫 에러로 종료.
pub fn load_all(dir: &Path) -> Result<Vec<Preset>, PresetError> {
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    visit(dir, &mut out)?;
    for p in &out {
        ensure_id_prefix(p)?;
        ensure_disclaimer(p)?;
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

fn visit(dir: &Path, out: &mut Vec<Preset>) -> Result<(), PresetError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            visit(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let body = std::fs::read_to_string(&path)?;
            let preset: Preset = serde_json::from_str(&body).map_err(|e| PresetError::Json {
                path: path.display().to_string(),
                source: e,
            })?;
            out.push(preset);
        }
    }
    Ok(())
}

/// `recommended_models[]`가 카탈로그에 존재하는지 cross-link 검증.
pub fn validate_cross_links(presets: &[Preset], known_models: &[String]) -> Vec<String> {
    let mut errors = Vec::new();
    for p in presets {
        for m in &p.recommended_models {
            if !known_models.contains(m) {
                errors.push(format!(
                    "preset '{}' 추천 모델 '{}' 카탈로그 미존재",
                    p.id, m
                ));
            }
        }
    }
    errors
}

/// 카테고리 별 preset 그룹화.
pub fn group_by_category(
    presets: &[Preset],
) -> std::collections::HashMap<PresetCategory, Vec<&Preset>> {
    let mut map = std::collections::HashMap::new();
    for p in presets {
        map.entry(p.category).or_insert_with(Vec::new).push(p);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_preset(id: &str, cat: PresetCategory, prompt: &str) -> Preset {
        Preset {
            id: id.into(),
            version: "2026-04-27.1".into(),
            category: cat,
            display_name_ko: "샘플".into(),
            subtitle_ko: "샘플 subtitle".into(),
            system_prompt_ko: prompt.into(),
            user_template_ko: "{{user_input}}".into(),
            example_user_message_ko: "예시 입력".into(),
            example_assistant_message_ko: "예시 응답".into(),
            recommended_models: vec!["exaone-4.0-1.2b-instruct".into()],
            fallback_models: vec![],
            min_context_tokens: 4096,
            tags: vec![],
            verification: VerificationTier::Community,
            license: "CC0-1.0".into(),
        }
    }

    #[test]
    fn category_str_and_all() {
        assert_eq!(PresetCategory::Coding.as_str(), "coding");
        assert_eq!(PresetCategory::all().len(), 7);
    }

    #[test]
    fn requires_disclaimer_legal_medical_only() {
        assert!(PresetCategory::Legal.requires_disclaimer());
        assert!(PresetCategory::Medical.requires_disclaimer());
        assert!(!PresetCategory::Coding.requires_disclaimer());
    }

    #[test]
    fn ensure_disclaimer_passes_when_present() {
        let p = make_preset(
            "legal/x",
            PresetCategory::Legal,
            "이 답변은 일반 정보예요. 실제 계약은 변호사 상담을 권해드려요.",
        );
        ensure_disclaimer(&p).unwrap();
    }

    #[test]
    fn ensure_disclaimer_fails_when_missing() {
        let p = make_preset("legal/x", PresetCategory::Legal, "법률 조항을 검토합니다.");
        let err = ensure_disclaimer(&p).unwrap_err();
        assert!(matches!(err, PresetError::MissingDisclaimer { .. }));
    }

    #[test]
    fn ensure_id_prefix_validates() {
        let ok = make_preset("coding/x", PresetCategory::Coding, "");
        ensure_id_prefix(&ok).unwrap();

        let bad = make_preset("translation/x", PresetCategory::Coding, "");
        let err = ensure_id_prefix(&bad).unwrap_err();
        assert!(matches!(err, PresetError::IdMismatch { .. }));
    }

    #[test]
    fn load_all_empty_dir_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let r = load_all(tmp.path()).unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn load_all_recurses_and_sorts() {
        let tmp = tempfile::tempdir().unwrap();
        let coding = tmp.path().join("coding");
        fs::create_dir(&coding).unwrap();
        let p1 = make_preset("coding/zebra", PresetCategory::Coding, "");
        let p2 = make_preset("coding/alpha", PresetCategory::Coding, "");
        fs::write(
            coding.join("zebra.json"),
            serde_json::to_string(&p1).unwrap(),
        )
        .unwrap();
        fs::write(
            coding.join("alpha.json"),
            serde_json::to_string(&p2).unwrap(),
        )
        .unwrap();
        let r = load_all(tmp.path()).unwrap();
        assert_eq!(r.len(), 2);
        // alphabetical id order.
        assert_eq!(r[0].id, "coding/alpha");
        assert_eq!(r[1].id, "coding/zebra");
    }

    #[test]
    fn load_all_rejects_legal_without_disclaimer() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("legal");
        fs::create_dir(&dir).unwrap();
        let bad = make_preset("legal/x", PresetCategory::Legal, "법률 조항을 검토합니다.");
        fs::write(dir.join("x.json"), serde_json::to_string(&bad).unwrap()).unwrap();
        let err = load_all(tmp.path()).unwrap_err();
        assert!(matches!(err, PresetError::MissingDisclaimer { .. }));
    }

    #[test]
    fn validate_cross_links_returns_error_for_unknown_model() {
        let p = make_preset("coding/x", PresetCategory::Coding, "");
        let known = vec!["other-model".to_string()];
        let errs = validate_cross_links(&[p], &known);
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("미존재"));
    }

    #[test]
    fn validate_cross_links_passes_when_all_known() {
        let p = make_preset("coding/x", PresetCategory::Coding, "");
        let known = vec!["exaone-4.0-1.2b-instruct".to_string()];
        assert!(validate_cross_links(&[p], &known).is_empty());
    }

    #[test]
    fn group_by_category_groups_correctly() {
        let p1 = make_preset("coding/x", PresetCategory::Coding, "");
        let p2 = make_preset("translation/y", PresetCategory::Translation, "");
        let p3 = make_preset("coding/z", PresetCategory::Coding, "");
        let presets = vec![p1, p2, p3];
        let g = group_by_category(&presets);
        assert_eq!(g.get(&PresetCategory::Coding).unwrap().len(), 2);
        assert_eq!(g.get(&PresetCategory::Translation).unwrap().len(), 1);
    }
}
