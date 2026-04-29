//! `TokenQuotaPipeline` — `scope.token_budget` 추적 + 초과 시 reject.
//!
//! 정책 (ADR-0025 §2, ADR-0022 §5):
//! - `ctx.token_budget`이 `Some(b)`이면 token quota 적용. `None`이면 no-op.
//! - request 단계: 본문 토큰 수 추정 후 `tokens_used`에 합산. 한도 초과 시 reject.
//! - response 단계: 응답 토큰 수 추정 후 합산. 응답 후 한도 초과는 audit만 기록 (응답은 이미 클라이언트로).
//!   v1은 conservative — request 시점에서 미리 차단을 우선.
//! - 토큰 추정: 응답에 `usage.total_tokens` 있으면 권장(있다고 가정), 없으면 whitespace-split × 1.3.
//! - 누적 검증: 같은 ctx로 chain 여러 번 호출 시 누적이 유지되어야 함.

use async_trait::async_trait;
use serde_json::Value;

use crate::error::PipelineError;
use crate::pipeline::{AuditEntry, Pipeline, PipelineContext, PipelineStage};

/// Token quota Pipeline.
pub struct TokenQuotaPipeline;

impl Default for TokenQuotaPipeline {
    fn default() -> Self {
        Self
    }
}

impl TokenQuotaPipeline {
    pub const ID: &'static str = "token-quota";

    pub fn new() -> Self {
        Self
    }
}

/// 본문에서 토큰 수를 추정.
///
/// 우선순위:
/// 1. `usage.total_tokens` 또는 `usage.prompt_tokens` (response shape).
/// 2. `messages[].content` whitespace count × 1.3.
/// 3. `choices[].message.content` whitespace count × 1.3.
fn estimate_tokens(body: &Value) -> u64 {
    if let Some(usage) = body.get("usage") {
        if let Some(total) = usage.get("total_tokens").and_then(|v| v.as_u64()) {
            return total;
        }
        let prompt = usage
            .get("prompt_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let completion = usage
            .get("completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if prompt + completion > 0 {
            return prompt + completion;
        }
    }
    let mut words = 0usize;
    if let Some(arr) = body.get("messages").and_then(|v| v.as_array()) {
        for m in arr {
            if let Some(c) = m.get("content").and_then(|v| v.as_str()) {
                words += c.split_whitespace().count();
            }
        }
    }
    if let Some(arr) = body.get("choices").and_then(|v| v.as_array()) {
        for ch in arr {
            if let Some(c) = ch
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|v| v.as_str())
            {
                words += c.split_whitespace().count();
            }
        }
    }
    // word count × 1.3 (heuristic) — float→u64 절상으로 정수 처리.
    ((words as f64) * 1.3).ceil() as u64
}

#[async_trait]
impl Pipeline for TokenQuotaPipeline {
    fn id(&self) -> &str {
        Self::ID
    }
    fn stage(&self) -> PipelineStage {
        PipelineStage::Both
    }

    async fn apply_request(
        &self,
        ctx: &mut PipelineContext,
        body: &mut Value,
    ) -> Result<(), PipelineError> {
        let estimated = estimate_tokens(body);
        match ctx.token_budget {
            None => {
                // budget 없으면 추정만 누적 (audit info), reject 안 함.
                ctx.tokens_used = ctx.tokens_used.saturating_add(estimated);
                ctx.record(AuditEntry::passed(Self::ID));
                Ok(())
            }
            Some(budget) => {
                let projected = ctx.tokens_used.saturating_add(estimated);
                if projected > budget {
                    ctx.record(AuditEntry::blocked(
                        Self::ID,
                        format!("projected {projected} > budget {budget}"),
                    ));
                    tracing::warn!(
                        target: "lmmaster.pipelines",
                        pipeline = Self::ID,
                        request_id = %ctx.request_id,
                        used = ctx.tokens_used,
                        estimated,
                        budget,
                        "request rejected — budget exceeded"
                    );
                    return Err(PipelineError::BudgetExceeded {
                        used: projected,
                        budget,
                    });
                }
                ctx.tokens_used = projected;
                ctx.record(AuditEntry::passed(Self::ID));
                Ok(())
            }
        }
    }

