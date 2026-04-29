// settings.ts — localStorage 기반 클라이언트 settings.
//
// 정책 (Phase 4.g):
// - v1은 클라 측 localStorage. v1.1에서 Tauri store / portable workspace로 이동.
// - 키: "lmmaster.settings.{section}.{key}".
// - jsdom env 외에 — 데스크톱 빌드에서도 storage 접근이 차단된 모드일 수 있어 try/catch.
const PREFIX = "lmmaster.settings";
const k = (section, key) => `${PREFIX}.${section}.${key}`;
const DEFAULT_SCAN_INTERVAL = 60;
function safeRead(key) {
    try {
        return globalThis.localStorage?.getItem(key) ?? null;
    }
    catch {
        return null;
    }
}
function safeWrite(key, value) {
    try {
        globalThis.localStorage?.setItem(key, value);
    }
    catch (e) {
        console.warn("settings write failed:", key, e);
    }
}
/** 자가 점검 주기 (분). 0=끔. 잘못된 값은 default로 떨어짐. */
export function getScanInterval() {
    const v = safeRead(k("general", "scan_interval_min"));
    if (v === null)
        return DEFAULT_SCAN_INTERVAL;
    const num = Number(v);
    if (num === 0 || num === 15 || num === 60)
        return num;
    return DEFAULT_SCAN_INTERVAL;
}
export function setScanInterval(min) {
    safeWrite(k("general", "scan_interval_min"), String(min));
}
/** Phase 5' 출시 알림 받기 — Workbench와 동일 키 영역. */
export function getNotifyOnPhase5() {
    return safeRead(k("general", "notify_phase5")) === "true";
}
export function setNotifyOnPhase5(v) {
    safeWrite(k("general", "notify_phase5"), String(v));
}
/**
 * env LMMASTER_ENCRYPT_DB는 Rust side에서만 — JS는 read-only hint.
 * 실제 값은 빌드 시점에 Rust에서 전달돼야 정확. v1은 placeholder.
 */
export function getEncryptDbHint() {
    return false;
}
