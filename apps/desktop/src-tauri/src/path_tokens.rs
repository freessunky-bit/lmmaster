//! Phase R-F.3 (ADR-0052 §S6 + ADR-0064 §F.3) — selected_path_token registry.
//!
//! 정책:
//! - GPT Pro 검수(2026-05-07) critical로 분류된 IPC raw filesystem path 표면을 token으로 교체.
//! - 사용자 명시 dialog 선택 path만 백엔드 registry에 token으로 등록.
//! - IPC는 token만 받음 → backend가 map lookup으로 PathBuf 복원.
//! - 24h soft TTL + UUID v4 36-char + Tokio RwLock (read 우세).
//! - Frontend localStorage 캐시 금지 — process 재시작 시 dangling pointer 방지.
//!
//! 보안:
//! - canonicalize() 후 저장 — symlink/`..` escape 차단.
//! - dialog cancel(`null`) → token 미발급 + frontend graceful 유지.
//! - Tauri Mobile은 desktop-only ADR이라 본 sub-phase 범위 외 (v2.x).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::State;
use tokio::sync::RwLock;
use uuid::Uuid;

/// 24h soft TTL — 진행 중 ingest는 kickoff에 1회 resolve → in-memory PathBuf로 보유라 영향 없음.
pub const TOKEN_TTL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PathTokenKind {
    File,
    Directory,
}

#[derive(Debug, Clone)]
pub struct TokenEntry {
    pub canonical_path: PathBuf,
    pub kind: PathTokenKind,
    pub issued_at: Instant,
}

#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PathTokenError {
    #[error("선택한 파일을 찾을 수 없어요. 다시 선택해 주세요.")]
    Unknown,
    #[error("파일 선택이 만료됐어요. 다시 선택해 주세요.")]
    Expired,
    #[error("선택한 경로가 허용 범위를 벗어났어요.")]
    OutOfScope,
}

pub struct PathTokenRegistry {
    inner: RwLock<HashMap<String, TokenEntry>>,
}

impl PathTokenRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: RwLock::new(HashMap::new()),
        })
    }

    /// dialog 결과 path를 canonicalize 후 token 발급.
    pub async fn issue(&self, canonical_path: PathBuf, kind: PathTokenKind) -> String {
        let token = Uuid::new_v4().to_string();
        let mut map = self.inner.write().await;
        map.insert(
            token.clone(),
            TokenEntry {
                canonical_path,
                kind,
                issued_at: Instant::now(),
            },
        );
        token
    }

    /// token으로 PathBuf 복원 + TTL 검사.
    pub async fn resolve(&self, token: &str) -> Result<PathBuf, PathTokenError> {
        let map = self.inner.read().await;
        let entry = map.get(token).ok_or(PathTokenError::Unknown)?;
        if entry.issued_at.elapsed() > TOKEN_TTL {
            return Err(PathTokenError::Expired);
        }
        Ok(entry.canonical_path.clone())
    }

    /// idempotent 회수.
    pub async fn revoke(&self, token: &str) {
        self.inner.write().await.remove(token);
    }

    /// 만료된 token 일괄 정리. lazy sweep — 호출 시점에만 실행.
    pub async fn sweep_expired(&self) -> usize {
        let mut map = self.inner.write().await;
        let before = map.len();
        map.retain(|_, e| e.issued_at.elapsed() <= TOKEN_TTL);
        before - map.len()
    }
}

