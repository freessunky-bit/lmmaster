// Portable workspace IPC — Phase 11'.
//
// Rust crates/portable-workspace의 ExportEvent / ImportEvent + ArchivePreview를 미러.
// 모든 events는 #[serde(tag = "kind", rename_all = "kebab-case")]에 1:1 대응.
import { Channel, invoke } from "@tauri-apps/api/core";
/** 진행 중 export를 시작. onEvent로 진행 이벤트 stream. terminal까지 자동 close. */
export async function startWorkspaceExport(options, onEvent) {
    const channel = new Channel();
    channel.onmessage = onEvent;
    return invoke("start_workspace_export", {
        req: options,
        onEvent: channel,
    });
}
/** 진행 중 export cancel — 미진행 export_id면 unknown-job 에러. */
export async function cancelWorkspaceExport(exportId) {
    return invoke("cancel_workspace_export", { exportId });
}
/** event가 export 종료 신호인지 (done / failed). */
export function isTerminalExportEvent(ev) {
    return ev.kind === "done" || ev.kind === "failed";
}
/** import 전 archive 미리보기. */
export async function verifyWorkspaceArchive(sourcePath) {
    return invoke("verify_workspace_archive", {
        sourcePath,
    });
}
/** import 시작. onEvent로 진행 이벤트 stream. */
export async function startWorkspaceImport(options, onEvent) {
    const channel = new Channel();
    channel.onmessage = onEvent;
    return invoke("start_workspace_import", {
        req: options,
        onEvent: channel,
    });
}
export async function cancelWorkspaceImport(importId) {
    return invoke("cancel_workspace_import", { importId });
}
export function isTerminalImportEvent(ev) {
    return ev.kind === "done" || ev.kind === "failed";
}
