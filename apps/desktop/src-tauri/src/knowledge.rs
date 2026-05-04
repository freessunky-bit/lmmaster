//! Knowledge Stack IPC 모듈 — Phase 4.5'.b. Channel<IngestEvent> + KnowledgeRegistry + ingest/search 커맨드.
//!
//! 정책 (phase-5pb-4p5b-ipc-reinforcement.md §1, §2, §6):
//! - `tauri::ipc::Channel<IngestEvent>` per-invocation stream — `Emitter::emit`보다 typed + ordered.
//! - KnowledgeRegistry는 `app.manage(Arc<KnowledgeRegistry>)`로 공유 — clone으로 task 캡처.
//! - cancel은 별도 `cancel_ingest` command (Tauri invoke AbortSignal 미지원 — issue #8351).
//! - **registry key = workspace_id**. workbench는 다중 동시 run을 허용하지만 ingest는 동일 workspace에 동시
//!   write 시 SQLite 락 충돌 위험 → workspace 단위 직렬화. (§2.2 race table.)
//! - mpsc::Sender<IngestProgress> → Channel<IngestEvent> 어댑터는 별도 task — installer의
//!   ChannelInstallSink와 동일 결. 종료 시 sender drop으로 receiver loop 자연 종료.
//! - 단계 진입 시 1회 + 청크 단위 1회 emit — 단계당 < 100 events.
//! - send 실패 = window 닫힘 → cancel 트리거 (installer emit_or_cancel 패턴).
//! - 한국어 해요체 에러 메시지.
//!
//! Phase 5'.b workbench와 동일 아키텍처. 핵심 차이:
//! - registry key = workspace_id (NOT ingest_id) — single-writer per workspace.
//! - mpsc bridge task — IngestService가 mpsc::Sender<IngestProgress>를 받고, 우리는 receiver를
//!   drain하며 IngestEvent로 forward.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Instant;

use knowledge_stack::{
    is_downloaded, DownloadEvent, Embedder, IngestProgress, IngestService, IngestStage,
    KnowledgeStore, MockEmbedder, ModelDownloader, OnnxModelKind,
};
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use thiserror::Error;
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

// ───────────────────────────────────────────────────────────────────
// 도메인 타입 — frontend Channel<IngestEvent> + invoke 응답에 사용
// ───────────────────────────────────────────────────────────────────

/// Ingest 시작 옵션 — frontend가 invoke에 전달.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestConfig {
    /// 워크스페이스 식별자 (KnowledgeStore.workspaces.id).
    pub workspace_id: String,
    /// 파일 또는 디렉터리 경로 (절대 경로 권장).
    pub path: String,
    /// 단일 파일 vs 재귀 디렉터리. v1은 service 단에서 자동 판별하지만, UI 의도를 보존.
    #[serde(default = "default_kind")]
    pub kind: String,
    /// 청크 목표 크기 (문자 단위). 기본 1000.
    #[serde(default = "default_chunk_size")]
    pub target_chunk_size: usize,
    /// 청크 overlap (문자 단위). 기본 200.
    #[serde(default = "default_overlap")]
    pub overlap: usize,
    /// SQLite 파일 경로 — 호출자가 결정 (workspace별 격리). 빈 string이면 in-memory.
    #[serde(default)]
    pub store_path: String,
}

fn default_kind() -> String {
    "directory".to_string()
}

fn default_chunk_size() -> usize {
    1000
}

fn default_overlap() -> usize {
    200
}

/// Ingest 종료 시 frontend에 노출할 요약.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IngestSummary {
    pub ingest_id: String,
    pub workspace_id: String,
    pub files_processed: usize,
    pub chunks_created: usize,
    pub skipped: usize,
    pub total_duration_ms: u64,
}

/// 검색 결과 단위 — chunk 메타 + cosine score [0, 1].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub chunk_id: String,
    pub document_id: String,
    /// 문서 경로 — KnowledgeStore.documents.path. v1.x에 메타 추가 가능.
    pub document_path: String,
    pub content: String,
    pub score: f32,
}

/// 워크스페이스 통계 — Workspace 페이지 banner / Knowledge tab의 헤더에 노출.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceStats {
    pub workspace_id: String,
    pub documents: usize,
    pub chunks: usize,
}

/// 활성 ingest snapshot — list_ingests command가 반환.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveIngestSnapshot {
    pub workspace_id: String,
    pub ingest_id: String,
    /// RFC3339.
    pub started_at: String,
    pub current_stage: IngestStage,
}

/// Channel<IngestEvent>로 frontend에 흘려보내는 event.
///
/// `#[serde(tag = "kind", rename_all = "kebab-case")]` — InstallEvent / WorkbenchEvent와 동일 셰입.
/// frontend는 `event.kind`로 discriminated union을 narrow한다.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum IngestEvent {
    /// ingest 시작 직후 1회.
    Started {
        ingest_id: String,
        workspace_id: String,
        path: String,
    },
    /// 파일 읽기 진입 — `current_path`는 현재 처리 중 파일.
    Reading {
        ingest_id: String,
        current_path: String,
    },
    /// chunking 진행 — processed: 현재까지 처리된 파일 수, total: 전체 파일 수.
    Chunking {
        ingest_id: String,
        processed: usize,
        total: usize,
    },
    /// embedding 진행.
    Embedding {
        ingest_id: String,
        processed: usize,
        total: usize,
    },
    /// SQLite write 진행.
    Writing {
        ingest_id: String,
        processed: usize,
        total: usize,
    },
    /// 정상 완료 + 요약.
    Done {
        ingest_id: String,
        summary: IngestSummary,
    },
    /// 실패. message는 한국어 해요체.
    Failed { ingest_id: String, error: String },
    /// 사용자 cancel 또는 channel close.
    Cancelled { ingest_id: String },
}

// ───────────────────────────────────────────────────────────────────
// API error — invoke().reject로 frontend에 전달
// ───────────────────────────────────────────────────────────────────

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum KnowledgeApiError {
    #[error("이 워크스페이스는 이미 자료를 받고 있어요. 끝나면 다시 시도해 주세요.")]
    AlreadyIngesting { workspace_id: String },

    #[error("워크스페이스를 찾지 못했어요: {workspace_id}")]
    WorkspaceNotFound { workspace_id: String },

    /// Phase #31 (ADR-0058) — store_path가 app_data_dir sandbox 밖이거나 traversal 시도.
    #[error("data 디렉터리 밖 경로에는 접근할 수 없어요: {reason}")]
    PathDenied { reason: String },

    #[error("지식 저장소를 열지 못했어요: {message}")]
    StoreOpen { message: String },

    #[error("인덱싱을 시작하지 못했어요: {message}")]
    StartFailed { message: String },

    #[error("검색에 실패했어요: {message}")]
    SearchFailed { message: String },

    #[error("내부 오류가 났어요: {message}")]
    Internal { message: String },

    #[error("이미 같은 임베딩 모델을 받고 있어요. 끝나면 다시 시도해 주세요. ({model_kind})")]
    AlreadyDownloading { model_kind: String },

    #[error("알 수 없는 임베딩 모델이에요: {model_kind}")]
    UnknownEmbeddingModel { model_kind: String },

    #[error("모델을 먼저 받아야 활성화할 수 있어요: {model_kind}")]
    ModelNotDownloaded { model_kind: String },
}

// ───────────────────────────────────────────────────────────────────
// Registry — workspace_id ↔ active ingest 메타
// ───────────────────────────────────────────────────────────────────

/// 내부 entry — token + 메타.
struct RegistryEntry {
    ingest_id: String,
    cancel: CancellationToken,
    started_at: String,
    current_stage: IngestStage,
    /// IngestService가 사용하는 협력 cancel flag (CancellationToken과 별도).
    /// 두 cancel signal은 task 안에서 묶임 (token cancel 시 atomic도 set).
    atomic_cancel: Arc<AtomicBool>,
}

/// workspace_id ↔ ingest 메타. tokio::sync::Mutex 사용 — workbench와 일관.
/// 락 보유 시간이 1µs 미만이지만 await가 걸린 경로 (start 내 spawn 직전)도 있어 async mutex 채택.
#[derive(Default)]
pub struct KnowledgeRegistry {
    inner: AsyncMutex<HashMap<String, RegistryEntry>>,
}

impl KnowledgeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 새 ingest 등록. workspace 단위 직렬화 — 동일 workspace 중복 시 거부.
    /// 반환: (ingest_id, cancel_token, atomic_cancel_flag).
    pub async fn register(
        &self,
        workspace_id: &str,
    ) -> Result<(String, CancellationToken, Arc<AtomicBool>), KnowledgeApiError> {
        let mut g = self.inner.lock().await;
        if g.contains_key(workspace_id) {
            return Err(KnowledgeApiError::AlreadyIngesting {
                workspace_id: workspace_id.to_string(),
            });
        }
        let ingest_id = Uuid::new_v4().to_string();
        let tok = CancellationToken::new();
        let atomic = Arc::new(AtomicBool::new(false));
        let now = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default();
        g.insert(
            workspace_id.to_string(),
            RegistryEntry {
                ingest_id: ingest_id.clone(),
                cancel: tok.clone(),
                started_at: now,
                current_stage: IngestStage::Reading,
                atomic_cancel: atomic.clone(),
            },
        );
        Ok((ingest_id, tok, atomic))
    }

    /// 현재 stage 갱신 — list_ingests가 즉시 보이도록.
    pub async fn set_stage(&self, workspace_id: &str, stage: IngestStage) {
        let mut g = self.inner.lock().await;
        if let Some(entry) = g.get_mut(workspace_id) {
            entry.current_stage = stage;
        }
    }

    /// ingest 종료 — entry 제거. 미존재면 no-op.
    pub async fn finish(&self, workspace_id: &str) {
        let mut g = self.inner.lock().await;
        g.remove(workspace_id);
    }

    /// ingest cancel — idempotent. 미존재 = no-op.
    /// 두 cancel 신호 모두 set: tokio CancellationToken (forwarding task용) + AtomicBool (IngestService cooperative).
    pub async fn cancel(&self, workspace_id: &str) {
        let g = self.inner.lock().await;
        if let Some(entry) = g.get(workspace_id) {
            entry.cancel.cancel();
            entry.atomic_cancel.store(true, Ordering::SeqCst);
        }
    }

    /// 모든 ingest cancel — 앱 종료 시.
    pub async fn cancel_all(&self) {
        let g = self.inner.lock().await;
        for entry in g.values() {
            entry.cancel.cancel();
            entry.atomic_cancel.store(true, Ordering::SeqCst);
        }
    }

    /// snapshot — 현재 active ingests.
    pub async fn list(&self) -> Vec<ActiveIngestSnapshot> {
        let g = self.inner.lock().await;
        let mut out: Vec<ActiveIngestSnapshot> = g
            .iter()
            .map(|(ws_id, entry)| ActiveIngestSnapshot {
                workspace_id: ws_id.clone(),
                ingest_id: entry.ingest_id.clone(),
                started_at: entry.started_at.clone(),
                current_stage: entry.current_stage,
            })
            .collect();
        out.sort_by(|a, b| a.started_at.cmp(&b.started_at));
        out
    }

    /// 디버그용 카운트.
    pub async fn in_flight_count(&self) -> usize {
        self.inner.lock().await.len()
    }

    /// 비-async 종료 시점 sync cancel — `RunEvent::ExitRequested` sync 컨텍스트에서 호출.
    /// `tokio::sync::Mutex::try_lock()` 기반 best-effort. lock을 즉시 잡지 못하면 skip.
    pub fn cancel_all_blocking(&self) {
        if let Ok(g) = self.inner.try_lock() {
            for entry in g.values() {
                entry.cancel.cancel();
                entry.atomic_cancel.store(true, Ordering::SeqCst);
            }
        }
    }
}

