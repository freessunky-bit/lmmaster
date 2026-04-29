//! Pipeline 에러 타입.
//!
//! 정책 (ADR-0025, phase-6p-updater-pipelines-decision.md §1.8):
//! - 사용자 향 메시지는 1차 한국어 해요체. 영어는 fallback.
//! - thiserror 기반, `Display`만으로 사용자에게 노출 가능.
//! - 5 variant 분리: Blocked / BudgetExceeded / Configuration / Internal / Cancelled.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PipelineError {
    /// Pipeline이 명시적으로 요청/응답을 차단했어요. (예: 정책 위반)
    #[error("필터({pipeline})가 요청을 차단했어요: {reason}")]
    Blocked { pipeline: String, reason: String },

    /// 토큰 한도 초과. `TokenQuotaPipeline`이 발행.
    #[error("토큰 한도({budget})를 초과했어요. 사용량: {used}")]
    BudgetExceeded { used: u64, budget: u64 },

    /// 설정 오류 — Pipeline 인스턴스 생성 또는 적용 시 잘못된 입력.
    #[error("필터 설정이 잘못되었어요: {0}")]
    Configuration(String),

    /// 일반 내부 오류 (regex 컴파일 실패, JSON 구조 누락 등).
    #[error("필터 처리 중 오류가 발생했어요: {0}")]
    Internal(String),

    /// 사용자 또는 상위 cancel 신호.
    #[error("사용자가 처리를 취소했어요")]
    Cancelled,
}

impl PipelineError {
    /// OpenAI envelope `type` 필드로 사용할 짧은 식별자.
    pub fn error_type(&self) -> &'static str {
        match self {
            Self::Blocked { .. } => "pipeline_blocked",
            Self::BudgetExceeded { .. } => "budget_exceeded",
            Self::Configuration(_) => "pipeline_configuration_error",
            Self::Internal(_) => "pipeline_internal_error",
            Self::Cancelled => "pipeline_cancelled",
        }
    }

    /// OpenAI envelope `code` 필드로 사용할 짧은 식별자.
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::Blocked { .. } => "pipeline_blocked",
            Self::BudgetExceeded { .. } => "budget_exceeded",
            Self::Configuration(_) => "pipeline_configuration_error",
            Self::Internal(_) => "pipeline_internal_error",
            Self::Cancelled => "pipeline_cancelled",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_message_korean_with_pipeline_and_reason() {
        let e = PipelineError::Blocked {
            pipeline: "pii-redact".into(),
            reason: "주민등록번호 노출".into(),
        };
        let msg = format!("{e}");
        assert!(msg.contains("필터"));
        assert!(msg.contains("차단"));
        assert!(msg.contains("pii-redact"));
        assert!(msg.contains("주민등록번호 노출"));
    }

    #[test]
    fn budget_exceeded_message_korean_with_used_and_budget() {
        let e = PipelineError::BudgetExceeded {
            used: 1500,
            budget: 1000,
        };
        let msg = format!("{e}");
        assert!(msg.contains("토큰 한도"));
        assert!(msg.contains("초과"));
        assert!(msg.contains("1500"));
        assert!(msg.contains("1000"));
    }

    #[test]
    fn configuration_message_korean_includes_detail() {
        let e = PipelineError::Configuration("regex 컴파일 실패".into());
        let msg = format!("{e}");
        assert!(msg.contains("필터 설정"));
        assert!(msg.contains("잘못"));
        assert!(msg.contains("regex 컴파일 실패"));
    }

    #[test]
    fn internal_message_korean_includes_detail() {
        let e = PipelineError::Internal("body 구조 누락".into());
        let msg = format!("{e}");
        assert!(msg.contains("필터 처리"));
        assert!(msg.contains("오류"));
        assert!(msg.contains("body 구조 누락"));
    }

    #[test]
    fn cancelled_message_korean() {
        let e = PipelineError::Cancelled;
        let msg = format!("{e}");
        assert!(msg.contains("사용자"));
        assert!(msg.contains("취소"));
    }

    #[test]
    fn error_type_and_code_are_distinct_per_variant() {
        // 5 variant 모두 type/code 매핑이 존재해야 envelope 변환이 가능해요.
        let variants = [
            PipelineError::Blocked {
                pipeline: "p".into(),
                reason: "r".into(),
            },
            PipelineError::BudgetExceeded { used: 1, budget: 1 },
            PipelineError::Configuration("c".into()),
            PipelineError::Internal("i".into()),
            PipelineError::Cancelled,
        ];
        for v in &variants {
            assert!(!v.error_type().is_empty());
            assert!(!v.error_code().is_empty());
        }
    }
}
