//! AppState — gateway 라우트 핸들러가 공유하는 상태.
//!
//! 정책 (ADR-0022 §3):
//! - `Semaphore(permits=1)` global — GPU contention 직렬화.
//! - reqwest 클라이언트 1개 재사용 — connection pool 효율.
//! - UpstreamProvider trait object — 어댑터 dispatch.

use std::sync::Arc;
use std::time::Duration;

use key_manager::KeyManager;
use tokio::sync::Semaphore;

use crate::upstream::UpstreamProvider;

#[derive(Clone)]
pub struct AppState {
    pub provider: Arc<dyn UpstreamProvider>,
    pub semaphore: Arc<Semaphore>,
    pub http: reqwest::Client,
    /// `None` = auth 미들웨어 비활성 (Phase 0/2 호환). v1.0 완성 시 항상 Some 권장.
    pub key_manager: Option<Arc<KeyManager>>,
}

impl AppState {
    pub fn new(provider: Arc<dyn UpstreamProvider>) -> Self {
        let http = reqwest::Client::builder()
            .pool_idle_timeout(Duration::from_secs(30))
            .tcp_keepalive(Duration::from_secs(10))
            .no_proxy()
            .build()
            .expect("reqwest client build");
        Self {
            provider,
            semaphore: Arc::new(Semaphore::new(1)),
            http,
            key_manager: None,
        }
    }

    pub fn with_key_manager(mut self, km: Arc<KeyManager>) -> Self {
        self.key_manager = Some(km);
        self
    }

    /// 테스트용 — semaphore permit 수 커스터마이즈.
    pub fn with_permits(provider: Arc<dyn UpstreamProvider>, permits: usize) -> Self {
        let mut s = Self::new(provider);
        s.semaphore = Arc::new(Semaphore::new(permits));
        s
    }
}
