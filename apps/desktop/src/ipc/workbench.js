// start_workbench_run / cancel_workbench_run / list_workbench_runs Tauri command 래퍼.
//
// Phase 5'.b — Channel<WorkbenchEvent>로 진행 이벤트를 받고, run_id를 즉시 Promise로 반환한다.
// install.ts 패턴과 동일 결.
import { Channel, invoke } from "@tauri-apps/api/core";
// ── helpers ───────────────────────────────────────────────────────────
/** terminal 이벤트 — 더 이상 stage 이벤트가 안 옴. */
export function isTerminalEvent(ev) {
    return ev.kind === "completed" || ev.kind === "failed" || ev.kind === "cancelled";
}
/**
 * Workbench run 시작. Channel<WorkbenchEvent>로 매 단계 진행을 전달받는다.
 *
 * 반환된 handle.cancel()은 Rust cancel_workbench_run을 호출 — 미존재면 no-op.
 *
 * 에러는 invoke().reject로 도달 — kind discriminant 기반 narrow.
 */
export async function startWorkbenchRun(config, options) {
    const channel = new Channel();
    channel.onmessage = options.onEvent;
    const run_id = await invoke("start_workbench_run", { config, onEvent: channel });
    return {
        run_id,
        cancel: () => cancelWorkbenchRun(run_id),
    };
}
/** 진행 중 run cancel — idempotent. 미존재 id면 no-op. */
export async function cancelWorkbenchRun(run_id) {
    await invoke("cancel_workbench_run", { runId: run_id });
}
/** 활성 run snapshot 목록. */
export async function listWorkbenchRuns() {
    return invoke("list_workbench_runs");
}
/** JSONL 파일 첫 N개 line preview — Step 1에서 사용. */
export async function previewJsonl(path, limit) {
    return invoke("workbench_preview_jsonl", { path, limit });
}
/** 정규화된 examples를 JSONL string으로 직렬화. */
export async function serializeExamples(examples) {
    return invoke("workbench_serialize_examples", { examples });
}
/** Workbench가 등록한 custom-model 목록 — 카탈로그 페이지 등에서 사용. */
export async function listCustomModels() {
    return invoke("list_custom_models");
}
export async function getArtifactStats() {
    return invoke("get_artifact_stats");
}
export async function cleanupArtifactsNow() {
    return invoke("cleanup_artifacts_now");
}
