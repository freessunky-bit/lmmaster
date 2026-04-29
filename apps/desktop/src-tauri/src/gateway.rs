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

    // Phase 6'.d — PipelinesState config 스냅샷으로 chain 빌드 + audit 채널 mount.
    // PipelinesState가 아직 manage되지 않았다면 default config로 fallback (모두 ON).
    let chain = build_chain_from_state(&app).await;
    if !chain.is_empty() {
        router = core_gateway::with_pipelines_audited(router, chain, audit_sender);
    } else {
        tracing::info!("pipeline chain empty — gateway runs without filter middleware");
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

/// 현재 `PipelinesState` snapshot으로부터 `PipelineChain`을 빌드.
///
/// 정책 (Phase 6'.d):
/// - state가 manage되어 있지 않으면 default config (3종 모두 ON)으로 fallback.
/// - v1 시드 3종(pii-redact / token-quota / observability)을 enabled 토글에 따라 추가.
/// - 사용자 토글 변경 시 동적 재구성은 Phase 6'.e — 본 페이즈는 시작 시점 스냅샷만 사용.
async fn build_chain_from_state(app: &AppHandle) -> pipelines::PipelineChain {
    use pipelines::{ObservabilityPipeline, PiiRedactPipeline, PipelineChain, TokenQuotaPipeline};

    let cfg = if let Some(state) = app.try_state::<Arc<PipelinesState>>() {
        state.snapshot_config().await
    } else {
        tracing::warn!("PipelinesState not yet managed — gateway uses default chain");
        crate::pipelines::PipelinesConfig::default()
    };

    let mut chain = PipelineChain::new();
    if cfg.pii_redact_enabled {
        chain = chain.add(Arc::new(PiiRedactPipeline::new()));
    }
    if cfg.token_quota_enabled {
        chain = chain.add(Arc::new(TokenQuotaPipeline::new()));
    }
    if cfg.observability_enabled {
        chain = chain.add(Arc::new(ObservabilityPipeline::new()));
    }
    chain
}
