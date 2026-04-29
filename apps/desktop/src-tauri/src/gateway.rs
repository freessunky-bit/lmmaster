//! Tauri supervisor가 관리하는 gateway lifecycle.
//!
//! - 단일 GatewayHandle 인스턴스가 supervisor와 IPC command 양쪽에서 사용된다.
//! - 내부 Mutex로 보호된 GatewayState를 frontend snapshot용으로 노출.
//! - cancellation token으로 graceful shutdown.

use std::sync::{Arc, Mutex};

use pipelines::AuditEntry;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::pipelines::PipelinesState;

#[derive(Serialize, Clone, Default, Debug)]
pub struct GatewayState {
    pub port: Option<u16>,
    pub status: GatewayStatus,
    pub error: Option<String>,
}

#[derive(Serialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum GatewayStatus {
    #[default]
    Booting,
    Listening,
    Failed,
    Stopping,
}

#[derive(Clone)]
pub struct GatewayHandle {
    inner: Arc<Inner>,
}

struct Inner {
    state: Mutex<GatewayState>,
    cancel: CancellationToken,
}

impl GatewayHandle {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(GatewayState::default()),
                cancel: CancellationToken::new(),
            }),
        }
    }

    pub fn snapshot(&self) -> GatewayState {
        self.inner
            .state
            .lock()
            .expect("gateway state mutex poisoned")
            .clone()
    }

    pub fn cancel(&self) {
        let already = self.inner.cancel.is_cancelled();
        self.inner.cancel.cancel();
        if !already {
            let mut s = self
                .inner
                .state
                .lock()
                .expect("gateway state mutex poisoned");
            s.status = GatewayStatus::Stopping;
        }
    }

    pub fn cancel_token(&self) -> CancellationToken {
        self.inner.cancel.clone()
    }

    fn set_listening(&self, port: u16) {
        let mut s = self
            .inner
            .state
            .lock()
            .expect("gateway state mutex poisoned");
        s.port = Some(port);
        s.status = GatewayStatus::Listening;
        s.error = None;
    }

    fn set_failed(&self, e: impl Into<String>) {
        let mut s = self
            .inner
            .state
            .lock()
            .expect("gateway state mutex poisoned");
        s.status = GatewayStatus::Failed;
        s.error = Some(e.into());
    }
}

impl Default for GatewayHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// supervisor entry. 실패 시 GatewayHandle에 상태를 기록하고 frontend로 이벤트 emit.
///
/// Phase 6'.d — `audit_sender`를 받아 PipelineLayer에 주입. PipelineChain은 PipelinesState의
/// 현재 config 스냅샷에 따라 정적 빌드 (사용자 토글에 따른 동적 재구성은 Phase 6'.e 예정).
pub async fn run(
    app: AppHandle,
    handle: GatewayHandle,
    audit_sender: mpsc::Sender<AuditEntry>,
) -> anyhow::Result<()> {
    let cancel = handle.cancel_token();

    // 0. listener bind. 실패 시 이벤트 + 상태 기록.
    let listener = match tokio::net::TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(e) => {
            let msg = format!("bind 127.0.0.1:0 failed: {e}");
            tracing::error!(error = %e, "gateway bind failed");
            handle.set_failed(&msg);
            let _ = app.emit("gateway://failed", &msg);
            return Err(e.into());
        }
    };

    // 1. 포트 추출은 serve가 listener를 consume하기 전에 해야 한다.
    let port = listener.local_addr()?.port();
    handle.set_listening(port);
    if let Err(e) = app.emit("gateway://ready", port) {
        tracing::warn!(error = %e, "failed to emit gateway://ready");
    }
    tracing::info!(port, "gateway listening");

    // 2. router build + serve.
    // Phase 3'.c+ — `LiveRegistryProvider`로 실제 라우팅 활성.
    // Ollama / LM Studio 어댑터 둘 다 시도 등록 + `/v1/chat/completions`가 모델 ID 보유 어댑터로 forward.
    // 외부 통신 0 정책: localhost 11434 / 1234만. cloud fallback 없음.
    // Phase 3'.b: KeyManager 주입으로 auth 미들웨어 활성.
    let provider: std::sync::Arc<dyn core_gateway::UpstreamProvider> = {
        if let Some(p) =
            app.try_state::<std::sync::Arc<crate::registry_provider::LiveRegistryProvider>>()
        {
            (*p).clone() as std::sync::Arc<dyn core_gateway::UpstreamProvider>
        } else {
            tracing::warn!(
                "LiveRegistryProvider not yet managed — gateway falls back to empty StaticProvider"
            );
            std::sync::Arc::new(core_gateway::StaticProvider::default())
        }
    };
    let mut state = core_gateway::AppState::new(provider);
    if let Some(km) = app.try_state::<std::sync::Arc<key_manager::KeyManager>>() {
        state = state.with_key_manager((*km).clone());
    } else {
        tracing::warn!("KeyManager not yet managed — gateway runs without auth");
    }
    let mut router = core_gateway::build_router(core_gateway::GatewayConfig::default(), state);

    // Phase 6'.d / 8'.c.2 — PipelinesState chain_swap을 PipelineLayer에 mount.
    // PipelinesState가 manage되지 않았다면 default config로 fallback chain을 만들어 자체 swap 보유.
    if let Some(state) = app.try_state::<Arc<PipelinesState>>() {
        let chain_swap = state.chain_swap();
        // 빈 chain이어도 layer를 mount — 사용자가 토글 ON으로 바꾸면 즉시 hot-reload 적용.
        router = core_gateway::with_pipelines_audited_swap(router, chain_swap, audit_sender);
        tracing::info!("pipeline layer mounted with hot-reload chain_swap (Phase 8'.c.2)");
    } else {
        tracing::warn!("PipelinesState not yet managed — gateway runs without pipeline middleware");
    }

    if let Err(e) = core_gateway::serve_with_shutdown(listener, router, cancel).await {
        let msg = format!("serve error: {e}");
        tracing::error!(error = %e, "gateway serve failed");
        handle.set_failed(&msg);
        let _ = app.emit("gateway://failed", &msg);
        return Err(e.into());
    }

    tracing::info!("gateway shutdown completed cleanly");
    Ok(())
}

// Phase 8'.c.2: chain 빌드는 `crate::pipelines::build_chain`으로 이동.
//   gateway는 `PipelinesState::chain_swap()` 핸들을 PipelineLayer에 직접 mount해
//   사용자 토글 시 hot-reload가 자동 작동.
