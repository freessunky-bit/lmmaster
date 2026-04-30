//! Source tier + URL template 처리.
//!
//! 정책 (Phase 1' 결정 §1):
//! - 4 tier: Vendor → GitHub Releases → jsDelivr → Bundled.
//! - sequential first-success — vendor 우선, 모두 실패 시 bundled.
//! - jsDelivr는 commit/tag 핀, `@main` 절대 금지 (ETag 안정 + 영구 캐시).

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::FetcherError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceTier {
    /// 매니페스트가 직접 호스트하는 vendor URL (옵션). v1에선 거의 사용 안 함.
    Vendor,
    /// LMmaster의 GitHub Releases assets — 1순위 네트워크 소스.
    Github,
    /// jsDelivr CDN GitHub mirror — geo-distributed fallback.
    Jsdelivr,
    /// Tauri bundle 안에 동봉된 stale-but-known-good snapshot.
    Bundled,
}

impl SourceTier {
    /// SQL CHECK 제약과 일치하는 lowercase 문자열.
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Vendor => "vendor",
            Self::Github => "github",
            Self::Jsdelivr => "jsdelivr",
            Self::Bundled => "bundled",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "vendor" => Some(Self::Vendor),
            "github" => Some(Self::Github),
            "jsdelivr" => Some(Self::Jsdelivr),
            "bundled" => Some(Self::Bundled),
            _ => None,
        }
    }

    /// 이 tier가 네트워크를 사용하는지. Bundled만 false.
    pub fn is_network(&self) -> bool {
        !matches!(self, Self::Bundled)
    }
}

/// 단일 source 설정. `url_template`은 `{id}` placeholder를 사용한다.
#[derive(Debug, Clone)]
pub struct SourceConfig {
    pub tier: SourceTier,
    /// `{id}` placeholder 포함. Bundled는 미사용 (FetcherOptions.bundled_dir에서 직접 join).
    pub url_template: String,
    pub timeout: Duration,
}

impl SourceConfig {
    /// `{id}` 치환 후 URL 반환.
    pub fn resolve_url(&self, manifest_id: &str) -> Result<String, FetcherError> {
        if manifest_id.is_empty() {
            return Err(FetcherError::EmptyManifestId);
        }
        // ID는 alpha-num + '-' / '_' 만 허용.
        if !manifest_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(FetcherError::UrlTemplate(format!(
                "허용되지 않는 문자가 manifest_id에 있어요: {manifest_id}"
            )));
        }
        Ok(self.url_template.replace("{id}", manifest_id))
    }

    /// `.minisig` URL 반환 — Phase 13'.g.2.b (ADR-0047).
    ///
    /// 정책:
    /// - 모든 network tier(jsDelivr / GitHub Releases / Vendor)는 `<body_url>.minisig`로 통일.
    /// - Bundled tier는 `is_network()=false` — `Err(NotNetworkTier)` 반환. caller가 별도 처리.
    /// - 보안: body와 같은 호스트에서만 fetch (cross-origin 시 변조 위험 회피).
    pub fn resolve_signature_url(&self, manifest_id: &str) -> Result<String, FetcherError> {
        if !self.tier.is_network() {
            return Err(FetcherError::UrlTemplate(format!(
                "Bundled tier는 .minisig URL이 없어요 (tier={:?})",
                self.tier
            )));
        }
        let body_url = self.resolve_url(manifest_id)?;
        Ok(format!("{body_url}.minisig"))
    }
}

