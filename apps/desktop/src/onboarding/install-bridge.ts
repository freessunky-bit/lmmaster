// Module-scope bridge — install actor의 onEvent 콜백을 actorRef.send로 forward.
// Phase 1A.4.c 보강 §2 Pattern 2.
//
// 정책:
// - 단일 onboarding actor만 존재 → singleton OK.
// - Provider 안 InstallEventBridge가 mount/unmount 시점에 set/clear.
// - install actor의 fromPromise 본체에서 getInstallEventBridge()로 읽어 forward.
// - cleared 상태(null)면 silent drop — 마법사 unmount 후 도착하는 이벤트.

import type { InstallEvent } from "../ipc/install-events";

let installEventBridge: ((e: InstallEvent) => void) | null = null;

export function setInstallEventBridge(fn: ((e: InstallEvent) => void) | null): void {
  installEventBridge = fn;
}

export function getInstallEventBridge(): ((e: InstallEvent) => void) | null {
  return installEventBridge;
}
