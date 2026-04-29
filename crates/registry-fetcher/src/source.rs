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
}

/// LMmaster 권장 기본 source 목록.
///
/// 호출자(주로 `apps/desktop/src-tauri`)가 GitHub release tag / jsDelivr commit hash를
/// 빌드 시점에 채워 넣는다. v1은 commit-pinned snapshot을 사용.
pub fn default_sources(github_tag: &str, jsdelivr_ref: &str) -> Vec<SourceConfig> {
    vec![
        // Vendor 미사용 시 비워둘 수 있다. v1 manifest에 vendor mirror 없으므로 기본 미포함.
        SourceConfig {
            tier: SourceTier::Github,
            url_template: format!(
                "https://github.com/lmmaster/lmmaster/releases/download/manifests-{github_tag}/{{id}}.json"
            ),
            timeout: Duration::from_secs(8),
        },
        SourceConfig {
            tier: SourceTier::Jsdelivr,
            url_template: format!(
                "https://cdn.jsdelivr.net/gh/lmmaster/lmmaster@{jsdelivr_ref}/manifests/apps/{{id}}.json"
            ),
            timeout: Duration::from_secs(6),
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

    #[test]
    fn default_sources_skip_bundled_url() {
        let s = default_sources("2026.04.27", "abc123");
        assert_eq!(s.len(), 3);
        assert_eq!(s[0].tier, SourceTier::Github);
        assert_eq!(s[1].tier, SourceTier::Jsdelivr);
        assert_eq!(s[2].tier, SourceTier::Bundled);
        assert!(s[1].url_template.contains("@abc123"));
    }
}
