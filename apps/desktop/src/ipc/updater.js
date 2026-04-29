// check_for_update / cancel_update_check / start_auto_update_poller / stop_auto_update_poller /
// get_auto_update_status Tauri command 래퍼. Phase 6'.b.
//
// Workbench (5'.b) ipc/workbench.ts와 동일 결 — Channel<UpdateEvent>로 진행 이벤트를 받고,
// check_id를 즉시 Promise로 반환. 자동 폴러는 single-slot — 두 번째 start는 backend가 거부.
import { Channel, invoke } from "@tauri-apps/api/core";
// ── helpers ───────────────────────────────────────────────────────────
/** terminal 이벤트 — 더 이상 같은 check_id로 event가 안 옴. */
export function isTerminalUpdateEvent(ev) {
    return (ev.kind === "outdated" ||
        ev.kind === "up-to-date" ||
        ev.kind === "failed" ||
        ev.kind === "cancelled");
}
/**
 * 단발 update check 시작. Channel<UpdateEvent>로 진행 이벤트를 받는다.
 *
 * 반환된 handle.cancel()은 backend cancel_update_check를 호출 — check_id 기준 idempotent.
 * 에러는 invoke().reject로 도달 — kind discriminant 기반 narrow.
 */
export async function checkForUpdate(repo, currentVersion, onEvent) {
    const channel = new Channel();
    channel.onmessage = onEvent;
    const check_id = await invoke("check_for_update", {
        repo,
        currentVersion,
        onEvent: channel,
    });
    return {
        check_id,
        cancel: () => cancelUpdateCheck(check_id),
    };
}
/** 진행 중 check cancel — idempotent. 미존재 id면 no-op. */
export async function cancelUpdateCheck(check_id) {
    await invoke("cancel_update_check", { checkId: check_id });
}
/**
 * 자동 폴러 시작 — single-slot. 이미 실행 중이면 backend가 PollerAlreadyRunning 거부.
 *
 * @param interval_secs 1h(3600)~24h(86400). 그 외는 backend가 IntervalOutOfRange 거부.
 */
export async function startAutoUpdatePoller(repo, currentVersion, interval_secs, onEvent) {
    const channel = new Channel();
    channel.onmessage = onEvent;
    await invoke("start_auto_update_poller", {
        repo,
        currentVersion,
        intervalSecs: interval_secs,
        onEvent: channel,
    });
}
/** 자동 폴러 중단 — idempotent. 미실행 = no-op. */
export async function stopAutoUpdatePoller() {
    await invoke("stop_auto_update_poller");
}
/** 자동 폴러 상태 조회. */
export async function getAutoUpdateStatus() {
    return invoke("get_auto_update_status");
}
