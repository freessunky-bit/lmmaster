// models.ts — OpenAI 호환 GET /v1/models, GET /v1/models/:id.

import type { LMmasterClient } from "./client";

export interface ModelObject {
  id: string;
  object: "model";
  owned_by: string;
  created: number;
}

export interface ModelsList {
  object: "list";
  data: ModelObject[];
}

export async function listModels(client: LMmasterClient): Promise<ModelsList> {
  const res = await client.fetchImpl(`${client.baseUrl}/models`, {
    method: "GET",
    headers: { ...client.authHeaders() },
  });
  await client.ensureOk(res);
  return (await res.json()) as ModelsList;
}

export async function retrieveModel(
  client: LMmasterClient,
  id: string,
): Promise<ModelObject> {
  const res = await client.fetchImpl(`${client.baseUrl}/models/${encodeURIComponent(id)}`, {
    method: "GET",
    headers: { ...client.authHeaders() },
  });
  await client.ensureOk(res);
  return (await res.json()) as ModelObject;
}
