// Portable workspace IPC — Phase 11'.
//
// Rust crates/portable-workspace의 ExportEvent / ImportEvent + ArchivePreview를 미러.
// 모든 events는 #[serde(tag = "kind", rename_all = "kebab-case")]에 1:1 대응.

import { Channel, invoke } from "@tauri-apps/api/core";

// ── 공통 ────────────────────────────────────────────────────────────────

export type ConflictPolicy = "skip" | "overwrite" | "rename";

export interface WorkspaceFingerprintView {
  os: string;
  arch: string;
  gpu_class:
    | "nvidia"
    | "amd"
    | "intel"
    | "apple"
    | "none"
    | "other";
  vram_bucket_mb: number;
  ram_bucket_mb: number;
  fingerprint_hash: string;
}

export type RepairTier = "green" | "yellow" | "red";

// ── Export ──────────────────────────────────────────────────────────────

export type ExportEvent =
  | {
      kind: "started";
      source_path: string;
      target_path: string;
    }
  | {
      kind: "counting";
      total_files: number;
      total_bytes: number;
    }
  | {
      kind: "compressing";
      processed: number;
      total: number;
      current_path: string;
    }
  | { kind: "encrypting" }
  | { kind: "finalizing" }
  | {
      kind: "done";
      sha256: string;
      archive_size_bytes: number;
      target_path: string;
    }
  | { kind: "failed"; error: string };

export interface ExportOptions {
  include_models: boolean;
  include_keys: boolean;
  key_passphrase?: string | null;
  target_path: string;
}

export interface ExportSummary {
  sha256: string;
  archive_size_bytes: number;
  files_count: number;
}

export interface StartExportResponse {
  export_id: string;
  summary: ExportSummary;
}

/** 진행 중 export를 시작. onEvent로 진행 이벤트 stream. terminal까지 자동 close. */
export async function startWorkspaceExport(
  options: ExportOptions,
  onEvent: (ev: ExportEvent) => void,
): Promise<StartExportResponse> {
  const channel = new Channel<ExportEvent>();
  channel.onmessage = onEvent;
  return invoke<StartExportResponse>("start_workspace_export", {
    req: options,
    onEvent: channel,
  });
}

/** 진행 중 export cancel — 미진행 export_id면 unknown-job 에러. */
export async function cancelWorkspaceExport(
  exportId: string,
): Promise<void> {
  return invoke<void>("cancel_workspace_export", { exportId });
}

/** event가 export 종료 신호인지 (done / failed). */
export function isTerminalExportEvent(ev: ExportEvent): boolean {
  return ev.kind === "done" || ev.kind === "failed";
}

// ── Import ──────────────────────────────────────────────────────────────

export type ImportEvent =
  | {
      kind: "started";
      source_path: string;
      target_path: string;
    }
  | { kind: "verifying" }
  | {
      kind: "extracting";
      processed: number;
      total: number;
    }
  | { kind: "decrypting-keys" }
  | { kind: "repair-tier"; tier: RepairTier }
  | {
      kind: "done";
      manifest_summary: string;
      repair_tier: RepairTier;
    }
  | { kind: "failed"; error: string };

export interface ImportOptions {
  source_path: string;
  target_workspace_root?: string | null;
  key_passphrase?: string | null;
  conflict_policy: ConflictPolicy;
  expected_sha256?: string | null;
}

export interface ImportSummary {
  repair_tier: "green" | "yellow" | "red";
  source_fingerprint: WorkspaceFingerprintView | null;
  manifest_summary: string;
}

export interface StartImportResponse {
  import_id: string;
  summary: ImportSummary;
}

export interface ArchivePreview {
  manifest_summary: string;
  source_fingerprint: WorkspaceFingerprintView | null;
  size_bytes: number;
  has_models: boolean;
  has_keys: boolean;
  entries_count: number;
}

/** import 전 archive 미리보기. */
export async function verifyWorkspaceArchive(
  sourcePath: string,
): Promise<ArchivePreview> {
  return invoke<ArchivePreview>("verify_workspace_archive", {
    sourcePath,
  });
}

/** import 시작. onEvent로 진행 이벤트 stream. */
export async function startWorkspaceImport(
  options: ImportOptions,
  onEvent: (ev: ImportEvent) => void,
): Promise<StartImportResponse> {
  const channel = new Channel<ImportEvent>();
  channel.onmessage = onEvent;
  return invoke<StartImportResponse>("start_workspace_import", {
    req: options,
    onEvent: channel,
  });
}

export async function cancelWorkspaceImport(
  importId: string,
): Promise<void> {
  return invoke<void>("cancel_workspace_import", { importId });
}

export function isTerminalImportEvent(ev: ImportEvent): boolean {
  return ev.kind === "done" || ev.kind === "failed";
}

// ── 에러 형태 (kind 기반 narrowing) ──────────────────────────────────────

export type PortableApiError =
  | { kind: "already-running"; id: string }
  | { kind: "unknown-job"; id: string }
  | { kind: "export-failed"; message: string }
  | { kind: "import-failed"; message: string }
  | { kind: "verify-failed"; message: string }
  | { kind: "disk"; message: string };
