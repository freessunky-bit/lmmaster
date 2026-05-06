//! manifest validator — Phase 23'.b (ADR-0062 §3 + §7).
//!
//! 정책:
//! - NSFW 라벨 + minor_safety_attestation 누락 → MinorSafetyMissing.
//! - keyword_scan_clean: false → MinorKeywordHit.
//! - 라이선스 블랙리스트 → LicenseBlacklisted.
//! - 라이선스 화이트리스트 (Apache-2 / MIT / BSD / CC-BY / OpenRAIL-M) → OK.
//! - 비상업 (CC-BY-NC) → `commercial: false` 일치 필수.

use thiserror::Error;

use crate::manifest::{ContentWarning, DatasetEntry};

#[derive(Debug, Error, PartialEq)]
pub enum DatasetValidationError {
    #[error("NSFW 데이터셋에 minor_safety_attestation이 누락됐어요: {0}")]
    MinorSafetyMissing(String),
    #[error("미성년 키워드 scan에서 hit이 발견됐어요: {0}")]
    MinorKeywordHit(String),
    #[error("HF NFAA 플래그가 false로 표시된 NSFW 데이터셋이에요: {0}")]
    NsfwWithoutNfaaFlag(String),
    #[error("라이선스가 블랙리스트에 있어요: {license} (entry: {entry_id})")]
    LicenseBlacklisted { entry_id: String, license: String },
    #[error("CC-BY-NC 라이선스인데 commercial=true로 표시됐어요: {0}")]
    NoncommercialMarkedCommercial(String),
}

/// 라이선스 화이트리스트 (Phase 23'.b ADR-0062 §3).
const LICENSE_WHITELIST: &[&str] = &[
    "apache-2.0",
    "apache-2",
    "mit",
    "bsd-2-clause",
    "bsd-3-clause",
    "cc-by-4.0",
    "cc-by-sa-4.0",
    "openrail-m",
    "openrail",
];

/// 비상업 라이선스 (CC-BY-NC*) — `commercial: false` 일치 필수.
const NONCOMMERCIAL_PREFIXES: &[&str] = &["cc-by-nc"];

/// 블랙리스트 라이선스 (proprietary / 미명시).
const LICENSE_BLACKLIST: &[&str] = &[
    "proprietary",
    "all-rights-reserved",
    "unspecified",
    "unknown",
];

/// 데이터셋 entry 검증.
pub fn validate_dataset_entry(entry: &DatasetEntry) -> Result<(), DatasetValidationError> {
    let lic_lower = entry.license.to_ascii_lowercase();

    // 1. 블랙리스트 검사.
    if LICENSE_BLACKLIST.iter().any(|b| lic_lower == *b) {
        return Err(DatasetValidationError::LicenseBlacklisted {
            entry_id: entry.id.clone(),
            license: entry.license.clone(),
        });
    }

    // 2. CC-BY-NC인데 commercial=true → mismatch.
    if NONCOMMERCIAL_PREFIXES
        .iter()
        .any(|p| lic_lower.starts_with(p))
        && entry.commercial
    {
        return Err(DatasetValidationError::NoncommercialMarkedCommercial(
            entry.id.clone(),
        ));
    }

    // 3. NSFW 라벨 데이터셋 검증.
    if matches!(entry.content_warning, Some(ContentWarning::RpExplicit)) {
        let attestation = entry
            .minor_safety_attestation
            .as_ref()
            .ok_or_else(|| DatasetValidationError::MinorSafetyMissing(entry.id.clone()))?;

        if !attestation.keyword_scan_clean {
            return Err(DatasetValidationError::MinorKeywordHit(entry.id.clone()));
        }

        if !attestation.hf_nfaa_flag {
            return Err(DatasetValidationError::NsfwWithoutNfaaFlag(
                entry.id.clone(),
            ));
        }
    }

    // 4. 라이선스가 화이트리스트에 없고 비상업도 아니면 *경고만* (Err X)로 두는 것이 정공.
    //    화이트리스트 외 ≠ 무조건 거부 — 큐레이터가 명시 검증한 라이선스 (Llama Community 등)는 OK.
    //    엄격 거부는 블랙리스트만.

    Ok(())
}

/// 라이선스 화이트리스트 매칭 — `community_insights` UI 표시용.
pub fn license_in_whitelist(license: &str) -> bool {
    let lower = license.to_ascii_lowercase();
    LICENSE_WHITELIST.iter().any(|w| lower == *w)
}

