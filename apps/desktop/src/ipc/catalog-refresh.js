// catalog-refresh IPC — Phase 1' integration.
// Rust src-tauri/src/registry_fetcher.rs의 LastRefresh / CatalogRefreshError serde 미러.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
/** 즉시 갱신 트리거 — 사용자가 "지금 갱신할게요" 누를 때. */
export async function refreshCatalogNow() {
    return invoke("refresh_catalog_now");
}
/** 마지막 갱신 결과 — 한 번도 안 됐으면 null. */
export async function getLastCatalogRefresh() {
    return invoke("get_last_catalog_refresh");
}
/** 자동 갱신 / 수동 갱신 모두 성공 시 emit. payload는 LastRefresh. */
export async function onCatalogRefreshed(cb) {
    return listen("catalog://refreshed", (e) => cb(e.payload));
}
