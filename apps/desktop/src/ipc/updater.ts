// check_for_update / cancel_update_check / start_auto_update_poller / stop_auto_update_poller /
// get_auto_update_status Tauri command 래퍼. Phase 6'.b.
//
// Workbench (5'.b) ipc/workbench.ts와 동일 결 — Channel<UpdateEvent>로 진행 이벤트를 받고,
// check_id를 즉시 Promise로 반환. 자동 폴러는 single-slot — 두 번째 start는 backend가 거부.

import { Channel, invoke } from "@tauri-apps/api/core";

// ── Backend 미러 타입 ─────────────────────────────────────────────────

/** Rust auto_updater::ReleaseInfo DTO 미러 — published_at은 RFC3339 string. */
export interface ReleaseInfo {
  version: string;
  published_at_iso: string;
  url: string;
  notes: string | null;
}

/** Rust updater::PollerStatus 미러. */
export interface PollerStatus {
  active: boolean;
  repo: string | null;
  interval_secs: number | null;
  /** 마지막 polling cycle 끝난 시각 (RFC3339). */
  last_check_iso: string | null;
}

/** Rust updater::UpdateEvent 미러 — kind 기반 discriminated union. */
export type UpdateEvent =
  | { kind: "started"; check_id: string; current_version: string; repo: string }
  | {
      kind: "outdated";
      check_id: string;
      current_version: string;
      latest: ReleaseInfo;
    }
  | { kind: "up-to-date"; check_id: string; current_version: string }
  | { kind: "failed"; check_id: string; error: string }
  | { kind: "cancelled"; check_id: string };

/** invoke().reject로 도달하는 backend 에러. */
export type UpdaterApiError =
  | { kind: "poller-already-running" }
  | { kind: "interval-out-of-range"; got: number }
  | { kind: "invalid-repo" }
  | { kind: "start-failed"; message: string };

// ── helpers ───────────────────────────────────────────────────────────

/** terminal 이벤트 — 더 이상 같은 check_id로 event가 안 옴. */
export function isTerminalUpdateEvent(ev: UpdateEvent): boolean {
  return (
    ev.kind === "outdated" ||
    ev.kind === "up-to-date" ||
    ev.kind === "failed" ||
    ev.kind === "cancelled"
  );
}

// ── Tauri command 래퍼 ───────────────────────────────────────────────

export interface CheckForUpdateHandle {
  /** uuid check_id. cancel lookup 키. */
  check_id: string;
  /** 진행 중 check를 cancel — idempotent. */
  cancel: () => Promise<void>;
}

/**
 * 단발 update check 시작. Channel<UpdateEvent>로 진행 이벤트를 받는다.
 *
 * 반환된 handle.cancel()은 backend cancel_update_check를 호출 — check_id 기준 idempotent.
 * 에러는 invoke().reject로 도달 — kind discriminant 기반 narrow.
 */
export async function checkForUpdate(
  repo: string,
  currentVersion: string,
  onEvent: (event: UpdateEvent) => void,
): Promise<CheckForUpdateHandle> {
  const channel = new Channel<UpdateEvent>();
  channel.onmessage = onEvent;
  const check_id = await invoke<string>("check_for_update", {
    repo,
    currentVersion,
    onEvent: channel,
  });
  return {
    check_id,
    cancel: () => cancelUpdateCheck(check_id),
  };
}

/** 진행 중 check cancel — idempotent. 미존재 id면 no-op. */
export async function cancelUpdateCheck(check_id: string): Promise<void> {
  await invoke<void>("cancel_update_check", { checkId: check_id });
}

/**
 * 자동 폴러 시작 — single-slot. 이미 실행 중이면 backend가 PollerAlreadyRunning 거부.
 *
 * @param interval_secs 1h(3600)~24h(86400). 그 외는 backend가 IntervalOutOfRange 거부.
 */
export async function startAutoUpdatePoller(
  repo: string,
  currentVersion: string,
  interval_secs: number,
  onEvent: (event: UpdateEvent) => void,
): Promise<void> {
  const channel = new Channel<UpdateEvent>();
  channel.onmessage = onEvent;
  await invoke<void>("start_auto_update_poller", {
    repo,
    currentVersion,
    intervalSecs: interval_secs,
    onEvent: channel,
  });
}

/** 자동 폴러 중단 — idempotent. 미실행 = no-op. */
export async function stopAutoUpdatePoller(): Promise<void> {
  await invoke<void>("stop_auto_update_poller");
}

/** 자동 폴러 상태 조회. */
export async function getAutoUpdateStatus(): Promise<PollerStatus> {
  return invoke<PollerStatus>("get_auto_update_status");
}