    async fn apply_response(
        &self,
        ctx: &mut PipelineContext,
        body: &mut Value,
    ) -> Result<(), PipelineError> {
        let estimated = estimate_tokens(body);
        ctx.tokens_used = ctx.tokens_used.saturating_add(estimated);
        if let Some(budget) = ctx.token_budget {
            if ctx.tokens_used > budget {
                // 응답은 이미 생성됐으므로 reject 안 하지만 audit으로 보존.
                ctx.record(AuditEntry::modified(
                    Self::ID,
                    format!(
                        "response made tokens_used={} exceed budget={}",
                        ctx.tokens_used, budget
                    ),
                ));
                tracing::warn!(
                    target: "lmmaster.pipelines",
                    pipeline = Self::ID,
                    request_id = %ctx.request_id,
                    used = ctx.tokens_used,
                    budget,
                    "response made budget exceeded (post-hoc, not blocked)"
                );
                return Ok(());
            }
        }
        ctx.record(AuditEntry::passed(Self::ID));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx_with_budget(b: u64) -> PipelineContext {
        let mut c = PipelineContext::new("test-req");
        c.token_budget = Some(b);
        c
    }

    #[tokio::test]
    async fn under_budget_passes() {
        let p = TokenQuotaPipeline::new();
        let mut c = ctx_with_budget(1000);
        let mut body = json!({"messages":[{"role":"user","content":"안녕"}]});
        p.apply_request(&mut c, &mut body).await.unwrap();
        assert!(c.tokens_used > 0);
        assert!(c.tokens_used < 1000);
        assert_eq!(c.audit_log.len(), 1);
        assert_eq!(c.audit_log[0].action, "passed");
    }

    #[tokio::test]
    async fn over_budget_rejects_with_budget_exceeded() {
        let p = TokenQuotaPipeline::new();
        // 매우 작은 budget(=1)에 비해 본문 추정치가 큼.
        let mut c = ctx_with_budget(1);
        let mut body = json!({"messages":[{"role":"user","content":"this prompt has many words and will exceed budget"}]});
        let res = p.apply_request(&mut c, &mut body).await;
        match res {
            Err(PipelineError::BudgetExceeded { used, budget }) => {
                assert_eq!(budget, 1);
                assert!(used > 1);
            }
            other => panic!("expected BudgetExceeded, got {other:?}"),
        }
        // audit log에 blocked 기록.
        assert_eq!(c.audit_log.len(), 1);
        assert_eq!(c.audit_log[0].action, "blocked");
    }

    #[tokio::test]
    async fn no_budget_means_no_op_no_reject() {
        let p = TokenQuotaPipeline::new();
        let mut c = PipelineContext::new("r");
        // budget = None.
        let mut body = json!({"messages":[{"role":"user","content":"a b c d e f g"}]});
        p.apply_request(&mut c, &mut body).await.unwrap();
        // 누적은 됨 (관찰성용).
        assert!(c.tokens_used > 0);
    }

    #[tokio::test]
    async fn accumulates_across_multiple_calls_in_same_context() {
        let p = TokenQuotaPipeline::new();
        let mut c = ctx_with_budget(1000);
        let mut body1 = json!({"messages":[{"role":"user","content":"first call words here"}]});
        let mut body2 = json!({"messages":[{"role":"user","content":"second call words here"}]});
        p.apply_request(&mut c, &mut body1).await.unwrap();
        let after_first = c.tokens_used;
        p.apply_request(&mut c, &mut body2).await.unwrap();
        let after_second = c.tokens_used;
        assert!(
            after_second > after_first,
            "토큰 누적이 유지돼야 해요 (after_first={after_first}, after_second={after_second})"
        );
    }

    #[tokio::test]
    async fn uses_usage_total_tokens_when_present() {
        let p = TokenQuotaPipeline::new();
        let mut c = ctx_with_budget(500);
        let mut body = json!({
            "choices":[{"message":{"role":"assistant","content":"ok"}}],
            "usage":{"total_tokens":100, "prompt_tokens": 40, "completion_tokens": 60}
        });
        p.apply_response(&mut c, &mut body).await.unwrap();
        assert_eq!(c.tokens_used, 100);
    }

    #[tokio::test]
    async fn response_post_hoc_exceed_does_not_reject() {
        let p = TokenQuotaPipeline::new();
        let mut c = ctx_with_budget(50);
        c.tokens_used = 40;
        let mut body = json!({"usage":{"total_tokens":100}});
        // 응답으로 인해 누적이 budget 넘지만 reject 안 됨.
        let res = p.apply_response(&mut c, &mut body).await;
        assert!(res.is_ok());
        assert!(c.tokens_used > 50);
        // post-hoc audit modified.
        let last = c.audit_log.last().unwrap();
        assert_eq!(last.action, "modified");
    }

    #[tokio::test]
    async fn pipeline_id_and_stage_correct() {
        let p = TokenQuotaPipeline::new();
        assert_eq!(p.id(), "token-quota");
        assert_eq!(p.stage(), PipelineStage::Both);
    }

    #[tokio::test]
    async fn empty_body_with_budget_is_ok_with_zero_tokens() {
        let p = TokenQuotaPipeline::new();
        let mut c = ctx_with_budget(100);
        let mut body = json!({});
        p.apply_request(&mut c, &mut body).await.unwrap();
        assert_eq!(c.tokens_used, 0);
    }
}