/// 비상업 라이선스 여부 — UI에 비상업 chip 표시.
pub fn license_is_noncommercial(license: &str) -> bool {
    let lower = license.to_ascii_lowercase();
    NONCOMMERCIAL_PREFIXES.iter().any(|p| lower.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::DatasetFormat;
    use crate::manifest::{DatasetCategory, DatasetSource, DatasetUseCase, MinorSafetyAttestation};
    use std::collections::BTreeMap;

    fn sample_entry(license: &str, content_warning: Option<ContentWarning>) -> DatasetEntry {
        DatasetEntry {
            id: "test/dataset".into(),
            display_name: "Test Dataset".into(),
            category: DatasetCategory::SftSeed,
            source: DatasetSource::HuggingFace {
                repo: "test/dataset".into(),
                file: None,
            },
            size_mb: 100,
            row_count: Some(1000),
            languages: vec!["en".into()],
            license: license.into(),
            commercial: true,
            content_warning,
            minor_safety_attestation: None,
            use_case: DatasetUseCase::SftSeed {
                format: "alpaca".into(),
                language: vec!["en".into()],
            },
            format: DatasetFormat::Jsonl,
            checksums: BTreeMap::new(),
            curator_note_ko: None,
            sources: vec![],
        }
    }

    fn sample_attestation(scan_clean: bool, nfaa: bool) -> MinorSafetyAttestation {
        MinorSafetyAttestation {
            verified_at: "2026-05-07T00:00:00Z".into(),
            verified_by: "lmmaster-curator".into(),
            keyword_scan_clean: scan_clean,
            hf_nfaa_flag: nfaa,
            license_whitelist: true,
            curator_note_ko: "테스트".into(),
        }
    }

    #[test]
    fn validate_apache_no_nsfw_ok() {
        let e = sample_entry("Apache-2.0", None);
        assert!(validate_dataset_entry(&e).is_ok());
    }

    #[test]
    fn validate_blacklisted_license_rejects() {
        let e = sample_entry("proprietary", None);
        assert!(matches!(
            validate_dataset_entry(&e),
            Err(DatasetValidationError::LicenseBlacklisted { .. })
        ));
    }

    #[test]
    fn validate_unknown_license_rejects() {
        let e = sample_entry("unspecified", None);
        assert!(matches!(
            validate_dataset_entry(&e),
            Err(DatasetValidationError::LicenseBlacklisted { .. })
        ));
    }

    #[test]
    fn validate_noncommercial_marked_commercial_rejects() {
        let mut e = sample_entry("CC-BY-NC-4.0", None);
        e.commercial = true;
        assert!(matches!(
            validate_dataset_entry(&e),
            Err(DatasetValidationError::NoncommercialMarkedCommercial(_))
        ));
    }

    #[test]
    fn validate_noncommercial_correctly_marked_ok() {
        let mut e = sample_entry("CC-BY-NC-4.0", None);
        e.commercial = false;
        assert!(validate_dataset_entry(&e).is_ok());
    }

    #[test]
    fn validate_nsfw_without_attestation_rejects() {
        let mut e = sample_entry("Apache-2.0", Some(ContentWarning::RpExplicit));
        e.minor_safety_attestation = None;
        assert!(matches!(
            validate_dataset_entry(&e),
            Err(DatasetValidationError::MinorSafetyMissing(_))
        ));
    }

    #[test]
    fn validate_nsfw_with_keyword_hit_rejects() {
        let mut e = sample_entry("Apache-2.0", Some(ContentWarning::RpExplicit));
        e.minor_safety_attestation = Some(sample_attestation(false, true));
        assert!(matches!(
            validate_dataset_entry(&e),
            Err(DatasetValidationError::MinorKeywordHit(_))
        ));
    }

    #[test]
    fn validate_nsfw_without_nfaa_rejects() {
        let mut e = sample_entry("Apache-2.0", Some(ContentWarning::RpExplicit));
        e.minor_safety_attestation = Some(sample_attestation(true, false));
        assert!(matches!(
            validate_dataset_entry(&e),
            Err(DatasetValidationError::NsfwWithoutNfaaFlag(_))
        ));
    }

    #[test]
    fn validate_nsfw_full_attestation_ok() {
        let mut e = sample_entry("Apache-2.0", Some(ContentWarning::RpExplicit));
        e.minor_safety_attestation = Some(sample_attestation(true, true));
        assert!(validate_dataset_entry(&e).is_ok());
    }

    #[test]
    fn license_in_whitelist_matches() {
        assert!(license_in_whitelist("Apache-2.0"));
        assert!(license_in_whitelist("MIT"));
        assert!(license_in_whitelist("CC-BY-4.0"));
        assert!(license_in_whitelist("OpenRAIL-M"));
        assert!(!license_in_whitelist("CC-BY-NC-4.0"));
        assert!(!license_in_whitelist("Llama-3-Community"));
    }

    #[test]
    fn license_is_noncommercial_matches() {
        assert!(license_is_noncommercial("CC-BY-NC-4.0"));
        assert!(license_is_noncommercial("CC-BY-NC-SA-4.0"));
        assert!(!license_is_noncommercial("Apache-2.0"));
        assert!(!license_is_noncommercial("MIT"));
    }

    #[test]
    fn error_messages_korean() {
        let e = DatasetValidationError::MinorSafetyMissing("test/x".into());
        let msg = e.to_string();
        assert!(msg.contains("minor_safety_attestation"));
        assert!(msg.contains("누락"));
    }
}
