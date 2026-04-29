//! 설치 lifecycle 이벤트 — Tauri `Channel<InstallEvent>`와 mpsc / closure 모두 호환.
//!
//! 정책 (Phase 1A.3.c 보강 리서치):
//! - `tauri::ipc::Channel<T>`는 `T: IpcResponse + 'static` (≈ `Serialize + Send + Sync`).
//! - 프론트는 `kind` discriminant로 분기하므로 `#[serde(tag = "kind", rename_all = "kebab-case")]`.
//! - `DownloadEvent`는 자체 `kind` field가 있어 newtype variant로 감싸면 `"0":` positional이 됨.
//!   대신 wrapper struct `Download { download: DownloadEvent }`로 감싸 안쪽 tag 보존.
//! - `Cancelled` / `Failed`는 단말 이벤트 — 이후 추가 송신 없음.

use serde::Serialize;

use crate::action::ActionOutcome;
use crate::progress::DownloadEvent;

/// 설치 진행 단일 시점 — `InstallSink::emit` 또는 `Channel::send`로 흘려보낸다.
///
/// 단방향(Rust→TS) 이벤트라 `Deserialize`는 derive하지 않는다 (`ActionOutcome`이 `&'static str`을 들고 있어
/// 그대로 Deserialize 안 됨 — 필요 시 별도 owned 미러 타입을 만든다).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum InstallEvent {
    /// 매니페스트 로드 + platform 분기 직후. method는 "download_and_run" 등 한 단어.
    Started {
        id: String,
        method: String,
        display_name: String,
    },
    /// Downloader 진행 이벤트 — 안쪽 tag 보존 위해 wrapper struct로 감싼다.
    Download { download: DownloadEvent },
    /// 압축 해제 단계 진행. starting/extracting/done 3-phase. extracting은 1회만 emit (장시간 단일 이벤트).
    Extract {
        phase: ExtractPhase,
        entries: u64,
        total_bytes: u64,
    },
    /// post_install_check 단계.
    PostCheck { status: PostCheckStatus },
    /// 정상 종료 — `outcome`은 ActionExecutor 결과 그대로.
    Finished { outcome: ActionOutcome },
    /// 단말 실패. code는 i18n key, message는 한국어 사용자 메시지.
    Failed { code: String, message: String },
    /// CancellationToken cancel 또는 channel close → 단말.
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtractPhase {
    /// 압축 해제 시작 — entries/total_bytes는 0.
    Starting,
    /// 진행 중 — 현재 단순화로 indeterminate (count는 0). future: per-entry progress.
    Extracting,
    /// 완료 — entries/total_bytes 최종값.
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PostCheckStatus {
    Pending,
    Passed,
    /// 명시적 실패 (HTTP non-2xx) 또는 deadline 초과.
    Failed,
    /// 매니페스트에 post_install_check 없음 → 자동 통과로 간주하지 않고 Skipped로 표시.
    Skipped,
}

/// `InstallEvent`를 받는 sink — Tauri `Channel<InstallEvent>` / closure / Vec 캡처 등 호환.
///
/// 반환은 `Ok(())` 또는 `Err` (channel closed). caller는 첫 Err에 cancel을 trigger해야 함.
pub trait InstallSink: Send + Sync {
    fn emit(&self, event: InstallEvent) -> Result<(), InstallSinkClosed>;
}

/// 채널이 닫힘 (window 닫힘 등). caller는 cancel + 종료 시그널.
#[derive(Debug, Clone, Copy)]
pub struct InstallSinkClosed;

impl std::fmt::Display for InstallSinkClosed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("install event sink closed")
    }
}

impl std::error::Error for InstallSinkClosed {}

/// `Fn(InstallEvent) -> Result<(), InstallSinkClosed>` 블랭킷 impl — closure 직접 패스.
impl<F> InstallSink for F
where
    F: Fn(InstallEvent) -> Result<(), InstallSinkClosed> + Send + Sync,
{
    fn emit(&self, event: InstallEvent) -> Result<(), InstallSinkClosed> {
        (self)(event)
    }
}

/// 무시 sink — 테스트/임시 용도.
pub struct NoopInstallSink;

impl InstallSink for NoopInstallSink {
    fn emit(&self, _: InstallEvent) -> Result<(), InstallSinkClosed> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_event_started_serializes_kebab() {
        let ev = InstallEvent::Started {
            id: "ollama".into(),
            method: "download_and_run".into(),
            display_name: "Ollama".into(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "started");
        assert_eq!(v["id"], "ollama");
        assert_eq!(v["method"], "download_and_run");
        assert_eq!(v["display_name"], "Ollama");
    }

    #[test]
    fn install_event_download_preserves_inner_tag() {
        let ev = InstallEvent::Download {
            download: DownloadEvent::Progress {
                downloaded: 1024,
                total: Some(2048),
                speed_bps: 512,
            },
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "download");
        // 내부 tag도 보존돼야 함.
        assert_eq!(v["download"]["kind"], "progress");
        assert_eq!(v["download"]["downloaded"], 1024);
    }

    #[test]
    fn install_event_extract_phases_serialize() {
        for (phase, expected) in [
            (ExtractPhase::Starting, "starting"),
            (ExtractPhase::Extracting, "extracting"),
            (ExtractPhase::Done, "done"),
        ] {
            let ev = InstallEvent::Extract {
                phase,
                entries: 0,
                total_bytes: 0,
            };
            let v = serde_json::to_value(&ev).unwrap();
            assert_eq!(v["kind"], "extract");
            assert_eq!(v["phase"], expected);
        }
    }

    #[test]
    fn install_event_post_check_statuses() {
        for (status, expected) in [
            (PostCheckStatus::Pending, "pending"),
            (PostCheckStatus::Passed, "passed"),
            (PostCheckStatus::Failed, "failed"),
            (PostCheckStatus::Skipped, "skipped"),
        ] {
            let ev = InstallEvent::PostCheck { status };
            let v = serde_json::to_value(&ev).unwrap();
            assert_eq!(v["kind"], "post-check");
            assert_eq!(v["status"], expected);
        }
    }

    #[test]
    fn install_event_failed_has_code_and_message() {
        let ev = InstallEvent::Failed {
            code: "download-failed".into(),
            message: "다운로드에 실패했어요".into(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "failed");
        assert_eq!(v["code"], "download-failed");
        assert!(v["message"].as_str().unwrap().contains("다운로드"));
    }

    #[test]
    fn install_event_cancelled_unit_variant() {
        let ev = InstallEvent::Cancelled;
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "cancelled");
        // Cancelled는 unit variant — kind 외 다른 키 없어야 함.
        assert_eq!(v.as_object().unwrap().len(), 1);
    }

    #[test]
    fn closure_fn_implements_install_sink() {
        use std::sync::Mutex;
        let captured = Mutex::new(Vec::<InstallEvent>::new());
        let sink = |ev: InstallEvent| -> Result<(), InstallSinkClosed> {
            captured.lock().unwrap().push(ev);
            Ok(())
        };
        sink.emit(InstallEvent::Cancelled).unwrap();
        assert_eq!(captured.lock().unwrap().len(), 1);
    }
}
