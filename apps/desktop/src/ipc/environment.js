// detect_environment Tauri command 래퍼 + 결과 타입 미러.
// 정책 (Phase 1A.4.b 보강 §2):
// - hardware-probe::HardwareReport + runtime-detector::DetectResult를 단일 invoke로 묶음.
// - 한국어 마법사가 직접 소비하는 형태 — 모든 enum은 kebab-case 일치.
import { invoke } from "@tauri-apps/api/core";
/**
 * hardware-probe + runtime-detector 통합 호출. Promise reject 시 EnvApiError로 캐치 가능.
 * 일반적으로 1.0~2.5s 소요 (cold).
 */
export async function detectEnvironment() {
    return invoke("detect_environment");
}
// ── 작은 표현 헬퍼 ────────────────────────────────────────────────────
const GIB = 1024 * 1024 * 1024;
export function formatGiB(bytes, fractionDigits = 1) {
    return `${(bytes / GIB).toFixed(fractionDigits)}GB`;
}
/** OS family를 한국어 친화 라벨로. */
export function osFamilyLabel(family) {
    switch (family) {
        case "windows":
            return "Windows";
        case "macos":
            return "macOS";
        case "linux":
            return "Linux";
        default:
            return "기타";
    }
}
export function runtimeKindLabel(kind) {
    switch (kind) {
        case "lm-studio":
            return "LM Studio";
        case "ollama":
            return "Ollama";
        case "llama-cpp":
            return "llama.cpp";
        case "kobold-cpp":
            return "KoboldCpp";
        case "vllm":
            return "vLLM";
    }
}
