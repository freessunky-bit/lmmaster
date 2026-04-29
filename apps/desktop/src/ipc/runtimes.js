// runtimes IPC — list_runtime_statuses / list_runtime_models.
// Rust apps/desktop/src-tauri/src/runtimes/commands.rs serde 미러.
//
// 정책 (phase-4-screens-decision.md §1.1 runtimes, phase-4c-runtimes-decision.md):
// - 어댑터 합산 status는 한 번의 invoke로 모음.
// - 특정 어댑터의 model 목록은 별도 invoke (선택된 카드일 때만).
// - LM Studio는 size_bytes를 0으로 리턴 — 그대로 표시.
import { invoke } from "@tauri-apps/api/core";
/** 모든 어댑터(Ollama / LM Studio)의 상태를 한 번에 가져온다. */
export async function listRuntimeStatuses() {
    return invoke("list_runtime_statuses");
}
/** 특정 어댑터에 로드된 모델 목록 — Unreachable이면 빈 상태로 화면 처리. */
export async function listRuntimeModels(runtimeKind) {
    return invoke("list_runtime_models", { runtimeKind });
}
