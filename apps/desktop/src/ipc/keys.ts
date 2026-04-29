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
  /**
   * Phase 8'.c.3 (ADR-0029) — 이 키에만 적용할 Pipeline 화이트리스트.
   * `null`/undefined = 전역 설정 따름. `[]` = 모든 Pipeline 비활성. 명시 vec = 그 ID만 활성.
   */
  enabled_pipelines?: string[] | null;
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

/**
 * Phase 8'.c.3 (ADR-0029) — 키별 Pipeline 화이트리스트만 부분 업데이트.
 * `enabled_pipelines = null` = 전역 토글 따름. `[]` = 모두 비활성. 명시 vec = 그 ID만 활성.
 */
export async function updateApiKeyPipelines(args: {
  id: string;
  enabled_pipelines: string[] | null;
}): Promise<void> {
  return invoke<void>("update_api_key_pipelines", {
    req: { id: args.id, enabled_pipelines: args.enabled_pipelines },
  });
}

/** Phase 8'.c.1 — UI에 노출되는 v1 시드 4종 ID. backend SEED_PIPELINE_IDS와 1:1. */
export const SEED_PIPELINE_IDS = [
  "pii-redact",
  "token-quota",
  "observability",
  "prompt-sanitize",
] as const;
export type SeedPipelineId = (typeof SEED_PIPELINE_IDS)[number];

/** 기본 scope helper — "/v1/*" + 단일 origin. */
export function defaultWebScope(origin: string): ApiKeyScope {
  return {
    models: ["*"],
    endpoints: ["/v1/*"],
    allowed_origins: [origin],
    expires_at: null,
    project_id: null,
    rate_limit: null,
    enabled_pipelines: null,
  };
}