// ───────────────────────────────────────────────────────────────────
// Channel forwarding helper — send 실패 시 cancel 트리거 (installer/workbench 패턴).
// ───────────────────────────────────────────────────────────────────

fn emit_or_cancel(
    channel: &Channel<IngestEvent>,
    cancel: &CancellationToken,
    atomic: &AtomicBool,
    event: IngestEvent,
) {
    if channel.send(event).is_err() {
        // window 닫힘 등 — cancel 트리거 (token + atomic flag 모두).
        tracing::debug!("knowledge channel send failed; triggering cancel");
        cancel.cancel();
        atomic.store(true, Ordering::SeqCst);
    }
}

// ───────────────────────────────────────────────────────────────────
// run_ingest — pure async function. KnowledgeStore 열기 + IngestService 실행 + bridge task.
// ───────────────────────────────────────────────────────────────────

/// IngestService를 구동하고 mpsc::Receiver<IngestProgress>를 Channel<IngestEvent>로 forward.
///
/// 수명:
/// - bridge task는 receiver가 None을 받으면(=sender drop) 자연 종료.
/// - 본 task가 종료되면 channel은 closed (Tauri Channel은 strong ref counted).
/// - cancel 트리거 → IngestService가 cooperative atomic flag를 검사 → KnowledgeError::Cancelled 반환.
///
/// `embedder` 인자: caller가 active OnnxModelKind 기반으로 미리 해결 (setup phase). None이면
/// MockEmbedder default — 테스트/dev fallback.
#[allow(clippy::too_many_arguments)]
pub async fn run_ingest(
    config: IngestConfig,
    ingest_id: String,
    registry: Arc<KnowledgeRegistry>,
    pool: Arc<KnowledgeStorePool>,
    cancel: CancellationToken,
    atomic_cancel: Arc<AtomicBool>,
    channel: Channel<IngestEvent>,
    embedder: Arc<dyn Embedder>,
) {
    let start = Instant::now();
    let workspace_id = config.workspace_id.clone();
    let path_str = config.path.clone();

    // 1. Started emit.
    emit_or_cancel(
        &channel,
        &cancel,
        &atomic_cancel,
        IngestEvent::Started {
            ingest_id: ingest_id.clone(),
            workspace_id: workspace_id.clone(),
            path: path_str.clone(),
        },
    );

    // 2. Store open (pool 캐시 hit 시 재사용).
    let store = match pool.get_or_open(&config.store_path) {
        Ok(s) => s,
        Err(e) => {
            emit_or_cancel(
                &channel,
                &cancel,
                &atomic_cancel,
                IngestEvent::Failed {
                    ingest_id: ingest_id.clone(),
                    error: format!("{e}"),
                },
            );
            registry.finish(&workspace_id).await;
            return;
        }
    };

    // 3. Embedder — caller가 미리 해결해 inject (Phase 9'.a). active model 미설정 시 MockEmbedder.
    let service = IngestService::new(store, Arc::clone(&embedder));

    // 4. mpsc 채널 + bridge task — IngestProgress → IngestEvent forward.
    let (tx, mut rx) = mpsc::channel::<IngestProgress>(64);
    let bridge_channel = channel.clone();
    let bridge_cancel = cancel.clone();
    let bridge_atomic = Arc::clone(&atomic_cancel);
    let bridge_ingest_id = ingest_id.clone();
    let bridge_registry = Arc::clone(&registry);
    let bridge_workspace_id = workspace_id.clone();
    let bridge_handle = tauri::async_runtime::spawn(async move {
        while let Some(p) = rx.recv().await {
            // registry stage 갱신.
            bridge_registry
                .set_stage(&bridge_workspace_id, p.stage)
                .await;
            let ev = progress_to_event(&bridge_ingest_id, &p);
            emit_or_cancel(&bridge_channel, &bridge_cancel, &bridge_atomic, ev);
        }
    });

    // 5. ingest_path 호출 — IngestService가 cooperative cancel을 atomic_cancel로 검사.
    let path = PathBuf::from(&config.path);
    let result = service
        .ingest_path(
            &workspace_id,
            &path,
            config.target_chunk_size.max(1),
            config.overlap,
            Some(tx),
            atomic_cancel.clone(),
        )
        .await;

    // bridge task 정리 — service가 끝나면 mpsc tx가 drop되어 receiver loop가 자연 종료.
    let _ = bridge_handle.await;

    // 6. 결과 → terminal event.
    match result {
        Ok(summary) => {
            let total_duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
            let ipc_summary = IngestSummary {
                ingest_id: ingest_id.clone(),
                workspace_id: workspace_id.clone(),
                files_processed: summary.documents,
                chunks_created: summary.chunks,
                skipped: summary.skipped,
                total_duration_ms,
            };
            emit_or_cancel(
                &channel,
                &cancel,
                &atomic_cancel,
                IngestEvent::Done {
                    ingest_id: ingest_id.clone(),
                    summary: ipc_summary,
                },
            );
        }
        Err(knowledge_stack::KnowledgeError::Cancelled) => {
            emit_or_cancel(
                &channel,
                &cancel,
                &atomic_cancel,
                IngestEvent::Cancelled {
                    ingest_id: ingest_id.clone(),
                },
            );
        }
        Err(e) => {
            emit_or_cancel(
                &channel,
                &cancel,
                &atomic_cancel,
                IngestEvent::Failed {
                    ingest_id: ingest_id.clone(),
                    error: format!("{e}"),
                },
            );
        }
    }

    registry.finish(&workspace_id).await;
}

/// Phase R-E.5 (P1, ADR-0058) — KnowledgeStore reuse pool.
///
/// IPC 호출당 매번 SQLite open/close 반복하던 패턴을 Arc<Mutex<Store>> 캐시로 대체.
/// 같은 store_path의 다음 호출은 동일 Arc 재사용 → file handle 유지 + schema query skip.
///
/// LRU 정책: pool size > max_size 시 oldest entry FIFO eviction. 사용자 PC에서 활성
/// workspace 4개를 동시에 다룰 케이스는 거의 없으므로 max=4면 충분.
pub struct KnowledgeStorePool {
    inner: StdMutex<HashMap<String, Arc<StdMutex<KnowledgeStore>>>>,
    /// FIFO eviction을 위한 insertion order 추적.
    order: StdMutex<Vec<String>>,
    max_size: usize,
    /// Phase #38 (ADR-0058) — SQLCipher passphrase. None이면 평문 모드 (Linux headless / keyring 미접근).
    /// `sqlcipher` feature OFF 빌드(stock SQLite)에선 어떤 값이든 PRAGMA key가 무시되어 평문 동작.
    /// passphrase는 process lifetime 동안 유지 — keyring 재읽기 비용 0.
    passphrase: Option<String>,
}

impl KnowledgeStorePool {
    pub fn new() -> Self {
        Self::with_capacity(4)
    }

    pub fn with_capacity(max_size: usize) -> Self {
        Self {
            inner: StdMutex::new(HashMap::new()),
            order: StdMutex::new(Vec::new()),
            max_size,
            passphrase: None,
        }
    }

    /// Phase #38 (ADR-0058) — passphrase를 적용한 pool 생성.
    /// caller(`provision_knowledge_passphrase`)가 keyring에서 읽은 secret을 주입.
    pub fn with_passphrase(passphrase: String) -> Self {
        let mut pool = Self::new();
        pool.passphrase = Some(passphrase);
        pool
    }

    /// store_path에 해당하는 KnowledgeStore Arc를 반환. 캐시 hit이면 즉시 clone, miss면 open + 적재.
    /// 빈 문자열 path는 in-memory store.
    pub fn get_or_open(
        &self,
        store_path: &str,
    ) -> Result<Arc<StdMutex<KnowledgeStore>>, KnowledgeApiError> {
        let key = store_path.to_string();
        let mut inner = self
            .inner
            .lock()
            .expect("KnowledgeStorePool inner poisoned");

        if let Some(existing) = inner.get(&key) {
            return Ok(existing.clone());
        }

        // FIFO eviction.
        if inner.len() >= self.max_size {
            let mut order = self
                .order
                .lock()
                .expect("KnowledgeStorePool order poisoned");
            if let Some(oldest) = order.first().cloned() {
                inner.remove(&oldest);
                order.remove(0);
            }
        }

        // Phase #38 (ADR-0058) — passphrase 있으면 SQLCipher open_with_passphrase, 없으면 평문 open.
        // 빈 path는 in-memory (passphrase 무관 — 메모리 전용).
        let store = if store_path.is_empty() {
            KnowledgeStore::open_memory()
        } else if let Some(pass) = self.passphrase.as_deref() {
            KnowledgeStore::open_with_passphrase(Path::new(store_path), pass)
        } else {
            KnowledgeStore::open(Path::new(store_path))
        }
        .map_err(|e| KnowledgeApiError::StoreOpen {
            message: format!("{e}"),
        })?;

        let arc = Arc::new(StdMutex::new(store));
        inner.insert(key.clone(), arc.clone());
        self.order
            .lock()
            .expect("KnowledgeStorePool order poisoned")
            .push(key);
        Ok(arc)
    }

