//! Telemetry — Phase 7'.b. opt-in scaffold + GlitchTip-compatible event submission.
//!
//! 정책 (ADR-0027 §5, ADR-0041, phase-7p-release-prep-reinforcement.md §5):
//! - 기본 비활성. 사용자가 명시적 opt-in 했을 때만 익명 사용 통계 + crash report 전송.
//! - GlitchTip self-hosted 전용. Sentry SaaS 거부 (외부 통신 0 위반).
//! - 실 endpoint는 환경변수 `LMMASTER_GLITCHTIP_DSN` 미설정 시 비활성 (queue retention only).
//! - panic hook이 panic 발생 시 submit_event(level: error, message: panic info) 호출 (opt-in 시).
//! - queue cap 200, 24h retention, oldest drop. backon 3회 retry.
//! - 한국어 해요체 에러 메시지.
//!
//! Sub-modules:
//! - `state` — TelemetryState + TelemetryConfig + opt-in toggle + 영속.
//! - `submit` — TelemetryEvent + EventQueue + GlitchTip POST 시도.

pub mod state;
pub mod submit;

// Re-export for backwards compat. lib.rs는 `telemetry::TelemetryState` 등 평면 경로로 접근.
pub use state::{
    get_telemetry_config, set_telemetry_enabled, submit_telemetry_event, TelemetryApiError,
    TelemetryConfig, TelemetryState,
};
pub use submit::{
    EventLevel, EventQueue, EventSubmitOutcome, TelemetryEvent, GLITCHTIP_DSN_ENV_VAR,
};
