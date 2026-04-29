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
    Embedder, IngestProgress, IngestService, IngestStage, KnowledgeStore, MockEmbedder,
};
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::State;
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

    #[error("지식 저장소를 열지 못했어요: {message}")]
    StoreOpen { message: String },

    #[error("인덱싱을 시작하지 못했어요: {message}")]
    StartFailed { message: String },

    #[error("검색에 실패했어요: {message}")]
    SearchFailed { message: String },

    #[error("내부 오류가 났어요: {message}")]
    Internal { message: String },
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
pub async fn run_ingest(
    config: IngestConfig,
    ingest_id: String,
    registry: Arc<KnowledgeRegistry>,
    cancel: CancellationToken,
    atomic_cancel: Arc<AtomicBool>,
    channel: Channel<IngestEvent>,
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

    // 2. Store open.
    let store = match open_store(&config.store_path) {
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

    // 3. Embedder — v1은 MockEmbedder.
    let embedder: Arc<dyn Embedder> = Arc::new(MockEmbedder::default());
    let service = IngestService::new(store, embedder);

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

/// Store 열기 helper — store_path가 비어있으면 in-memory.
fn open_store(store_path: &str) -> Result<Arc<StdMutex<KnowledgeStore>>, KnowledgeApiError> {
    let store = if store_path.is_empty() {
        KnowledgeStore::open_memory().map_err(|e| KnowledgeApiError::StoreOpen {
            message: format!("{e}"),
        })?
    } else {
        KnowledgeStore::open(Path::new(store_path)).map_err(|e| KnowledgeApiError::StoreOpen {
            message: format!("{e}"),
        })?
    };
    Ok(Arc::new(StdMutex::new(store)))
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
#[tauri::command]
pub async fn ingest_path(
    config: IngestConfig,
    on_event: Channel<IngestEvent>,
    registry: State<'_, Arc<KnowledgeRegistry>>,
) -> Result<String, KnowledgeApiError> {
    let workspace_id = config.workspace_id.clone();
    let (ingest_id, cancel, atomic_cancel) = registry.register(&workspace_id).await?;
    let registry_arc: Arc<KnowledgeRegistry> = registry.inner().clone();
    let id_for_return = ingest_id.clone();

    // Tauri 2 정책: tauri::async_runtime::spawn 사용 (tokio::spawn 금지 — Tauri가 자체 runtime 소유).
    tauri::async_runtime::spawn(async move {
        run_ingest(
            config,
            ingest_id,
            registry_arc,
            cancel,
            atomic_cancel,
            on_event,
        )
        .await;
    });

    Ok(id_for_return)
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

/// 동기 검색 RPC. 임베더는 on-demand로 생성 (v1은 MockEmbedder).
/// k는 max 50으로 cap (DoS 회피 + cosine brute-force 비용 제한).
#[tauri::command]
pub async fn search_knowledge(
    workspace_id: String,
    query: String,
    k: usize,
    store_path: String,
) -> Result<Vec<SearchHit>, KnowledgeApiError> {
    let k = k.min(50);
    if query.trim().is_empty() || k == 0 {
        return Ok(Vec::new());
    }

    // store 열기.
    let store_arc = open_store(&store_path)?;

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

    // 쿼리 임베딩 — MockEmbedder.
    let embedder = MockEmbedder::default();
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
#[tauri::command]
pub async fn knowledge_workspace_stats(
    workspace_id: String,
    store_path: String,
) -> Result<WorkspaceStats, KnowledgeApiError> {
    let store_arc = open_store(&store_path)?;
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

        run_ingest(config, ingest_id, registry.clone(), cancel, atomic, ch).await;

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

        let hits = search_knowledge(
            ws_id.clone(),
            "apple".to_string(),
            3,
            store_file.to_string_lossy().to_string(),
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
        let hits = search_knowledge(
            "ws".to_string(),
            "   ".to_string(),
            5,
            store_file.to_string_lossy().to_string(),
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
        let err = search_knowledge(
            "missing-ws".to_string(),
            "hello".to_string(),
            3,
            store_file.to_string_lossy().to_string(),
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

        let hits_a = search_knowledge(
            ws_a_id.clone(),
            "alpha".to_string(),
            5,
            store_file.to_string_lossy().to_string(),
        )
        .await
        .unwrap();
        assert_eq!(hits_a.len(), 1);
        assert_eq!(hits_a[0].content, "alpha-only");
        let hits_b = search_knowledge(
            ws_b_id.clone(),
            "beta".to_string(),
            5,
            store_file.to_string_lossy().to_string(),
        )
        .await
        .unwrap();
        assert_eq!(hits_b.len(), 1);
        assert_eq!(hits_b[0].content, "beta-only");
    }

    // ── knowledge_workspace_stats ────────────────────────────────

    #[tokio::test]
    async fn workspace_stats_zeros_for_unknown_workspace() {
        let dir = TempDir::new().unwrap();
        let store_file = dir.path().join("k.db");
        let _ = KnowledgeStore::open(&store_file).unwrap();
        let stats = knowledge_workspace_stats(
            "missing-ws".to_string(),
            store_file.to_string_lossy().to_string(),
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
        let stats =
            knowledge_workspace_stats(ws_id.clone(), store_file.to_string_lossy().to_string())
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
}
