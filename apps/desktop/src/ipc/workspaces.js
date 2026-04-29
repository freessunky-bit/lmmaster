// Workspaces 관리 IPC 래퍼 — Phase 8'.1.
//
// 정책 (ADR-0038):
// - Backend Rust commands `list_workspaces / get_active_workspace / create_workspace /
//   rename_workspace / delete_workspace / set_active_workspace` 미러.
// - 영속은 backend가 담당 — frontend는 단순 호출 + 이벤트 listen.
// - `workspaces://changed` 이벤트로 active/list 변경을 push 받아 재구독.
// - 에러 kind 기반 discriminated union — UI 한국어 메시지 매핑은 호출자 책임.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
const EVENT_CHANGED = "workspaces://changed";
// ── command 래퍼 ─────────────────────────────────────────────────────
export async function listWorkspaces() {
    return invoke("list_workspaces");
}
export async function getActiveWorkspace() {
    return invoke("get_active_workspace");
}
export async function createWorkspace(name, description) {
    return invoke("create_workspace", {
        name,
        description: description ?? null,
    });
}
export async function renameWorkspace(id, newName) {
    return invoke("rename_workspace", {
        id,
        newName,
    });
}
export async function deleteWorkspace(id) {
    await invoke("delete_workspace", { id });
}
export async function setActiveWorkspace(id) {
    await invoke("set_active_workspace", { id });
}
// ── 이벤트 ──────────────────────────────────────────────────────────
/** workspaces://changed 이벤트 listen. unlisten 함수 반환. */
export async function onWorkspacesChanged(callback) {
    const unlisten = await listen(EVENT_CHANGED, (e) => {
        callback(e.payload);
    });
    return unlisten;
}
