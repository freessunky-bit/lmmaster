// Phase 13'.c — Crash 뷰어 IPC wrapper.
// 실제 invoke 키는 src-tauri/src/crash.rs 와 일치해야 한다.

import { invoke } from "@tauri-apps/api/core";

export interface CrashSummary {
  /** 파일 이름 (예: `crash-2026-04-30T08-12-34Z.txt`). */
  filename: string;
  /** 파일 크기 (byte). */
  size_bytes: number;
  /** 파일명에서 추출한 RFC3339 timestamp 시도. 실패 시 null. */
  ts_rfc3339: string | null;
  /** UNIX epoch ms — 정렬 기준. */
  mtime_ms: number;
}

export type CrashIpcError =
  | { kind: "not-initialized" }
  | { kind: "invalid-filename" }
  | { kind: "not-found" }
  | { kind: "too-large"; bytes: number }
  | { kind: "io"; message: string };

/** crash 디렉터리에서 `crash-*.txt` 파일을 mtime DESC로 정렬해 최대 limit개 반환. */
export async function listCrashReports(limit?: number): Promise<CrashSummary[]> {
  return invoke<CrashSummary[]>("list_crash_reports", { limit });
}

/** 단일 crash 파일을 통째로 읽어서 반환. 1 MB 초과 시 거부. */
export async function readCrashLog(filename: string): Promise<string> {
  return invoke<string>("read_crash_log", { filename });
}
