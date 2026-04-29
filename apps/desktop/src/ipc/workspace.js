// workspace IPC — fingerprint 조회 + 3-tier repair.
// Rust crates/portable-workspace의 WorkspaceFingerprint / RepairTier / RepairReport 미러.
import { invoke } from "@tauri-apps/api/core";
/** 현재 host fingerprint + 저장된 것과 비교한 tier (액션은 적용 안 함). */
export async function getWorkspaceFingerprint() {
    return invoke("get_workspace_fingerprint");
}
/** 실제 repair 적용 — cache invalidate + manifest 갱신 + fingerprint 저장. */
export async function checkWorkspaceRepair() {
    return invoke("check_workspace_repair");
}