/// LMmaster 권장 기본 source 목록.
///
/// 정책 (Phase 13'.a 보강 리서치):
/// - **jsDelivr 1순위 → GitHub Releases 2순위 → Bundled fallback** — 한국 latency 우선
///   (jsDelivr Seoul/Incheon POP 2-3ms, GitHub 100-200ms).
/// - Repo: `freessunky-bit/lmmaster` (현 GitHub URL).
/// - jsDelivr ref는 commit hash 또는 tag 권장 — 빌드 시점에 호출자가 결정.
///   `@main`은 매 push마다 캐시 무효 + 영구캐시 의미 사라져 비추 (research §1 함정).
pub fn default_sources(github_tag: &str, jsdelivr_ref: &str) -> Vec<SourceConfig> {
    vec![
        SourceConfig {
            tier: SourceTier::Jsdelivr,
            url_template: format!(
                "https://cdn.jsdelivr.net/gh/freessunky-bit/lmmaster@{jsdelivr_ref}/manifests/apps/{{id}}.json"
            ),
            timeout: Duration::from_secs(6),
        },
        SourceConfig {
            tier: SourceTier::Github,
            url_template: format!(
                "https://github.com/freessunky-bit/lmmaster/releases/download/manifests-{github_tag}/{{id}}.json"
            ),
            timeout: Duration::from_secs(8),
        },
        SourceConfig {
            tier: SourceTier::Bundled,
            url_template: String::new(), // bundled는 url 미사용
            timeout: Duration::from_millis(500),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_str_round_trip() {
        for tier in [
            SourceTier::Vendor,
            SourceTier::Github,
            SourceTier::Jsdelivr,
            SourceTier::Bundled,
        ] {
            let s = tier.as_db_str();
            assert_eq!(SourceTier::from_db_str(s), Some(tier));
        }
        assert_eq!(SourceTier::from_db_str("nope"), None);
    }

    #[test]
    fn is_network_only_bundled_false() {
        assert!(SourceTier::Vendor.is_network());
        assert!(SourceTier::Github.is_network());
        assert!(SourceTier::Jsdelivr.is_network());
        assert!(!SourceTier::Bundled.is_network());
    }

    #[test]
    fn resolve_url_substitutes_id() {
        let s = SourceConfig {
            tier: SourceTier::Github,
            url_template: "https://example.com/{id}.json".into(),
            timeout: Duration::from_secs(1),
        };
        assert_eq!(
            s.resolve_url("ollama").unwrap(),
            "https://example.com/ollama.json"
        );
    }

    #[test]
    fn resolve_url_rejects_empty_id() {
        let s = SourceConfig {
            tier: SourceTier::Github,
            url_template: "x/{id}".into(),
            timeout: Duration::from_secs(1),
        };
        assert!(matches!(
            s.resolve_url(""),
            Err(FetcherError::EmptyManifestId)
        ));
    }

    #[test]
    fn resolve_url_rejects_bad_chars() {
        let s = SourceConfig {
            tier: SourceTier::Github,
            url_template: "x/{id}".into(),
            timeout: Duration::from_secs(1),
        };
        assert!(matches!(
            s.resolve_url("../etc/passwd"),
            Err(FetcherError::UrlTemplate(_))
        ));
        assert!(matches!(
            s.resolve_url("a b"),
            Err(FetcherError::UrlTemplate(_))
        ));
    }

    // ── Phase 13'.g.2.b — .minisig URL invariants ──────────────────

    #[test]
    fn resolve_signature_url_appends_minisig() {
        let s = SourceConfig {
            tier: SourceTier::Jsdelivr,
            url_template: "https://cdn.jsdelivr.net/gh/x/y@abc/manifests/{id}.json".into(),
            timeout: Duration::from_secs(1),
        };
        assert_eq!(
            s.resolve_signature_url("catalog").unwrap(),
            "https://cdn.jsdelivr.net/gh/x/y@abc/manifests/catalog.json.minisig"
        );
    }

    #[test]
    fn resolve_signature_url_works_for_github_releases() {
        let s = SourceConfig {
            tier: SourceTier::Github,
            url_template: "https://github.com/x/y/releases/download/manifests-2026.05.01/{id}.json"
                .into(),
            timeout: Duration::from_secs(1),
        };
        let url = s.resolve_signature_url("catalog").unwrap();
        assert!(url.ends_with("catalog.json.minisig"));
        assert!(url.contains("github.com"));
    }

    #[test]
    fn resolve_signature_url_rejects_bundled() {
        let s = SourceConfig {
            tier: SourceTier::Bundled,
            url_template: String::new(),
            timeout: Duration::from_secs(1),
        };
        assert!(matches!(
            s.resolve_signature_url("catalog"),
            Err(FetcherError::UrlTemplate(_))
        ));
    }

    #[test]
    fn resolve_signature_url_rejects_bad_id_via_resolve_url() {
        let s = SourceConfig {
            tier: SourceTier::Github,
            url_template: "https://example.com/{id}.json".into(),
            timeout: Duration::from_secs(1),
        };
        // resolve_url의 ID 검증을 그대로 통과. ../etc/passwd → reject.
        assert!(matches!(
            s.resolve_signature_url("../etc/passwd"),
            Err(FetcherError::UrlTemplate(_))
        ));
    }

    #[test]
    fn default_sources_jsdelivr_first_then_github_then_bundled() {
        // Phase 13'.a 보강 리서치 — 한국 latency 우선으로 jsDelivr 1순위 swap.
        let s = default_sources("2026.04.27", "abc123");
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].tier, SourceTier::Jsdelivr);
        assert_eq!(s[1].tier, SourceTier::Github);
        assert_eq!(s[2].tier, SourceTier::Bundled);
        assert!(s[0].url_template.contains("@abc123"));
        assert!(s[0].url_template.contains("freessunky-bit/lmmaster"));
        assert!(s[1].url_template.contains("freessunky-bit/lmmaster"));
    }
}
