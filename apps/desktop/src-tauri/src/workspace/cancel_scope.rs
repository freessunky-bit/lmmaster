//! Workspace cancellation scope — Phase R-E.7 (ADR-0058).
//!
//! 정책:
//! - Workspace 전환 시 이전 workspace의 in-flight operation 모두 cancel cascade.
//! - opt-in 등록 — operation이 명시적으로 register 호출. 미등록 op는 영향 없음 (백워드 호환).
//! - 토큰은 `CancellationToken` clone — 등록 후 op가 자체 흐름으로 cancel 발화 가능.
//!
//! 사용 패턴:
//! ```ignore
//! let scope: Arc<WorkspaceCancellationScope> = state.inner().clone();
//! let cancel = CancellationToken::new();
//! scope.register(&workspace_id, cancel.clone());
//! // ... op 진행 ...
//! // workspace 전환 시 자동 cancel.cancel() 발화
//! ```

use std::collections::HashMap;
use std::sync::Mutex;

use tokio_util::sync::CancellationToken;

/// Workspace 전환 시 cascade cancel을 위한 op-token 인덱스.
#[derive(Default)]
pub struct WorkspaceCancellationScope {
    /// workspace_id → 활성 토큰 목록. drop된 토큰(cancel 후)도 그대로 남고 cancel 호출은 idempotent.
    inner: Mutex<HashMap<String, Vec<CancellationToken>>>,
}

impl WorkspaceCancellationScope {
    pub fn new() -> Self {
        Self::default()
    }

    /// 새 op token을 workspace에 등록. workspace 전환 시 자동 cancel 대상.
    pub fn register(&self, workspace_id: &str, token: CancellationToken) {
        let mut inner = self
            .inner
            .lock()
            .expect("WorkspaceCancellationScope poisoned");
        inner
            .entry(workspace_id.to_string())
            .or_default()
            .push(token);
    }

    /// 특정 workspace의 모든 등록 토큰 cancel + drop. 전환 시 호출.
    pub fn cancel_workspace(&self, workspace_id: &str) {
        let mut inner = self
            .inner
            .lock()
            .expect("WorkspaceCancellationScope poisoned");
        if let Some(tokens) = inner.remove(workspace_id) {
            tracing::debug!(
                workspace_id,
                count = tokens.len(),
                "cancelling workspace cascade"
            );
            for tok in tokens {
                tok.cancel();
            }
        }
    }

    /// 모든 workspace 토큰 cancel (앱 종료 시 호출).
    pub fn cancel_all(&self) {
        let mut inner = self
            .inner
            .lock()
            .expect("WorkspaceCancellationScope poisoned");
        for (workspace_id, tokens) in inner.drain() {
            tracing::debug!(
                workspace_id,
                count = tokens.len(),
                "cancelling workspace on shutdown"
            );
            for tok in tokens {
                tok.cancel();
            }
        }
    }

    /// 테스트 / 진단 — 현재 추적 중인 workspace 갯수.
    #[cfg(test)]
    pub fn workspace_count(&self) -> usize {
        self.inner
            .lock()
            .expect("WorkspaceCancellationScope poisoned")
            .len()
    }

    /// 테스트 / 진단 — 특정 workspace의 토큰 갯수.
    #[cfg(test)]
    pub fn token_count(&self, workspace_id: &str) -> usize {
        self.inner
            .lock()
            .expect("WorkspaceCancellationScope poisoned")
            .get(workspace_id)
            .map(|v| v.len())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_then_cancel_workspace_cascades() {
        let scope = WorkspaceCancellationScope::new();
        let t1 = CancellationToken::new();
        let t2 = CancellationToken::new();
        scope.register("ws-a", t1.clone());
        scope.register("ws-a", t2.clone());
        assert_eq!(scope.token_count("ws-a"), 2);

        scope.cancel_workspace("ws-a");
        assert!(t1.is_cancelled());
        assert!(t2.is_cancelled());
        assert_eq!(scope.token_count("ws-a"), 0);
    }

    #[test]
    fn cancel_workspace_does_not_affect_other_workspaces() {
        let scope = WorkspaceCancellationScope::new();
        let t_a = CancellationToken::new();
        let t_b = CancellationToken::new();
        scope.register("ws-a", t_a.clone());
        scope.register("ws-b", t_b.clone());

        scope.cancel_workspace("ws-a");
        assert!(t_a.is_cancelled());
        assert!(!t_b.is_cancelled(), "ws-b는 영향 없음");
    }

    #[test]
    fn cancel_all_drains_everything() {
        let scope = WorkspaceCancellationScope::new();
        let t_a = CancellationToken::new();
        let t_b = CancellationToken::new();
        scope.register("ws-a", t_a.clone());
        scope.register("ws-b", t_b.clone());

        scope.cancel_all();
        assert!(t_a.is_cancelled());
        assert!(t_b.is_cancelled());
        assert_eq!(scope.workspace_count(), 0);
    }

    #[test]
    fn cancel_unknown_workspace_is_noop() {
        let scope = WorkspaceCancellationScope::new();
        scope.cancel_workspace("missing"); // panic 없어야
        assert_eq!(scope.workspace_count(), 0);
    }

    #[test]
    fn register_creates_workspace_entry() {
        let scope = WorkspaceCancellationScope::new();
        let t = CancellationToken::new();
        scope.register("ws-x", t);
        assert_eq!(scope.workspace_count(), 1);
        assert_eq!(scope.token_count("ws-x"), 1);
    }
}
