// catalog IPC — get_catalog / get_recommendation.
// Rust crates/model-registry의 ModelEntry/Recommendation serde 미러.
import { invoke } from "@tauri-apps/api/core";
/** 카탈로그 entries — category가 없으면 전체. */
export async function getCatalog(category) {
    return invoke("get_catalog", { category });
}
/** 카테고리별 결정적 추천. host fingerprint 미보장 시 host-not-probed reject. */
export async function getRecommendation(category) {
    return invoke("get_recommendation", { category });
}
