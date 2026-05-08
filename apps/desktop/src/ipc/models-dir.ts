// Phase 8'.c.4 (ADR-0066) Q1 helper — 모델 폴더 열기 IPC helper.

import { invoke } from "@tauri-apps/api/core";

/** 모델 폴더 경로 read — UI 표시용. 결과는 `app_local_data_dir/models`. */
export async function getModelsDir(): Promise<string> {
  return invoke<string>("get_models_dir");
}

/** 모델 폴더를 OS 파일 탐색기로 열기. 없으면 생성. */
export async function openModelsDir(): Promise<void> {
  await invoke<void>("open_models_dir");
}