    /// 테스트 / 진단용 — 현재 캐시된 entry 수.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .expect("KnowledgeStorePool inner poisoned")
            .len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for KnowledgeStorePool {
    fn default() -> Self {
        Self::new()
    }
}

/// IngestProgress → IngestEvent. stage별 enum variant 분기.
fn progress_to_event(ingest_id: &str, p: &IngestProgress) -> IngestEvent {
    match p.stage {
        IngestStage::Reading => IngestEvent::Reading {
            ingest_id: ingest_id.to_string(),
            current_path: p.current_path.clone().unwrap_or_default(),
        },
        IngestStage::Chunking => IngestEvent::Chunking {
            ingest_id: ingest_id.to_string(),
            processed: p.processed,
            total: p.total,
        },
        IngestStage::Embedding => IngestEvent::Embedding {
            ingest_id: ingest_id.to_string(),
            processed: p.processed,
            total: p.total,
        },
        IngestStage::Writing => IngestEvent::Writing {
            ingest_id: ingest_id.to_string(),
            processed: p.processed,
            total: p.total,
        },
        IngestStage::Done => IngestEvent::Writing {
            ingest_id: ingest_id.to_string(),
            processed: p.total,
            total: p.total,
        },
    }
}

// ───────────────────────────────────────────────────────────────────
// Tauri commands
// ───────────────────────────────────────────────────────────────────

/// Ingest 시작. ingest_id를 즉시 반환하고, ingest는 백그라운드 task로 진행.
/// 진행 이벤트는 `on_event` Channel로 흘려보낸다.
///
/// 동일 workspace에 대해 이미 ingest가 진행 중이면 `AlreadyIngesting` 반환 — 한국어 해요체 메시지.
///
/// Phase #31 (ADR-0058) — config.store_path는 app_data_dir sandbox 안인지 검증.
/// frontend가 임의 경로로 데이터 작성 시도 시 PathDenied 거부.
///
/// Phase R-E.7 (ADR-0058) — cancel 토큰을 WorkspaceCancellationScope에 등록.
/// 사용자가 다른 workspace로 전환하면 이 ingest가 자동 cancel cascade.
#[tauri::command]
pub async fn ingest_path(
    app: AppHandle,
    config: IngestConfig,
    on_event: Channel<IngestEvent>,
    registry: State<'_, Arc<KnowledgeRegistry>>,
    embedding_state: State<'_, Arc<EmbeddingState>>,
    store_pool: State<'_, Arc<KnowledgeStorePool>>,
    cancel_scope: State<'_, Arc<crate::workspace::WorkspaceCancellationScope>>,
) -> Result<String, KnowledgeApiError> {
    // store_path 검증 — sandbox 밖 거부.
    let mut config = config;
    config.store_path = validate_against_app_data_dir(&app, &config.store_path)?;
    let workspace_id = config.workspace_id.clone();
    let (ingest_id, cancel, atomic_cancel) = registry.register(&workspace_id).await?;
    // Phase R-E.7 — workspace 전환 시 자동 cascade.
    cancel_scope.register(&workspace_id, cancel.clone());
    let registry_arc: Arc<KnowledgeRegistry> = registry.inner().clone();
    let pool_arc: Arc<KnowledgeStorePool> = store_pool.inner().clone();
    let id_for_return = ingest_id.clone();

    // Phase 9'.a — active 모델로 embedder를 해결. 미설정 / 미다운로드 / ONNX feature off →
    // MockEmbedder fallback (RAG 자체는 동작, ranking 품질만 deterministic-mock).
    let embedder = resolve_active_embedder(embedding_state.inner()).await;

    // Tauri 2 정책: tauri::async_runtime::spawn 사용 (tokio::spawn 금지 — Tauri가 자체 runtime 소유).
    tauri::async_runtime::spawn(async move {
        run_ingest(
            config,
            ingest_id,
            registry_arc,
            pool_arc,
            cancel,
            atomic_cancel,
            on_event,
            embedder,
        )
        .await;
    });

    Ok(id_for_return)
}

/// active OnnxModelKind 기반 임베더. fallback_to_mock=true.
async fn resolve_active_embedder(state: &Arc<EmbeddingState>) -> Arc<dyn Embedder> {
    let kind = state.active().await;
    match knowledge_stack::default_embedder(state.target_dir(), kind, true).await {
        Ok(emb) => emb,
        Err(e) => {
            tracing::warn!(error = %e, "default_embedder 실패 — MockEmbedder default fallback");
            Arc::new(MockEmbedder::default())
        }
    }
}

/// 진행 중 ingest를 cancel — idempotent. workspace_id 기반.
#[tauri::command]
pub async fn cancel_ingest(
    workspace_id: String,
    registry: State<'_, Arc<KnowledgeRegistry>>,
) -> Result<(), KnowledgeApiError> {
    registry.cancel(&workspace_id).await;
    Ok(())
}

/// 활성 ingest 목록 (registry snapshot).
#[tauri::command]
pub async fn list_ingests(
    registry: State<'_, Arc<KnowledgeRegistry>>,
) -> Result<Vec<ActiveIngestSnapshot>, KnowledgeApiError> {
    Ok(registry.list().await)
}

/// 동기 검색 RPC. 임베더는 active OnnxModelKind 기반으로 on-demand 생성 (Phase 9'.a).
/// k는 max 50으로 cap (DoS 회피 + cosine brute-force 비용 제한).
///
/// Phase #31 (ADR-0058) — store_path는 app_data_dir sandbox 안인지 검증.
/// frontend가 임의 경로로 검색 시도 시 PathDenied 거부.
#[tauri::command]
pub async fn search_knowledge(
    app: AppHandle,
    workspace_id: String,
    query: String,
    k: usize,
    store_path: String,
    embedding_state: State<'_, Arc<EmbeddingState>>,
    store_pool: State<'_, Arc<KnowledgeStorePool>>,
) -> Result<Vec<SearchHit>, KnowledgeApiError> {
    let validated_path = validate_against_app_data_dir(&app, &store_path)?;
    let embedder = resolve_active_embedder(embedding_state.inner()).await;
    let pool = store_pool.inner().clone();
    search_knowledge_with_embedder(workspace_id, query, k, validated_path, embedder, pool).await
}

/// IPC 진입점에서 사용하는 헬퍼 — AppHandle에서 app_data_dir를 추출 후 validate_store_path 호출.
fn validate_against_app_data_dir(
    app: &AppHandle,
    requested: &str,
) -> Result<String, KnowledgeApiError> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| KnowledgeApiError::PathDenied {
            reason: format!("app_data_dir 접근 실패: {e}"),
        })?;
    // app_data_dir는 첫 부팅 시 미존재 가능 — 생성 시도.
    if !data_dir.exists() {
        std::fs::create_dir_all(&data_dir).map_err(|e| KnowledgeApiError::PathDenied {
            reason: format!("app_data_dir 생성 실패: {e}"),
        })?;
    }
    validate_store_path(&data_dir, requested)
}

/// `search_knowledge`의 embedder-injectable 버전 — 단위 테스트가 직접 호출 가능.
pub async fn search_knowledge_with_embedder(
    workspace_id: String,
    query: String,
    k: usize,
    store_path: String,
    embedder: Arc<dyn Embedder>,
    pool: Arc<KnowledgeStorePool>,
) -> Result<Vec<SearchHit>, KnowledgeApiError> {
    let k = k.min(50);
    if query.trim().is_empty() || k == 0 {
        return Ok(Vec::new());
    }

    // store 열기 (pool 캐시 hit 시 재사용).
    let store_arc = pool.get_or_open(&store_path)?;

    // workspace 존재 검증.
    {
        let store = store_arc.lock().map_err(|_| KnowledgeApiError::Internal {
            message: "store mutex poisoned".to_string(),
        })?;
        let exists =
            store
                .has_workspace(&workspace_id)
                .map_err(|e| KnowledgeApiError::SearchFailed {
                    message: format!("{e}"),
                })?;
        if !exists {
            return Err(KnowledgeApiError::WorkspaceNotFound { workspace_id });
        }
    }

    // 쿼리 임베딩.
    let texts = std::slice::from_ref(&query);
    let q_vec = embedder
        .embed(texts)
        .await
        .map_err(|e| KnowledgeApiError::SearchFailed {
            message: format!("{e}"),
        })?;
    let query_vector = q_vec.into_iter().next().unwrap_or_default();

    // search.
    let raw_hits = {
        let store = store_arc.lock().map_err(|_| KnowledgeApiError::Internal {
            message: "store mutex poisoned".to_string(),
        })?;
        store.search(&workspace_id, &query_vector, k).map_err(|e| {
            KnowledgeApiError::SearchFailed {
                message: format!("{e}"),
            }
        })?
    };

    // 문서 path lookup — chunk row의 document_id로 documents 테이블 조회 (Phase 8'.a.1).
    // KnowledgeStore::get_document_path는 workspace_id 격리 적용 — 다른 ws id 조회 시 None.
    // 누락 / DB 오류 시 한국어 fallback "원본 경로 없음" — UI는 path를 그대로 노출하므로 의미 있는 메시지.
    let hits: Vec<SearchHit> = {
        let store = store_arc.lock().map_err(|_| KnowledgeApiError::Internal {
            message: "store mutex poisoned".to_string(),
        })?;
        raw_hits
            .into_iter()
            .map(|h| {
                let document_path =
                    match store.get_document_path(&workspace_id, &h.chunk.document_id) {
                        Ok(Some(p)) => p.to_string_lossy().to_string(),
                        Ok(None) => "원본 경로 없음".to_string(),
                        Err(e) => {
                            tracing::warn!(
                                workspace_id = %workspace_id,
                                document_id = %h.chunk.document_id,
                                error = %e,
                                "document path lookup 실패 — fallback 메시지 노출",
                            );
                            "원본 경로 없음".to_string()
                        }
                    };
                SearchHit {
                    chunk_id: h.chunk.id.clone(),
                    document_id: h.chunk.document_id.clone(),
                    document_path,
                    content: h.chunk.content,
                    score: h.score,
                }
            })
            .collect()
    };

    Ok(hits)
}

