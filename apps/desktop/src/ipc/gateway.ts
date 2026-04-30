// Typed wrappers around Tauri IPC for gateway state.
// 실제 invoke 키와 event 이름은 src-tauri/src/{commands.rs, gateway.rs}와 일치해야 한다.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type GatewayStatus = "booting" | "listening" | "failed" | "stopping";

export interface GatewayState {
  port: number | null;
  status: GatewayStatus;
  error: string | null;
}

export async function getGatewayStatus(): Promise<GatewayState> {
  return invoke<GatewayState>("get_gateway_status");
}

export async function onGatewayReady(
  cb: (port: number) => void
): Promise<UnlistenFn> {
  return listen<number>("gateway://ready", (e) => cb(e.payload));
}

export async function onGatewayFailed(
  cb: (error: string) => void
): Promise<UnlistenFn> {
  return listen<string>("gateway://failed", (e) => cb(e.payload));
}

// ── Phase 13'.b — Diagnostics 실 데이터 IPC ──────────────────────

/** core-gateway::usage_log::RequestRecord 미러. */
export interface RequestRecord {
  /** UNIX epoch ms. */
  ts_ms: number;
  method: string;
  path: string;
  status: number;
  ms: number;
}

export interface Percentiles {
  p50_ms: number;
  p95_ms: number;
  count: number;
}

/** 60s latency sparkline — 30 bucket 평균 ms. 빈 bucket은 0. */
export async function getGatewayLatencySparkline(): Promise<number[]> {
  return invoke<number[]>("get_gateway_latency_sparkline");
}

/** 최근 N개 요청 메타. 최근 → 오래된 순서. */
export async function getGatewayRecentRequests(
  limit?: number,
): Promise<RequestRecord[]> {
  return invoke<RequestRecord[]>("get_gateway_recent_requests", { limit });
}

/** p50/p95/count 스냅샷. */
export async function getGatewayPercentiles(): Promise<Percentiles> {
  return invoke<Percentiles>("get_gateway_percentiles");
}
