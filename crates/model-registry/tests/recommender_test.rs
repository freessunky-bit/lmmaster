//! Recommender 통합 테스트 — Phase 2'.a 결정 노트의 5가지 보정 검증.
//!
//! 시나리오:
//! - host_low(8GB RAM, no GPU) → lightweight (소형 CPU friendly)
//! - host_mid(16GB RAM, RTX 3060 12GB) → balanced (7-8B 모델)
//! - host_high(64GB RAM, RTX 4090 24GB) → 32B/12.8B 모델
//! - host_tiny(4GB RAM, no GPU) → 가장 작은 모델만, lightweight=fallback
//! - 결정성 invariant — 동일 입력 → 동일 출력 100회 반복.
//! - id 충돌 → load_layered overlay가 덮어쓰기.
//! - 잘못된 카테고리에 fitness 0이 아닌 가중 점수.

use std::path::PathBuf;

use model_registry::{
    Catalog, ExclusionReason, Maturity, ModelEntry, ModelSource, QuantOption, VerificationInfo,
    VerificationTier,
};
use shared_types::{HostFingerprint, ModelCategory, RuntimeKind};

fn snapshot_dir() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // crates/
    p.pop(); // workspace root
    p.push("manifests/snapshot/models");
    p
}

fn host_low() -> HostFingerprint {
    HostFingerprint {
        os: "windows".into(),
        arch: "x86_64".into(),
        cpu: "Intel i5-8400".into(),
        ram_mb: 8 * 1024,
        gpu_vendor: None,
        gpu_model: None,
        vram_mb: None,
    }
}

fn host_mid() -> HostFingerprint {
    HostFingerprint {
        os: "windows".into(),
        arch: "x86_64".into(),
        cpu: "AMD Ryzen 5 5600X".into(),
        ram_mb: 16 * 1024,
        gpu_vendor: Some("nvidia".into()),
        gpu_model: Some("RTX 3060 12GB".into()),
        vram_mb: Some(12 * 1024),
    }
}

fn host_high() -> HostFingerprint {
    HostFingerprint {
        os: "windows".into(),
        arch: "x86_64".into(),
        cpu: "AMD Ryzen 9 7950X".into(),
        ram_mb: 64 * 1024,
        gpu_vendor: Some("nvidia".into()),
        gpu_model: Some("RTX 4090".into()),
        vram_mb: Some(24 * 1024),
    }
}

fn host_tiny() -> HostFingerprint {
    HostFingerprint {
        os: "linux".into(),
        arch: "x86_64".into(),
        cpu: "Atom".into(),
        ram_mb: 4 * 1024,
        gpu_vendor: None,
        gpu_model: None,
        vram_mb: None,
    }
}

fn make_entry(id: &str, cat: ModelCategory, install_mb: u64) -> ModelEntry {
    ModelEntry {
        id: id.into(),
        display_name: id.into(),
        category: cat,
        model_family: "x".into(),
        source: ModelSource::DirectUrl {
            url: "https://x".into(),
        },
        runner_compatibility: vec![RuntimeKind::LlamaCpp],
        quantization_options: vec![QuantOption {
            label: "Q4_K_M".into(),
            size_mb: install_mb,
            sha256: "0".repeat(64),
            file_path: None,
        }],
        min_vram_mb: None,
        rec_vram_mb: None,
        min_ram_mb: 1024,
        rec_ram_mb: 2048,
        install_size_mb: install_mb,
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
    }
}

#[test]
fn snapshot_loads_eight_seed_entries() {
    let dir = snapshot_dir();
    assert!(dir.exists(), "snapshot dir not found: {}", dir.display());
    let cat = Catalog::load_from_dir(&dir).expect("snapshot must parse");
    let ids: Vec<&str> = cat.entries().iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids.len(), 8, "expected 8 seed entries, got: {:?}", ids);
    assert!(ids.contains(&"exaone-4.0-1.2b-instruct"));
    assert!(ids.contains(&"exaone-3.5-7.8b-instruct"));
    assert!(ids.contains(&"exaone-4.0-32b-instruct"));
    assert!(ids.contains(&"hcx-seed-8b"));
    assert!(ids.contains(&"polyglot-ko-12.8b"));
    assert!(ids.contains(&"qwen-2.5-coder-3b-instruct"));
    assert!(ids.contains(&"llama-3.2-3b-instruct"));
    assert!(ids.contains(&"whisper-large-v3-korean"));
}