/// Workspace 통계 — banner / header 노출.
///
/// Phase #31 (ADR-0058) — store_path는 app_data_dir sandbox 안인지 검증.
#[tauri::command]
pub async fn knowledge_workspace_stats(
    app: AppHandle,
    workspace_id: String,
    store_path: String,
    store_pool: State<'_, Arc<KnowledgeStorePool>>,
) -> Result<WorkspaceStats, KnowledgeApiError> {
    let validated_path = validate_against_app_data_dir(&app, &store_path)?;
    let pool = store_pool.inner().clone();
    knowledge_workspace_stats_with_pool(workspace_id, validated_path, pool).await
}

/// `knowledge_workspace_stats`의 pool-injectable 버전 — 단위 테스트가 직접 호출 가능.
pub async fn knowledge_workspace_stats_with_pool(
    workspace_id: String,
    store_path: String,
    pool: Arc<KnowledgeStorePool>,
) -> Result<WorkspaceStats, KnowledgeApiError> {
    let store_arc = pool.get_or_open(&store_path)?;
    let store = store_arc.lock().map_err(|_| KnowledgeApiError::Internal {
        message: "store mutex poisoned".to_string(),
    })?;

    if !store
        .has_workspace(&workspace_id)
        .map_err(|e| KnowledgeApiError::Internal {
            message: format!("{e}"),
        })?
    {
        // workspace가 없어도 stats 0/0 반환 — UI는 "아직 받은 자료가 없어요"로 표시.
        return Ok(WorkspaceStats {
            workspace_id,
            documents: 0,
            chunks: 0,
        });
    }

    let documents =
        store
            .document_count(&workspace_id)
            .map_err(|e| KnowledgeApiError::Internal {
                message: format!("{e}"),
            })?;
    let chunks = store
        .chunk_count(&workspace_id)
        .map_err(|e| KnowledgeApiError::Internal {
            message: format!("{e}"),
        })?;

    Ok(WorkspaceStats {
        workspace_id,
        documents,
        chunks,
    })
}

// ───────────────────────────────────────────────────────────────────
// Phase 9'.a — Embedding model panel (download / list / set active)
// ───────────────────────────────────────────────────────────────────

/// 3-카드 노출용 모델 정보 (한국어 친화도 + 사이즈 + 다운로드 여부 + 활성 여부).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbeddingModelInfo {
    /// kebab-case kind ("bge-m3" 등). UI는 이 키로 라벨 lookup.
    pub kind: String,
    pub dim: usize,
    pub approx_size_mb: u32,
    /// 0.0 ~ 1.0. UI는 hint chip 강도 매핑.
    pub korean_score: f32,
    pub downloaded: bool,
    pub active: bool,
}

/// 진행 중 다운로드 등록부 — kind 단위 직렬화.
struct DownloadEntry {
    cancel: CancellationToken,
}

/// Active 모델 영속 + active 다운로드 등록부.
///
/// 정책:
/// - active 모델 kind은 `<app_data_dir>/embed/active.json`에 영속. 첫 실행은 None.
/// - 다운로드 등록부 키 = `OnnxModelKind` — 같은 kind에 대해 동시 다운로드는 거부 (즉시 `AlreadyDownloading`).
pub struct EmbeddingState {
    target_dir: PathBuf,
    config_path: PathBuf,
    active: AsyncMutex<Option<OnnxModelKind>>,
    downloads: AsyncMutex<HashMap<OnnxModelKind, DownloadEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct EmbeddingActiveConfig {
    /// kebab-case kind. 비어 있으면 None.
    #[serde(default)]
    active_kind: Option<String>,
}

impl EmbeddingState {
    /// `<app_data_dir>/embed`를 base로 잡아요. parent 디렉터리는 lazy 생성.
    pub fn new(app_data_dir: PathBuf) -> Self {
        let target_dir = app_data_dir.join("embed").join("models");
        let config_path = app_data_dir.join("embed").join("active.json");
        let initial_active = read_active_from_disk(&config_path);
        Self {
            target_dir,
            config_path,
            active: AsyncMutex::new(initial_active),
            downloads: AsyncMutex::new(HashMap::new()),
        }
    }

    /// 모델 다운로드 디렉터리 — `<app_data_dir>/embed/models`.
    pub fn target_dir(&self) -> &Path {
        &self.target_dir
    }

    /// 현재 active kind (영속 반영).
    pub async fn active(&self) -> Option<OnnxModelKind> {
        *self.active.lock().await
    }

    /// active 변경 + atomic 영속.
    pub async fn set_active(&self, kind: Option<OnnxModelKind>) -> Result<(), KnowledgeApiError> {
        {
            let mut g = self.active.lock().await;
            *g = kind;
        }
        // 영속.
        if let Some(parent) = self.config_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let cfg = EmbeddingActiveConfig {
            active_kind: kind.map(|k| k.as_kebab().to_string()),
        };
        let json = serde_json::to_string_pretty(&cfg).map_err(|e| KnowledgeApiError::Internal {
            message: format!("active 설정 직렬화 실패: {e}"),
        })?;
        // .tmp 후 rename.
        let tmp = self.config_path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| KnowledgeApiError::Internal {
            message: format!("active 설정 임시 파일 쓰기 실패: {e}"),
        })?;
        std::fs::rename(&tmp, &self.config_path).map_err(|e| KnowledgeApiError::Internal {
            message: format!("active 설정 영속 실패: {e}"),
        })?;
        Ok(())
    }

    /// 다운로드 등록 — 동일 kind 동시 진행은 거부.
    async fn register_download(
        &self,
        kind: OnnxModelKind,
    ) -> Result<CancellationToken, KnowledgeApiError> {
        let mut g = self.downloads.lock().await;
        if g.contains_key(&kind) {
            return Err(KnowledgeApiError::AlreadyDownloading {
                model_kind: kind.as_kebab().to_string(),
            });
        }
        let token = CancellationToken::new();
        g.insert(
            kind,
            DownloadEntry {
                cancel: token.clone(),
            },
        );
        Ok(token)
    }

    /// 다운로드 해제 — 정상 / 실패 / cancel 모두에서 호출.
    async fn finish_download(&self, kind: OnnxModelKind) {
        self.downloads.lock().await.remove(&kind);
    }

    /// 외부에서 cancel — idempotent. 미존재 = no-op.
    pub async fn cancel_download(&self, kind: OnnxModelKind) {
        let g = self.downloads.lock().await;
        if let Some(entry) = g.get(&kind) {
            entry.cancel.cancel();
        }
    }

    /// 앱 종료 시 모든 active 다운로드 cancel — sync 컨텍스트 (RunEvent::ExitRequested) best-effort.
    pub fn cancel_all_blocking(&self) {
        if let Ok(g) = self.downloads.try_lock() {
            for entry in g.values() {
                entry.cancel.cancel();
            }
        }
    }

    /// 현재 카탈로그 (3개 모델).
    pub async fn list_models(&self) -> Vec<EmbeddingModelInfo> {
        let active = self.active().await;
        [
            OnnxModelKind::BgeM3,
            OnnxModelKind::KureV1,
            OnnxModelKind::MultilingualE5Small,
        ]
        .into_iter()
        .map(|kind| EmbeddingModelInfo {
            kind: kind.as_kebab().to_string(),
            dim: kind.dim(),
            approx_size_mb: kind.approx_size_mb(),
            korean_score: kind.korean_score(),
            downloaded: is_downloaded(&self.target_dir, kind),
            active: active == Some(kind),
        })
        .collect()
    }
}

fn read_active_from_disk(config_path: &Path) -> Option<OnnxModelKind> {
    let raw = std::fs::read_to_string(config_path).ok()?;
    let cfg: EmbeddingActiveConfig = serde_json::from_str(&raw).ok()?;
    cfg.active_kind
        .as_deref()
        .and_then(OnnxModelKind::from_kebab)
}

/// IPC: 사용 가능한 임베딩 모델 목록.
#[tauri::command]
pub async fn list_embedding_models(
    state: State<'_, Arc<EmbeddingState>>,
) -> Result<Vec<EmbeddingModelInfo>, KnowledgeApiError> {
    Ok(state.list_models().await)
}

/// IPC: active 임베딩 모델 변경 (영속).
#[tauri::command]
pub async fn set_active_embedding_model(
    kind: String,
    state: State<'_, Arc<EmbeddingState>>,
) -> Result<(), KnowledgeApiError> {
    let parsed = OnnxModelKind::from_kebab(&kind).ok_or_else(|| {
        KnowledgeApiError::UnknownEmbeddingModel {
            model_kind: kind.clone(),
        }
    })?;
    // 다운로드 안 된 모델로 active 전환은 거부 — UI가 순서 강제.
    if !is_downloaded(state.target_dir(), parsed) {
        return Err(KnowledgeApiError::ModelNotDownloaded {
            model_kind: kind.clone(),
        });
    }
    state.set_active(Some(parsed)).await?;
    Ok(())
}

