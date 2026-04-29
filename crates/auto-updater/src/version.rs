//! semver 비교 — `is_outdated(current, latest)`.
//!
//! 정책 (phase-6p-updater-pipelines-decision.md §4):
//! - `semver::Version` 표준 비교. pre-release / build metadata 처리는 spec 따름:
//!   * `1.0.0-beta < 1.0.0` (pre-release는 정식보다 낮음).
//!   * `1.0.0+build1 == 1.0.0+build2` (build metadata는 비교 무시).
//! - 둘 중 하나라도 파싱 실패 → `UpdaterError::InvalidVersion(원본 문자열)`.
//! - "v" prefix는 흔한 GitHub release tag 관행 (`v1.2.3`)이므로 leading 'v'/'V' 한 글자 strip 후 파싱.

use crate::error::UpdaterError;

/// `current` 보다 `latest`가 높으면 true.
///
/// 같으면 false (= up-to-date), `latest < current` (downgrade)도 false 반환.
/// 둘 다 정식 semver 형식이어야 함. 잘못된 입력은 `Err(InvalidVersion)`.
///
/// # 예시
/// ```
/// use auto_updater::is_outdated;
/// assert!(is_outdated("1.0.0", "1.0.1").unwrap());
/// assert!(!is_outdated("1.0.1", "1.0.0").unwrap());
/// assert!(!is_outdated("1.0.0", "1.0.0").unwrap());
/// ```
pub fn is_outdated(current: &str, latest: &str) -> Result<bool, UpdaterError> {
    let cur = parse_lenient(current)?;
    let lat = parse_lenient(latest)?;
    Ok(lat > cur)
}

fn parse_lenient(s: &str) -> Result<semver::Version, UpdaterError> {
    let trimmed = s.trim();
    let stripped = trimmed
        .strip_prefix('v')
        .or_else(|| trimmed.strip_prefix('V'))
        .unwrap_or(trimmed);
    let mut version = semver::Version::parse(stripped)
        .map_err(|_| UpdaterError::InvalidVersion(s.to_string()))?;
    // semver 2.0 spec: build metadata MUST NOT affect version precedence.
    // Rust `semver` crate's Ord implementation differs from spec (it orders builds);
    // strip explicitly to honor the documented policy.
    version.build = semver::BuildMetadata::EMPTY;
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_bump_outdated() {
        assert!(is_outdated("1.0.0", "1.0.1").unwrap());
    }

    #[test]
    fn downgrade_not_outdated() {
        assert!(!is_outdated("1.0.1", "1.0.0").unwrap());
    }

    #[test]
    fn equal_not_outdated() {
        assert!(!is_outdated("1.0.0", "1.0.0").unwrap());
    }

    #[test]
    fn minor_bump_outdated() {
        assert!(is_outdated("1.0.5", "1.1.0").unwrap());
    }

    #[test]
    fn major_bump_outdated() {
        assert!(is_outdated("1.9.9", "2.0.0").unwrap());
    }

    #[test]
    fn pre_release_lower_than_release() {
        // semver spec: `1.0.0-beta < 1.0.0`.
        assert!(is_outdated("1.0.0-beta", "1.0.0").unwrap());
        // 역방향: 1.0.0이 1.0.0-beta보다 높으므로 outdated=false.
        assert!(!is_outdated("1.0.0", "1.0.0-beta").unwrap());
    }

    #[test]
    fn pre_release_ordering() {
        // alpha < beta < rc.
        assert!(is_outdated("1.0.0-alpha", "1.0.0-beta").unwrap());
        assert!(is_outdated("1.0.0-beta", "1.0.0-rc.1").unwrap());
    }

    #[test]
    fn build_metadata_ignored() {
        // semver spec: build metadata는 ordering에 영향 없음.
        assert!(!is_outdated("1.0.0+build1", "1.0.0+build2").unwrap());
        assert!(!is_outdated("1.0.0+build2", "1.0.0+build1").unwrap());
    }

    #[test]
    fn invalid_current_returns_error() {
        let err = is_outdated("not.a.version", "1.0.0").unwrap_err();
        assert!(matches!(err, UpdaterError::InvalidVersion(_)));
        let msg = format!("{err}");
        assert!(msg.contains("not.a.version"));
    }

    #[test]
    fn invalid_latest_returns_error() {
        let err = is_outdated("1.0.0", "garbage").unwrap_err();
        assert!(matches!(err, UpdaterError::InvalidVersion(_)));
        let msg = format!("{err}");
        assert!(msg.contains("garbage"));
    }

    #[test]
    fn empty_string_invalid() {
        assert!(is_outdated("", "1.0.0").is_err());
        assert!(is_outdated("1.0.0", "").is_err());
    }

    #[test]
    fn v_prefix_stripped() {
        // GitHub release tag 관행: "v1.2.3".
        assert!(is_outdated("v1.0.0", "v1.0.1").unwrap());
        assert!(is_outdated("v1.0.0", "1.0.1").unwrap());
        assert!(is_outdated("1.0.0", "v1.0.1").unwrap());
        // 대문자 V도 허용.
        assert!(is_outdated("V1.0.0", "V1.0.1").unwrap());
    }

    #[test]
    fn whitespace_trimmed() {
        assert!(is_outdated("  1.0.0  ", "  1.0.1  ").unwrap());
    }

    #[test]
    fn complex_pre_release_with_build() {
        // pre-release + build → pre-release만 비교에 사용.
        assert!(is_outdated("1.0.0-rc.1+build1", "1.0.0").unwrap());
        assert!(!is_outdated("1.0.0+build1", "1.0.0-rc.1+build2").unwrap());
    }
}