/// Phase R-F.3 — frontend에서 dialog plugin 결과 path를 token으로 등록하는 IPC.
///
/// 흐름:
/// 1. frontend가 `tauri-plugin-dialog`의 `open()` 호출 → 사용자 선택 path 반환.
/// 2. frontend가 path + kind를 `issue_path_token`에 전달.
/// 3. backend가 canonicalize() 후 token 발급 → frontend 반환.
/// 4. frontend는 이후 IPC 호출에 token만 보냄.
#[tauri::command]
pub async fn issue_path_token(
    path: String,
    kind: String,
    registry: State<'_, Arc<PathTokenRegistry>>,
) -> Result<String, PathTokenError> {
    let canonical = std::path::PathBuf::from(&path)
        .canonicalize()
        .map_err(|_| PathTokenError::OutOfScope)?;
    let kind = match kind.as_str() {
        "file" => PathTokenKind::File,
        "directory" => PathTokenKind::Directory,
        _ => return Err(PathTokenError::OutOfScope),
    };
    Ok(registry.issue(canonical, kind).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path() -> PathBuf {
        std::env::temp_dir()
    }

    #[tokio::test]
    async fn issue_resolve_round_trip_canonical_path() {
        let registry = PathTokenRegistry::new();
        let path = temp_path();
        let canonical = path.canonicalize().unwrap();
        let token = registry
            .issue(canonical.clone(), PathTokenKind::Directory)
            .await;
        let resolved = registry.resolve(&token).await.unwrap();
        assert_eq!(resolved, canonical);
    }

    #[tokio::test]
    async fn unknown_token_returns_unknown_error() {
        let registry = PathTokenRegistry::new();
        let result = registry.resolve("nonexistent-token").await;
        assert!(matches!(result, Err(PathTokenError::Unknown)));
    }

    #[tokio::test]
    async fn revoke_then_resolve_returns_unknown() {
        let registry = PathTokenRegistry::new();
        let token = registry.issue(temp_path(), PathTokenKind::Directory).await;
        registry.revoke(&token).await;
        let result = registry.resolve(&token).await;
        assert!(matches!(result, Err(PathTokenError::Unknown)));
    }

    #[tokio::test]
    async fn revoke_idempotent_for_unknown_token() {
        let registry = PathTokenRegistry::new();
        // 미존재 token revoke는 panic 없이 통과.
        registry.revoke("ghost-token").await;
        registry.revoke("ghost-token").await;
    }

    #[tokio::test]
    async fn concurrent_issue_resolve_no_deadlock() {
        let registry = PathTokenRegistry::new();
        let canonical = temp_path().canonicalize().unwrap();
        let mut handles = Vec::new();
        for _ in 0..100 {
            let r = Arc::clone(&registry);
            let p = canonical.clone();
            handles.push(tokio::spawn(async move {
                let t = r.issue(p.clone(), PathTokenKind::Directory).await;
                r.resolve(&t).await.unwrap();
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
    }

    #[tokio::test]
    async fn uuid_v4_collision_zero_for_10k_issues() {
        let registry = PathTokenRegistry::new();
        let canonical = temp_path().canonicalize().unwrap();
        let mut tokens = std::collections::HashSet::new();
        for _ in 0..10_000 {
            let t = registry
                .issue(canonical.clone(), PathTokenKind::Directory)
                .await;
            assert!(tokens.insert(t), "UUID v4 collision detected");
        }
    }

    #[tokio::test]
    async fn sweep_expired_removes_old_tokens() {
        let registry = PathTokenRegistry::new();
        let canonical = temp_path().canonicalize().unwrap();
        let token = registry
            .issue(canonical.clone(), PathTokenKind::Directory)
            .await;
        // issued_at을 강제로 과거로 (TTL 초과).
        {
            let mut map = registry.inner.write().await;
            if let Some(entry) = map.get_mut(&token) {
                entry.issued_at = Instant::now() - TOKEN_TTL - Duration::from_secs(1);
            }
        }
        let removed = registry.sweep_expired().await;
        assert_eq!(removed, 1);
        let result = registry.resolve(&token).await;
        assert!(matches!(result, Err(PathTokenError::Unknown)));
    }

    #[test]
    fn path_token_kind_serde_kebab_case() {
        let v = serde_json::to_string(&PathTokenKind::File).unwrap();
        assert_eq!(v, r#""file""#);
        let v = serde_json::to_string(&PathTokenKind::Directory).unwrap();
        assert_eq!(v, r#""directory""#);
    }

    #[test]
    fn path_token_error_serde_kebab_tag() {
        let e = PathTokenError::Expired;
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "expired");
    }
}
