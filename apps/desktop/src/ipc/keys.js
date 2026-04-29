// keys IPC — create / list / revoke API keys.
// Rust crates/key-manager의 ApiKey/Scope serde 미러.
import { invoke } from "@tauri-apps/api/core";
/** 신규 API 키 발급. plaintext_once는 1회만 노출. */
export async function createApiKey(args) {
    return invoke("create_api_key", {
        req: { alias: args.alias, scope: args.scope },
    });
}
/** 모든 키 목록 (revoked 포함). */
export async function listApiKeys() {
    return invoke("list_api_keys");
}
/** 키 회수 — idempotent. */
export async function revokeApiKey(id) {
    return invoke("revoke_api_key", { id });
}
/** 기본 scope helper — "/v1/*" + 단일 origin. */
export function defaultWebScope(origin) {
    return {
        models: ["*"],
        endpoints: ["/v1/*"],
        allowed_origins: [origin],
        expires_at: null,
        project_id: null,
        rate_limit: null,
    };
}
