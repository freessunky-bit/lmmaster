// scanner IPC — start_scan / get_last_scan + scan:summary listener.
// Rust crates/scanner의 ScanSummary serde 미러.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
/** 즉시 자가 점검 실행. Promise reject 시 ScanApiError 캐치 가능. */
export async function startScan() {
    return invoke("start_scan");
}
/** 마지막 캐시된 점검 결과. 한 번도 안 됐으면 null. */
export async function getLastScan() {
    return invoke("get_last_scan");
}
/** scan:summary event 구독 — broadcast subscriber가 forward. */
export async function onScanSummary(cb) {
    return listen("scan:summary", (e) => cb(e.payload));
}
