//! Pipeline chain — ordered execution wrapper.
//!
//! 정책 (ADR-0025 §1, phase-6p-updater-pipelines-decision.md §5):
//! - `apply_request`은 forward 순서 (insertion 순). `apply_response`은 reverse 순서 (LIFO).
//! - 한 Pipeline이 `Err` 반환 시 chain 중단 + audit 기록 보존.
//! - 빈 chain은 no-op (Ok).
//! - chain은 stateless. `PipelineContext`는 호출자가 보관.

use std::sync::Arc;

use serde_json::Value;

use crate::error::PipelineError;
use crate::pipeline::{Pipeline, PipelineContext, PipelineStage};

/// `Vec<Arc<dyn Pipeline>>` 위에 builder + 실행 메서드를 얇게 얹은 wrapper.
#[derive(Clone, Default)]
pub struct PipelineChain {
    pipelines: Vec<Arc<dyn Pipeline>>,
}

impl PipelineChain {
    pub fn new() -> Self {
        Self {
            pipelines: Vec::new(),
        }
    }

    /// builder — 한 Pipeline 추가. (clippy::should_implement_trait는 builder pattern이라 무시.)
    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, p: Arc<dyn Pipeline>) -> Self {
        self.pipelines.push(p);
        self
    }

    /// 등록된 Pipeline 슬라이스 (audit / 디버깅 용).
    pub fn pipelines(&self) -> &[Arc<dyn Pipeline>] {
        &self.pipelines
    }

    /// chain이 비어 있는지.
    pub fn is_empty(&self) -> bool {
        self.pipelines.is_empty()
    }

    /// chain 길이.
    pub fn len(&self) -> usize {
        self.pipelines.len()
    }

    /// request 단계 — forward 순으로 실행. 한 Pipeline이 Err 반환 시 즉시 중단.
    ///
    /// stage가 `Response`이면 skip. `Both` / `Request`는 호출.
    pub async fn apply_request(
        &self,
        ctx: &mut PipelineContext,
        body: &mut Value,
    ) -> Result<(), PipelineError> {
        for p in &self.pipelines {
            if matches!(p.stage(), PipelineStage::Response) {
                continue;
            }
            p.apply_request(ctx, body).await?;
        }
        Ok(())
    }

    /// response 단계 — reverse 순으로 실행 (LIFO). 한 Pipeline이 Err 반환 시 즉시 중단.
    ///
    /// stage가 `Request`이면 skip. `Both` / `Response`는 호출.
    pub async fn apply_response(
        &self,
        ctx: &mut PipelineContext,
        body: &mut Value,
    ) -> Result<(), PipelineError> {
        for p in self.pipelines.iter().rev() {
            if matches!(p.stage(), PipelineStage::Request) {
                continue;
            }
            p.apply_response(ctx, body).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::AuditEntry;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    /// 호출 순서를 기록하는 테스트용 Pipeline.
    struct TestPipeline {
        id: String,
        order_log: Arc<Mutex<Vec<String>>>,
        fail_on_request: bool,
    }

    #[async_trait]
    impl Pipeline for TestPipeline {
        fn id(&self) -> &str {
            &self.id
        }
        fn stage(&self) -> PipelineStage {
            PipelineStage::Both
        }
        async fn apply_request(
            &self,
            ctx: &mut PipelineContext,
            _body: &mut Value,
        ) -> Result<(), PipelineError> {
            self.order_log
                .lock()
                .unwrap()
                .push(format!("req:{}", self.id));
            if self.fail_on_request {
                ctx.record(AuditEntry::blocked(&self.id, "fail"));
                return Err(PipelineError::Blocked {
                    pipeline: self.id.clone(),
                    reason: "fail".into(),
                });
            }
            ctx.record(AuditEntry::passed(&self.id));
            Ok(())
        }
        async fn apply_response(
            &self,
            ctx: &mut PipelineContext,
            _body: &mut Value,
        ) -> Result<(), PipelineError> {
            self.order_log
                .lock()
                .unwrap()
                .push(format!("resp:{}", self.id));
            ctx.record(AuditEntry::passed(&self.id));
            Ok(())
        }
    }

    fn make(id: &str, log: Arc<Mutex<Vec<String>>>) -> Arc<dyn Pipeline> {
        Arc::new(TestPipeline {
            id: id.into(),
            order_log: log,
            fail_on_request: false,
        })
    }

    #[tokio::test]
    async fn empty_chain_is_ok_no_op() {
        let chain = PipelineChain::new();
        let mut ctx = PipelineContext::new("r");
        let mut body = serde_json::json!({"messages": []});
        let snapshot = body.clone();
        chain.apply_request(&mut ctx, &mut body).await.unwrap();
        chain.apply_response(&mut ctx, &mut body).await.unwrap();
        assert_eq!(body, snapshot);
        assert!(ctx.audit_log.is_empty());
    }

    #[tokio::test]
    async fn single_pipeline_runs_once_in_each_stage() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let chain = PipelineChain::new().add(make("a", log.clone()));
        let mut ctx = PipelineContext::new("r");
        let mut body = serde_json::json!({});

        chain.apply_request(&mut ctx, &mut body).await.unwrap();
        chain.apply_response(&mut ctx, &mut body).await.unwrap();

        let order = log.lock().unwrap().clone();
        assert_eq!(order, vec!["req:a", "resp:a"]);
        assert_eq!(ctx.audit_log.len(), 2);
    }

    #[tokio::test]
    async fn request_runs_forward_response_runs_reverse() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let chain = PipelineChain::new()
            .add(make("a", log.clone()))
            .add(make("b", log.clone()))
            .add(make("c", log.clone()));
        let mut ctx = PipelineContext::new("r");
        let mut body = serde_json::json!({});

        chain.apply_request(&mut ctx, &mut body).await.unwrap();
        chain.apply_response(&mut ctx, &mut body).await.unwrap();

        let order = log.lock().unwrap().clone();
        // forward = a, b, c; reverse = c, b, a.
        assert_eq!(
            order,
            vec!["req:a", "req:b", "req:c", "resp:c", "resp:b", "resp:a"]
        );
    }

    #[tokio::test]
    async fn error_short_circuits_chain() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let failing = Arc::new(TestPipeline {
            id: "fail".into(),
            order_log: log.clone(),
            fail_on_request: true,
        });
        let chain = PipelineChain::new()
            .add(make("a", log.clone()))
            .add(failing)
            .add(make("c", log.clone()));
        let mut ctx = PipelineContext::new("r");
        let mut body = serde_json::json!({});

        let res = chain.apply_request(&mut ctx, &mut body).await;
        assert!(matches!(res, Err(PipelineError::Blocked { .. })));

        // c는 호출되지 않아야 해요.
        let order = log.lock().unwrap().clone();
        assert_eq!(order, vec!["req:a", "req:fail"]);
        // audit_log는 a (passed) + fail (blocked) — 2 entries.
        assert_eq!(ctx.audit_log.len(), 2);
        assert_eq!(ctx.audit_log[0].action, "passed");
        assert_eq!(ctx.audit_log[1].action, "blocked");
    }

    #[tokio::test]
    async fn audit_log_populated_per_pipeline() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let chain = PipelineChain::new()
            .add(make("a", log.clone()))
            .add(make("b", log));
        let mut ctx = PipelineContext::new("r");
        let mut body = serde_json::json!({});
        chain.apply_request(&mut ctx, &mut body).await.unwrap();
        assert_eq!(ctx.audit_log.len(), 2);
        assert_eq!(ctx.audit_log[0].pipeline_id, "a");
        assert_eq!(ctx.audit_log[1].pipeline_id, "b");
    }

    /// stage-specific Pipeline은 다른 stage에서 skip되어야 함.
    struct ReqOnlyPipeline {
        id: String,
        called_request: AtomicUsize,
        called_response: AtomicUsize,
    }
    #[async_trait]
    impl Pipeline for ReqOnlyPipeline {
        fn id(&self) -> &str {
            &self.id
        }
        fn stage(&self) -> PipelineStage {
            PipelineStage::Request
        }
        async fn apply_request(
            &self,
            _ctx: &mut PipelineContext,
            _body: &mut Value,
        ) -> Result<(), PipelineError> {
            self.called_request.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn apply_response(
            &self,
            _ctx: &mut PipelineContext,
            _body: &mut Value,
        ) -> Result<(), PipelineError> {
            self.called_response.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn stage_request_only_skips_response_stage() {
        let req_only = Arc::new(ReqOnlyPipeline {
            id: "req-only".into(),
            called_request: AtomicUsize::new(0),
            called_response: AtomicUsize::new(0),
        });
        let chain = PipelineChain::new().add(req_only.clone());
        let mut ctx = PipelineContext::new("r");
        let mut body = serde_json::json!({});

        chain.apply_request(&mut ctx, &mut body).await.unwrap();
        chain.apply_response(&mut ctx, &mut body).await.unwrap();

        assert_eq!(req_only.called_request.load(Ordering::SeqCst), 1);
        assert_eq!(
            req_only.called_response.load(Ordering::SeqCst),
            0,
            "stage=Request인 Pipeline은 response 단계에서 skip되어야 해요"
        );
    }
}
