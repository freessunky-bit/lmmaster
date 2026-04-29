//! preset-registry 통합 테스트 — manifests/presets/ 5 sample 로드.

use std::path::PathBuf;

use preset_registry::{group_by_category, load_all, validate_cross_links, PresetCategory};

fn presets_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p.push("manifests/presets");
    p
}

#[test]
fn snapshot_loads_full_korean_preset_library() {
    let dir = presets_dir();
    assert!(dir.exists(), "presets dir not found: {}", dir.display());
    let presets = load_all(&dir).expect("snapshot must load");
    let ids: Vec<&str> = presets.iter().map(|p| p.id.as_str()).collect();
    // 5 sample은 항상 포함.
    assert!(ids.contains(&"coding/refactor-extract-method"));
    assert!(ids.contains(&"translation/ko-en-tech"));
    assert!(ids.contains(&"legal/contract-clause-review"));
    assert!(ids.contains(&"marketing/instagram-copy"));
    assert!(ids.contains(&"education/middleschool-math-tutor"));
    // Phase 4.h 잔여 — 7 카테고리 × ~14 = 99+ presets (목표 100+).
    assert!(
        presets.len() >= 99,
        "expected >= 99 presets, found {} (목표 100+, 7 카테고리 × ~14)",
        presets.len()
    );
}

#[test]
fn legal_presets_include_disclaimer_keyword() {
    let presets = load_all(&presets_dir()).unwrap();
    for p in &presets {
        if p.category == PresetCategory::Legal {
            assert!(
                p.system_prompt_ko.contains("변호사") || p.system_prompt_ko.contains("disclaimer"),
                "legal preset {} missing disclaimer keyword",
                p.id
            );
        }
    }
}

#[test]
fn group_by_category_covers_loaded_presets() {
    let presets = load_all(&presets_dir()).unwrap();
    let groups = group_by_category(&presets);
    // Phase 4.h 잔여 — 7 카테고리 모두 채워져 있어야 함.
    assert!(groups.contains_key(&PresetCategory::Coding));
    assert!(groups.contains_key(&PresetCategory::Translation));
    assert!(groups.contains_key(&PresetCategory::Legal));
    assert!(groups.contains_key(&PresetCategory::Marketing));
    assert!(groups.contains_key(&PresetCategory::Education));
    assert!(groups.contains_key(&PresetCategory::Medical));
    assert!(groups.contains_key(&PresetCategory::Research));
}

#[test]
fn cross_link_validation_with_real_catalog_models() {
    let presets = load_all(&presets_dir()).unwrap();
    // Phase 2'.a 카탈로그 8 시드 모델 + 추가 (qwen 2.5 코더는 카탈로그에 있음).
    let known_models: Vec<String> = vec![
        "exaone-4.0-1.2b-instruct",
        "exaone-3.5-7.8b-instruct",
        "exaone-4.0-32b-instruct",
        "hcx-seed-8b",
        "polyglot-ko-12.8b",
        "qwen-2.5-coder-3b-instruct",
        "llama-3.2-3b-instruct",
        "whisper-large-v3-korean",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let errors = validate_cross_links(&presets, &known_models);
    assert!(errors.is_empty(), "cross-link errors: {:?}", errors);
}

#[test]
fn ids_alphabetical_within_each_category() {
    let presets = load_all(&presets_dir()).unwrap();
    let ids: Vec<&str> = presets.iter().map(|p| p.id.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted);
}