#[test]
fn host_low_picks_lightweight_korean_first() {
    let cat = Catalog::load_from_dir(&snapshot_dir()).unwrap();
    let r = cat.recommend(&host_low(), ModelCategory::AgentGeneral);
    // CPU-only 8GB → 1.2B EXAONE이 best. 7.8B/8B는 VRAM 부족으로 제외.
    assert_eq!(r.best_choice.as_deref(), Some("exaone-4.0-1.2b-instruct"));
    // lightweight도 같은 — 5GB 이하.
    assert_eq!(
        r.lightweight_choice.as_deref(),
        Some("exaone-4.0-1.2b-instruct")
    );
    // 32B는 반드시 제외 (VRAM 22GB 요구).
    let excluded_ids: Vec<&str> = r
        .excluded
        .iter()
        .map(|e| match e {
            ExclusionReason::InsufficientVram { id, .. }
            | ExclusionReason::InsufficientRam { id, .. }
            | ExclusionReason::IncompatibleRuntime { id }
            | ExclusionReason::Deprecated { id } => id.as_str(),
        })
        .collect();
    assert!(excluded_ids.contains(&"exaone-4.0-32b-instruct"));
}

#[test]
fn host_mid_picks_balanced_korean() {
    let cat = Catalog::load_from_dir(&snapshot_dir()).unwrap();
    let r = cat.recommend(&host_mid(), ModelCategory::AgentGeneral);
    // 16GB RAM + 12GB VRAM → 7.8B/8B Korean이 잘 맞음. EXAONE 3.5 7.8B 또는 HCX-SEED 8B.
    let best = r.best_choice.expect("best must exist");
    assert!(
        best == "exaone-3.5-7.8b-instruct" || best == "hcx-seed-8b",
        "unexpected best: {}",
        best
    );
    // 32B는 VRAM 부족 (22GB 요구, 12GB 있음).
    let excluded_ids: Vec<&str> = r
        .excluded
        .iter()
        .map(|e| match e {
            ExclusionReason::InsufficientVram { id, .. }
            | ExclusionReason::InsufficientRam { id, .. }
            | ExclusionReason::IncompatibleRuntime { id }
            | ExclusionReason::Deprecated { id } => id.as_str(),
        })
        .collect();
    assert!(excluded_ids.contains(&"exaone-4.0-32b-instruct"));
}

#[test]
fn host_high_unlocks_32b_for_coding() {
    let cat = Catalog::load_from_dir(&snapshot_dir()).unwrap();
    let r = cat.recommend(&host_high(), ModelCategory::Coding);
    // 24GB VRAM은 32B Q4_K_M (req 24GB) — 제외되지 않음.
    let excluded_ids: Vec<&str> = r
        .excluded
        .iter()
        .map(|e| match e {
            ExclusionReason::InsufficientVram { id, .. }
            | ExclusionReason::InsufficientRam { id, .. }
            | ExclusionReason::IncompatibleRuntime { id }
            | ExclusionReason::Deprecated { id } => id.as_str(),
        })
        .collect();
    assert!(
        !excluded_ids.contains(&"exaone-4.0-32b-instruct"),
        "32B should fit on RTX 4090 (24GB)"
    );
    // best는 둘 중 하나 — Qwen Coder 3B(headroom 보너스로 우위) 또는 32B.
    let best = r.best_choice.expect("best must exist");
    assert!(
        best == "exaone-4.0-32b-instruct" || best == "qwen-2.5-coder-3b-instruct",
        "best should be a coding model, got: {}",
        best
    );
}

#[test]
fn host_with_huge_vram_picks_32b_for_coding() {
    // 32GB VRAM (RTX 6000 Ada) → headroom 32 >= 24*1.3=31.2 → +5 보너스.
    let host = HostFingerprint {
        os: "windows".into(),
        arch: "x86_64".into(),
        cpu: "Threadripper".into(),
        ram_mb: 128 * 1024,
        gpu_vendor: Some("nvidia".into()),
        gpu_model: Some("RTX 6000 Ada".into()),
        vram_mb: Some(32 * 1024),
    };
    let cat = Catalog::load_from_dir(&snapshot_dir()).unwrap();
    let r = cat.recommend(&host, ModelCategory::Coding);
    assert_eq!(r.best_choice.as_deref(), Some("exaone-4.0-32b-instruct"));
}