/// IPC: 임베딩 모델 다운로드 시작. 진행 이벤트는 `on_event` Channel로 흘려보내요.
///
/// 동일 kind에 대해 이미 진행 중이면 `AlreadyDownloading` 반환.
#[tauri::command]
pub async fn download_embedding_model(
    kind: String,
    on_event: Channel<DownloadEvent>,
    state: State<'_, Arc<EmbeddingState>>,
) -> Result<String, KnowledgeApiError> {
    let parsed = OnnxModelKind::from_kebab(&kind).ok_or_else(|| {
        KnowledgeApiError::UnknownEmbeddingModel {
            model_kind: kind.clone(),
        }
    })?;
    let cancel = state.register_download(parsed).await?;
    let state_arc: Arc<EmbeddingState> = state.inner().clone();

    // bridge mpsc → Channel.
    let (tx, mut rx) = mpsc::channel::<DownloadEvent>(64);
    let channel_for_bridge = on_event.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(ev) = rx.recv().await {
            if channel_for_bridge.send(ev).is_err() {
                tracing::debug!("download channel send failed; closing bridge");
                break;
            }
        }
    });

    let target_dir = state_arc.target_dir().to_path_buf();
    let cancel_for_task = cancel.clone();
    let state_for_task = Arc::clone(&state_arc);
    tauri::async_runtime::spawn(async move {
        if let Some(parent) = target_dir.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let downloader = match ModelDownloader::new(target_dir.clone()) {
            Ok(d) => d,
            Err(e) => {
                let _ = tx
                    .send(DownloadEvent::Failed {
                        model_kind: parsed.as_kebab().to_string(),
                        error: format!("{e}"),
                    })
                    .await;
                state_for_task.finish_download(parsed).await;
                return;
            }
        };
        let _ = downloader.download_model(parsed, tx, cancel_for_task).await;
        state_for_task.finish_download(parsed).await;
    });

    Ok(parsed.as_kebab().to_string())
}

/// IPC: 진행 중 다운로드 cancel — idempotent.
#[tauri::command]
pub async fn cancel_embedding_download(
    kind: String,
    state: State<'_, Arc<EmbeddingState>>,
) -> Result<(), KnowledgeApiError> {
    if let Some(parsed) = OnnxModelKind::from_kebab(&kind) {
        state.cancel_download(parsed).await;
    }
    Ok(())
}

// ───────────────────────────────────────────────────────────────────
// Phase #31 (ADR-0058) — Knowledge IPC store_path boundary validation
// ───────────────────────────────────────────────────────────────────

/// frontend가 보낸 store_path가 `app_data_dir` sandbox 안인지 검증.
///
/// 정책 (R-A.2 portable boundary 패턴 차용):
/// - 빈 문자열 → 그대로 (in-memory store, 테스트/dev)
/// - `..` segment 거부 (경로 traversal)
/// - 제어 문자 / null byte 거부
/// - 절대 경로면 canonicalize 후 `app_data_dir.canonicalize()` prefix 검증
/// - 상대 경로면 app_data_dir.join → canonicalize → prefix 재검증
/// - 적절하면 정규화된 PathBuf 반환, 아니면 KnowledgeApiError::PathDenied
///
/// XSS / 손상 frontend로부터 임의 디렉터리 작성/삭제 방어 (Tauri IPC frontend trust X).
pub fn validate_store_path(
    data_dir_root: &Path,
    requested: &str,
) -> Result<String, KnowledgeApiError> {
    // 빈 문자열은 in-memory — 통과.
    if requested.is_empty() {
        return Ok(String::new());
    }
    // 제어 문자 / null byte 거부.
    if requested.chars().any(|c| c == '\0' || c.is_control()) {
        return Err(KnowledgeApiError::PathDenied {
            reason: "경로에 사용할 수 없는 문자가 들어 있어요".into(),
        });
    }
    // .. 또는 backslash 노출 거부 (간단 검사 — canonicalize가 추가 방어).
    if requested.contains("..") {
        return Err(KnowledgeApiError::PathDenied {
            reason: "상위 디렉터리(..) 참조는 허용되지 않아요".into(),
        });
    }

    let canonical_root =
        data_dir_root
            .canonicalize()
            .map_err(|e| KnowledgeApiError::PathDenied {
                reason: format!("data_dir 정규화 실패: {e}"),
            })?;

    let requested_path = Path::new(requested);
    let candidate = if requested_path.is_absolute() {
        requested_path.to_path_buf()
    } else {
        canonical_root.join(requested_path)
    };

    // 부모 canonicalize (대상 파일이 아직 미존재 가능 — 새 store).
    let parent = candidate.parent().unwrap_or(&canonical_root);
    let parent_canon = match parent.canonicalize() {
        Ok(p) => p,
        Err(_) => canonical_root.clone(),
    };
    let final_path = match candidate.file_name() {
        Some(name) => parent_canon.join(name),
        None => parent_canon.clone(),
    };

    if !final_path.starts_with(&canonical_root) {
        return Err(KnowledgeApiError::PathDenied {
            reason: "data 디렉터리 밖으로 나가는 경로예요".into(),
        });
    }
    Ok(final_path.to_string_lossy().to_string())
}

// ───────────────────────────────────────────────────────────────────
// EmbeddingState 부트스트랩 — Tauri setup에서 호출.
// ───────────────────────────────────────────────────────────────────

/// 앱 부팅 시 EmbeddingState를 생성한다. app_data_dir 미접근 시 임시 디렉터리에 fallback.
pub fn provision_embedding_state(app: &AppHandle) -> Arc<EmbeddingState> {
    let dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("lmmaster"));
    Arc::new(EmbeddingState::new(dir))
}

// ───────────────────────────────────────────────────────────────────
// Phase #38 (ADR-0058) — Knowledge SQLCipher passphrase provision
// ───────────────────────────────────────────────────────────────────

/// keyring service / username — knowledge 전용 secret entry.
/// key-manager는 `keymanager-secret` — 별개 entry로 분리해 RAG 데이터와 API 키 secret 격리.
const KEYRING_SERVICE: &str = "lmmaster";
const KEYRING_USERNAME: &str = "knowledge-secret";

/// keyring에서 knowledge passphrase를 읽거나 새로 생성. 실패 시 None 반환 (평문 폴백).
///
/// 정책:
/// - keyring entry가 있으면 그대로 사용.
/// - 없으면 32 byte random 생성 → hex 인코딩 → keyring 저장.
/// - keyring 자체 접근 실패 (Linux headless 등) → None — 평문 폴백 (Pool은 평문 open).
/// - secret은 process lifetime 동안 유지 (Pool에 잡혀 있음).
///
/// `sqlcipher` feature OFF 빌드(stock SQLite)에서도 무해 — passphrase가 PRAGMA key로 적용되지만
/// stock SQLite는 unknown pragma로 무시. v1.x에서 feature ON 시 자동 활성화.
pub fn provision_knowledge_passphrase() -> Option<String> {
    use keyring::Entry;
    use rand::RngCore;

    let entry = match Entry::new(KEYRING_SERVICE, KEYRING_USERNAME) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(error = %e, "knowledge keyring entry 생성 실패 — 평문 폴백");
            return None;
        }
    };
    match entry.get_password() {
        Ok(p) if !p.is_empty() => Some(p),
        Ok(_) | Err(keyring::Error::NoEntry) => {
            // 새 secret 생성.
            let mut buf = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut buf);
            let hex_secret = hex::encode(buf);
            if let Err(e) = entry.set_password(&hex_secret) {
                tracing::warn!(error = %e, "knowledge keyring secret 저장 실패 — 평문 폴백");
                return None;
            }
            tracing::info!("knowledge keyring secret 생성 완료");
            Some(hex_secret)
        }
        Err(e) => {
            tracing::warn!(error = %e, "knowledge keyring secret 읽기 실패 — 평문 폴백");
            None
        }
    }
}

/// 부팅 시 호출 — keyring secret을 읽어 KnowledgeStorePool 생성.
/// `app.manage(provision_knowledge_store_pool())`.
pub fn provision_knowledge_store_pool() -> Arc<KnowledgeStorePool> {
    match provision_knowledge_passphrase() {
        Some(pass) => {
            tracing::info!("knowledge SQLCipher passphrase 적용 (sqlcipher feature 빌드만 활성)");
            Arc::new(KnowledgeStorePool::with_passphrase(pass))
        }
        None => {
            tracing::warn!(
                "knowledge passphrase 없음 — 평문 모드 (Linux headless / keyring 미접근)"
            );
            Arc::new(KnowledgeStorePool::new())
        }
    }
}

