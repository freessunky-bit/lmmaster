import { LMmasterClient } from "./client";
import type { ApiKeyScope, IssuedKey } from "./types";

export async function issueApiKey(
  client: LMmasterClient,
  alias: string,
  scope: ApiKeyScope,
  options: { project_id?: string; expires_at?: string } = {}
): Promise<IssuedKey> {
  const url = client.baseUrl.replace(/\/v1\/?$/, "") + "/_admin/keys/issue";
  const res = await client.fetchImpl(url, {
    method: "POST",
    headers: { "Content-Type": "application/json", ...client.authHeaders() },
    body: JSON.stringify({ alias, scope, ...options }),
  });
  if (!res.ok) throw new Error(`admin ${res.status}`);
  return res.json() as Promise<IssuedKey>;
}

export async function listApiKeys(client: LMmasterClient): Promise<unknown> {
  const url = client.baseUrl.replace(/\/v1\/?$/, "") + "/_admin/keys";
  const res = await client.fetchImpl(url, { headers: client.authHeaders() });
  if (!res.ok) throw new Error(`admin ${res.status}`);
  return res.json();
}
