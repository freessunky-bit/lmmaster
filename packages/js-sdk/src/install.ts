// install/progress 관련 SDK — 관리 endpoint(/_admin/*)를 호출.
// admin scope 키 또는 GUI IPC만 호출 가능. 외부 웹앱은 일반 chat만 사용.

import { LMmasterClient } from "./client";
import type { InstallProgress } from "./types";

export async function getInstalledRuntimes(client: LMmasterClient): Promise<unknown> {
  return adminGet(client, "/runtimes");
}

export async function getInstalledModels(client: LMmasterClient): Promise<unknown> {
  return adminGet(client, "/models/installed");
}

export async function getRecommendedModels(client: LMmasterClient): Promise<unknown> {
  return adminGet(client, "/models/recommended");
}

export async function installRuntime(client: LMmasterClient, kind: string): Promise<unknown> {
  return adminPost(client, "/runtimes/install", { kind });
}

export async function installModel(client: LMmasterClient, model_id: string, quant?: string): Promise<unknown> {
  return adminPost(client, "/models/install", { model_id, quant });
}

export async function getInstallProgress(client: LMmasterClient, job_id: string): Promise<InstallProgress> {
  return adminGet(client, `/installs/${job_id}`) as Promise<InstallProgress>;
}

export async function getGatewayStatus(client: LMmasterClient): Promise<unknown> {
  return adminGet(client, "/status");
}

async function adminGet(client: LMmasterClient, path: string): Promise<unknown> {
  const url = client.baseUrl.replace(/\/v1\/?$/, "") + "/_admin" + path;
  const res = await client.fetchImpl(url, { headers: client.authHeaders() });
  if (!res.ok) throw new Error(`admin ${res.status}`);
  return res.json();
}
async function adminPost(client: LMmasterClient, path: string, body: unknown): Promise<unknown> {
  const url = client.baseUrl.replace(/\/v1\/?$/, "") + "/_admin" + path;
  const res = await client.fetchImpl(url, {
    method: "POST",
    headers: { "Content-Type": "application/json", ...client.authHeaders() },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`admin ${res.status}`);
  return res.json();
}