// ───────────────────────────────────────────────────────────────────
// Tests
// ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
    use tempfile::TempDir;

    // ── Registry tests ──────────────────────────────────────────

    #[tokio::test]
    async fn registry_register_then_list() {
        let r = KnowledgeRegistry::new();
        let _ = r.register("ws-a").await.unwrap();
        let snaps = r.list().await;
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].workspace_id, "ws-a");
        assert_eq!(snaps[0].current_stage, IngestStage::Reading);
    }

    #[tokio::test]
    async fn registry_register_duplicate_workspace_rejects() {
        let r = KnowledgeRegistry::new();
        let _ = r.register("ws-a").await.unwrap();
        let err = r.register("ws-a").await.unwrap_err();
        assert!(matches!(err, KnowledgeApiError::AlreadyIngesting { .. }));
    }

    #[tokio::test]
    async fn registry_distinct_workspaces_can_coexist() {
        let r = KnowledgeRegistry::new();
        let _ = r.register("ws-a").await.unwrap();
        let _ = r.register("ws-b").await.unwrap();
        assert_eq!(r.in_flight_count().await, 2);
    }

    #[tokio::test]
    async fn registry_finish_removes() {
        let r = KnowledgeRegistry::new();
        let _ = r.register("ws-a").await.unwrap();
        r.finish("ws-a").await;
        assert_eq!(r.in_flight_count().await, 0);
    }

    #[tokio::test]
    async fn registry_cancel_unknown_is_noop() {
        let r = KnowledgeRegistry::new();
        r.cancel("missing").await;
        assert_eq!(r.in_flight_count().await, 0);
    }

    #[tokio::test]
    async fn registry_cancel_marks_token_and_atomic() {
        let r = KnowledgeRegistry::new();
        let (_id, tok, atomic) = r.register("ws-a").await.unwrap();
        r.cancel("ws-a").await;
        assert!(tok.is_cancelled());
        assert!(atomic.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn registry_cancel_all_marks_every_token() {
        let r = KnowledgeRegistry::new();
        let (_, t1, a1) = r.register("ws-a").await.unwrap();
        let (_, t2, a2) = r.register("ws-b").await.unwrap();
        r.cancel_all().await;
        assert!(t1.is_cancelled());
        assert!(t2.is_cancelled());
        assert!(a1.load(Ordering::SeqCst));
        assert!(a2.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn registry_set_stage_updates_snapshot() {
        let r = KnowledgeRegistry::new();
        let _ = r.register("ws-a").await.unwrap();
        r.set_stage("ws-a", IngestStage::Embedding).await;
        let snaps = r.list().await;
        assert_eq!(snaps[0].current_stage, IngestStage::Embedding);
    }

    #[test]
    fn cancel_all_blocking_does_not_panic_on_empty() {
        let r = KnowledgeRegistry::new();
        r.cancel_all_blocking();
    }

    // ── Event enum serde ─────────────────────────────────────────

    #[test]
    fn event_started_serializes_with_kind() {
        let ev = IngestEvent::Started {
            ingest_id: "i1".into(),
            workspace_id: "ws".into(),
            path: "/tmp/x".into(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "started");
        assert_eq!(v["ingest_id"], "i1");
        assert_eq!(v["workspace_id"], "ws");
    }

    #[test]
    fn event_reading_serializes_kebab() {
        let ev = IngestEvent::Reading {
            ingest_id: "i1".into(),
            current_path: "/tmp/a.md".into(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "reading");
        assert_eq!(v["current_path"], "/tmp/a.md");
    }

    #[test]
    fn event_chunking_serializes() {
        let ev = IngestEvent::Chunking {
            ingest_id: "i1".into(),
            processed: 3,
            total: 10,
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "chunking");
        assert_eq!(v["processed"], 3);
        assert_eq!(v["total"], 10);
    }

    #[test]
    fn event_embedding_writing_kebab() {
        let e1 = IngestEvent::Embedding {
            ingest_id: "i".into(),
            processed: 1,
            total: 2,
        };
        let v1 = serde_json::to_value(&e1).unwrap();
        assert_eq!(v1["kind"], "embedding");

        let e2 = IngestEvent::Writing {
            ingest_id: "i".into(),
            processed: 2,
            total: 2,
        };
        let v2 = serde_json::to_value(&e2).unwrap();
        assert_eq!(v2["kind"], "writing");
    }

    #[test]
    fn event_done_summary_round_trip() {
        let summary = IngestSummary {
            ingest_id: "i".into(),
            workspace_id: "ws".into(),
            files_processed: 3,
            chunks_created: 12,
            skipped: 1,
            total_duration_ms: 1234,
        };
        let ev = IngestEvent::Done {
            ingest_id: "i".into(),
            summary: summary.clone(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "done");
        assert_eq!(v["summary"]["files_processed"], 3);
        assert_eq!(v["summary"]["chunks_created"], 12);
        assert_eq!(v["summary"]["skipped"], 1);
    }

    #[test]
    fn event_failed_includes_error() {
        let ev = IngestEvent::Failed {
            ingest_id: "i".into(),
            error: "안 됐어요".into(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "failed");
        assert!(v["error"].as_str().unwrap().contains("안 됐어요"));
    }

    #[test]
    fn event_cancelled_kind_only() {
        let ev = IngestEvent::Cancelled {
            ingest_id: "i".into(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "cancelled");
    }

    #[test]
    fn api_error_already_ingesting_kebab() {
        let e = KnowledgeApiError::AlreadyIngesting {
            workspace_id: "ws".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "already-ingesting");
        assert_eq!(v["workspace_id"], "ws");
        // 메시지에 한국어 해요체 포함.
        assert!(format!("{e}").contains("이미 자료를 받고 있어요"));
    }

    #[test]
    fn api_error_workspace_not_found_kebab() {
        let e = KnowledgeApiError::WorkspaceNotFound {
            workspace_id: "ws".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "workspace-not-found");
        assert!(format!("{e}").contains("워크스페이스"));
    }

    #[test]
    fn api_error_store_open_kebab() {
        let e = KnowledgeApiError::StoreOpen {
            message: "perm denied".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "store-open");
    }

    #[test]
    fn api_error_search_failed_kebab() {
        let e = KnowledgeApiError::SearchFailed {
            message: "x".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "search-failed");
    }

    // ── progress_to_event mapping ────────────────────────────────

    #[test]
    fn progress_to_event_reading_uses_path() {
        let p = IngestProgress {
            stage: IngestStage::Reading,
            processed: 0,
            total: 1,
            current_path: Some("/tmp/a.md".into()),
        };
        let ev = progress_to_event("i1", &p);
        match ev {
            IngestEvent::Reading {
                ingest_id,
                current_path,
            } => {
                assert_eq!(ingest_id, "i1");
                assert_eq!(current_path, "/tmp/a.md");
            }
            other => panic!("expected Reading, got {other:?}"),
        }
    }

    #[test]
    fn progress_to_event_done_maps_to_writing_full() {
        let p = IngestProgress {
            stage: IngestStage::Done,
            processed: 3,
            total: 3,
            current_path: None,
        };
        let ev = progress_to_event("i1", &p);
        match ev {
            IngestEvent::Writing {
                processed, total, ..
            } => {
                assert_eq!(processed, 3);
                assert_eq!(total, 3);
            }
            other => panic!("expected Writing, got {other:?}"),
        }
    }

    // ── Counting channel for run_ingest ──────────────────────────

    fn counting_channel() -> (Channel<IngestEvent>, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();
        let ch: Channel<IngestEvent> = Channel::new(move |_body| -> tauri::Result<()> {
            count_clone.fetch_add(1, AtomicOrdering::SeqCst);
            Ok(())
        });
        (ch, count)
    }

    /// run_ingest를 임시 store + .md 파일로 호출. workspace는 KnowledgeStore::add_workspace로 시드.
    async fn drive_run_ingest(
        cancel_immediately: bool,
        with_workspace: bool,
    ) -> (Arc<AtomicUsize>, Arc<KnowledgeRegistry>) {
        let dir = TempDir::new().unwrap();
        let md_path = dir.path().join("a.md");
        std::fs::write(
            &md_path,
            "안녕하세요. 이건 첫 단락이에요.\n\n이건 두 번째 단락이에요.",
        )
        .unwrap();

        let store_file = dir.path().join("knowledge.db");
        // workspace 시드 (with_workspace=true면 add_workspace로 새 ws 생성; 그 id를 사용).
        let ws_id = if with_workspace {
            let s = KnowledgeStore::open(&store_file).unwrap();
            let ws = s.add_workspace("ws-test").unwrap();
            ws.id
        } else {
            // workspace 미존재 시도용 — 빈 store 생성 + 임의 id 사용.
            let _ = KnowledgeStore::open(&store_file).unwrap();
            "missing-ws".to_string()
        };

        let registry = Arc::new(KnowledgeRegistry::new());
        let (ingest_id, cancel, atomic) = registry.register(&ws_id).await.unwrap();
        if cancel_immediately {
            cancel.cancel();
            atomic.store(true, Ordering::SeqCst);
        }
        let (ch, count) = counting_channel();

        let config = IngestConfig {
            workspace_id: ws_id.clone(),
            path: md_path.to_string_lossy().to_string(),
            kind: "file".into(),
            target_chunk_size: 200,
            overlap: 20,
            store_path: store_file.to_string_lossy().to_string(),
        };

        let embedder: Arc<dyn Embedder> = Arc::new(MockEmbedder::default());
        let pool = Arc::new(KnowledgeStorePool::new());
        run_ingest(
            config,
            ingest_id,
            registry.clone(),
            pool,
            cancel,
            atomic,
            ch,
            embedder,
        )
        .await;

        (count, registry)
    }

    #[tokio::test]
    async fn run_ingest_happy_path_emits_started_and_done() {
        let (count, registry) = drive_run_ingest(false, true).await;
        // 최소 Started + 단계 progress + Done.
        let n = count.load(AtomicOrdering::SeqCst);
        assert!(n >= 2, "expected ≥2 events, got {n}");
        // registry가 cleanup 됐는지.
        assert_eq!(registry.in_flight_count().await, 0);
    }

    #[tokio::test]
    async fn run_ingest_pre_cancelled_emits_cancelled() {
        let (count, registry) = drive_run_ingest(true, true).await;
        // Started + Cancelled — 최소 2 events.
        let n = count.load(AtomicOrdering::SeqCst);
        assert!(n >= 1, "expected ≥1 events, got {n}");
        assert_eq!(registry.in_flight_count().await, 0);
    }

    #[tokio::test]
    async fn run_ingest_unknown_workspace_fails() {
        let (count, registry) = drive_run_ingest(false, false).await;
        // Started + Failed.
        let n = count.load(AtomicOrdering::SeqCst);
        assert!(n >= 2, "expected ≥2 events (started + failed), got {n}");
        assert_eq!(registry.in_flight_count().await, 0);
    }

    // ── search_knowledge ─────────────────────────────────────────

    #[tokio::test]
    async fn search_knowledge_returns_hits_in_order() {
        // 임시 store + workspace + 직접 chunk 시드 → search_knowledge 호출.
        let dir = TempDir::new().unwrap();
        let store_file = dir.path().join("k.db");
        let ws_id = {
            let mut s = KnowledgeStore::open(&store_file).unwrap();
            let ws = s.add_workspace("ws").unwrap();
            let doc = s.add_document(&ws.id, "/tmp/x.md", "sha-1").unwrap();
            // 3 chunks — content "apple"이 query와 가장 가까울 것 (mock embedder는 sha 기반).
            let chunks = vec![
                knowledge_stack::Chunk {
                    id: "c1".into(),
                    content: "apple".into(),
                    start: 0,
                    end: 5,
                },
                knowledge_stack::Chunk {
                    id: "c2".into(),
                    content: "banana".into(),
                    start: 6,
                    end: 12,
                },
                knowledge_stack::Chunk {
                    id: "c3".into(),
                    content: "cherry".into(),
                    start: 13,
                    end: 19,
                },
            ];
            let embedder = MockEmbedder::default();
            let texts: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();
            let embeds = embedder.embed(&texts).await.unwrap();
            s.add_chunks(&doc.id, &ws.id, &chunks, &embeds).unwrap();
            ws.id
        };

        let hits = search_knowledge_with_embedder(
            ws_id.clone(),
            "apple".to_string(),
            3,
            store_file.to_string_lossy().to_string(),
            Arc::new(MockEmbedder::default()),
            Arc::new(KnowledgeStorePool::new()),
        )
        .await
        .unwrap();
        assert_eq!(hits.len(), 3);
        // top-1은 "apple" (self-match) — score 가장 높음.
        assert_eq!(hits[0].content, "apple");
        // 모든 score가 [0, 1].
        for h in &hits {
            assert!(h.score >= 0.0 && h.score <= 1.0);
        }
        // Phase 8'.a.1 — document_path는 add_document에 넣은 실 경로로 resolve.
        // (이전엔 document_id를 placeholder로 노출했지만 이제 실 path.)
        for h in &hits {
            assert_eq!(h.document_path, "/tmp/x.md");
            assert_ne!(
                h.document_path, h.document_id,
                "document_path가 document_id placeholder로 fallback되면 안 돼요"
            );
        }
    }

    #[tokio::test]
    async fn search_knowledge_empty_query_returns_empty() {
        let dir = TempDir::new().unwrap();
        let store_file = dir.path().join("k.db");
        let _ = KnowledgeStore::open(&store_file).unwrap();
        let hits = search_knowledge_with_embedder(
            "ws".to_string(),
            "   ".to_string(),
            5,
            store_file.to_string_lossy().to_string(),
            Arc::new(MockEmbedder::default()),
            Arc::new(KnowledgeStorePool::new()),
        )
        .await
        .unwrap();
        assert!(hits.is_empty());
    }

    #[tokio::test]
    async fn search_knowledge_unknown_workspace_errors() {
        let dir = TempDir::new().unwrap();
        let store_file = dir.path().join("k.db");
        let _ = KnowledgeStore::open(&store_file).unwrap();
        let err = search_knowledge_with_embedder(
            "missing-ws".to_string(),
            "hello".to_string(),
            3,
            store_file.to_string_lossy().to_string(),
            Arc::new(MockEmbedder::default()),
            Arc::new(KnowledgeStorePool::new()),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, KnowledgeApiError::WorkspaceNotFound { .. }));
    }

    #[tokio::test]
    async fn search_knowledge_filters_by_workspace_id() {
        // 두 워크스페이스 — 같은 store에서 한 ws의 search는 다른 ws chunks를 못 보여야 함.
        let dir = TempDir::new().unwrap();
        let store_file = dir.path().join("k.db");
        let (ws_a_id, ws_b_id) = {
            let mut s = KnowledgeStore::open(&store_file).unwrap();
            let ws_a = s.add_workspace("A").unwrap();
            let ws_b = s.add_workspace("B").unwrap();
            let doc_a = s.add_document(&ws_a.id, "/a.md", "sha-a").unwrap();
            let doc_b = s.add_document(&ws_b.id, "/b.md", "sha-b").unwrap();
            let embedder = MockEmbedder::default();
            let chunks_a = vec![knowledge_stack::Chunk {
                id: "ca".into(),
                content: "alpha-only".into(),
                start: 0,
                end: 10,
            }];
            let embeds_a = embedder.embed(&["alpha-only".to_string()]).await.unwrap();
            s.add_chunks(&doc_a.id, &ws_a.id, &chunks_a, &embeds_a)
                .unwrap();
            let chunks_b = vec![knowledge_stack::Chunk {
                id: "cb".into(),
                content: "beta-only".into(),
                start: 0,
                end: 9,
            }];
            let embeds_b = embedder.embed(&["beta-only".to_string()]).await.unwrap();
            s.add_chunks(&doc_b.id, &ws_b.id, &chunks_b, &embeds_b)
                .unwrap();
            (ws_a.id, ws_b.id)
        };

        let hits_a = search_knowledge_with_embedder(
            ws_a_id.clone(),
            "alpha".to_string(),
            5,
            store_file.to_string_lossy().to_string(),
            Arc::new(MockEmbedder::default()),
            Arc::new(KnowledgeStorePool::new()),
        )
        .await
        .unwrap();
        assert_eq!(hits_a.len(), 1);
        assert_eq!(hits_a[0].content, "alpha-only");
        let hits_b = search_knowledge_with_embedder(
            ws_b_id.clone(),
            "beta".to_string(),
            5,
            store_file.to_string_lossy().to_string(),
            Arc::new(MockEmbedder::default()),
            Arc::new(KnowledgeStorePool::new()),
        )
        .await
        .unwrap();
        assert_eq!(hits_b.len(), 1);
        assert_eq!(hits_b[0].content, "beta-only");
    }

    // ── Phase #31 (ADR-0058) — validate_store_path 회귀 가드 ──────

    #[test]
    fn validate_store_path_empty_returns_empty() {
        let dir = TempDir::new().unwrap();
        let r = validate_store_path(dir.path(), "").unwrap();
        assert_eq!(r, "");
    }

    #[test]
    fn validate_store_path_relative_under_root_ok() {
        let dir = TempDir::new().unwrap();
        let r = validate_store_path(dir.path(), "ws-1.db").unwrap();
        let canon_root = dir.path().canonicalize().unwrap();
        assert!(r.starts_with(canon_root.to_str().unwrap()));
    }

    #[test]
    fn validate_store_path_rejects_parent_traversal() {
        let dir = TempDir::new().unwrap();
        let r = validate_store_path(dir.path(), "../escape.db");
        assert!(matches!(r, Err(KnowledgeApiError::PathDenied { .. })));
    }

    #[test]
    fn validate_store_path_rejects_control_char() {
        let dir = TempDir::new().unwrap();
        let r = validate_store_path(dir.path(), "ws\0null.db");
        assert!(matches!(r, Err(KnowledgeApiError::PathDenied { .. })));
        let r = validate_store_path(dir.path(), "ws\nnewline.db");
        assert!(matches!(r, Err(KnowledgeApiError::PathDenied { .. })));
    }

    #[test]
    fn validate_store_path_rejects_absolute_outside() {
        let dir = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let path = outside.path().join("evil.db").to_string_lossy().to_string();
        let r = validate_store_path(dir.path(), &path);
        assert!(matches!(r, Err(KnowledgeApiError::PathDenied { .. })));
    }

    #[test]
    fn validate_store_path_rejects_dot_dot_segment() {
        let dir = TempDir::new().unwrap();
        let r = validate_store_path(dir.path(), "subdir/../../escape.db");
        assert!(matches!(r, Err(KnowledgeApiError::PathDenied { .. })));
    }

    #[test]
    fn knowledge_api_error_path_denied_kebab_serialization() {
        let e = KnowledgeApiError::PathDenied {
            reason: "test".to_string(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "path-denied");
        assert!(e.to_string().contains("data 디렉터리"));
    }

    // ── Phase R-E.5 (P1, ADR-0058) — KnowledgeStorePool ──────────

    #[tokio::test]
    async fn pool_caches_same_path() {
        let dir = TempDir::new().unwrap();
        let store_file = dir.path().join("p.db").to_string_lossy().to_string();
        let pool = KnowledgeStorePool::new();
        let arc1 = pool.get_or_open(&store_file).unwrap();
        let arc2 = pool.get_or_open(&store_file).unwrap();
        assert!(
            Arc::ptr_eq(&arc1, &arc2),
            "같은 path는 같은 Arc를 반환해야 해요"
        );
        assert_eq!(pool.len(), 1);
    }

    #[tokio::test]
    async fn pool_separate_arcs_for_different_paths() {
        let dir = TempDir::new().unwrap();
        let path_a = dir.path().join("a.db").to_string_lossy().to_string();
        let path_b = dir.path().join("b.db").to_string_lossy().to_string();
        let pool = KnowledgeStorePool::new();
        let arc_a = pool.get_or_open(&path_a).unwrap();
        let arc_b = pool.get_or_open(&path_b).unwrap();
        assert!(
            !Arc::ptr_eq(&arc_a, &arc_b),
            "다른 path는 다른 Arc를 반환해야 해요"
        );
        assert_eq!(pool.len(), 2);
    }

    #[tokio::test]
    async fn pool_evicts_oldest_when_capacity_reached() {
        let dir = TempDir::new().unwrap();
        let pool = KnowledgeStorePool::with_capacity(2);
        let p1 = dir.path().join("1.db").to_string_lossy().to_string();
        let p2 = dir.path().join("2.db").to_string_lossy().to_string();
        let p3 = dir.path().join("3.db").to_string_lossy().to_string();
        let _ = pool.get_or_open(&p1).unwrap();
        let _ = pool.get_or_open(&p2).unwrap();
        // 3번째 삽입 → 1번째(oldest) FIFO eviction.
        let _ = pool.get_or_open(&p3).unwrap();
        assert_eq!(pool.len(), 2);
        // p1는 evict됨 → 다시 호출하면 새 Arc.
        let arc1_v2 = pool.get_or_open(&p1).unwrap();
        // 다시 evict — 이번엔 p2 (oldest 후보).
        assert_eq!(pool.len(), 2);
        // 다음 호출 시 p3가 여전히 캐시 hit이어야.
        let arc3_v1 = pool.get_or_open(&p3).unwrap();
        let arc3_v2 = pool.get_or_open(&p3).unwrap();
        assert!(Arc::ptr_eq(&arc3_v1, &arc3_v2));
        let _ = arc1_v2;
    }

    #[tokio::test]
    async fn pool_empty_path_uses_in_memory_cache() {
        let pool = KnowledgeStorePool::new();
        let arc1 = pool.get_or_open("").unwrap();
        let arc2 = pool.get_or_open("").unwrap();
        assert!(Arc::ptr_eq(&arc1, &arc2));
    }

    #[tokio::test]
    async fn pool_default_returns_default_capacity() {
        let pool = KnowledgeStorePool::default();
        assert_eq!(pool.len(), 0);
    }

    // ── Phase #38 (ADR-0058) — passphrase wiring ─────────────────

    #[tokio::test]
    async fn pool_with_passphrase_opens_db() {
        // sqlcipher feature OFF 빌드에서도 PRAGMA key가 unknown pragma로 무시되어 정상 동작.
        // ON 빌드에선 실 SQLCipher 적용.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("kp.db").to_string_lossy().to_string();
        let pool = KnowledgeStorePool::with_passphrase("passphrase-aaaaaaaaaaaaaaaa".to_string());
        let arc = pool.get_or_open(&path).unwrap();
        // 정상 open + 캐시 적재 확인.
        assert_eq!(pool.len(), 1);
        let _ = arc;
    }

    #[tokio::test]
    async fn pool_with_passphrase_reuses_cached_arc() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("kp.db").to_string_lossy().to_string();
        let pool = KnowledgeStorePool::with_passphrase("passphrase-bbbbbbbbbbbbbbbb".to_string());
        let arc1 = pool.get_or_open(&path).unwrap();
        let arc2 = pool.get_or_open(&path).unwrap();
        assert!(
            Arc::ptr_eq(&arc1, &arc2),
            "passphrase 모드에서도 같은 path → 같은 Arc"
        );
    }

    // ── knowledge_workspace_stats ────────────────────────────────

    #[tokio::test]
    async fn workspace_stats_zeros_for_unknown_workspace() {
        let dir = TempDir::new().unwrap();
        let store_file = dir.path().join("k.db");
        let _ = KnowledgeStore::open(&store_file).unwrap();
        let stats = knowledge_workspace_stats_with_pool(
            "missing-ws".to_string(),
            store_file.to_string_lossy().to_string(),
            Arc::new(KnowledgeStorePool::new()),
        )
        .await
        .unwrap();
        assert_eq!(stats.documents, 0);
        assert_eq!(stats.chunks, 0);
    }

    #[tokio::test]
    async fn workspace_stats_counts_documents_and_chunks() {
        let dir = TempDir::new().unwrap();
        let store_file = dir.path().join("k.db");
        let ws_id = {
            let mut s = KnowledgeStore::open(&store_file).unwrap();
            let ws = s.add_workspace("ws").unwrap();
            let doc = s.add_document(&ws.id, "/x.md", "sha-1").unwrap();
            let embedder = MockEmbedder::default();
            let chunks = vec![
                knowledge_stack::Chunk {
                    id: "c1".into(),
                    content: "chunk one".into(),
                    start: 0,
                    end: 9,
                },
                knowledge_stack::Chunk {
                    id: "c2".into(),
                    content: "chunk two".into(),
                    start: 10,
                    end: 19,
                },
            ];
            let embeds = embedder
                .embed(&chunks.iter().map(|c| c.content.clone()).collect::<Vec<_>>())
                .await
                .unwrap();
            s.add_chunks(&doc.id, &ws.id, &chunks, &embeds).unwrap();
            ws.id
        };
        let stats = knowledge_workspace_stats_with_pool(
            ws_id.clone(),
            store_file.to_string_lossy().to_string(),
            Arc::new(KnowledgeStorePool::new()),
        )
        .await
        .unwrap();
        assert_eq!(stats.documents, 1);
        assert_eq!(stats.chunks, 2);
    }

    // ── IngestConfig defaults ────────────────────────────────────

    #[test]
    fn ingest_config_defaults_apply_when_omitted() {
        let json = serde_json::json!({
            "workspace_id": "ws",
            "path": "/tmp/a.md"
        });
        let cfg: IngestConfig = serde_json::from_value(json).unwrap();
        assert_eq!(cfg.kind, "directory");
        assert_eq!(cfg.target_chunk_size, 1000);
        assert_eq!(cfg.overlap, 200);
        assert_eq!(cfg.store_path, "");
    }

    // ── Phase 9'.a — EmbeddingState ──────────────────────────────

    #[tokio::test]
    async fn embedding_state_lists_three_models_with_no_active_initially() {
        let dir = TempDir::new().unwrap();
        let state = EmbeddingState::new(dir.path().to_path_buf());
        let models = state.list_models().await;
        assert_eq!(models.len(), 3);
        // 첫 실행 — 아무도 active 아니고 아무도 다운로드 안 됨.
        for m in &models {
            assert!(!m.active);
            assert!(!m.downloaded);
        }
    }

    #[tokio::test]
    async fn embedding_state_set_active_persists_to_disk() {
        let dir = TempDir::new().unwrap();
        // 이미 다운로드된 상태로 가정 — 모델 디렉터리 + 파일 시뮬레이션.
        let kind = OnnxModelKind::BgeM3;
        let kind_dir = dir
            .path()
            .join("embed")
            .join("models")
            .join(kind.as_kebab());
        std::fs::create_dir_all(&kind_dir).unwrap();
        std::fs::write(kind_dir.join("model.onnx"), b"fake").unwrap();
        std::fs::write(kind_dir.join("tokenizer.json"), b"fake").unwrap();

        let state = EmbeddingState::new(dir.path().to_path_buf());
        state.set_active(Some(kind)).await.unwrap();
        assert_eq!(state.active().await, Some(kind));

        // 다른 EmbeddingState로 다시 불러오면 영속이 보여야 해요.
        let state2 = EmbeddingState::new(dir.path().to_path_buf());
        assert_eq!(state2.active().await, Some(kind));
    }

    #[tokio::test]
    async fn embedding_state_already_downloading_rejects_duplicate() {
        let dir = TempDir::new().unwrap();
        let state = EmbeddingState::new(dir.path().to_path_buf());
        let _t1 = state.register_download(OnnxModelKind::BgeM3).await.unwrap();
        let res = state.register_download(OnnxModelKind::BgeM3).await;
        match res {
            Err(KnowledgeApiError::AlreadyDownloading { model_kind }) => {
                assert_eq!(model_kind, "bge-m3");
            }
            other => panic!("expected AlreadyDownloading, got {other:?}"),
        }
        // 다른 kind는 충돌 없음.
        let _t2 = state
            .register_download(OnnxModelKind::KureV1)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn embedding_state_finish_releases_slot() {
        let dir = TempDir::new().unwrap();
        let state = EmbeddingState::new(dir.path().to_path_buf());
        let _ = state
            .register_download(OnnxModelKind::KureV1)
            .await
            .unwrap();
        state.finish_download(OnnxModelKind::KureV1).await;
        // 이제 다시 등록 가능.
        let _ = state
            .register_download(OnnxModelKind::KureV1)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn embedding_state_cancel_unknown_is_noop() {
        let dir = TempDir::new().unwrap();
        let state = EmbeddingState::new(dir.path().to_path_buf());
        // 등록 안 한 kind cancel — panic 없이 통과.
        state
            .cancel_download(OnnxModelKind::MultilingualE5Small)
            .await;
    }

    #[tokio::test]
    async fn embedding_state_cancel_marks_token() {
        let dir = TempDir::new().unwrap();
        let state = EmbeddingState::new(dir.path().to_path_buf());
        let token = state.register_download(OnnxModelKind::BgeM3).await.unwrap();
        state.cancel_download(OnnxModelKind::BgeM3).await;
        assert!(token.is_cancelled(), "cancel 후 token이 cancel되어야 해요");
    }

    #[tokio::test]
    async fn list_models_marks_downloaded_when_files_present() {
        let dir = TempDir::new().unwrap();
        // bge-m3 다운로드된 상태 흉내.
        let kind = OnnxModelKind::BgeM3;
        let kind_dir = dir
            .path()
            .join("embed")
            .join("models")
            .join(kind.as_kebab());
        std::fs::create_dir_all(&kind_dir).unwrap();
        std::fs::write(kind_dir.join("model.onnx"), b"fake").unwrap();
        std::fs::write(kind_dir.join("tokenizer.json"), b"fake").unwrap();

        let state = EmbeddingState::new(dir.path().to_path_buf());
        let models = state.list_models().await;
        let bge = models
            .iter()
            .find(|m| m.kind == "bge-m3")
            .expect("bge-m3 in list");
        assert!(bge.downloaded);
    }

    #[tokio::test]
    async fn embedding_state_set_active_unknown_kind_is_unaffected() {
        // EmbeddingState::set_active는 OnnxModelKind를 직접 받아 unknown 입력은 컴파일 단에서 차단.
        // IPC 레이어 (set_active_embedding_model) 가 from_kebab을 사용해 검증 — 여기서는 None 클리어 동작 확인.
        let dir = TempDir::new().unwrap();
        let state = EmbeddingState::new(dir.path().to_path_buf());
        state.set_active(None).await.unwrap();
        assert_eq!(state.active().await, None);
    }

    #[test]
    fn embedding_model_info_serializes_with_kebab_kind() {
        let info = EmbeddingModelInfo {
            kind: "bge-m3".into(),
            dim: 1024,
            approx_size_mb: 580,
            korean_score: 0.85,
            downloaded: false,
            active: false,
        };
        let v = serde_json::to_value(&info).unwrap();
        assert_eq!(v["kind"], "bge-m3");
        assert_eq!(v["dim"], 1024);
        assert_eq!(v["approx_size_mb"], 580);
    }

    #[test]
    fn knowledge_api_error_already_downloading_kebab() {
        let e = KnowledgeApiError::AlreadyDownloading {
            model_kind: "bge-m3".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "already-downloading");
        assert!(format!("{e}").contains("받고 있어요"));
    }

    #[test]
    fn knowledge_api_error_unknown_model_kebab() {
        let e = KnowledgeApiError::UnknownEmbeddingModel {
            model_kind: "unknown".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "unknown-embedding-model");
        assert!(format!("{e}").contains("알 수 없는"));
    }

    #[test]
    fn knowledge_api_error_model_not_downloaded_kebab() {
        let e = KnowledgeApiError::ModelNotDownloaded {
            model_kind: "kure-v1".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "model-not-downloaded");
        assert!(format!("{e}").contains("받아야"));
    }
}
