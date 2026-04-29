import { LMmasterClient } from "./client";

export async function bindProject(
  client: LMmasterClient,
  name: string,
  host: string
): Promise<unknown> {
  const url = client.baseUrl.replace(/\/v1\/?$/, "") + "/_admin/projects/bind";
  const res = await client.fetchImpl(url, {
    method: "POST",
    headers: { "Content-Type": "application/json", ...client.authHeaders() },
    body: JSON.stringify({ name, host }),
  });
  if (!res.ok) throw new Error(`admin ${res.status}`);
  return res.json();
}

export async function getProjectBindings(client: LMmasterClient): Promise<unknown> {
  const url = client.baseUrl.replace(/\/v1\/?$/, "") + "/_admin/projects";
  const res = await client.fetchImpl(url, { headers: client.authHeaders() });
  if (!res.ok) throw new Error(`admin ${res.status}`);
  return res.json();
}
