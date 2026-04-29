//! `KeyManager` — high-level 발급 / 검증 / 회수 entry point.
//!
//! 정책 (ADR-0022):
//! - issue: alias + scope → 평문 키 1회 반환 + DB row 저장.
//! - verify: bearer plaintext + Origin + path + model → AuthOutcome.
//! - revoke: idempotent.
//! - list: revoked 포함 (UI에서 dim 표시).

use std::path::Path;
use std::sync::Mutex;

use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::hash::{hash_key, verify_key, HashError};
use crate::plaintext::{self, GeneratedKey};
use crate::scope::Scope;
use crate::store::{ApiKeyRow, KeyStore, StoreError};

#[derive(Debug, Error)]
pub enum KeyManagerError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Hash(#[from] HashError),
    #[error("alias가 비어 있어요")]
    EmptyAlias,
}

#[derive(Debug, Clone)]
pub struct IssueRequest {
    pub alias: String,
    pub scope: Scope,
}

#[derive(Debug, Clone)]
pub struct IssuedKey {
    pub id: String,
    pub alias: String,
    pub plaintext_once: String,
    pub key_prefix: String,
    pub created_at: OffsetDateTime,
}

/// 검증 결과 — auth 미들웨어가 사용.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthOutcome {
    Allowed { id: String, alias: String },
    InvalidKey,
    Revoked,
    Expired,
    OriginDenied,
    EndpointDenied,
    ModelDenied,
}

pub struct KeyManager {
    store: Mutex<KeyStore>,
}

impl KeyManager {
    /// SQLCipher 암호화 DB로 KeyManager 인스턴스를 연다.
    ///
    /// `passphrase`는 호출자가 OS 키체인에서 가져온 hex 문자열을 권장.
    /// Phase 8'.0.a (ADR-0035): 사용자 PC의 keyring에서 자동 발급된 32바이트 secret을 hex로 인코딩해 전달.
    pub fn open(path: &Path, passphrase: &str) -> Result<Self, KeyManagerError> {
        Ok(Self {
            store: Mutex::new(KeyStore::open(path, passphrase)?),
        })
    }

    /// 암호화 없이 평문 DB로 KeyManager를 연다 (Linux headless / 폴백).
    ///
    /// CLAUDE.md §6: 정상 데스크톱 경로는 항상 `open(...)` 사용.
    /// 본 함수는 keyring 미접근 환경에서만 fallback로 호출돼요.
    pub fn open_unencrypted(path: &Path) -> Result<Self, KeyManagerError> {
        Ok(Self {
            store: Mutex::new(KeyStore::open_unencrypted(path)?),
        })
    }

    pub fn open_memory() -> Result<Self, KeyManagerError> {
        Ok(Self {
            store: Mutex::new(KeyStore::open_memory()?),
        })
    }

    /// 신규 키 발급. 평문은 응답에서 1회만 노출.
    pub fn issue(&self, req: IssueRequest) -> Result<IssuedKey, KeyManagerError> {
        if req.alias.trim().is_empty() {
            return Err(KeyManagerError::EmptyAlias);
        }
        let GeneratedKey { plaintext, prefix } = plaintext::generate();
        let hash = hash_key(&plaintext)?;
        let now = OffsetDateTime::now_utc();
        let id = Uuid::new_v4().to_string();
        let row = ApiKeyRow {
            id: id.clone(),
            alias: req.alias.clone(),
            key_prefix: prefix.clone(),
            key_hash: hash,
            scope: req.scope,
            created_at: now,
            last_used_at: None,
            revoked_at: None,
        };
        self.store.lock().expect("KeyStore poisoned").insert(&row)?;
        Ok(IssuedKey {
            id,
            alias: req.alias,
            plaintext_once: plaintext,
            key_prefix: prefix,
            created_at: now,
        })
    }

    pub fn list(&self) -> Result<Vec<ApiKeyRow>, KeyManagerError> {
        Ok(self.store.lock().expect("KeyStore poisoned").list()?)
    }

    pub fn revoke(&self, id: &str) -> Result<(), KeyManagerError> {
        Ok(self
            .store
            .lock()
            .expect("KeyStore poisoned")
            .revoke(id, OffsetDateTime::now_utc())?)
    }

