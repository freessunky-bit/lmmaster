// Phase 13'.h.2.e.1 — LlamaCpp binary path settings IPC helper.

import { invoke } from "@tauri-apps/api/core";

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
