// scanner IPC — start_scan / get_last_scan + scan:summary listener.
// Rust crates/scanner의 ScanSummary serde 미러.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Severity = "info" | "warn" | "error";

export interface CheckResult {
  id: string;
  severity: Severity;
  title_ko: string;
  detail_ko: string;
  recommendation?: string;
}

export type SummarySource = "llm" | "deterministic";

export interface ScanSummary {
  /** SystemTime serialized as RFC-ish — Rust serde_json은 ISO 비슷하게 만듬. UI에선 그대로 표시 안 함. */
  started_at: unknown;
  checks: CheckResult[];
  summary_korean: string;
  summary_source: SummarySource;
  model_used?: string;
  took_ms: number;
}

export type ScanApiError =
  | { kind: "already-running" }
  | { kind: "internal"; message: string };

/** 즉시 자가 점검 실행. Promise reject 시 ScanApiError 캐치 가능. */
export async function startScan(): Promise<ScanSummary> {
  return invoke<ScanSummary>("start_scan");
}

/** 마지막 캐시된 점검 결과. 한 번도 안 됐으면 null. */
export async function getLastScan(): Promise<ScanSummary | null> {
  return invoke<ScanSummary | null>("get_last_scan");
}

/** scan:summary event 구독 — broadcast subscriber가 forward. */
export async function onScanSummary(
  cb: (summary: ScanSummary) => void,
): Promise<UnlistenFn> {
  return listen<ScanSummary>("scan:summary", (e) => cb(e.payload));
}
