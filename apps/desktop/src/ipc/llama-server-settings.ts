// Phase 13'.h.2.e.1 — LlamaCpp binary path settings IPC helper.

import { Channel, invoke } from "@tauri-apps/api/core";

/** 현재 저장된 binary path 반환 — null이면 미설정. */
export async function getLlamaServerPath(): Promise<string | null> {
  return invoke<string | null>("get_llama_server_path");
}

/** path_token으로 binary path 등록 (file picker 후 호출). */
export async function setLlamaServerPath(pathToken: string): Promise<void> {
  await invoke<void>("set_llama_server_path", { pathToken });
}

/** 등록된 path 초기화. */
export async function clearLlamaServerPath(): Promise<void> {
  await invoke<void>("clear_llama_server_path");
}

// Phase 13'.h.2.f.1 — llama-server 자동 install IPC.

export type LlamaInstallEvent =
  | { kind: "status"; status: string }
  | {
      kind: "progress";
      completed_bytes: number;
      total_bytes: number;
      speed_bps: number;
    }
  | { kind: "completed"; binary_path: string }
  | { kind: "failed"; message: string };

/** GPU 자동 감지 + 적합 빌드 다운로드 + extract + settings 자동 등록. */
export async function installLlamaCppRuntime(
  onEvent: (event: LlamaInstallEvent) => void,
): Promise<string> {
  const channel = new Channel<LlamaInstallEvent>();
  channel.onmessage = onEvent;
  return invoke<string>("install_llama_cpp_runtime", { channel });
}
