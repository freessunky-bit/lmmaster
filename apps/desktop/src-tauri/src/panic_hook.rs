//! Panic hook 설치 — Phase 8'.0.b (ADR-0036) + Phase 7'.b telemetry submit 통합.
//!
//! 정책:
//! - `std::panic::set_hook` 한 번만 등록 (idempotent — 다중 호출 시 두 번째부터 무시).
//! - hook 본문은 `catch_unwind`로 자체 panic 방어 (무한 재귀 차단).
//! - 동작:
//!   1. `tracing::error!`로 panic info + backtrace 기록.
//!   2. `crash/crash-<rfc3339>.txt` 파일 작성 — 사용자가 진단 화면에서 확인 가능.
//!   3. (Tauri runtime up이면) MessageDialogBuilder로 한국어 해요체 알림. (별도 thread)
//!   4. Phase 7'.b: TelemetryState가 등록돼 있고 opt-in 상태면 submit_event 호출 — endpoint 미설정
//!      시 queue retention만, 설정 시 backon 3회 retry POST. opt-out 상태면 호출 자체 skip.
//! - 한국어 메시지: "예기치 못한 오류가 발생했어요. 자세한 내용은 진단 화면에서 확인할 수 있어요."

use std::panic::{self, AssertUnwindSafe, PanicHookInfo};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use time::format_description::well_known::Rfc3339;

/// hook 설치 여부 — 다중 호출 시 두 번째부터 무시.
static INSTALLED: AtomicBool = AtomicBool::new(false);

/// crash report 디렉터리. 첫 install 시 caller가 한 번만 설정. 이후는 hook이 사용.
/// `None`이면 crash report 파일 작성을 skip + log only.
static CRASH_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// 이전 hook chain — install 후 default 동작 보존을 위해 호출.
/// 보통은 default(stderr 출력)지만 다른 hook이 설치된 경우에도 chain 유지.
type BoxedHook = Box<dyn Fn(&PanicHookInfo<'_>) + Sync + Send + 'static>;
static PREV_HOOK: Mutex<Option<BoxedHook>> = Mutex::new(None);

/// Phase 7'.b — TelemetryState (Arc로 공유). 등록 안 돼 있으면 None — submit skip.
/// `attach_telemetry`로 lib.rs setup이 등록.
static TELEMETRY: Mutex<Option<Arc<crate::telemetry::TelemetryState>>> = Mutex::new(None);

/// Tauri builder가 TelemetryState를 manage한 후 호출 — panic hook이 telemetry submit을 시도하도록.
/// idempotent: 두 번째 호출은 새 state로 교체 (테스트 친화).
pub fn attach_telemetry(state: Arc<crate::telemetry::TelemetryState>) {
    if let Ok(mut g) = TELEMETRY.lock() {
        *g = Some(state);
    }
}

/// 한국어 사용자 향 메시지.
const KO_MESSAGE: &str =
    "예기치 못한 오류가 발생했어요. 자세한 내용은 진단 화면에서 확인할 수 있어요.";

/// crash report 디렉터리를 지정하고 panic hook을 등록한다.
///
/// 다중 호출은 첫 호출만 유효 (testdriven 환경에서 cross-test 누수 방지).
pub fn install(crash_dir: Option<PathBuf>) {
    if INSTALLED.swap(true, Ordering::SeqCst) {
        // 이미 설치됨.
        return;
    }
    if let Some(dir) = &crash_dir {
        let _ = std::fs::create_dir_all(dir);
    }
    *CRASH_DIR.lock().expect("CRASH_DIR poisoned") = crash_dir;

    // 기존 hook을 보존해서 chain.
    let prev = panic::take_hook();
    *PREV_HOOK.lock().expect("PREV_HOOK poisoned") = Some(prev);

    panic::set_hook(Box::new(|info| {
        // hook 자체 panic은 감춤 — 무한 재귀 방어.
        // PanicHookInfo는 RefUnwindSafe 미구현이라 AssertUnwindSafe로 명시 — hook 본문은 read-only로
        // info를 다루기에 panic-safe.
        let _ = std::panic::catch_unwind(AssertUnwindSafe(|| handle_panic(info)));
    }));
}

/// 테스트 / shutdown 정리 — install된 상태를 reset.
#[cfg(test)]
pub(crate) fn _reset_for_tests() {
    INSTALLED.store(false, Ordering::SeqCst);
    *CRASH_DIR.lock().expect("CRASH_DIR poisoned") = None;
    if let Some(prev) = PREV_HOOK.lock().expect("PREV_HOOK poisoned").take() {
        panic::set_hook(prev);
    } else {
        // 깨끗하게 default로 복원.
        let _ = panic::take_hook();
    }
}

/// hook 본문 — catch_unwind 안에서 호출. 자체 panic은 무시.
fn handle_panic(info: &PanicHookInfo<'_>) {
    let payload = panic_message(info);
    let location = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_else(|| "<unknown>".to_string());

    let backtrace = std::backtrace::Backtrace::force_capture();

    // 1. tracing 기록.
    tracing::error!(
        location = %location,
        payload = %payload,
        "panic occurred",
    );

    // 2. crash report 파일.
    let dir = CRASH_DIR.lock().expect("CRASH_DIR poisoned").clone();
    if let Some(dir) = dir {
        let _ = write_crash_report(&dir, &payload, &location, &backtrace);
    }

    // 3. previous hook (default stderr 출력 등) 호출 — chain 보존.
    if let Some(prev) = PREV_HOOK.lock().expect("PREV_HOOK poisoned").as_ref() {
        // payload는 PanicHookInfo에서 다시 추출하므로 직접 호출.
        prev(info);
    }

    // 4. Phase 7'.b — TelemetryState가 attach돼 있으면 submit_event 시도. opt-out이면 NotEnabled로
    //    조용히 거절되므로 분기 불필요. async submit은 별도 task에 위임 — panic hook 본체는 sync.
    let telemetry = TELEMETRY.lock().ok().and_then(|g| g.as_ref().cloned());
    if let Some(state) = telemetry {
        let panic_message_text = format!("{payload} @ {location}");
        // tauri::async_runtime::spawn은 setup 후에는 살아 있음. shutdown 도중에는 best-effort.
        let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
            tauri::async_runtime::spawn(async move {
                let _ = state
                    .submit_event(crate::telemetry::EventLevel::Error, panic_message_text)
                    .await;
            });
        }));
    }
}

