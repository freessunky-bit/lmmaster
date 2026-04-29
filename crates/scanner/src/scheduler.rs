//! tokio-cron-scheduler wiring + on-launch grace.
//!
//! 정책:
//! - 6시간 주기 cron + 5분 후 on-launch grace + UI on-demand 트리거.
//! - 모든 트리거가 동일한 `Scanner::scan_now()` 경유 — `AlreadyRunning`은 silent skip.
//! - Job 클로저는 `Send + Sync + 'static` — `Arc<Scanner>` clone으로 캡처.

use std::sync::Arc;
use std::time::Duration;

use tokio_cron_scheduler::{Job, JobScheduler};

use crate::error::ScannerError;
use crate::Scanner;

/// 6시간 cron 표현식 (UTC). `JobSchedulerError` → `ScannerError::Scheduler`.
pub const DEFAULT_CRON_SIX_HOURS: &str = "0 0 */6 * * *";

/// `JobScheduler`를 만들고 cron + on-launch 트리거를 등록.
pub async fn build(
    scanner: Arc<Scanner>,
    cron_expr: Option<&str>,
    launch_grace: Option<Duration>,
) -> Result<JobScheduler, ScannerError> {
    let sched = JobScheduler::new().await?;

    if let Some(expr) = cron_expr {
        let scanner_for_cron = Arc::clone(&scanner);
        let job = Job::new_async(expr, move |_uuid, _l| {
            let s = Arc::clone(&scanner_for_cron);
            Box::pin(async move {
                match s.scan_now().await {
                    Ok(_) => tracing::debug!("scheduled scan completed"),
                    Err(ScannerError::AlreadyRunning) => {
                        tracing::debug!("scheduled scan skipped — already running");
                    }
                    Err(e) => tracing::warn!(error = %e, "scheduled scan failed"),
                }
            })
        })?;
        sched.add(job).await?;
    }

    if let Some(grace) = launch_grace {
        let scanner_for_grace = Arc::clone(&scanner);
        // tokio_cron_scheduler에 일회성 job이 따로 있지만 단순화 위해 spawn 사용.
        tokio::spawn(async move {
            tokio::time::sleep(grace).await;
            match scanner_for_grace.scan_now().await {
                Ok(_) => tracing::debug!("on-launch grace scan completed"),
                Err(ScannerError::AlreadyRunning) => {
                    tracing::debug!("on-launch grace scan skipped");
                }
                Err(e) => tracing::warn!(error = %e, "on-launch grace scan failed"),
            }
        });
    }

    Ok(sched)
}
