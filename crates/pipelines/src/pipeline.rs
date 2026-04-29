//! Pipeline trait + 컨텍스트 구조체.
//!
//! 정책 (ADR-0025 §1, phase-6p-updater-pipelines-decision.md §4.3):
//! - `Pipeline`은 `Send + Sync` async-trait — `Arc<dyn Pipeline>`으로 chain에 보관.
//! - `apply_request`/`apply_response` 두 단계 분리. 한 단계만 처리하는 Pipeline은 다른 단계에서 Ok(()) 반환.
//! - `PipelineContext`는 chain 내 Pipeline 간 데이터 전달 (token quota 누적 등).
//! - `AuditEntry`는 Pipeline 실행 결과 (passed/modified/blocked) 1줄 기록 — chain 끝나면 tracing emit.
//! - chain 처리 위치는 v1에서 *full response*만. SSE relay는 별도 우회 (게이트웨이 layer 책임).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

use crate::error::PipelineError;

/// Pipeline이 어떤 단계(들)에 적용되는지.
///
/// `Both`이면 chain은 request/response 양 단계에서 호출. `Request`/`Response`만이면
/// 다른 단계는 Ok(()) 반환하는 어댑터로 처리할 수도 있고, chain 빌더가 호출 자체를 skip할 수도 있어요.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PipelineStage {
    Request,
    Response,
    Both,
}

/// Pipeline 인터페이스.
#[async_trait]
pub trait Pipeline: Send + Sync {
    /// 식별자 (audit log + per-route activation 키).
    fn id(&self) -> &str;

    /// 어느 단계에 적용되는지.
    fn stage(&self) -> PipelineStage;

    /// request body 변경/거부.
    async fn apply_request(
        &self,
        ctx: &mut PipelineContext,
        body: &mut Value,
    ) -> Result<(), PipelineError>;

    /// response body 변경/거부.
    async fn apply_response(
        &self,
        ctx: &mut PipelineContext,
        body: &mut Value,
    ) -> Result<(), PipelineError>;
}

/// Pipeline 실행 컨텍스트 — chain 내 Pipeline 간 공유.
///
/// `audit_log`는 Pipeline이 push. `token_budget`/`tokens_used`는 `TokenQuotaPipeline`이 mutate.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PipelineContext {
    pub request_id: String,
    pub project_id: Option<String>,
    pub model: Option<String>,
    pub user_agent: Option<String>,
    /// `scope.token_budget` (ADR-0022 §5). `None`이면 budget 검사 skip.
    pub token_budget: Option<u64>,
    /// 누적 토큰 — `TokenQuotaPipeline`이 매 호출마다 증가시킴.
    pub tokens_used: u64,
    pub audit_log: Vec<AuditEntry>,
    /// Phase 8'.c.3 (ADR-0029) — 인증된 키의 Pipeline 화이트리스트.
    ///
    /// `None` = 전역 토글을 그대로 따름. `Some(Vec)` = 명시 override
    /// (해당 ID에 포함된 Pipeline만 chain에 반영). `Some(빈 Vec)` = 모든 Pipeline 비활성.
    /// `PipelineLayer`가 chain.apply_request 전에 sub-chain으로 필터링.
    pub principal_key_pipelines: Option<Vec<String>>,
}

impl PipelineContext {
    /// 빈 audit log + 기본값으로 컨텍스트 생성.
    pub fn new(request_id: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            ..Default::default()
        }
    }

    /// audit 항목 추가 (passed/modified/blocked).
    pub fn record(&mut self, entry: AuditEntry) {
        self.audit_log.push(entry);
    }
}

/// 단일 Pipeline 실행 결과 audit 1줄.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub pipeline_id: String,
    /// "passed" / "modified" / "blocked".
    pub action: String,
    #[serde(with = "time::serde::rfc3339")]
    pub timestamp: OffsetDateTime,
    pub details: Option<String>,
}

impl AuditEntry {
    /// 변경 없이 통과한 경우.
    pub fn passed(pipeline_id: impl Into<String>) -> Self {
        Self {
            pipeline_id: pipeline_id.into(),
            action: "passed".into(),
            timestamp: OffsetDateTime::now_utc(),
            details: None,
        }
    }

