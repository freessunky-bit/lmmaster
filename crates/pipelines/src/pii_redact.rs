//! `PiiRedactPipeline` — 한국어 PII 정규식 redact (v1 시드).
//!
//! 정책 (ADR-0025 §2):
//! - 4종 PII detect: 주민등록번호 / 휴대폰 / 신용카드 / 이메일.
//! - request `messages[].content` + response `choices[].message.content` 모두 적용.
//! - 정규식은 `OnceLock`으로 1회 컴파일 + 재사용.
//! - 변경 시 `AuditEntry::modified` 기록. 변경 없으면 audit 미기록 (no false-positive log).

use async_trait::async_trait;
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

use crate::error::PipelineError;
use crate::pipeline::{AuditEntry, Pipeline, PipelineContext, PipelineStage};

/// PII redact Pipeline 인스턴스.
pub struct PiiRedactPipeline;

impl Default for PiiRedactPipeline {
    fn default() -> Self {
        Self
    }
}

impl PiiRedactPipeline {
    pub const ID: &'static str = "pii-redact";

    pub fn new() -> Self {
        Self
    }
}

/// 정규식 패턴 묶음 — 1회만 컴파일.
struct PiiPatterns {
    /// 주민등록번호 — 6-7 (예: 991231-1234567).
    rrn: Regex,
    /// 휴대폰 — 010-1234-5678 / +82-10-1234-5678.
    phone: Regex,
    /// 신용카드 — 4-4-4-4 16자리.
    card: Regex,
    /// 이메일 — 표준 형태.
    email: Regex,
}

fn patterns() -> &'static PiiPatterns {
    static P: OnceLock<PiiPatterns> = OnceLock::new();
    P.get_or_init(|| PiiPatterns {
        // \b로 경계를 잡으면 한국어 텍스트와 인접한 ASCII 숫자 패턴이 정확히 매칭돼요.
        rrn: Regex::new(r"\b\d{6}-\d{7}\b").expect("rrn regex"),
        phone: Regex::new(r"(?:\+82-?10-\d{3,4}-\d{4}|01[0-9]-\d{3,4}-\d{4})")
            .expect("phone regex"),
        card: Regex::new(r"\b\d{4}-\d{4}-\d{4}-\d{4}\b").expect("card regex"),
        email: Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").expect("email regex"),
    })
}

/// 입력 문자열을 4단계 PII 패턴으로 redact. 변경 횟수 합산을 같이 반환.
fn redact_text(input: &str) -> (String, usize) {
    let p = patterns();
    let mut count = 0usize;

    // 휴대폰을 카드/주민등록보다 먼저 → "+82-10-..." 같은 휴대폰 형태가 카드 16자리로 잘못 잡히는 사고를 방지.
    let after_phone = p.phone.replace_all(input, |_caps: &regex::Captures| {
        count += 1;
        "[REDACTED-휴대폰]".to_string()
    });
    let after_rrn = p.rrn.replace_all(&after_phone, |_caps: &regex::Captures| {
        count += 1;
        "[REDACTED-주민]".to_string()
    });
    let after_card = p.card.replace_all(&after_rrn, |_caps: &regex::Captures| {
        count += 1;
        "[REDACTED-카드]".to_string()
    });
    let after_email = p.email.replace_all(&after_card, |_caps: &regex::Captures| {
        count += 1;
        "[REDACTED-이메일]".to_string()
    });

    (after_email.into_owned(), count)
}

/// `messages[].content`를 mutate (string인 경우만 — content가 array part여도 지금은 skip).
fn redact_messages(messages: &mut [Value]) -> usize {
    let mut count = 0usize;
    for msg in messages.iter_mut() {
        if let Some(content) = msg.get("content").and_then(|v| v.as_str()) {
            let (redacted, c) = redact_text(content);
            if c > 0 {
                if let Some(map) = msg.as_object_mut() {
                    map.insert("content".into(), Value::String(redacted));
                }
                count += c;
            }
        }
    }
    count
}

