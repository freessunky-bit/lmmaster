// ingest_path / cancel_ingest / search_knowledge / list_ingests / knowledge_workspace_stats
// Tauri command 래퍼. Phase 4.5'.b.
//
// Workbench (5'.b) ipc/workbench.ts와 동일 결 — Channel<IngestEvent>로 진행 이벤트를 받고,
// ingest_id를 즉시 Promise로 반환. workspace_id 단위 직렬화 — 동일 workspace에 동시 ingest 시도 시
// backend가 AlreadyIngesting 거부 (한국어 해요체 메시지).
import { Channel, invoke } from "@tauri-apps/api/core";
// ── helpers ───────────────────────────────────────────────────────────
/** terminal 이벤트 — 더 이상 progress 이벤트가 안 옴. */
export function isTerminalIngestEvent(ev) {
    return ev.kind === "done" || ev.kind === "failed" || ev.kind === "cancelled";
}
/**
 * Ingest 시작. Channel<IngestEvent>로 매 단계 진행을 전달받는다.
 *
 * 반환된 handle.cancel()은 backend cancel_ingest를 호출 — workspace_id 기준 idempotent.
 * 동일 workspace에 이미 ingest가 진행 중이면 invoke가 reject (AlreadyIngesting).
 *
 * 에러는 invoke().reject로 도달 — kind discriminant 기반 narrow.
 */
export async function startIngest(config, onEvent) {
    const channel = new Channel();
    channel.onmessage = onEvent;
    const ingest_id = await invoke("ingest_path", {
        config,
        onEvent: channel,
    });
    return {
        ingest_id,
        cancel: () => cancelIngest(config.workspace_id),
    };
}
/** 진행 중 ingest cancel — idempotent. workspace_id 기반. */
export async function cancelIngest(workspace_id) {
    await invoke("cancel_ingest", { workspaceId: workspace_id });
}
/**
 * 동기 검색 RPC. v1은 MockEmbedder. k는 max 50으로 backend가 cap.
 *
 * @param workspace_id 검색 대상 워크스페이스.
 * @param query 텍스트 쿼리.
 * @param k 반환할 hit 수.
 * @param store_path SQLite 파일 경로.
 */
export async function searchKnowledge(workspace_id, query, k, store_path) {
    return invoke("search_knowledge", {
        workspaceId: workspace_id,
        query,
        k,
        storePath: store_path,
    });
}
/** 활성 ingest snapshot 목록. */
export async function listIngests() {
    return invoke("list_ingests");
}
/** Workspace 통계 — banner / Knowledge tab 헤더 노출용. */
export async function workspaceStats(workspace_id, store_path) {
    return invoke("knowledge_workspace_stats", {
        workspaceId: workspace_id,
        storePath: store_path,
    });
}
