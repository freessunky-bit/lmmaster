// settings.ts — localStorage 기반 클라이언트 settings.
//
// 정책 (Phase 4.g):
// - v1은 클라 측 localStorage. v1.1에서 Tauri store / portable workspace로 이동.
// - 키: "lmmaster.settings.{section}.{key}".
// - jsdom env 외에 — 데스크톱 빌드에서도 storage 접근이 차단된 모드일 수 있어 try/catch.

const PREFIX = "lmmaster.settings";
const k = (section: string, key: string): string =>
  `${PREFIX}.${section}.${key}`;

/** 자가 점검 주기 — 0=끔, 15분, 1시간 단위. */
export type ScanIntervalValue = 0 | 15 | 60;

const DEFAULT_SCAN_INTERVAL: ScanIntervalValue = 60;

function safeRead(key: string): string | null {
  try {
    return globalThis.localStorage?.getItem(key) ?? null;
  } catch {
    return null;
  }
}

function safeWrite(key: string, value: string): void {
  try {
    globalThis.localStorage?.setItem(key, value);
  } catch (e) {
    console.warn("settings write failed:", key, e);
  }
}

/** 자가 점검 주기 (분). 0=끔. 잘못된 값은 default로 떨어짐. */
export function getScanInterval(): ScanIntervalValue {
  const v = safeRead(k("general", "scan_interval_min"));
  if (v === null) return DEFAULT_SCAN_INTERVAL;
  const num = Number(v);
  if (num === 0 || num === 15 || num === 60) return num as ScanIntervalValue;
  return DEFAULT_SCAN_INTERVAL;
}

export function setScanInterval(min: ScanIntervalValue): void {
  safeWrite(k("general", "scan_interval_min"), String(min));
}

/** Phase 5' 출시 알림 받기 — Workbench와 동일 키 영역. */
export function getNotifyOnPhase5(): boolean {
  return safeRead(k("general", "notify_phase5")) === "true";
}

export function setNotifyOnPhase5(v: boolean): void {
  safeWrite(k("general", "notify_phase5"), String(v));
}

/**
 * env LMMASTER_ENCRYPT_DB는 Rust side에서만 — JS는 read-only hint.
 * 실제 값은 빌드 시점에 Rust에서 전달돼야 정확. v1은 placeholder.
 */
export function getEncryptDbHint(): boolean {
  return false;
}

/**
 * Phase 7'.b — 자동 갱신 채널 토글 (stable / beta).
 *
 * 정책 (phase-7p-release-prep-reinforcement.md §5.1, ADR-0027 §5):
 * - 기본 stable. 사용자가 명시적으로 "베타 참여"를 켜면 beta.
 * - tauri-plugin-updater는 endpoint 동적 변경 미지원 — frontend가 conditional 사용.
 * - beta 사용자는 정식 release 나오면 안내(별도 toast — v1.x).
 */
export type UpdateChannel = "stable" | "beta";
const DEFAULT_UPDATE_CHANNEL: UpdateChannel = "stable";

export function getUpdateChannel(): UpdateChannel {
  const v = safeRead(k("update", "channel"));
  return v === "beta" ? "beta" : DEFAULT_UPDATE_CHANNEL;
}

export function setUpdateChannel(channel: UpdateChannel): void {
  safeWrite(k("update", "channel"), channel);
}
