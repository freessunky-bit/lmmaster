// runtimes IPC — list_runtime_statuses / list_runtime_models.
// Rust apps/desktop/src-tauri/src/runtimes/commands.rs serde 미러.
//
// 정책 (phase-4-screens-decision.md §1.1 runtimes, phase-4c-runtimes-decision.md):
// - 어댑터 합산 status는 한 번의 invoke로 모음.
// - 특정 어댑터의 model 목록은 별도 invoke (선택된 카드일 때만).
// - LM Studio는 size_bytes를 0으로 리턴 — 그대로 표시.

import { invoke } from "@tauri-apps/api/core";
import type { RuntimeKind } from "./catalog";

export interface RuntimeStatus {
  kind: RuntimeKind;
  installed: boolean;
  version: string | null;
  running: boolean;
  latency_ms: number | null;
  model_count: number;
  /** RFC3339. */
  last_ping_at: string | null;
}

export interface RuntimeModelView {
  runtime_kind: RuntimeKind;
  id: string;
  size_bytes: number;
  digest: string;
}

export type RuntimesApiError =
  | { kind: "unreachable"; message: string }
  | { kind: "internal"; message: string };

/** 모든 어댑터(Ollama / LM Studio)의 상태를 한 번에 가져온다. */
export async function listRuntimeStatuses(): Promise<RuntimeStatus[]> {
  return invoke<RuntimeStatus[]>("list_runtime_statuses");
}

/** 특정 어댑터에 로드된 모델 목록 — Unreachable이면 빈 상태로 화면 처리. */
export async function listRuntimeModels(
  runtimeKind: RuntimeKind,
): Promise<RuntimeModelView[]> {
  return invoke<RuntimeModelView[]>("list_runtime_models", { runtimeKind });
}
