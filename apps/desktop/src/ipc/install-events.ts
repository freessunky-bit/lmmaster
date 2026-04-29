// InstallEvent — Rust crates/installer/src/install_event.rs의 Serialize 미러.
// kind 기반 discriminated union. 변형은 #[serde(tag = "kind", rename_all = "kebab-case")]에 1:1 대응.
//
// Phase 1A.3.c: 수동 미러. Phase 후순위에서 tauri-specta로 자동 생성 예정 (ADR-0015).

export type ExtractPhase = "starting" | "extracting" | "done";
export type PostCheckStatus = "pending" | "passed" | "failed" | "skipped";

// ── DownloadEvent — crates/installer/src/progress.rs ───────────────────────

export type DownloadEvent =
  | { kind: "started"; url: string; total: number | null; resume_from: number }
  | {
      kind: "progress";
      downloaded: number;
      total: number | null;
      speed_bps: number;
    }
  | { kind: "verified"; sha256_hex: string }
  | { kind: "finished"; final_path: string; bytes: number }
  | { kind: "retrying"; attempt: number; delay_ms: number; reason: string };

// ── ActionOutcome — crates/installer/src/action.rs ─────────────────────────

export type ActionOutcome =
  | {
      kind: "success";
      method: string;
      exit_code: number | null;
      post_install_check_passed: boolean | null;
    }
  | { kind: "success-reboot-required"; method: string; exit_code: number }
  | { kind: "opened-url"; url: string };

// ── InstallEvent ──────────────────────────────────────────────────────────

export type InstallEvent =
  | {
      kind: "started";
      id: string;
      method: string;
      display_name: string;
    }
  | { kind: "download"; download: DownloadEvent }
  | {
      kind: "extract";
      phase: ExtractPhase;
      entries: number;
      total_bytes: number;
    }
  | { kind: "post-check"; status: PostCheckStatus }
  | { kind: "finished"; outcome: ActionOutcome }
  | { kind: "failed"; code: string; message: string }
  | { kind: "cancelled" };

// ── Type guards (간단한 narrowing 헬퍼) ────────────────────────────────────

export function isTerminal(ev: InstallEvent): boolean {
  return (
    ev.kind === "finished" || ev.kind === "failed" || ev.kind === "cancelled"
  );
}

export function totalProgress(
  ev: InstallEvent
): { downloaded: number; total: number | null } | null {
  if (ev.kind === "download" && ev.download.kind === "progress") {
    return { downloaded: ev.download.downloaded, total: ev.download.total };
  }
  return null;
}
