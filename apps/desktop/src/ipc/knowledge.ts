// ingest_path / cancel_ingest / search_knowledge / list_ingests / knowledge_workspace_stats
// Tauri command 래퍼. Phase 4.5'.b.
//
// Workbench (5'.b) ipc/workbench.ts와 동일 결 — Channel<IngestEvent>로 진행 이벤트를 받고,
// ingest_id를 즉시 Promise로 반환. workspace_id 단위 직렬화 — 동일 workspace에 동시 ingest 시도 시
// backend가 AlreadyIngesting 거부 (한국어 해요체 메시지).

import { Channel, invoke } from "@tauri-apps/api/core";

// ── Backend 미러 타입 ─────────────────────────────────────────────────

/** Rust knowledge_stack::IngestStage 미러 — kebab-case. */
export type IngestStage =
  | "reading"
  | "chunking"
  | "embedding"
  | "writing"
  | "done";

/** Rust knowledge::IngestConfig 미러. backend 기본값과 일치. */
export interface IngestConfig {
  workspace_id: string;
  path: string;
  /** "file" 또는 "directory". v1 backend는 자동 판별하지만 UI 의도 보존용. */
  kind?: "file" | "directory";
  /** 청크 목표 크기 (문자 단위). 기본 1000. */
  target_chunk_size?: number;
  /** 청크 overlap (문자 단위). 기본 200. */
  overlap?: number;
  /** SQLite 파일 경로 — workspace별 격리. 빈 string이면 in-memory (테스트 전용). */
  store_path?: string;
}

/** Rust knowledge::IngestSummary 미러. */
export interface IngestSummary {
  ingest_id: string;
  workspace_id: string;
  files_processed: number;
  chunks_created: number;
  skipped: number;
  total_duration_ms: number;
}

/** Rust knowledge::SearchHit 미러. */
export interface SearchHit {
  chunk_id: string;
  document_id: string;
  document_path: string;
  content: string;
  score: number;
}

/** Rust knowledge::WorkspaceStats 미러. */
export interface WorkspaceStats {
  workspace_id: string;
  documents: number;
  chunks: number;
}

/** Rust knowledge::ActiveIngestSnapshot 미러 — list_ingests 반환. */
export interface ActiveIngestSnapshot {
  workspace_id: string;
  ingest_id: string;
  /** RFC3339. */
  started_at: string;
  current_stage: IngestStage;
}

/** Rust knowledge::IngestEvent 미러 — kind 기반 discriminated union. */
export type IngestEvent =
  | { kind: "started"; ingest_id: string; workspace_id: string; path: string }
  | { kind: "reading"; ingest_id: string; current_path: string }
  | { kind: "chunking"; ingest_id: string; processed: number; total: number }
  | { kind: "embedding"; ingest_id: string; processed: number; total: number }
  | { kind: "writing"; ingest_id: string; processed: number; total: number }
  | { kind: "done"; ingest_id: string; summary: IngestSummary }
  | { kind: "failed"; ingest_id: string; error: string }
  | { kind: "cancelled"; ingest_id: string };

/** invoke().reject로 도달하는 backend 에러. */
export type KnowledgeApiError =
  | { kind: "already-ingesting"; workspace_id: string }
  | { kind: "workspace-not-found"; workspace_id: string }
  | { kind: "store-open"; message: string }
  | { kind: "start-failed"; message: string }
  | { kind: "search-failed"; message: string }
  | { kind: "internal"; message: string };

// ── helpers ───────────────────────────────────────────────────────────

/** terminal 이벤트 — 더 이상 progress 이벤트가 안 옴. */
export function isTerminalIngestEvent(ev: IngestEvent): boolean {
  return ev.kind === "done" || ev.kind === "failed" || ev.kind === "cancelled";
}

// ── Tauri command 래퍼 ───────────────────────────────────────────────

export interface StartIngestHandle {
  /** uuid ingest_id. cancel/list lookup 키. */
  ingest_id: string;
  /** 진행 중 ingest를 cancel — idempotent. */
  cancel: () => Promise<void>;
}

/**
 * Ingest 시작. Channel<IngestEvent>로 매 단계 진행을 전달받는다.
 *
 * 반환된 handle.cancel()은 backend cancel_ingest를 호출 — workspace_id 기준 idempotent.
 * 동일 workspace에 이미 ingest가 진행 중이면 invoke가 reject (AlreadyIngesting).
 *
 * 에러는 invoke().reject로 도달 — kind discriminant 기반 narrow.
 */
export async function startIngest(
  config: IngestConfig,
  onEvent: (event: IngestEvent) => void,
): Promise<StartIngestHandle> {
  const channel = new Channel<IngestEvent>();
  channel.onmessage = onEvent;
  const ingest_id = await invoke<string>("ingest_path", {
    config,
    onEvent: channel,
  });
  return {
    ingest_id,
    cancel: () => cancelIngest(config.workspace_id),
  };
}

/** 진행 중 ingest cancel — idempotent. workspace_id 기반. */
export async function cancelIngest(workspace_id: string): Promise<void> {
  await invoke<void>("cancel_ingest", { workspaceId: workspace_id });
}

/**
 * 동기 검색 RPC. v1은 MockEmbedder. k는 max 50으로 backend가 cap.
 *
 * @param workspace_id 검색 대상 워크스페이스.
 * @param query 텍스트 쿼리.
 * @param k 반환할 hit 수.
 * @param store_path SQLite 파일 경로.
 */
export async function searchKnowledge(
  workspace_id: string,
  query: string,
  k: number,
  store_path: string,
): Promise<SearchHit[]> {
  return invoke<SearchHit[]>("search_knowledge", {
    workspaceId: workspace_id,
    query,
    k,
    storePath: store_path,
  });
}

/** 활성 ingest snapshot 목록. */
export async function listIngests(): Promise<ActiveIngestSnapshot[]> {
  return invoke<ActiveIngestSnapshot[]>("list_ingests");
}

/** Workspace 통계 — banner / Knowledge tab 헤더 노출용. */
export async function workspaceStats(
  workspace_id: string,
  store_path: string,
): Promise<WorkspaceStats> {
  return invoke<WorkspaceStats>("knowledge_workspace_stats", {
    workspaceId: workspace_id,
    storePath: store_path,
  });
}