#[test]
fn host_tiny_falls_back_to_smallest() {
    let cat = Catalog::load_from_dir(&snapshot_dir()).unwrap();
    let r = cat.recommend(&host_tiny(), ModelCategory::AgentGeneral);
    // 4GB RAM → 1.2B만 가능 (min_ram_mb=4096).
    let best = r.best_choice.unwrap();
    assert_eq!(best, "exaone-4.0-1.2b-instruct");
}

#[test]
fn determinism_invariant() {
    let cat = Catalog::load_from_dir(&snapshot_dir()).unwrap();
    let host = host_mid();
    let first = cat.recommend(&host, ModelCategory::AgentGeneral);
    for _ in 0..100 {
        let next = cat.recommend(&host, ModelCategory::AgentGeneral);
        assert_eq!(first, next, "recommendation must be deterministic");
    }
}

#[test]
fn lightweight_cliff_prevention() {
    // 보정-4: install_size_mb > 5000인 모델만 있으면 lightweight = None.
    let entries = vec![make_entry("big1", ModelCategory::AgentGeneral, 7000)];
    let cat = Catalog::from_entries(entries);
    let r = cat.recommend(&host_high(), ModelCategory::AgentGeneral);
    assert!(r.best_choice.is_some());
    assert!(r.lightweight_choice.is_none());
}

#[test]
fn lexicographic_tie_breaker() {
    // 보정-3: 동점이면 (maturity, install_size, id) 순.
    let mut a = make_entry("zeta", ModelCategory::AgentGeneral, 1000);
    let mut b = make_entry("alpha", ModelCategory::AgentGeneral, 1000);
    // 동일 점수 — install_size 같음 — id 알파벳 순.
    a.maturity = Maturity::Stable;
    b.maturity = Maturity::Stable;
    let cat = Catalog::from_entries(vec![a, b]);
    let r = cat.recommend(&host_high(), ModelCategory::AgentGeneral);
    assert_eq!(r.best_choice.as_deref(), Some("alpha"));
}

#[test]
fn maturity_overrides_lex_id() {
    // Stable이 Beta보다 우선.
    let mut a = make_entry("alpha", ModelCategory::AgentGeneral, 1000);
    let mut b = make_entry("beta", ModelCategory::AgentGeneral, 1000);
    a.maturity = Maturity::Beta;
    b.maturity = Maturity::Stable;
    let cat = Catalog::from_entries(vec![a, b]);
    let r = cat.recommend(&host_high(), ModelCategory::AgentGeneral);
    // beta(stable maturity)가 alpha(beta maturity)보다 점수 5 높음.
    assert_eq!(r.best_choice.as_deref(), Some("beta"));
}

#[test]
fn deprecated_excluded() {
    let mut a = make_entry("good", ModelCategory::AgentGeneral, 1000);
    let mut b = make_entry("old", ModelCategory::AgentGeneral, 1000);
    a.maturity = Maturity::Stable;
    b.maturity = Maturity::Deprecated;
    let cat = Catalog::from_entries(vec![a, b]);
    let r = cat.recommend(&host_high(), ModelCategory::AgentGeneral);
    assert_eq!(r.best_choice.as_deref(), Some("good"));
    assert_eq!(r.excluded.len(), 1);
    matches!(r.excluded[0], ExclusionReason::Deprecated { .. });
}

#[test]
fn category_asymmetric_match() {
    // 보정-2: agent ↔ coding은 인접(+5), agent ↔ stt는 other(0).
    let cat = Catalog::load_from_dir(&snapshot_dir()).unwrap();
    let agent = cat.recommend(&host_high(), ModelCategory::AgentGeneral);
    let coding = cat.recommend(&host_high(), ModelCategory::Coding);
    let stt = cat.recommend(&host_high(), ModelCategory::SoundStt);
    // agent / coding은 best가 다를 수 있지만 둘 다 카탈로그에 적합한 추천이 있어야 함.
    assert!(agent.best_choice.is_some());
    assert!(coding.best_choice.is_some());
    assert!(stt.best_choice.is_some());
    // STT는 whisper가 best.
    assert_eq!(stt.best_choice.as_deref(), Some("whisper-large-v3-korean"));
}

