//! `ApiKeyScope` — 5차원 권한 (ADR-0022 §5).
//!
//! 정책:
//! - models / endpoints: glob 매칭 (`*`, `?`만, `**`은 미지원).
//! - allowed_origins: 정확 매칭 (scheme + host + port 일치).
//! - expires_at: RFC3339 ISO 시각, None = 무기한.
//! - rate_limit / project_id: schema만 — enforce는 v1.1.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// 권한 — `ApiKey.scope` 필드.
///
/// Phase 8'.c.3 (ADR-0029): `enabled_pipelines`로 per-key Pipelines override를 추가했어요.
/// `None` = 전역 토글을 그대로 따름. `Some(Vec)` = 명시 화이트리스트로 글로벌 토글을 덮어써요.
/// `Some(빈 Vec)` = 모든 Pipeline 비활성 (이 키만 raw passthrough).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Scope {
    /// glob 패턴. 빈 vec = 거부 (어떤 모델도 호출 불가).
    pub models: Vec<String>,
    /// glob 패턴. 빈 vec = 거부.
    pub endpoints: Vec<String>,
    /// 정확 매칭. 빈 vec = 어떤 origin도 거부 (단, header 없는 server-to-server는 정책 분기).
    pub allowed_origins: Vec<String>,
    /// RFC3339. None = 무기한.
    pub expires_at: Option<String>,
    /// Phase 6' (v1은 None).
    pub project_id: Option<String>,
    /// schema만 — enforce v1.1.
    pub rate_limit: Option<RateLimit>,
    /// Phase 8'.c.3 — 이 키에만 적용할 Pipeline ID 화이트리스트.
    /// `None` (default) = 전역 설정 따름. `Some(Vec)` = 명시 override.
    /// `serde(default)`로 기존 직렬화된 키와 호환 — 누락된 필드는 None으로 deserialize.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled_pipelines: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RateLimit {
    pub per_minute: Option<u32>,
    pub per_day: Option<u64>,
}

impl Scope {
    /// 모델 ID가 scope.models의 어떤 glob에 매치되는가.
    pub fn allows_model(&self, model: &str) -> bool {
        self.models.iter().any(|p| glob_match(p, model))
    }

    /// 요청 path가 scope.endpoints의 어떤 glob에 매치되는가.
    pub fn allows_endpoint(&self, path: &str) -> bool {
        self.endpoints.iter().any(|p| glob_match(p, path))
    }

    /// Origin 헤더 값(`https://x.com:443`)이 정확히 매치되는가.
    pub fn allows_origin(&self, origin: &str) -> bool {
        self.allowed_origins.iter().any(|o| o == origin)
    }

    /// 만료 여부 — `expires_at` 파싱 + 현재시각 비교. None = 무기한.
    pub fn is_expired(&self, now: OffsetDateTime) -> bool {
        let Some(s) = self.expires_at.as_deref() else {
            return false;
        };
        match OffsetDateTime::parse(s, &time::format_description::well_known::Rfc3339) {
            Ok(at) => now >= at,
            Err(_) => true, // parse 실패 → 안전하게 만료 처리.
        }
    }
}

/// 단순 glob 매칭 — `*` (any chars), `?` (single char), 그 외 literal.
///
/// `**` 재귀 매칭은 미지원 — endpoint/model glob에 불필요.
/// 빈 패턴은 빈 입력만 매치.
pub fn glob_match(pattern: &str, input: &str) -> bool {
    glob_match_rec(pattern.as_bytes(), input.as_bytes())
}