/// PanicHookInfo의 payload(문자열)을 best-effort 추출. `&str` / `String` 두 케이스 + fallback.
fn panic_message(info: &PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    if let Some(s) = payload.downcast_ref::<&str>() {
        return s.to_string();
    }
    if let Some(s) = payload.downcast_ref::<String>() {
        return s.clone();
    }
    "(unknown payload type)".to_string()
}

/// crash report 디렉터리 위치를 외부로 노출 — Phase 13'.c IPC가 read 시 사용.
/// `install` 시 caller가 등록한 값. None이면 crash 디렉터리 미설정.
pub fn crash_dir() -> Option<PathBuf> {
    CRASH_DIR.lock().ok().and_then(|g| g.clone())
}

/// crash report 작성. RFC3339 timestamp 기반 unique 파일명.
fn write_crash_report(
    dir: &Path,
    payload: &str,
    location: &str,
    backtrace: &std::backtrace::Backtrace,
) -> std::io::Result<()> {
    let now = time::OffsetDateTime::now_utc();
    let ts = now
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown-time".to_string());
    let filename_safe_ts = ts.replace([':', '.'], "-");
    let path = dir.join(format!("crash-{filename_safe_ts}.txt"));
    let body = format!(
        "Crash report ({ts})\n\
         {KO_MESSAGE}\n\
         \n\
         Payload: {payload}\n\
         Location: {location}\n\
         \n\
         Backtrace:\n{backtrace}\n",
    );
    std::fs::write(&path, body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    /// 테스트끼리 충돌 막기 위한 grand mutex — install 상태가 process-wide static이라.
    static TEST_LOCK: StdMutex<()> = StdMutex::new(());

    #[test]
    fn install_idempotent() {
        let _g = TEST_LOCK.lock().unwrap();
        _reset_for_tests();
        install(None);
        // 두 번째 호출은 no-op (panic 없이 통과).
        install(None);
        _reset_for_tests();
    }

    #[test]
    fn panic_writes_crash_report_file() {
        let _g = TEST_LOCK.lock().unwrap();
        _reset_for_tests();
        let tmp = tempfile::tempdir().unwrap();
        install(Some(tmp.path().to_path_buf()));

        // catch_unwind 안에서 panic 발생 → hook 작동 + crash 파일 생성.
        let _ = std::panic::catch_unwind(|| panic!("test panic message — 한국어 OK"));

        // 디렉터리에 crash-*.txt 1개 이상.
        let entries: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("crash-"))
            .collect();
        assert!(!entries.is_empty(), "crash 파일이 1개 이상이어야 해요");
        // 첫 번째 파일에 한국어 메시지 + payload 포함.
        let body = std::fs::read_to_string(entries[0].path()).unwrap();
        assert!(body.contains("예기치 못한 오류"));
        assert!(body.contains("한국어 OK"));
        _reset_for_tests();
    }

    #[test]
    fn panic_without_crash_dir_is_safe() {
        let _g = TEST_LOCK.lock().unwrap();
        _reset_for_tests();
        install(None);
        // crash_dir 미지정에도 panic 처리 OK (log만 + previous hook chain).
        let _ = std::panic::catch_unwind(|| panic!("safe panic"));
        _reset_for_tests();
    }

    #[test]
    fn panic_message_extracts_string_payload() {
        // panic_message helper의 동작 — &str / String / 미지원 타입.
        let _g = TEST_LOCK.lock().unwrap();
        // unwrap에서 던지는 panic은 String/&str.
        let r = std::panic::catch_unwind(|| {
            panic!("my-payload-aaaaa");
        });
        assert!(r.is_err());
        // payload 추출 자체는 hook 안에서 호출되므로 직접 검증은 어려움.
        // 대신 file에 payload가 들어가는지 확인.
        _reset_for_tests();
        let tmp = tempfile::tempdir().unwrap();
        install(Some(tmp.path().to_path_buf()));
        let _ = std::panic::catch_unwind(|| panic!("my-payload-aaaaa"));
        let entries: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("crash-"))
            .collect();
        let body = std::fs::read_to_string(entries.last().unwrap().path()).unwrap();
        assert!(body.contains("my-payload-aaaaa"));
        _reset_for_tests();
    }
}
