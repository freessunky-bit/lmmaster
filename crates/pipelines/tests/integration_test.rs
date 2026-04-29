//! Phase 6'.b — pipelines crate 통합 테스트.
//!
//! 검증 invariant (ADR-0025 §6 + phase-6p-updater-pipelines-decision.md §5):
//! - 3종 시드 chain (PiiRedact → TokenQuota → Observability) 풀 실행 정상.
//! - budget 초과 시 chain 중단 + 그 전까지의 audit 보존.
//! - request forward / response reverse 순서.
//! - PipelineContext / AuditEntry serde round-trip.
//! - 빈 chain은 body 변경 없음.

use std::sync::Arc;

use pipelines::{
    AuditEntry, ObservabilityPipeline, PiiRedactPipeline, PipelineChain, PipelineContext,
    PipelineError, PipelineStage, TokenQuotaPipeline,
};
use serde_json::json;

#[tokio::test]
async fn full_chain_pii_quota_observability_on_mock_request() {
    let chain = PipelineChain::new()
        .add(Arc::new(PiiRedactPipeline::new()))
        .add(Arc::new(TokenQuotaPipeline::new()))
        .add(Arc::new(ObservabilityPipeline::new()));

    let mut ctx = PipelineContext::new("req-int-1");
    ctx.project_id = Some("proj-int".into());
    ctx.model = Some("exaone".into());
    ctx.token_budget = Some(10_000);

    let mut request_body = json!({
        "model": "exaone",
        "messages": [
            {"role": "system", "content": "친절하게 한국어로 답해주세요."},
            {"role": "user", "content": "내 이메일 alice@example.com 으로 회신 주세요. 010-1234-5678도 가능해요."}
        ]
    });

    chain
        .apply_request(&mut ctx, &mut request_body)
        .await
        .expect("chain apply_request must succeed");

    // PII redact 검증.
    let user_msg = request_body["messages"][1]["content"].as_str().unwrap();
    assert!(
        user_msg.contains("[REDACTED-이메일]"),
        "email must be redacted: {user_msg}"
    );
    assert!(
        user_msg.contains("[REDACTED-휴대폰]"),
        "phone must be redacted: {user_msg}"
    );
    assert!(!user_msg.contains("alice@example.com"));

    // 3 Pipeline × 1 stage = 3 audit entries (PiiRedact modified + TokenQuota passed + Observability passed).
    assert_eq!(ctx.audit_log.len(), 3);
    assert_eq!(ctx.audit_log[0].pipeline_id, "pii-redact");
    assert_eq!(ctx.audit_log[0].action, "modified");
    assert_eq!(ctx.audit_log[1].pipeline_id, "token-quota");
    assert_eq!(ctx.audit_log[1].action, "passed");
    assert_eq!(ctx.audit_log[2].pipeline_id, "observability");

    // tokens_used 누적.
    assert!(ctx.tokens_used > 0);
}

#[tokio::test]
async fn full_chain_response_runs_in_reverse() {
    let chain = PipelineChain::new()
        .add(Arc::new(PiiRedactPipeline::new()))
        .add(Arc::new(TokenQuotaPipeline::new()))
        .add(Arc::new(ObservabilityPipeline::new()));

    let mut ctx = PipelineContext::new("req-int-2");
    ctx.token_budget = Some(10_000);

    let mut response_body = json!({
        "id": "chatcmpl-1",
        "choices": [
            {"index": 0, "message": {"role": "assistant", "content": "이메일 user@example.com 로 보내드릴게요."}}
        ],
        "usage": {"total_tokens": 50}
    });

    chain
        .apply_response(&mut ctx, &mut response_body)
        .await
        .expect("chain apply_response must succeed");

    // PII redact 검증.
    let assistant_msg = response_body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap();
    assert!(assistant_msg.contains("[REDACTED-이메일]"));

    // reverse 순서 — observability(2) → quota(1) → pii(0).
    assert_eq!(ctx.audit_log.len(), 3);
    assert_eq!(ctx.audit_log[0].pipeline_id, "observability");
    assert_eq!(ctx.audit_log[1].pipeline_id, "token-quota");
    assert_eq!(ctx.audit_log[2].pipeline_id, "pii-redact");
}