/// `choices[].message.content` (full response shape) +
/// `choices[].delta.content` (SSE streaming chunk shape, Phase 8'.c.4)를 mutate.
fn redact_choices(choices: &mut [Value]) -> usize {
    let mut count = 0usize;
    for choice in choices.iter_mut() {
        // 1) message.content (non-streaming).
        let msg_content = choice
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());
        if let Some(content) = msg_content {
            let (redacted, c) = redact_text(&content);
            if c > 0 {
                if let Some(message) = choice.get_mut("message").and_then(|m| m.as_object_mut()) {
                    message.insert("content".into(), Value::String(redacted));
                }
                count += c;
            }
        }

        // 2) delta.content (SSE chunk).
        let delta_content = choice
            .get("delta")
            .and_then(|d| d.get("content"))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());
        if let Some(content) = delta_content {
            let (redacted, c) = redact_text(&content);
            if c > 0 {
                if let Some(delta) = choice.get_mut("delta").and_then(|d| d.as_object_mut()) {
                    delta.insert("content".into(), Value::String(redacted));
                }
                count += c;
            }
        }
    }
    count
}

#[async_trait]
impl Pipeline for PiiRedactPipeline {
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
        let mut total = 0usize;
        if let Some(arr) = body.get_mut("messages").and_then(|v| v.as_array_mut()) {
            total += redact_messages(arr);
        }
        if total > 0 {
            ctx.record(AuditEntry::modified(
                Self::ID,
                format!("redacted {total} PII match(es) in request"),
            ));
            tracing::info!(
                target: "lmmaster.pipelines",
                pipeline = Self::ID,
                request_id = %ctx.request_id,
                redactions = total,
                "request body PII redacted"
            );
        }
        Ok(())
    }

    async fn apply_response(
        &self,
        ctx: &mut PipelineContext,
        body: &mut Value,
    ) -> Result<(), PipelineError> {
        let mut total = 0usize;
        if let Some(arr) = body.get_mut("choices").and_then(|v| v.as_array_mut()) {
            total += redact_choices(arr);
        }
        if total > 0 {
            ctx.record(AuditEntry::modified(
                Self::ID,
                format!("redacted {total} PII match(es) in response"),
            ));
            tracing::info!(
                target: "lmmaster.pipelines",
                pipeline = Self::ID,
                request_id = %ctx.request_id,
                redactions = total,
                "response body PII redacted"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx() -> PipelineContext {
        PipelineContext::new("test-req")
    }

    #[tokio::test]
    async fn redacts_email_in_request_messages() {
        let p = PiiRedactPipeline::new();
        let mut c = ctx();
        let mut body =
            json!({"messages":[{"role":"user","content":"내 이메일은 alice@example.com 이에요"}]});
        p.apply_request(&mut c, &mut body).await.unwrap();
        let s = body["messages"][0]["content"].as_str().unwrap();
        assert!(s.contains("[REDACTED-이메일]"));
        assert!(!s.contains("alice@example.com"));
        assert_eq!(c.audit_log.len(), 1);
        assert_eq!(c.audit_log[0].action, "modified");
    }

    #[tokio::test]
    async fn redacts_phone_010_format() {
        let p = PiiRedactPipeline::new();
        let mut c = ctx();
        let mut body =
            json!({"messages":[{"role":"user","content":"전화는 010-1234-5678 이에요"}]});
        p.apply_request(&mut c, &mut body).await.unwrap();
        let s = body["messages"][0]["content"].as_str().unwrap();
        assert!(s.contains("[REDACTED-휴대폰]"));
        assert!(!s.contains("010-1234-5678"));
    }

    #[tokio::test]
    async fn redacts_phone_international_format() {
        let p = PiiRedactPipeline::new();
        let mut c = ctx();
        let mut body = json!({"messages":[{"role":"user","content":"전화 +82-10-1234-5678 으로 연락 주세요"}]});
        p.apply_request(&mut c, &mut body).await.unwrap();
        let s = body["messages"][0]["content"].as_str().unwrap();
        assert!(
            s.contains("[REDACTED-휴대폰]"),
            "international format must be detected, got: {s}"
        );
    }

    #[tokio::test]
    async fn redacts_rrn() {
        let p = PiiRedactPipeline::new();
        let mut c = ctx();
        let mut body =
            json!({"messages":[{"role":"user","content":"주민등록번호 991231-1234567 입니다"}]});
        p.apply_request(&mut c, &mut body).await.unwrap();
        let s = body["messages"][0]["content"].as_str().unwrap();
        assert!(s.contains("[REDACTED-주민]"));
    }

    #[tokio::test]
    async fn redacts_credit_card() {
        let p = PiiRedactPipeline::new();
        let mut c = ctx();
        let mut body =
            json!({"messages":[{"role":"user","content":"카드는 1234-5678-9012-3456 입니다"}]});
        p.apply_request(&mut c, &mut body).await.unwrap();
        let s = body["messages"][0]["content"].as_str().unwrap();
        assert!(s.contains("[REDACTED-카드]"));
    }

    #[tokio::test]
    async fn redacts_multiple_pii_in_same_message() {
        let p = PiiRedactPipeline::new();
        let mut c = ctx();
        let mut body = json!({"messages":[{"role":"user","content":"alice@example.com 010-1111-2222 991231-1234567"}]});
        p.apply_request(&mut c, &mut body).await.unwrap();
        let s = body["messages"][0]["content"].as_str().unwrap();
        assert!(s.contains("[REDACTED-이메일]"));
        assert!(s.contains("[REDACTED-휴대폰]"));
        assert!(s.contains("[REDACTED-주민]"));
        assert_eq!(c.audit_log.len(), 1);
        let detail = c.audit_log[0].details.as_deref().unwrap_or("");
        assert!(
            detail.contains("3"),
            "audit detail should report 3 redactions, got: {detail}"
        );
    }

    #[tokio::test]
    async fn no_pii_means_no_audit_entry() {
        let p = PiiRedactPipeline::new();
        let mut c = ctx();
        let mut body =
            json!({"messages":[{"role":"user","content":"안녕하세요. 오늘 날씨 어때요?"}]});
        let snapshot = body.clone();
        p.apply_request(&mut c, &mut body).await.unwrap();
        assert_eq!(body, snapshot);
        assert!(
            c.audit_log.is_empty(),
            "no-match should not produce audit entry"
        );
    }

    #[tokio::test]
    async fn redacts_response_choices_message_content() {
        let p = PiiRedactPipeline::new();
        let mut c = ctx();
        let mut body = json!({
            "choices":[
                {"index":0, "message":{"role":"assistant","content":"답변: 010-1234-5678로 연락하세요"}}
            ]
        });
        p.apply_response(&mut c, &mut body).await.unwrap();
        let s = body["choices"][0]["message"]["content"].as_str().unwrap();
        assert!(s.contains("[REDACTED-휴대폰]"));
        assert_eq!(c.audit_log.len(), 1);
        let detail = c.audit_log[0].details.as_deref().unwrap_or("");
        assert!(detail.contains("response"));
    }

    #[tokio::test]
    async fn idempotent_double_apply_is_stable() {
        let p = PiiRedactPipeline::new();
        let mut c = ctx();
        let mut body = json!({"messages":[{"role":"user","content":"alice@example.com"}]});
        p.apply_request(&mut c, &mut body).await.unwrap();
        let after_first = body.clone();
        p.apply_request(&mut c, &mut body).await.unwrap();
        // 두 번째 호출은 변경 없음 (이미 redacted) → 결과 동일.
        assert_eq!(body, after_first);
    }

    #[tokio::test]
    async fn pipeline_id_and_stage_are_correct() {
        let p = PiiRedactPipeline::new();
        assert_eq!(p.id(), "pii-redact");
        assert_eq!(p.stage(), PipelineStage::Both);
    }

    #[tokio::test]
    async fn no_messages_field_is_no_op() {
        let p = PiiRedactPipeline::new();
        let mut c = ctx();
        let mut body = json!({"model":"test","stream":false});
        let snapshot = body.clone();
        p.apply_request(&mut c, &mut body).await.unwrap();
        assert_eq!(body, snapshot);
        assert!(c.audit_log.is_empty());
    }

    /// Phase 8'.c.4 — SSE streaming chunk shape: `choices[].delta.content`도 redact 되어야.
    #[tokio::test]
    async fn redacts_streaming_delta_content() {
        let p = PiiRedactPipeline::new();
        let mut c = ctx();
        let mut body = json!({
            "choices":[
                {"index":0, "delta":{"role":"assistant","content":"전화 010-1234-5678 으로"}}
            ]
        });
        p.apply_response(&mut c, &mut body).await.unwrap();
        let s = body["choices"][0]["delta"]["content"].as_str().unwrap();
        assert!(
            s.contains("[REDACTED-휴대폰]"),
            "delta.content가 redact 되어야 해요: {s}"
        );
        assert!(!s.contains("010-1234-5678"));
    }
}