    /// body가 변경된 경우 (PII redact / sanitize 등).
    pub fn modified(pipeline_id: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            pipeline_id: pipeline_id.into(),
            action: "modified".into(),
            timestamp: OffsetDateTime::now_utc(),
            details: Some(details.into()),
        }
    }

    /// 차단된 경우 (chain 중단).
    pub fn blocked(pipeline_id: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            pipeline_id: pipeline_id.into(),
            action: "blocked".into(),
            timestamp: OffsetDateTime::now_utc(),
            details: Some(details.into()),
        }
    }

    /// `timestamp`을 RFC3339 ISO 문자열로 포맷.
    ///
    /// Phase 6'.d — Tauri 측 `AuditEntryDto`의 `timestamp_iso` 필드와 1:1 대응.
    /// 직접 의존성을 만들지 않기 위해 변환 헬퍼만 제공해요.
    /// format 실패 시 빈 문자열 반환 (정상 환경에서는 발생 X).
    pub fn timestamp_iso(&self) -> String {
        self.timestamp
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_context_new_sets_request_id_and_defaults() {
        let ctx = PipelineContext::new("req-123");
        assert_eq!(ctx.request_id, "req-123");
        assert!(ctx.project_id.is_none());
        assert!(ctx.audit_log.is_empty());
        assert_eq!(ctx.tokens_used, 0);
    }

    #[test]
    fn pipeline_context_record_appends_audit_in_order() {
        let mut ctx = PipelineContext::new("r");
        ctx.record(AuditEntry::passed("a"));
        ctx.record(AuditEntry::modified("b", "redacted email"));
        ctx.record(AuditEntry::blocked("c", "budget exceeded"));
        assert_eq!(ctx.audit_log.len(), 3);
        assert_eq!(ctx.audit_log[0].pipeline_id, "a");
        assert_eq!(ctx.audit_log[1].action, "modified");
        assert_eq!(ctx.audit_log[2].action, "blocked");
        assert_eq!(ctx.audit_log[2].details.as_deref(), Some("budget exceeded"));
    }

    #[test]
    fn audit_entry_serde_round_trip_preserves_fields() {
        let entry = AuditEntry::modified("pii-redact", "redacted 1 emails");
        let json = serde_json::to_string(&entry).expect("serialize");
        let back: AuditEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.pipeline_id, "pii-redact");
        assert_eq!(back.action, "modified");
        assert_eq!(back.details.as_deref(), Some("redacted 1 emails"));
    }

    #[test]
    fn pipeline_stage_serde_kebab_case() {
        let req = serde_json::to_string(&PipelineStage::Request).unwrap();
        let resp = serde_json::to_string(&PipelineStage::Response).unwrap();
        let both = serde_json::to_string(&PipelineStage::Both).unwrap();
        assert_eq!(req, "\"request\"");
        assert_eq!(resp, "\"response\"");
        assert_eq!(both, "\"both\"");

        let back: PipelineStage = serde_json::from_str("\"request\"").unwrap();
        assert_eq!(back, PipelineStage::Request);
    }

    // ── Phase 6'.d — timestamp_iso ───────────────────────────────────────

    #[test]
    fn audit_entry_timestamp_iso_returns_rfc3339_with_t_and_z() {
        let entry = AuditEntry::passed("a");
        let iso = entry.timestamp_iso();
        // RFC3339의 핵심 마커 — 'T' 구분자 + UTC 'Z' (또는 ±offset).
        assert!(iso.contains('T'), "ISO 문자열에 'T'가 있어야 해요: {iso}");
        assert!(
            iso.contains('Z') || iso.contains('+') || iso.contains('-'),
            "RFC3339 timezone marker가 있어야 해요: {iso}"
        );
        assert!(!iso.is_empty());
    }

    #[test]
    fn audit_entry_timestamp_iso_round_trips_through_serde() {
        // serde 직렬화의 RFC3339와 timestamp_iso() 결과가 동일해야 dto 변환이 일관돼요.
        let entry = AuditEntry::modified("p", "redacted");
        let iso = entry.timestamp_iso();
        // serde 직렬화로 timestamp 필드만 추출.
        let json: Value = serde_json::to_value(&entry).expect("to_value");
        let serde_iso = json
            .get("timestamp")
            .and_then(|v| v.as_str())
            .expect("timestamp str")
            .to_string();
        assert_eq!(iso, serde_iso);
    }

    #[test]
    fn audit_entry_timestamp_iso_each_variant_has_iso() {
        // passed / modified / blocked 모두 timestamp_iso가 비어있지 않아야 해요.
        let p = AuditEntry::passed("p1");
        let m = AuditEntry::modified("p2", "d");
        let b = AuditEntry::blocked("p3", "r");
        for entry in [&p, &m, &b] {
            let iso = entry.timestamp_iso();
            assert!(!iso.is_empty(), "{} timestamp_iso empty", entry.pipeline_id);
            assert!(iso.contains('T'));
        }
    }

    #[test]
    fn audit_entry_timestamp_iso_parseable_back_to_offset_datetime() {
        // 출력된 ISO를 다시 파싱했을 때 같은 시점이 복원되어야 변환이 lossless.
        let entry = AuditEntry::passed("x");
        let iso = entry.timestamp_iso();
        let parsed =
            OffsetDateTime::parse(&iso, &time::format_description::well_known::Rfc3339).unwrap();
        // unix_timestamp 기준으로 동일 (sub-second는 RFC3339 포맷 정밀도 내).
        assert_eq!(parsed.unix_timestamp(), entry.timestamp.unix_timestamp());
    }
}
