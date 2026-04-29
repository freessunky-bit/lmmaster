//! 진행률 이벤트. Tauri Channel<T> / mpsc Sender / closure 어느 sink든 호환되도록
//! Sized + Clone + Serialize 형식으로 노출.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DownloadEvent {
    /// 다운로드 시작 — total은 Content-Length가 있으면 Some.
    Started {
        url: String,
        total: Option<u64>,
        resume_from: u64,
    },
    /// 누적 진행률. 호출자가 256KB 또는 100ms 누적 후 emit해야 함 (downloader가 throttle 처리).
    Progress {
        downloaded: u64,
        total: Option<u64>,
        speed_bps: u64,
    },
    /// sha256 검증 성공.
    Verified { sha256_hex: String },
    /// 최종 경로로 atomic rename 성공.
    Finished { final_path: PathBuf, bytes: u64 },
    /// retry 직전 — caller에게 사용자 한국어 안내 기회 제공.
    Retrying {
        attempt: u32,
        delay_ms: u64,
        reason: String,
    },
}

/// 진행률 sink — 어떤 종류든 동일 인터페이스로 받는다.
/// Tauri Channel<DownloadEvent>::send는 Result를 반환하므로 wrap 시 .ok() 사용.
pub trait ProgressSink: Send + Sync {
    fn emit(&self, event: DownloadEvent);
}

/// `Fn(DownloadEvent) + Send + Sync + 'static`을 ProgressSink로 사용 가능.
impl<F> ProgressSink for F
where
    F: Fn(DownloadEvent) + Send + Sync,
{
    fn emit(&self, event: DownloadEvent) {
        (self)(event)
    }
}

/// 어떤 sink도 받지 않을 때 사용하는 no-op sink — `&NoopSink`로 전달.
pub struct NoopSink;
impl ProgressSink for NoopSink {
    fn emit(&self, _: DownloadEvent) {}
}
