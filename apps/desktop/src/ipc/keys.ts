// keys IPC — create / list / revoke API keys.
// Rust crates/key-manager의 ApiKey/Scope serde 미러.

import { invoke } from "@tauri-apps/api/core";

export interface RateLimit {
  per_minute?: number | null;
  per_day?: number | null;
}

export interface ApiKeyScope {
  models: string[];
  endpoints: string[];
  allowed_origins: string[];
  expires_at?: string | null;
  project_id?: string | null;
  rate_limit?: RateLimit | null;
}

export interface ApiKeyView {
  id: string;
  alias: string;
  key_prefix: string;
  scope: ApiKeyScope;
  /** RFC3339 */
  created_at: string;
  last_used_at?: string | null;
  revoked_at?: string | null;
}

export interface CreatedKey {
  id: string;
  alias: string;
  key_prefix: string;
  /** 1회만 노출 — 사용자에게 카피 후 폐기 책임. */
  plaintext_once: string;
  created_at: string;
}

export type KeyApiError =
  | { kind: "empty-alias" }
  | { kind: "store"; message: string }
  | { kind: "internal"; message: string };

/** 신규 API 키 발급. plaintext_once는 1회만 노출. */
export async function createApiKey(args: {
  alias: string;
  scope: ApiKeyScope;
}): Promise<CreatedKey> {
  return invoke<CreatedKey>("create_api_key", {
    req: { alias: args.alias, scope: args.scope },
  });
}

/** 모든 키 목록 (revoked 포함). */
export async function listApiKeys(): Promise<ApiKeyView[]> {
  return invoke<ApiKeyView[]>("list_api_keys");
}

/** 키 회수 — idempotent. */
export async function revokeApiKey(id: string): Promise<void> {
  return invoke<void>("revoke_api_key", { id });
}

/** 기본 scope helper — "/v1/*" + 단일 origin. */
export function defaultWebScope(origin: string): ApiKeyScope {
  return {
    models: ["*"],
    endpoints: ["/v1/*"],
    allowed_origins: [origin],
    expires_at: null,
    project_id: null,
    rate_limit: null,
  };
}
