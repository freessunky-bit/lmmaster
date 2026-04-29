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
