// discovery.ts — gateway 자동 탐색 + ping health + launch URL.
//
// 정책 (ADR-0022):
// - gateway는 127.0.0.1:0 (OS 할당 포트)에 바인딩.
// - 사용자가 baseUrl 모를 수 있어 후보 포트 ping 또는 custom scheme 실행 유도.

import type { LMmasterClient } from "./client";

export interface HealthResponse {
  status: string;
  version: string;
}

/** gateway가 살아있는지 확인. 실패는 null 반환 (throw 안 함 — 사용자 분기 단순화). */
export async function pingHealth(
  client: LMmasterClient,
): Promise<HealthResponse | null> {
  try {
    const url = client.baseUrl.replace(/\/v1\/?$/, "") + "/health";
    const res = await client.fetchImpl(url, { method: "GET" });
    if (!res.ok) return null;
    return (await res.json()) as HealthResponse;
  } catch {
    return null;
  }
}

/** 미설치/미실행 시 custom URL scheme으로 데스크톱 앱 실행 유도. */
export function buildLaunchUrl(returnTo?: string): string {
  const u = new URL("lmmaster://launch");
  if (returnTo) u.searchParams.set("return_to", returnTo);
  return u.toString();
}

/** 후보 포트 순회로 살아있는 gateway baseUrl(+/v1) 자동 탐색. 없으면 null. */
export async function autoFindGateway(
  client: LMmasterClient,
  candidatePorts: number[] = [43117, 43118, 43119],
  timeoutMs = 500,
): Promise<string | null> {
  for (const p of candidatePorts) {
    const root = `http://127.0.0.1:${p}`;
    try {
      const ctl = new AbortController();
      const timer = setTimeout(() => ctl.abort(), timeoutMs);
      const res = await client.fetchImpl(`${root}/health`, {
        method: "GET",
        signal: ctl.signal,
      });
      clearTimeout(timer);
      if (res.ok) return `${root}/v1`;
    } catch {
      // 다음 후보 포트.
    }
  }
  return null;
}
