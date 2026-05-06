//! DatasetEntry manifest — Phase 23'.a (ADR-0061).
//!
//! 정책:
//! - DatasetCategory enum (별도, parallel structure).
//! - DatasetUseCase tagged enum (`#[serde(tag = "kind")]`) — kebab-case.
//! - ModelTier / Maturity / ContentWarning은 shared-types 또는 별도 정의.
//! - 본 sub-phase는 schema 정의 + 단위 테스트만. loader / IPC는 Phase 23'.c.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::format::{ChunkStrategy, DatasetFormat};

/// 데이터셋 카테고리 — ModelCategory와 직교 축 (ADR-0061 §1).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DatasetCategory {
    /// SFT 시드 — instruction-tuning 직접 사용.
    SftSeed,
    /// LoRA 시드 — 베이스 모델 hint 동반.
    LoraSeed,
    /// RAG corpus — 청크/임베딩 후 검색 시드.
    RagCorpus,
    /// Persona / character — 캐릭터 narrative.
    PersonaSeed,
    /// 평가 / 벤치마크 — KMMLU / KoBEST 등.
    EvalBenchmark,
}

/// DatasetUseCase tagged enum — kebab-case `kind` (ADR-0061 §2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DatasetUseCase {
    SftSeed {
        format: String, // "openai-chat" | "alpaca" | "sharegpt" 등
        language: Vec<String>,
    },
    LoraSeed {
        base_model_hint: Option<String>,
        target_layers: Option<Vec<String>>,
    },
    RagCorpus {
        chunk_strategy: ChunkStrategy,
        default_chunk_size: u32,
    },
    PersonaSeed {
        count: u64,
        narrative_field: String,
    },
    EvalBenchmark {
        metric_keys: Vec<String>,
    },
}

/// 데이터셋 source — HuggingFace / DirectUrl / Bundled (model entry parallel).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum DatasetSource {
    HuggingFace { repo: String, file: Option<String> },
    DirectUrl { url: String },
    Bundled { path: String },
}

/// minor_safety_attestation — Phase 23'.b (ADR-0062 §1).
///
/// NSFW 라벨(`content_warning: rp-explicit`) 데이터셋은 *반드시* 본 필드 포함.
/// 누락 시 validator가 `Err(MinorSafetyMissing)` 반환.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MinorSafetyAttestation {
    /// 큐레이터가 본문 검증한 timestamp (RFC3339).
    pub verified_at: String,
    /// 큐레이터 식별자 — v1은 "lmmaster-curator" 단일.
    pub verified_by: String,
    /// 미성년 키워드 정규식 hit 0건 검증 결과.
    pub keyword_scan_clean: bool,
    /// HF NFAA (Not-For-All-Audiences) 플래그 보유.
    pub hf_nfaa_flag: bool,
    /// 라이선스가 화이트리스트 (Apache-2/MIT/CC-BY/OpenRAIL-M)에 포함.
    pub license_whitelist: bool,
    /// 큐레이터 메모 (한국어 해요체).
    pub curator_note_ko: String,
}

/// ContentWarning — model entry와 같은 enum 재사용 (별도 정의 — shared-types에 통합 가능).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ContentWarning {
    /// 성인 RP / NSFW 데이터셋. 토글 활성 시만 노출.
    RpExplicit,
}

/// 데이터셋 매니페스트 entry — Phase 23'.a 핵심 schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DatasetEntry {
    pub id: String,
    pub display_name: String,
    pub category: DatasetCategory,
    pub source: DatasetSource,
    pub size_mb: u64,
    #[serde(default)]
    pub row_count: Option<u64>,
    pub languages: Vec<String>,
    pub license: String,
    #[serde(default = "default_commercial")]
    pub commercial: bool,
    #[serde(default)]
    pub content_warning: Option<ContentWarning>,
    #[serde(default)]
    pub minor_safety_attestation: Option<MinorSafetyAttestation>,
    pub use_case: DatasetUseCase,
    pub format: DatasetFormat,
    #[serde(default)]
    pub checksums: BTreeMap<String, String>,
    #[serde(default)]
    pub curator_note_ko: Option<String>,
    #[serde(default)]
    pub sources: Vec<String>,
}

fn default_commercial() -> bool {
    true
}

