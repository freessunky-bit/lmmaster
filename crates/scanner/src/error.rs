//! ScannerError — soft errors (LLM 관련)는 caller에서 deterministic fallback으로 흡수.

#[derive(Debug, thiserror::Error)]
pub enum ScannerError {
    #[error("환경 점검 중 오류가 발생했어요: {0}")]
    Probe(String),

    #[error("Ollama에 연결할 수 없어요")]
    OllamaUnreachable,

    #[error("Ollama 응답이 너무 늦어요")]
    OllamaTimeout,

    #[error("적합한 한국어 모델이 설치되어 있지 않아요")]
    OllamaModelMissing,

    /// LLM 결과가 한국어가 아니거나 형식이 잘못됨 — caller가 deterministic으로 fallback.
    #[error("AI 요약 결과가 유효하지 않아요 — 기본 템플릿으로 대체했어요")]
    LlmValidationFailed(&'static str),

    #[error("이미 점검이 진행 중이에요")]
    AlreadyRunning,

    #[error("스케줄러 오류: {0}")]
    Scheduler(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("HTTP 오류: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON 오류: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<tokio_cron_scheduler::JobSchedulerError> for ScannerError {
    fn from(e: tokio_cron_scheduler::JobSchedulerError) -> Self {
        Self::Scheduler(e.to_string())
    }
}
