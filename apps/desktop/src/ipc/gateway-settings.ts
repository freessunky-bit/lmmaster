// Phase 8'.c.4 (ADR-0066) — 게이트웨이 사내망 노출 settings IPC helper.

import { invoke } from "@tauri-apps/api/core";

/** 현재 사내망 노출 여부 — settings.json + env fallback. */
export async function getGatewayAllowExternal(): Promise<boolean> {
  return invoke<boolean>("get_gateway_allow_external");
}

/**
 * 사내망 노출 토글 — settings.json save + env 즉시 갱신.
 *
 * 변경 후 게이트웨이 재시작이 필요해요. 호출 측에서 사용자에게 "재시작 후 적용" 안내 + 앱 재시작 권유.
 */
export async function setGatewayAllowExternal(allow: boolean): Promise<void> {
  await invoke<void>("set_gateway_allow_external", { allow });
}

/**
 * LAN IP 후보 목록. RFC 1918 private 범위만 (10/8, 172.16-31, 192.168/16).
 * 빈 배열 = 사내망 IP 감지 실패 (VPN 단독 / 가상 어댑터만 있는 환경).
 */
export async function listLanAddresses(): Promise<string[]> {
  return invoke<string[]>("list_lan_addresses");
}