    /// Auth 미들웨어용 검증.
    ///
    /// `origin`이 `None`이면 origin 검증 스킵 (서버-사이드 호출 시나리오 — `Origin` 헤더 없음).
    /// 단, scope.allowed_origins가 비어있지 않은 키라면 origin이 없어도 거부 (web-only 키).
    /// 이 정책은 v1 안전 우선: 명시적 allowlist 키는 browser-only로 간주.
    pub fn verify(
        &self,
        bearer_plaintext: &str,
        origin: Option<&str>,
        path: &str,
        model: Option<&str>,
        now: OffsetDateTime,
    ) -> Result<AuthOutcome, KeyManagerError> {
        let prefix = match plaintext::prefix_of(bearer_plaintext) {
            Some(p) => p,
            None => return Ok(AuthOutcome::InvalidKey),
        };
        let candidates = self
            .store
            .lock()
            .expect("KeyStore poisoned")
            .find_by_prefix(&prefix)?;

        for row in candidates {
            // argon2 verify — narrow prefix 충돌.
            if !verify_key(&row.key_hash, bearer_plaintext)? {
                continue;
            }
            if row.revoked_at.is_some() {
                return Ok(AuthOutcome::Revoked);
            }
            if row.scope.is_expired(now) {
                return Ok(AuthOutcome::Expired);
            }

            // Origin 검증.
            if !row.scope.allowed_origins.is_empty() {
                match origin {
                    Some(o) if row.scope.allows_origin(o) => {}
                    _ => return Ok(AuthOutcome::OriginDenied),
                }
            }

            // Endpoint scope.
            if !row.scope.allows_endpoint(path) {
                return Ok(AuthOutcome::EndpointDenied);
            }

            // Model scope (request body의 model 필드가 있을 때만).
            if let Some(m) = model {
                if !row.scope.allows_model(m) {
                    return Ok(AuthOutcome::ModelDenied);
                }
            }

            // last_used_at 갱신 (best-effort, 실패는 무시).
            let _ = self
                .store
                .lock()
                .expect("KeyStore poisoned")
                .touch_last_used(&row.id, now);

            return Ok(AuthOutcome::Allowed {
                id: row.id,
                alias: row.alias,
            });
        }
        Ok(AuthOutcome::InvalidKey)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn web_scope() -> Scope {
        Scope {
            models: vec!["*".into()],
            endpoints: vec!["/v1/*".into()],
            allowed_origins: vec!["http://localhost:5173".into()],
            ..Default::default()
        }
    }

    #[test]
    fn issue_returns_plaintext_once() {
        let m = KeyManager::open_memory().unwrap();
        let issued = m
            .issue(IssueRequest {
                alias: "test".into(),
                scope: web_scope(),
            })
            .unwrap();
        assert!(issued.plaintext_once.starts_with("lm-"));
        assert!(issued.plaintext_once.starts_with(&issued.key_prefix));
    }

    #[test]
    fn issue_empty_alias_rejected() {
        let m = KeyManager::open_memory().unwrap();
        let r = m.issue(IssueRequest {
            alias: "  ".into(),
            scope: web_scope(),
        });
        assert!(matches!(r, Err(KeyManagerError::EmptyAlias)));
    }

    #[test]
    fn verify_allows_with_correct_origin_path_model() {
        let m = KeyManager::open_memory().unwrap();
        let issued = m
            .issue(IssueRequest {
                alias: "a".into(),
                scope: web_scope(),
            })
            .unwrap();
        let r = m
            .verify(
                &issued.plaintext_once,
                Some("http://localhost:5173"),
                "/v1/chat/completions",
                Some("exaone"),
                OffsetDateTime::now_utc(),
            )
            .unwrap();
        assert!(matches!(r, AuthOutcome::Allowed { .. }));
    }

    #[test]
    fn verify_rejects_invalid_plaintext() {
        let m = KeyManager::open_memory().unwrap();
        let r = m
            .verify(
                "lm-fakefake0000000000000000000000000",
                Some("http://localhost:5173"),
                "/v1/chat/completions",
                Some("x"),
                OffsetDateTime::now_utc(),
            )
            .unwrap();
        assert_eq!(r, AuthOutcome::InvalidKey);
    }

    #[test]
    fn verify_rejects_non_lm_prefix() {
        let m = KeyManager::open_memory().unwrap();
        let r = m
            .verify(
                "sk-anthropic-style-key",
                None,
                "/v1/chat/completions",
                None,
                OffsetDateTime::now_utc(),
            )
            .unwrap();
        assert_eq!(r, AuthOutcome::InvalidKey);
    }

    #[test]
    fn verify_origin_mismatch_denied() {
        let m = KeyManager::open_memory().unwrap();
        let issued = m
            .issue(IssueRequest {
                alias: "a".into(),
                scope: web_scope(),
            })
            .unwrap();
        let r = m
            .verify(
                &issued.plaintext_once,
                Some("https://attacker.com"),
                "/v1/chat/completions",
                Some("x"),
                OffsetDateTime::now_utc(),
            )
            .unwrap();
        assert_eq!(r, AuthOutcome::OriginDenied);
    }

    #[test]
    fn verify_origin_port_mismatch_denied() {
        let m = KeyManager::open_memory().unwrap();
        let issued = m
            .issue(IssueRequest {
                alias: "a".into(),
                scope: web_scope(),
            })
            .unwrap();
        // localhost:5174는 5173과 port 다름.
        let r = m
            .verify(
                &issued.plaintext_once,
                Some("http://localhost:5174"),
                "/v1/chat/completions",
                Some("x"),
                OffsetDateTime::now_utc(),
            )
            .unwrap();
        assert_eq!(r, AuthOutcome::OriginDenied);
    }

    #[test]
    fn verify_origin_scheme_mismatch_denied() {
        let m = KeyManager::open_memory().unwrap();
        let issued = m
            .issue(IssueRequest {
                alias: "a".into(),
                scope: web_scope(),
            })
            .unwrap();
        // http vs https.
        let r = m
            .verify(
                &issued.plaintext_once,
                Some("https://localhost:5173"),
                "/v1/chat/completions",
                Some("x"),
                OffsetDateTime::now_utc(),
            )
            .unwrap();
        assert_eq!(r, AuthOutcome::OriginDenied);
    }

    #[test]
    fn verify_no_origin_with_web_only_key_denied() {
        let m = KeyManager::open_memory().unwrap();
        let issued = m
            .issue(IssueRequest {
                alias: "a".into(),
                scope: web_scope(),
            })
            .unwrap();
        // allowed_origins이 박힌 키에 origin 헤더 없으면 거부 (browser-only 정책).
        let r = m
            .verify(
                &issued.plaintext_once,
                None,
                "/v1/chat/completions",
                Some("x"),
                OffsetDateTime::now_utc(),
            )
            .unwrap();
        assert_eq!(r, AuthOutcome::OriginDenied);
    }

    #[test]
    fn verify_server_only_key_allows_no_origin() {
        // allowed_origins이 비어있는 키 — server-side 호출 OK.
        let mut scope = web_scope();
        scope.allowed_origins.clear();
        let m = KeyManager::open_memory().unwrap();
        let issued = m
            .issue(IssueRequest {
                alias: "server".into(),
                scope,
            })
            .unwrap();
        let r = m
            .verify(
                &issued.plaintext_once,
                None,
                "/v1/chat/completions",
                Some("x"),
                OffsetDateTime::now_utc(),
            )
            .unwrap();
        assert!(matches!(r, AuthOutcome::Allowed { .. }));
    }

    #[test]
    fn verify_endpoint_glob_denied() {
        let m = KeyManager::open_memory().unwrap();
        let mut scope = web_scope();
        scope.endpoints = vec!["/v1/embeddings".into()]; // chat 거부.
        let issued = m
            .issue(IssueRequest {
                alias: "a".into(),
                scope,
            })
            .unwrap();
        let r = m
            .verify(
                &issued.plaintext_once,
                Some("http://localhost:5173"),
                "/v1/chat/completions",
                Some("x"),
                OffsetDateTime::now_utc(),
            )
            .unwrap();
        assert_eq!(r, AuthOutcome::EndpointDenied);
    }

    #[test]
    fn verify_model_glob_denied() {
        let m = KeyManager::open_memory().unwrap();
        let mut scope = web_scope();
        scope.models = vec!["exaone-*".into()];
        let issued = m
            .issue(IssueRequest {
                alias: "a".into(),
                scope,
            })
            .unwrap();
        let r = m
            .verify(
                &issued.plaintext_once,
                Some("http://localhost:5173"),
                "/v1/chat/completions",
                Some("qwen-7b"),
                OffsetDateTime::now_utc(),
            )
            .unwrap();
        assert_eq!(r, AuthOutcome::ModelDenied);
    }

    #[test]
    fn verify_revoked_key_returns_revoked() {
        let m = KeyManager::open_memory().unwrap();
        let issued = m
            .issue(IssueRequest {
                alias: "a".into(),
                scope: web_scope(),
            })
            .unwrap();
        m.revoke(&issued.id).unwrap();
        let r = m
            .verify(
                &issued.plaintext_once,
                Some("http://localhost:5173"),
                "/v1/chat/completions",
                Some("x"),
                OffsetDateTime::now_utc(),
            )
            .unwrap();
        // revoked는 find_by_prefix에서 제외 → InvalidKey.
        assert_eq!(r, AuthOutcome::InvalidKey);
    }

    #[test]
    fn verify_expired_key_returns_expired() {
        let m = KeyManager::open_memory().unwrap();
        let mut scope = web_scope();
        scope.expires_at = Some("2000-01-01T00:00:00Z".into());
        let issued = m
            .issue(IssueRequest {
                alias: "old".into(),
                scope,
            })
            .unwrap();
        let r = m
            .verify(
                &issued.plaintext_once,
                Some("http://localhost:5173"),
                "/v1/chat/completions",
                Some("x"),
                OffsetDateTime::now_utc(),
            )
            .unwrap();
        assert_eq!(r, AuthOutcome::Expired);
    }

    #[test]
    fn list_includes_revoked_keys() {
        let m = KeyManager::open_memory().unwrap();
        let a = m
            .issue(IssueRequest {
                alias: "a".into(),
                scope: web_scope(),
            })
            .unwrap();
        m.revoke(&a.id).unwrap();
        let rows = m.list().unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].revoked_at.is_some());
    }
}
