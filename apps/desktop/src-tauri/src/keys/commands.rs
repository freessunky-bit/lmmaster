//! Tauri commands — create/list/revoke API keys.
//!
//! 정책 (ADR-0022 §10):
//! - create: 평문은 응답에서 1회만 표시 후 폐기.
//! - list: 키 목록 (revoked 포함, prefix만 노출).
//! - revoke: idempotent.

use std::sync::Arc;

use key_manager::{IssueRequest, KeyManager, Scope};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::format_description::well_known::Rfc3339;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum KeyApiError {
    #[error("alias가 비어 있어요")]
    EmptyAlias,
    #[error("키 저장소 오류: {message}")]
    Store { message: String },
    #[error("내부 오류: {message}")]
    Internal { message: String },
    #[error(
        "scope의 endpoints / models 둘 다 비어 있으면 모든 호출이 차단돼요. 최소 1개는 채워주세요."
    )]
    EmptyScope,
}

impl From<key_manager::KeyManagerError> for KeyApiError {
    fn from(e: key_manager::KeyManagerError) -> Self {
        match e {
            key_manager::KeyManagerError::EmptyAlias => Self::EmptyAlias,
            key_manager::KeyManagerError::Store(s) => Self::Store {
                message: s.to_string(),
            },
            key_manager::KeyManagerError::Hash(h) => Self::Internal {
                message: h.to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateKeyRequest {
    pub alias: String,
    pub scope: Scope,
}

/// Phase 8'.c.3 (ADR-0029) — frontend가 ApiKeyEditPanel에서 보내는 부분 업데이트.
///
/// 정책: scope 전체를 갈아치우는 대신 *Pipelines 화이트리스트*만 갱신해요. 다른 필드(models /
/// origins / endpoints 등)는 v1.x에서 별도 update API로 추가 (현재는 회수 후 재발급).
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateKeyPipelinesRequest {
    pub id: String,
    /// `None` = 전역 토글 따름. `Some(Vec)` = 명시 화이트리스트 (빈 vec은 모두 비활성).
    pub enabled_pipelines: Option<Vec<String>>,
}

/// Phase 13'.c — scope 전체 교체 요청.
///
/// 정책:
/// - models / endpoints가 둘 다 비어있는 scope는 거부 (호출이 모두 차단되는 무용 키 방지).
/// - 평문 재발급 없이 필터만 갱신.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateKeyScopeRequest {
    pub id: String,
    pub scope: Scope,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreatedKey {
    pub id: String,
    pub alias: String,
    pub key_prefix: String,
    /// 평문 — 1회만 응답.
    pub plaintext_once: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyView {
    pub id: String,
    pub alias: String,
    pub key_prefix: String,
    pub scope: Scope,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

/// 신규 키 발급 — 평문 1회 reveal.
#[tauri::command]
pub fn create_api_key(
    km: tauri::State<'_, Arc<KeyManager>>,
    req: CreateKeyRequest,
) -> Result<CreatedKey, KeyApiError> {
    let issued = km.issue(IssueRequest {
        alias: req.alias.trim().to_string(),
        scope: req.scope,
    })?;
    Ok(CreatedKey {
        id: issued.id,
        alias: issued.alias,
        key_prefix: issued.key_prefix,
        plaintext_once: issued.plaintext_once,
        created_at: issued
            .created_at
            .format(&Rfc3339)
            .unwrap_or_else(|_| String::new()),
    })
}

/// 모든 키 목록 (revoked 포함). 평문 미포함.
#[tauri::command]
pub fn list_api_keys(
    km: tauri::State<'_, Arc<KeyManager>>,
) -> Result<Vec<ApiKeyView>, KeyApiError> {
    let rows = km.list()?;
    Ok(rows
        .into_iter()
        .map(|r| ApiKeyView {
            id: r.id,
            alias: r.alias,
            key_prefix: r.key_prefix,
            scope: r.scope,
            created_at: r
                .created_at
                .format(&Rfc3339)
                .unwrap_or_else(|_| String::new()),
            last_used_at: r.last_used_at.and_then(|t| t.format(&Rfc3339).ok()),
            revoked_at: r.revoked_at.and_then(|t| t.format(&Rfc3339).ok()),
        })
        .collect())
}

/// 키 회수 — idempotent.
#[tauri::command]
pub fn revoke_api_key(
    km: tauri::State<'_, Arc<KeyManager>>,
    id: String,
) -> Result<(), KeyApiError> {
    km.revoke(&id)?;
    Ok(())
}

/// Phase 8'.c.3 (ADR-0029) — 키별 Pipeline 화이트리스트 부분 업데이트.
///
/// 정책:
/// - `enabled_pipelines = None` → 전역 토글 따름.
/// - `enabled_pipelines = Some(Vec)` → 명시 화이트리스트 (빈 vec은 모두 비활성).
/// - 다른 scope 필드는 보존 — 회수 / 재발급 없이 안전한 부분 갱신.
#[tauri::command]
pub fn update_api_key_pipelines(
    km: tauri::State<'_, Arc<KeyManager>>,
    req: UpdateKeyPipelinesRequest,
) -> Result<(), KeyApiError> {
    km.update_enabled_pipelines(&req.id, req.enabled_pipelines)?;
    Ok(())
}

/// Phase 13'.c — scope 전체 교체.
///
/// 정책:
/// - 평문 재발급 없이 models / endpoints / allowed_origins / expires_at / enabled_pipelines 갱신.
/// - 빈 scope (models + endpoints 둘 다 빔)는 `EmptyScope`로 거부 — 무용 키 방지.
/// - revoked 키도 편집 허용 (재활성은 별도 정책 — v1은 unsupported).
#[tauri::command]
pub fn update_api_key_scope(
    km: tauri::State<'_, Arc<KeyManager>>,
    req: UpdateKeyScopeRequest,
) -> Result<(), KeyApiError> {
    if req.scope.models.is_empty() && req.scope.endpoints.is_empty() {
        return Err(KeyApiError::EmptyScope);
    }
    km.update_scope(&req.id, req.scope)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_alias_kind_serializes_kebab() {
        let v = serde_json::to_value(KeyApiError::EmptyAlias).unwrap();
        assert_eq!(v["kind"], "empty-alias");
    }

    #[test]
    fn store_kind_serializes_kebab() {
        let v = serde_json::to_value(KeyApiError::Store {
            message: "x".into(),
        })
        .unwrap();
        assert_eq!(v["kind"], "store");
    }

    #[test]
    fn empty_scope_kind_serializes_kebab() {
        let v = serde_json::to_value(KeyApiError::EmptyScope).unwrap();
        assert_eq!(v["kind"], "empty-scope");
    }
}
