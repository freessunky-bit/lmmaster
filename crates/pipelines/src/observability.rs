//! `ObservabilityPipeline` — request_id / project_id / model / pipeline_id 트레이싱.
//!
//! 정책 (ADR-0025 §4):
//! - 매 stage마다 `tracing::info!`로 1줄 (target = "lmmaster.pipelines").
//! - prompt/completion length를 분석용으로 기록.
//! - 순수 side-effect — body는 변경하지 않음. 항상 Ok.
//! - audit_log에 `passed` 1 entry 추가.

use async_trait::async_trait;
use serde_json::Value;

use crate::error::PipelineError;
use crate::pipeline::{AuditEntry, Pipeline, PipelineContext, PipelineStage};

/// Observability Pipeline.
pub struct ObservabilityPipeline;

impl Default for ObservabilityPipeline {
    fn default() -> Self {
        Self
    }
}

impl ObservabilityPipeline {
    pub const ID: &'static str = "observability";

    pub fn new() -> Self {
        Self
    }
}

fn prompt_length(body: &Value) -> usize {
    body.get("messages")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|m| {
                    m.get("content")
                        .and_then(|c| c.as_str())
                        .map(|s| s.len())
                        .unwrap_or(0)
                })
                .sum()
        })
        .unwrap_or(0)
}

fn completion_length(body: &Value) -> usize {
    body.get("choices")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|ch| {
                    ch.get("message")
                        .and_then(|m| m.get("content"))
                        .and_then(|c| c.as_str())
                        .map(|s| s.len())
                        .unwrap_or(0)
                })
                .sum()
        })
        .unwrap_or(0)
}

#[async_trait]
impl Pipeline for ObservabilityPipeline {
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
        let prompt_len = prompt_length(body);
        tracing::info!(
            target: "lmmaster.pipelines",
            pipeline = Self::ID,
            stage = "request",
            request_id = %ctx.request_id,
            project_id = ctx.project_id.as_deref().unwrap_or(""),
            model = ctx.model.as_deref().unwrap_or(""),
            user_agent = ctx.user_agent.as_deref().unwrap_or(""),
            prompt_chars = prompt_len,
            "pipeline observability — request"
        );
        ctx.record(AuditEntry::passed(Self::ID));
        Ok(())
    }

    async fn apply_response(
        &self,
        ctx: &mut PipelineContext,
        body: &mut Value,
    ) -> Result<(), PipelineError> {
        let completion_len = completion_length(body);
        tracing::info!(
            target: "lmmaster.pipelines",
            pipeline = Self::ID,
            stage = "response",
            request_id = %ctx.request_id,
            project_id = ctx.project_id.as_deref().unwrap_or(""),
            model = ctx.model.as_deref().unwrap_or(""),
            tokens_used = ctx.tokens_used,
            completion_chars = completion_len,
            "pipeline observability — response"
        );
        ctx.record(AuditEntry::passed(Self::ID));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use tracing::subscriber::set_default;
    use tracing_subscriber::{fmt, layer::SubscriberExt, Registry};

    /// MakeWriter 구현체 — 테스트에서 tracing 출력을 캡처.
    #[derive(Clone, Default)]
    struct CaptureWriter {
        buf: Arc<Mutex<Vec<u8>>>,
    }

    impl std::io::Write for CaptureWriter {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.buf.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CaptureWriter {
        type Writer = CaptureWriter;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    fn captured_string(writer: &CaptureWriter) -> String {
        String::from_utf8(writer.buf.lock().unwrap().clone()).unwrap_or_default()
    }

    #[tokio::test]
    async fn request_emits_tracing_with_expected_fields() {
        let writer = CaptureWriter::default();
        let layer = fmt::layer()
            .with_writer(writer.clone())
            .with_target(true)
            .with_ansi(false);
        let subscriber = Registry::default().with(layer);

        let p = ObservabilityPipeline::new();
        let mut c = PipelineContext::new("req-abc");
        c.project_id = Some("proj-1".into());
        c.model = Some("exaone".into());
        let mut body = json!({"messages":[{"role":"user","content":"hello"}]});

        // set_default returns a guard that uninstalls subscriber on drop — async-friendly.
        let _guard = set_default(subscriber);
        p.apply_request(&mut c, &mut body).await.unwrap();
        drop(_guard);

        let out = captured_string(&writer);
        assert!(
            out.contains("lmmaster.pipelines"),
            "target missing in: {out}"
        );
        assert!(out.contains("req-abc"), "request_id missing in: {out}");
        assert!(out.contains("exaone"), "model missing in: {out}");
        assert!(out.contains("proj-1"), "project_id missing in: {out}");
    }

    #[tokio::test]
    async fn response_emits_tracing_with_completion_chars() {
        let writer = CaptureWriter::default();
        let layer = fmt::layer()
            .with_writer(writer.clone())
            .with_target(true)
            .with_ansi(false);
        let subscriber = Registry::default().with(layer);

        let p = ObservabilityPipeline::new();
        let mut c = PipelineContext::new("req-resp");
        c.tokens_used = 42;
        let mut body =
            json!({"choices":[{"message":{"role":"assistant","content":"hello world"}}]});

        let _guard = set_default(subscriber);
        p.apply_response(&mut c, &mut body).await.unwrap();
        drop(_guard);

        let out = captured_string(&writer);
        assert!(out.contains("req-resp"));
        assert!(
            out.contains("completion_chars=11"),
            "completion length missing: {out}"
        );
        assert!(out.contains("tokens_used=42"));
    }

    #[tokio::test]
    async fn pipeline_does_not_modify_body() {
        let p = ObservabilityPipeline::new();
        let mut c = PipelineContext::new("r");
        let mut body = json!({"messages":[{"role":"user","content":"hello"}]});
        let snapshot = body.clone();
        p.apply_request(&mut c, &mut body).await.unwrap();
        assert_eq!(body, snapshot);
        let mut response_body = json!({"choices":[{"message":{"content":"x"}}]});
        let snapshot2 = response_body.clone();
        p.apply_response(&mut c, &mut response_body).await.unwrap();
        assert_eq!(response_body, snapshot2);
    }

    #[tokio::test]
    async fn pipeline_id_and_stage_correct() {
        let p = ObservabilityPipeline::new();
        assert_eq!(p.id(), "observability");
        assert_eq!(p.stage(), PipelineStage::Both);
    }

    #[tokio::test]
    async fn audit_entries_appended_for_each_stage() {
        let p = ObservabilityPipeline::new();
        let mut c = PipelineContext::new("r");
        let mut body = json!({"messages":[{"role":"user","content":"hi"}]});
        p.apply_request(&mut c, &mut body).await.unwrap();
        p.apply_response(&mut c, &mut body).await.unwrap();
        assert_eq!(c.audit_log.len(), 2);
        assert!(c.audit_log.iter().all(|e| e.action == "passed"));
    }
}
