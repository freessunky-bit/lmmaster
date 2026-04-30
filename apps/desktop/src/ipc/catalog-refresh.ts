// catalog-refresh IPC — Phase 1' integration.
// Rust src-tauri/src/registry_fetcher.rs의 LastRefresh / CatalogRefreshError serde 미러.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** registry_fetcher의 LastRefresh struct 미러. */
export interface LastRefresh {
  /** UNIX epoch ms. */
  at_ms: number;
  fetched_count: number;
  failed_count: number;
  outcome: "ok" | "partial" | "failed";
}

export type CatalogRefreshError =
  | { kind: "already-running" }
  | { kind: "not-initialized" }
  | { kind: "interval-out-of-range" }
  | { kind: "no-manifests" }
  | { kind: "scheduler-setup"; message: string }
  | { kind: "internal"; message: string };

/** 즉시 갱신 트리거 — 사용자가 "지금 갱신할게요" 누를 때. */
export async function refreshCatalogNow(): Promise<LastRefresh> {
  return invoke<LastRefresh>("refresh_catalog_now");
}

/** 마지막 갱신 결과 — 한 번도 안 됐으면 null. */
export async function getLastCatalogRefresh(): Promise<LastRefresh | null> {
  return invoke<LastRefresh | null>("get_last_catalog_refresh");
}

/** 자동 갱신 / 수동 갱신 모두 성공 시 emit. payload는 LastRefresh. */
export async function onCatalogRefreshed(
  cb: (payload: LastRefresh) => void,
): Promise<UnlistenFn> {
  return listen<LastRefresh>("catalog://refreshed", (e) => cb(e.payload));
}

// ── Phase 13'.g.2.c (ADR-0047) — Catalog minisign 서명 검증 상태 ────

/** registry_fetcher::CatalogSignatureStatus 미러. kind discriminated union. */
export type CatalogSignatureStatus =
  | { kind: "disabled"; at_ms: number }
  | { kind: "verified"; at_ms: number; source: string }
  | { kind: "failed"; at_ms: number; reason: string }
  | { kind: "missing-signature"; at_ms: number }
  | { kind: "bundled-fallback"; at_ms: number };

/** 마지막 catalog 서명 검증 결과 — 한 번도 검증 시도 안 됐으면 null. */
export async function getCatalogSignatureStatus(): Promise<CatalogSignatureStatus | null> {
  return invoke<CatalogSignatureStatus | null>("get_catalog_signature_status");
}