/// `manifests/apps/datasets-bundle.json` 합본 schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DatasetBundle {
    pub schema_version: u32,
    pub generated_at: String,
    pub entries: Vec<DatasetEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dataset_category_round_trip_kebab_case() {
        for (cat, expected) in [
            (DatasetCategory::SftSeed, "sft-seed"),
            (DatasetCategory::LoraSeed, "lora-seed"),
            (DatasetCategory::RagCorpus, "rag-corpus"),
            (DatasetCategory::PersonaSeed, "persona-seed"),
            (DatasetCategory::EvalBenchmark, "eval-benchmark"),
        ] {
            let v = serde_json::to_value(cat).unwrap();
            assert_eq!(v.as_str(), Some(expected), "serialize {cat:?}");
            let parsed: DatasetCategory = serde_json::from_value(v).unwrap();
            assert_eq!(parsed, cat, "round-trip {cat:?}");
        }
    }

    #[test]
    fn dataset_use_case_tagged_round_trip() {
        let uc = DatasetUseCase::PersonaSeed {
            count: 7_000_000,
            narrative_field: "persona".into(),
        };
        let v = serde_json::to_value(&uc).unwrap();
        assert_eq!(v["kind"], "persona-seed");
        assert_eq!(v["count"], 7_000_000);
        let parsed: DatasetUseCase = serde_json::from_value(v).unwrap();
        assert_eq!(parsed, uc);
    }

    #[test]
    fn dataset_source_kebab_case() {
        let src = DatasetSource::HuggingFace {
            repo: "nvidia/Nemotron-Personas-Korea".into(),
            file: None,
        };
        let v = serde_json::to_value(&src).unwrap();
        assert_eq!(v["type"], "hugging-face");
        assert_eq!(v["repo"], "nvidia/Nemotron-Personas-Korea");
    }

    #[test]
    fn dataset_entry_round_trip_minimal() {
        let e = DatasetEntry {
            id: "huggingface-krew-korean-rp".into(),
            display_name: "huggingface-KREW/korean-role-playing".into(),
            category: DatasetCategory::SftSeed,
            source: DatasetSource::HuggingFace {
                repo: "huggingface-KREW/korean-role-playing".into(),
                file: None,
            },
            size_mb: 99,
            row_count: Some(35_300),
            languages: vec!["ko".into()],
            license: "Apache-2.0".into(),
            commercial: true,
            content_warning: None,
            minor_safety_attestation: None,
            use_case: DatasetUseCase::SftSeed {
                format: "sharegpt".into(),
                language: vec!["ko".into()],
            },
            format: DatasetFormat::Parquet,
            checksums: BTreeMap::new(),
            curator_note_ko: Some("한국어 native RP 시드 1순위.".into()),
            sources: vec!["huggingface.co/datasets/huggingface-KREW/korean-role-playing".into()],
        };
        let s = serde_json::to_string(&e).unwrap();
        let parsed: DatasetEntry = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed.id, e.id);
        assert_eq!(parsed.size_mb, 99);
        assert_eq!(parsed.languages, vec!["ko"]);
    }

    #[test]
    fn dataset_entry_with_minor_safety_attestation() {
        let attestation = MinorSafetyAttestation {
            verified_at: "2026-05-07T00:00:00Z".into(),
            verified_by: "lmmaster-curator".into(),
            keyword_scan_clean: true,
            hf_nfaa_flag: true,
            license_whitelist: true,
            curator_note_ko: "본문 100 row 검토 완료. 미성년 묘사 0건.".into(),
        };
        let v = serde_json::to_value(&attestation).unwrap();
        assert_eq!(v["keyword_scan_clean"], true);
        let parsed: MinorSafetyAttestation = serde_json::from_value(v).unwrap();
        assert_eq!(parsed.verified_by, "lmmaster-curator");
    }

    #[test]
    fn dataset_bundle_round_trip() {
        let bundle = DatasetBundle {
            schema_version: 1,
            generated_at: "2026-05-07T00:00:00Z".into(),
            entries: vec![],
        };
        let v = serde_json::to_value(&bundle).unwrap();
        assert_eq!(v["schema_version"], 1);
        let parsed: DatasetBundle = serde_json::from_value(v).unwrap();
        assert_eq!(parsed.entries.len(), 0);
    }

    #[test]
    fn legacy_entry_without_optional_fields_parses() {
        let json = json!({
            "id": "legacy",
            "display_name": "Legacy",
            "category": "sft-seed",
            "source": {"type": "direct-url", "url": "https://x"},
            "size_mb": 100,
            "languages": ["en"],
            "license": "MIT",
            "use_case": {"kind": "sft-seed", "format": "alpaca", "language": ["en"]},
            "format": "jsonl"
        });
        let parsed: DatasetEntry = serde_json::from_value(json).unwrap();
        assert!(parsed.commercial); // default true
        assert!(parsed.minor_safety_attestation.is_none());
        assert!(parsed.checksums.is_empty());
    }
}
