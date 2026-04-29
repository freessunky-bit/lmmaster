// workspace IPC — fingerprint 조회 + 3-tier repair.
// Rust crates/portable-workspace의 WorkspaceFingerprint / RepairTier / RepairReport 미러.

import { invoke } from "@tauri-apps/api/core";

export type GpuClass = "nvidia" | "amd" | "intel" | "apple" | "none" | "other";

export type RepairTier = "green" | "yellow" | "red";

export interface WorkspaceFingerprint {
  os: string;
  arch: string;
  gpu_class: GpuClass;
  vram_bucket_mb: number;
  ram_bucket_mb: number;
  fingerprint_hash: string;
}

export interface WorkspaceStatus {
  fingerprint: WorkspaceFingerprint;
  previous: WorkspaceFingerprint | null;
  tier: RepairTier;
  workspace_root: string;
}

export interface RepairReport {
  tier: RepairTier;
  invalidated_caches: string[];
  invalidated_runtimes: number;
  models_preserved: number;
}

export type WorkspaceApiError =
  | { kind: "host-not-probed" }
  | { kind: "disk"; message: string }
  | { kind: "internal"; message: string };

/** 현재 host fingerprint + 저장된 것과 비교한 tier (액션은 적용 안 함). */
export async function getWorkspaceFingerprint(): Promise<WorkspaceStatus> {
  return invoke<WorkspaceStatus>("get_workspace_fingerprint");
}

/** 실제 repair 적용 — cache invalidate + manifest 갱신 + fingerprint 저장. */
export async function checkWorkspaceRepair(): Promise<RepairReport> {
  return invoke<RepairReport>("check_workspace_repair");
}
