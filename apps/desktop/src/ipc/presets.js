// preset IPC — get_presets / get_preset.
// Rust crates/preset-registry의 Preset / PresetCategory / VerificationTier serde 미러.
//
// Phase 4.h — 한국어 preset 카탈로그 (7 카테고리 × ~14 = 99+).
// 의료 / 법률 카테고리는 system_prompt_ko에 disclaimer 키워드 포함이 build-time 의무.
import { invoke } from "@tauri-apps/api/core";
/** 모든 preset 또는 카테고리 필터된 preset 목록. id 알파벳 순. */
export async function getPresets(category) {
    return invoke("get_presets", { category });
}
/** id로 단일 preset 조회. 없으면 null. */
export async function getPreset(id) {
    return invoke("get_preset", { id });
}
/** 카테고리 한국어 라벨 — Catalog Drawer / preset chooser에서 사용. */
export function categoryLabelKo(c) {
    switch (c) {
        case "coding":
            return "코딩";
        case "translation":
            return "번역";
        case "legal":
            return "법률";
        case "marketing":
            return "마케팅";
        case "medical":
            return "의료";
        case "education":
            return "교육";
        case "research":
            return "리서치";
    }
}
/** 카테고리 영어 라벨 — i18n fallback / 로그용. */
export function categoryLabelEn(c) {
    switch (c) {
        case "coding":
            return "Coding";
        case "translation":
            return "Translation";
        case "legal":
            return "Legal";
        case "marketing":
            return "Marketing";
        case "medical":
            return "Medical";
        case "education":
            return "Education";
        case "research":
            return "Research";
    }
}