fn glob_match_rec(pat: &[u8], input: &[u8]) -> bool {
    if pat.is_empty() {
        return input.is_empty();
    }
    if pat[0] == b'*' {
        // 0개 이상 매칭 — greedy + backtrack.
        if pat.len() == 1 {
            return true; // trailing *
        }
        for i in 0..=input.len() {
            if glob_match_rec(&pat[1..], &input[i..]) {
                return true;
            }
        }
        return false;
    }
    if input.is_empty() {
        return false;
    }
    if pat[0] == b'?' || pat[0] == input[0] {
        return glob_match_rec(&pat[1..], &input[1..]);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_literal_match() {
        assert!(glob_match("exaone", "exaone"));
        assert!(!glob_match("exaone", "qwen"));
    }

    #[test]
    fn glob_star_prefix() {
        assert!(glob_match("exaone-*", "exaone-3.5-7.8b"));
        assert!(!glob_match("exaone-*", "qwen-3b"));
        assert!(glob_match("exaone-*", "exaone-")); // 0자도 매치
    }

    #[test]
    fn glob_star_suffix() {
        assert!(glob_match("*.gguf", "model.gguf"));
        assert!(!glob_match("*.gguf", "model.bin"));
    }

    #[test]
    fn glob_star_middle() {
        assert!(glob_match("exaone-*-instruct", "exaone-3.5-7.8b-instruct"));
        assert!(!glob_match("exaone-*-instruct", "exaone-3.5-7.8b-base"));
    }

    #[test]
    fn glob_question_mark() {
        assert!(glob_match("v?", "v1"));
        assert!(!glob_match("v?", "v10"));
    }

    #[test]
    fn glob_full_wildcard() {
        assert!(glob_match("*", ""));
        assert!(glob_match("*", "anything"));
    }

    #[test]
    fn glob_endpoint_pattern() {
        assert!(glob_match("/v1/chat/*", "/v1/chat/completions"));
        assert!(!glob_match("/v1/chat/*", "/v1/embeddings"));
        assert!(glob_match("/v1/*", "/v1/models"));
    }

    #[test]
    fn allows_origin_exact_match() {
        let s = Scope {
            allowed_origins: vec!["https://x.com".into(), "http://localhost:5173".into()],
            ..Default::default()
        };
        assert!(s.allows_origin("https://x.com"));
        assert!(s.allows_origin("http://localhost:5173"));
        // port mismatch.
        assert!(!s.allows_origin("https://x.com:443"));
        // scheme mismatch.
        assert!(!s.allows_origin("http://x.com"));
        // host mismatch.
        assert!(!s.allows_origin("https://y.com"));
        // null/empty.
        assert!(!s.allows_origin(""));
    }

    #[test]
    fn empty_scope_denies_everything() {
        let s = Scope::default();
        assert!(!s.allows_model("anything"));
        assert!(!s.allows_endpoint("/v1/chat/completions"));
        assert!(!s.allows_origin("https://x.com"));
    }

    #[test]
    fn full_wildcard_allows_everything() {
        let s = Scope {
            models: vec!["*".into()],
            endpoints: vec!["*".into()],
            ..Default::default()
        };
        assert!(s.allows_model("any-model"));
        assert!(s.allows_endpoint("/any/path"));
    }

    #[test]
    fn is_expired_unset_is_false() {
        let s = Scope::default();
        assert!(!s.is_expired(OffsetDateTime::now_utc()));
    }

    #[test]
    fn is_expired_past_is_true() {
        let s = Scope {
            expires_at: Some("2000-01-01T00:00:00Z".into()),
            ..Default::default()
        };
        assert!(s.is_expired(OffsetDateTime::now_utc()));
    }

    #[test]
    fn is_expired_future_is_false() {
        let s = Scope {
            expires_at: Some("2099-12-31T23:59:59Z".into()),
            ..Default::default()
        };
        assert!(!s.is_expired(OffsetDateTime::now_utc()));
    }

    #[test]
    fn is_expired_invalid_format_treated_as_expired() {
        // 안전 정책 — parse 실패는 만료로.
        let s = Scope {
            expires_at: Some("not-a-date".into()),
            ..Default::default()
        };
        assert!(s.is_expired(OffsetDateTime::now_utc()));
    }

    // ── Phase 8'.c.3 — enabled_pipelines per-key override ──────────────

    #[test]
    fn enabled_pipelines_default_is_none() {
        let s = Scope::default();
        assert!(s.enabled_pipelines.is_none());
    }

    #[test]
    fn enabled_pipelines_serde_round_trip_with_some() {
        let s = Scope {
            enabled_pipelines: Some(vec!["pii-redact".into(), "token-quota".into()]),
            ..Default::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("enabled_pipelines"));
        let back: Scope = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back.enabled_pipelines,
            Some(vec!["pii-redact".to_string(), "token-quota".to_string()])
        );
    }

    #[test]
    fn enabled_pipelines_serde_round_trip_with_empty_vec() {
        // 빈 vec = 모든 pipeline 비활성. 기존 None과 다른 의미.
        let s = Scope {
            enabled_pipelines: Some(vec![]),
            ..Default::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Scope = serde_json::from_str(&json).unwrap();
        assert_eq!(back.enabled_pipelines, Some(vec![]));
    }

    #[test]
    fn enabled_pipelines_omitted_in_legacy_json_deserializes_none() {
        // 기존 v1 데이터 호환 — JSON에 필드 자체가 없을 때 None으로.
        let legacy =
            r#"{"models":["*"],"endpoints":["/v1/*"],"allowed_origins":["http://localhost:5173"]}"#;
        let s: Scope = serde_json::from_str(legacy).unwrap();
        assert!(s.enabled_pipelines.is_none());
        assert_eq!(s.models, vec!["*"]);
    }

    #[test]
    fn enabled_pipelines_none_is_skipped_in_serialize() {
        // skip_serializing_if = "Option::is_none" → JSON 출력에 필드 자체가 없어요.
        let s = Scope {
            models: vec!["*".into()],
            ..Default::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(
            !json.contains("enabled_pipelines"),
            "None은 직렬화에서 제외돼야 해요: {json}"
        );
    }
}