#[test]
fn fallback_choice_is_smallest_stable() {
    let cat = Catalog::load_from_dir(&snapshot_dir()).unwrap();
    let r = cat.recommend(&host_high(), ModelCategory::AgentGeneral);
    // 시드 중 가장 작은 stable은 EXAONE 4.0 1.2B (760MB).
    assert_eq!(
        r.fallback_choice.as_deref(),
        Some("exaone-4.0-1.2b-instruct")
    );
}

#[test]
fn host_with_no_gpu_excludes_vram_required_models() {
    let cat = Catalog::load_from_dir(&snapshot_dir()).unwrap();
    let r = cat.recommend(&host_low(), ModelCategory::AgentGeneral);
    // host_low는 VRAM 없음 → min_vram_mb 있는 모델 다 제외.
    let excluded_ids: Vec<&str> = r
        .excluded
        .iter()
        .map(|e| match e {
            ExclusionReason::InsufficientVram { id, .. }
            | ExclusionReason::InsufficientRam { id, .. }
            | ExclusionReason::IncompatibleRuntime { id }
            | ExclusionReason::Deprecated { id } => id.as_str(),
        })
        .collect();
    // EXAONE 3.5 7.8B (min 6GB VRAM), HCX-SEED 8B (min 6GB), Polyglot 12.8B, 32B, qwen 3B (min 3GB).
    assert!(excluded_ids.contains(&"exaone-3.5-7.8b-instruct"));
    assert!(excluded_ids.contains(&"hcx-seed-8b"));
    assert!(excluded_ids.contains(&"polyglot-ko-12.8b"));
    assert!(excluded_ids.contains(&"exaone-4.0-32b-instruct"));
}

#[test]
fn id_collision_first_wins_in_load_from_dir() {
    use std::fs;
    let tmp = tempfile::tempdir().unwrap();
    let m1 = serde_json::json!({
        "schema_version": 1,
        "generated_at": "2026-04-27T00:00:00Z",
        "entries": [{
            "id": "dup", "display_name": "First", "category": "agent-general",
            "model_family": "x", "source": { "type": "direct-url", "url": "https://1" },
            "runner_compatibility": ["llama-cpp"], "quantization_options": [],
            "min_ram_mb": 1024, "rec_ram_mb": 2048, "install_size_mb": 100,
            "tool_support": false, "vision_support": false, "structured_output_support": false,
            "license": "MIT", "maturity": "stable",
            "portable_suitability": 5, "on_device_suitability": 5, "fine_tune_suitability": 5
        }]
    });
    let m2 = serde_json::json!({
        "schema_version": 1,
        "generated_at": "2026-04-27T00:00:00Z",
        "entries": [{
            "id": "dup", "display_name": "Second", "category": "agent-general",
            "model_family": "x", "source": { "type": "direct-url", "url": "https://2" },
            "runner_compatibility": ["llama-cpp"], "quantization_options": [],
            "min_ram_mb": 1024, "rec_ram_mb": 2048, "install_size_mb": 100,
            "tool_support": false, "vision_support": false, "structured_output_support": false,
            "license": "MIT", "maturity": "stable",
            "portable_suitability": 5, "on_device_suitability": 5, "fine_tune_suitability": 5
        }]
    });
    fs::write(
        tmp.path().join("a.json"),
        serde_json::to_string(&m1).unwrap(),
    )
    .unwrap();
    fs::write(
        tmp.path().join("b.json"),
        serde_json::to_string(&m2).unwrap(),
    )
    .unwrap();
    let cat = Catalog::load_from_dir(tmp.path()).unwrap();
    assert_eq!(cat.entries().len(), 1);
    // a.json이 먼저 (paths.sort()) — "First"가 살아남음.
    assert_eq!(cat.entries()[0].display_name, "First");
}

#[test]
fn verified_tier_boosts_score() {
    let mut a = make_entry("verified-one", ModelCategory::AgentGeneral, 1000);
    let mut b = make_entry("community-one", ModelCategory::AgentGeneral, 1000);
    a.verification = VerificationInfo {
        tier: VerificationTier::Verified,
        verified_at: Some("2026".into()),
        verified_by: Some("curator".into()),
    };
    b.verification = VerificationInfo {
        tier: VerificationTier::Community,
        ..Default::default()
    };
    let cat = Catalog::from_entries(vec![b, a]);
    let r = cat.recommend(&host_high(), ModelCategory::AgentGeneral);
    assert_eq!(r.best_choice.as_deref(), Some("verified-one"));
}
