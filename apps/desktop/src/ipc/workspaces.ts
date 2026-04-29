// Workspaces 관리 IPC 래퍼 — Phase 8'.1.
//
// 정책 (ADR-0038):
// - Backend Rust commands `list_workspaces / get_active_workspace / create_workspace /
//   rename_workspace / delete_workspace / set_active_workspace` 미러.
// - 영속은 backend가 담당 — frontend는 단순 호출 + 이벤트 listen.
// - `workspaces://changed` 이벤트로 active/list 변경을 push 받아 재구독.
// - 에러 kind 기반 discriminated union — UI 한국어 메시지 매핑은 호출자 책임.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// ── Backend 미러 타입 ─────────────────────────────────────────────────

/** Rust workspaces::WorkspaceInfo 미러. */
export interface WorkspaceInfo {
  /** UUID v4. */
  id: string;
  /** 사용자 표시명. */
  name: string;
  /** 사용자 메모 — null 가능. */
  description: string | null;
  /** RFC3339 생성 시각. */
  created_at_iso: string;
  /** RFC3339 마지막 active 시각. null이면 아직 한 번도 active가 아니었어요. */
  last_used_iso: string | null;
}

/** invoke().reject로 도달하는 backend 에러. */
export type WorkspacesApiError =
  | { kind: "not-found"; id: string }
  | { kind: "duplicate-name"; name: string }
  | { kind: "empty-name" }
  | { kind: "cannot-delete-only-workspace" }
  | { kind: "persist"; message: string }
  | { kind: "internal"; message: string };

/** workspaces://changed 이벤트 페이로드. */
export interface WorkspacesChangedPayload {
  active_id: string;
  workspaces: WorkspaceInfo[];
}

const EVENT_CHANGED = "workspaces://changed";

// ── command 래퍼 ─────────────────────────────────────────────────────

export async function listWorkspaces(): Promise<WorkspaceInfo[]> {
  return invoke<WorkspaceInfo[]>("list_workspaces");
}

export async function getActiveWorkspace(): Promise<WorkspaceInfo> {
  return invoke<WorkspaceInfo>("get_active_workspace");
}

export async function createWorkspace(
  name: string,
  description?: string,
): Promise<WorkspaceInfo> {
  return invoke<WorkspaceInfo>("create_workspace", {
    name,
    description: description ?? null,
  });
}

export async function renameWorkspace(
  id: string,
  newName: string,
): Promise<WorkspaceInfo> {
  return invoke<WorkspaceInfo>("rename_workspace", {
    id,
    newName,
  });
}

export async function deleteWorkspace(id: string): Promise<void> {
  await invoke<void>("delete_workspace", { id });
}

export async function setActiveWorkspace(id: string): Promise<void> {
  await invoke<void>("set_active_workspace", { id });
}

// ── 이벤트 ──────────────────────────────────────────────────────────

/** workspaces://changed 이벤트 listen. unlisten 함수 반환. */
export async function onWorkspacesChanged(
  callback: (payload: WorkspacesChangedPayload) => void,
): Promise<() => void> {
  const unlisten = await listen<WorkspacesChangedPayload>(EVENT_CHANGED, (e) => {
    callback(e.payload);
  });
  return unlisten;
}
