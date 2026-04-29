// install / cancel_install Tauri command 래퍼.
// Channel<InstallEvent>로 진행 이벤트를 받고, ActionOutcome을 Promise로 반환한다.

import { Channel, invoke } from "@tauri-apps/api/core";

import type { ActionOutcome, InstallEvent } from "./install-events";

export type InstallApiError =
  | { kind: "already-installing"; id: string }
  | { kind: "manifest-not-found"; message: string }
  | { kind: "manifest-parse"; message: string }
  | { kind: "cache-dir-create"; message: string }
  | { kind: "runner"; code: string; message: string };

export interface InstallAppOptions {
  /// 이벤트 콜백 — 모든 단계의 InstallEvent를 받는다. terminal 이벤트(finished/failed/cancelled) 후에도
  /// 정리만 하면 됨 — Tauri Channel은 command 종료 시 자동 close.
  onEvent: (event: InstallEvent) => void;
}

/**
 * 매니페스트 id로 앱을 설치한다. 진행 이벤트는 onEvent로 흘러오고, 최종 ActionOutcome이 resolve.
 *
 * Promise reject 시 InstallApiError로 캐치 가능 (kind 기반 분기).
 */
export async function installApp(
  id: string,
  options: InstallAppOptions
): Promise<ActionOutcome> {
  const channel = new Channel<InstallEvent>();
  channel.onmessage = options.onEvent;
  return invoke<ActionOutcome>("install_app", { id, channel });
}

/**
 * 진행 중 설치를 cancel. 미진행 id면 no-op.
 *
 * 실제 종료(추가 emit + Promise resolve/reject)는 download/extract/post-check 단계가 cancel을
 * 인식하는 시점까지 약간의 지연이 있을 수 있다.
 */
export async function cancelInstall(id: string): Promise<void> {
  await invoke<void>("cancel_install", { id });
}
