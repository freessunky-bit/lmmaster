//! 크로스 플랫폼 OS shutdown 신호 helper.
//!
//! Unix: ctrl_c + SIGTERM 둘 중 먼저 오는 신호.
//! Windows: ctrl_c (Ctrl+C / Ctrl+Break) — SIGTERM 등가물 없음.
//! Tauri 환경에선 `RunEvent::ExitRequested`가 일반적인 종료 트리거이므로
//! 이 함수는 standalone 실행/CLI 진단용으로 주로 사용된다.

pub async fn os_shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::warn!(error = %e, "failed to install ctrl_c handler");
        }
    };

    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(e) => tracing::warn!(error = %e, "failed to install SIGTERM handler"),
        }
    };

    #[cfg(not(unix))]
    let term = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("os signal: ctrl_c"),
        _ = term => tracing::info!("os signal: SIGTERM"),
    }
}