#[tokio::test]
async fn budget_exceeded_short_circuits_with_partial_audit_preserved() {
    let chain = PipelineChain::new()
        .add(Arc::new(PiiRedactPipeline::new()))
        .add(Arc::new(TokenQuotaPipeline::new()))
        .add(Arc::new(ObservabilityPipeline::new()));

    let mut ctx = PipelineContext::new("req-int-3");
    // budget = 1로 매우 작게 설정 → token-quota Pipeline이 차단해야 함.
    ctx.token_budget = Some(1);
    ctx.tokens_used = 0;

    let mut body = json!({
        "messages":[
            {"role":"user","content":"이 프롬프트는 budget=1을 초과해서 차단되어야 해요. alice@example.com 도 redact 되었어야 해요."}
        ]
    });

    let res = chain.apply_request(&mut ctx, &mut body).await;
    assert!(matches!(res, Err(PipelineError::BudgetExceeded { .. })));

    // 1) PII redact는 차단 *전에* 적용되어야 함 (forward 순서).
    let s = body["messages"][0]["content"].as_str().unwrap();
    assert!(
        s.contains("[REDACTED-이메일]"),
        "forward 순서이므로 quota 차단 전에 PII redact가 끝났어야 해요. got: {s}"
    );

    // 2) audit log: pii (modified) → quota (blocked). Observability는 호출되지 않아야 함.
    assert_eq!(ctx.audit_log.len(), 2);
    assert_eq!(ctx.audit_log[0].pipeline_id, "pii-redact");
    assert_eq!(ctx.audit_log[0].action, "modified");
    assert_eq!(ctx.audit_log[1].pipeline_id, "token-quota");
    assert_eq!(ctx.audit_log[1].action, "blocked");
}

#[tokio::test]
async fn pipeline_context_serde_round_trip() {
    let mut ctx = PipelineContext::new("req-rt");
    ctx.project_id = Some("proj-rt".into());
    ctx.model = Some("exaone".into());
    ctx.user_agent = Some("Mozilla".into());
    ctx.token_budget = Some(8000);
    ctx.tokens_used = 42;
    ctx.audit_log.push(AuditEntry::passed("a"));
    ctx.audit_log.push(AuditEntry::modified("b", "redacted 2"));
    ctx.audit_log.push(AuditEntry::blocked("c", "boom"));

    let json = serde_json::to_string(&ctx).expect("serialize");
    let back: PipelineContext = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(back.request_id, "req-rt");
    assert_eq!(back.project_id.as_deref(), Some("proj-rt"));
    assert_eq!(back.token_budget, Some(8000));
    assert_eq!(back.tokens_used, 42);
    assert_eq!(back.audit_log.len(), 3);
    assert_eq!(back.audit_log[2].action, "blocked");
    assert_eq!(back.audit_log[1].details.as_deref(), Some("redacted 2"));
}

#[tokio::test]
async fn empty_chain_is_no_op_on_body() {
    let chain = PipelineChain::new();
    assert_eq!(chain.len(), 0);
    assert!(chain.is_empty());

    let mut ctx = PipelineContext::new("r");
    let mut body = json!({"messages":[{"role":"user","content":"unchanged 010-1234-5678"}]});
    let snapshot = body.clone();
    chain.apply_request(&mut ctx, &mut body).await.unwrap();
    chain.apply_response(&mut ctx, &mut body).await.unwrap();
    assert_eq!(body, snapshot);
    assert!(ctx.audit_log.is_empty());
    assert_eq!(ctx.tokens_used, 0);
}

#[tokio::test]
async fn pipelines_slice_exposes_inserted_pipelines_in_order() {
    let chain = PipelineChain::new()
        .add(Arc::new(PiiRedactPipeline::new()))
        .add(Arc::new(TokenQuotaPipeline::new()))
        .add(Arc::new(ObservabilityPipeline::new()));

    let ids: Vec<&str> = chain.pipelines().iter().map(|p| p.id()).collect();
    assert_eq!(ids, vec!["pii-redact", "token-quota", "observability"]);
    let stages: Vec<PipelineStage> = chain.pipelines().iter().map(|p| p.stage()).collect();
    assert!(stages.iter().all(|s| matches!(s, PipelineStage::Both)));
}
