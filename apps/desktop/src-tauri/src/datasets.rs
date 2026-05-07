//! Dataset 카탈로그 IPC — Phase 23'.c.2.d.3.
//!
//! 정책:
//! - dataset store는 *workspace 횡단 전역* — `app_data_dir/datasets.db` 고정 경로.
//! - 기존 `KnowledgeStorePool` 재사용 (passphrase 자동 적용 — Phase R-B/#38).
//! - `tokio::task::spawn_blocking` + `StdMutex.lock()`으로 rusqlite Connection !Sync 우회.
//! - 한국어 해요체 에러 메시지 (#[error] Display).
//! - Channel<DatasetIngestEvent> per-invocation typed stream — knowledge.rs IngestEvent와 동형.
//! - DatasetIngestRegistry — 활성 import id → cancel atomic. AlreadyImporting 가드.
//!
//! 흐름 (`dataset_import_start`):
//! 1. registry.register(import_id, cancel)
//! 2. embedder = active OnnxModelKind / MockEmbedder fallback (knowledge::EmbeddingState)
//! 3. store = KnowledgeStorePool.get_or_open(datasets.db) → KnowledgeStore.add_dataset
//! 4. 백그라운드 spawn: service.run + run_writer + bridge_progress_to_channel 3-task join
//! 5. 결과 emit Done summary / Failed / Cancelled

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use dataset_importer::{
    run_writer, ChunkConfigParams, DatasetChunker, DatasetIngestService,
    DatasetIngestStage as ImporterStage, EmbeddedChunk, IngestProgress as ImporterProgress,
    IngestRequest, IngestStats, SampleStrategy, DEFAULT_EMBEDDING_BATCH_SIZE,
    DEFAULT_WRITER_BATCH_SIZE,
};
use knowledge_stack::{AddDatasetInput, DatasetRow, Embedder, KnowledgeStore, MockEmbedder};
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use thiserror::Error;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::knowledge::{EmbeddingState, KnowledgeStorePool};

/// Frontend 노출용 DTO — `DatasetRow`의 1:1 매핑.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetSummary {
    pub id: String,
    pub repo: String,
    pub config: String,
    pub split: String,
    pub license: String,
    pub minor_safety: bool,
    /// `serde_json::to_string(&SampleStrategy)` 결과 — frontend 측이 다시 parse.
    pub sample_strategy: String,
    pub embedding_dim: usize,
    pub total_chunks: u64,
    pub created_at: String,
}

impl From<DatasetRow> for DatasetSummary {
    fn from(r: DatasetRow) -> Self {
        Self {
            id: r.id,
            repo: r.repo,
            config: r.config,
            split: r.split,
            license: r.license,
            minor_safety: r.minor_safety,
            sample_strategy: r.sample_strategy,
            embedding_dim: r.embedding_dim,
            total_chunks: r.total_chunks,
            created_at: r.created_at,
        }
    }
}

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DatasetApiError {
    #[error("데이터셋 저장소를 열 수 없어요: {message}")]
    StoreOpen { message: String },

    #[error("데이터셋 작업이 실패했어요: {message}")]
    StoreFailed { message: String },

    #[error("미성년 보호 동의가 필요해요. 18세 이상 사용자가 NSFW 데이터셋임을 확인해 주세요.")]
    MinorSafetyRequired,

    #[error("샘플 전략을 직렬화하지 못했어요: {message}")]
    InvalidSampleStrategy { message: String },

    #[error("내부 에러가 발생했어요: {message}")]
    Internal { message: String },
}

/// `app_data_dir/datasets.db` — workspace 횡단 dataset 카탈로그 store path.
///
/// app_data_dir 미존재 시 자동 생성. 호출 결과 string은 `KnowledgeStorePool::get_or_open`에 전달.
fn resolve_dataset_store_path(app: &AppHandle) -> Result<String, DatasetApiError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| DatasetApiError::StoreOpen {
            message: format!("app_data_dir 접근 실패: {e}"),
        })?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| DatasetApiError::StoreOpen {
            message: format!("디렉터리 생성 실패: {e}"),
        })?;
    }
    Ok(dir.join("datasets.db").to_string_lossy().to_string())
}

/// 등록된 데이터셋 목록 (최신순).
#[tauri::command]
pub async fn list_datasets(
    app: AppHandle,
    store_pool: State<'_, Arc<KnowledgeStorePool>>,
) -> Result<Vec<DatasetSummary>, DatasetApiError> {
    let path = resolve_dataset_store_path(&app)?;
    let store = store_pool
        .inner()
        .get_or_open(&path)
        .map_err(|e| DatasetApiError::StoreOpen {
            message: format!("{e}"),
        })?;

    let datasets = tokio::task::spawn_blocking(move || {
        let s = store.lock().expect("dataset store mutex poisoned");
        s.list_datasets()
    })
    .await
    .map_err(|e| DatasetApiError::Internal {
        message: format!("join: {e}"),
    })?
    .map_err(|e| DatasetApiError::StoreFailed {
        message: format!("{e}"),
    })?;

    Ok(datasets.into_iter().map(DatasetSummary::from).collect())
}

/// 데이터셋 삭제 (cascade — dataset_chunks도 함께 제거).
#[tauri::command]
pub async fn delete_dataset(
    app: AppHandle,
    dataset_id: String,
    store_pool: State<'_, Arc<KnowledgeStorePool>>,
) -> Result<(), DatasetApiError> {
    let path = resolve_dataset_store_path(&app)?;
    let store = store_pool
        .inner()
        .get_or_open(&path)
        .map_err(|e| DatasetApiError::StoreOpen {
            message: format!("{e}"),
        })?;

    tokio::task::spawn_blocking(move || {
        let mut s = store.lock().expect("dataset store mutex poisoned");
        s.delete_dataset(&dataset_id)
    })
    .await
    .map_err(|e| DatasetApiError::Internal {
        message: format!("join: {e}"),
    })?
    .map_err(|e| DatasetApiError::StoreFailed {
        message: format!("{e}"),
    })?;

    Ok(())
}

// ───────────────────────────────────────────────────────────────────
// Phase 23'.c.2.d.3.2 — Channel<DatasetIngestEvent> + import_start/cancel
// ───────────────────────────────────────────────────────────────────

/// Frontend invoke 입력 — `dataset_import_start`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetImportConfig {
    pub repo: String,
    pub config: String,
    pub split: String,
    pub license: String,
    /// ADR-0062 — `false`면 IPC 거부 (NSFW 데이터셋 미성년 보호 attestation).
    pub minor_safety_attestation: bool,
    /// `serde_json::to_string(&SampleStrategy)`로 인코딩되어 DB에 저장.
    pub sample: SampleStrategy,
    /// row 텍스트 합성 컬럼 (예: ["persona"] / ["persona", "province"]).
    pub text_columns: Vec<String>,
}

/// 완료 시 frontend에 노출할 요약.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DatasetImportSummary {
    pub dataset_id: String,
    pub rows_processed: u64,
    pub chunks_generated: u64,
    pub chunks_embedded: u64,
    pub chunks_inserted: u64,
}

/// Channel<T> — typed per-invocation stream. `kind` 필드로 frontend가 discriminated union 식별.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DatasetIngestEvent {
    /// import_start 직후 1회.
    Started {
        import_id: String,
        dataset_id: String,
        repo: String,
    },
    /// HF parquet URL 목록 fetch 완료.
    Manifest { import_id: String, urls: u32 },
    /// 각 parquet shard 진입.
    Downloading {
        import_id: String,
        urls_fetched: u32,
        urls_total: u32,
    },
    /// row → chunk 진행 (정기).
    Chunking {
        import_id: String,
        rows: u64,
        chunks_generated: u64,
        chunks_embedded: u64,
    },
    /// embed batch flush 직후.
    Embedding { import_id: String, chunks: u64 },
    /// SQLCipher INSERT 완료 — Done 직전 1회.
    Writing { import_id: String, inserted: u64 },
    /// 정상 완료 + 요약.
    Done {
        import_id: String,
        summary: DatasetImportSummary,
    },
    /// 실패 (한국어 해요체).
    Failed { import_id: String, error: String },
    /// 사용자 cancel 또는 channel close.
    Cancelled { import_id: String },
}

/// 활성 import id → cancel atomic. AlreadyImporting 가드 + cancel routing.
pub struct DatasetIngestRegistry {
    inner: StdMutex<HashMap<String, Arc<AtomicBool>>>,
}

impl DatasetIngestRegistry {
    pub fn new() -> Self {
        Self {
            inner: StdMutex::new(HashMap::new()),
        }
    }

    /// 새 import 등록 — uuid v4 import_id + cancel atomic 반환.
    pub fn register(&self) -> (String, Arc<AtomicBool>) {
        let id = Uuid::new_v4().to_string();
        let cancel = Arc::new(AtomicBool::new(false));
        self.inner
            .lock()
            .expect("dataset registry poisoned")
            .insert(id.clone(), Arc::clone(&cancel));
        (id, cancel)
    }

    /// import_id에 해당하는 cancel atomic을 true로 설정 — idempotent.
    pub fn cancel(&self, import_id: &str) {
        if let Some(c) = self
            .inner
            .lock()
            .expect("dataset registry poisoned")
            .get(import_id)
        {
            c.store(true, Ordering::Relaxed);
        }
    }

    /// import 종료 시 호출 — 메모리 누수 방지.
    pub fn unregister(&self, import_id: &str) {
        self.inner
            .lock()
            .expect("dataset registry poisoned")
            .remove(import_id);
    }
}

impl Default for DatasetIngestRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Tauri setup에서 호출 — Arc<DatasetIngestRegistry>를 manage.
pub fn provision_dataset_ingest_registry() -> Arc<DatasetIngestRegistry> {
    Arc::new(DatasetIngestRegistry::new())
}

/// active OnnxModelKind 기반 임베더. fallback_to_mock=true (knowledge::resolve_active_embedder 패턴).
async fn resolve_dataset_embedder(state: &Arc<EmbeddingState>) -> Arc<dyn Embedder> {
    let kind = state.active().await;
    match knowledge_stack::default_embedder(state.target_dir(), kind, true).await {
        Ok(emb) => emb,
        Err(e) => {
            tracing::warn!(error = %e, "default_embedder 실패 — MockEmbedder fallback");
            Arc::new(MockEmbedder::default())
        }
    }
}

/// Dataset import 시작. import_id를 즉시 반환하고 ingest는 백그라운드 task.
///
/// 정책 (ADR-0062):
/// - `config.minor_safety_attestation = false`면 `MinorSafetyRequired` 즉시 거부.
/// - `dataset_import_cancel(import_id)`로 사용자 취소.
/// - 진행 이벤트는 `on_event` Channel로 흐름 (Started → Manifest → Downloading → Chunking
///   → Embedding → Writing → Done/Failed/Cancelled).
#[tauri::command]
pub async fn dataset_import_start(
    app: AppHandle,
    config: DatasetImportConfig,
    on_event: Channel<DatasetIngestEvent>,
    registry: State<'_, Arc<DatasetIngestRegistry>>,
    store_pool: State<'_, Arc<KnowledgeStorePool>>,
    embedding_state: State<'_, Arc<EmbeddingState>>,
) -> Result<String, DatasetApiError> {
    if !config.minor_safety_attestation {
        return Err(DatasetApiError::MinorSafetyRequired);
    }

    let path = resolve_dataset_store_path(&app)?;
    let store = store_pool
        .inner()
        .get_or_open(&path)
        .map_err(|e| DatasetApiError::StoreOpen {
            message: format!("{e}"),
        })?;

    let embedder = resolve_dataset_embedder(embedding_state.inner()).await;
    let embedding_dim = embedder.dim();

    let strategy_json = serde_json::to_string(&config.sample).map_err(|e| {
        DatasetApiError::InvalidSampleStrategy {
            message: format!("{e}"),
        }
    })?;

    // add_dataset (idempotent — UNIQUE 충돌 시 기존 row 반환).
    let store_for_add = Arc::clone(&store);
    let repo = config.repo.clone();
    let cfg_str = config.config.clone();
    let split = config.split.clone();
    let license = config.license.clone();
    let strategy_clone = strategy_json.clone();
    let dataset_row = tokio::task::spawn_blocking(move || {
        let s = store_for_add.lock().expect("dataset store mutex poisoned");
        s.add_dataset(AddDatasetInput {
            repo: &repo,
            config: &cfg_str,
            split: &split,
            license: &license,
            minor_safety: true,
            sample_strategy: &strategy_clone,
            embedding_dim,
        })
    })
    .await
    .map_err(|e| DatasetApiError::Internal {
        message: format!("add_dataset join: {e}"),
    })?
    .map_err(|e| DatasetApiError::StoreFailed {
        message: format!("{e}"),
    })?;

    let (import_id, cancel) = registry.inner().register();
    let import_id_for_return = import_id.clone();
    let dataset_id = dataset_row.id.clone();
    let registry_arc: Arc<DatasetIngestRegistry> = registry.inner().clone();

    let _ = on_event.send(DatasetIngestEvent::Started {
        import_id: import_id.clone(),
        dataset_id: dataset_id.clone(),
        repo: config.repo.clone(),
    });

    tauri::async_runtime::spawn(async move {
        run_dataset_import(
            config,
            import_id,
            dataset_id,
            store,
            embedder,
            cancel,
            on_event,
            registry_arc,
        )
        .await;
    });

    Ok(import_id_for_return)
}

/// 진행 중 import를 cancel — idempotent. import_id 미존재 시 no-op.
#[tauri::command]
pub async fn dataset_import_cancel(
    import_id: String,
    registry: State<'_, Arc<DatasetIngestRegistry>>,
) -> Result<(), DatasetApiError> {
    registry.inner().cancel(&import_id);
    Ok(())
}

/// 백그라운드 task 본체 — service.run + run_writer + bridge_progress 3-task join.
#[allow(clippy::too_many_arguments)]
async fn run_dataset_import(
    config: DatasetImportConfig,
    import_id: String,
    dataset_id: String,
    store: Arc<StdMutex<KnowledgeStore>>,
    embedder: Arc<dyn Embedder>,
    cancel: Arc<AtomicBool>,
    on_event: Channel<DatasetIngestEvent>,
    registry: Arc<DatasetIngestRegistry>,
) {
    let chunker = match DatasetChunker::with_char_fallback(ChunkConfigParams::default_kure_v1()) {
        Ok(c) => c,
        Err(e) => {
            let _ = on_event.send(DatasetIngestEvent::Failed {
                import_id: import_id.clone(),
                error: format!("{e}"),
            });
            registry.unregister(&import_id);
            return;
        }
    };

    let service = match DatasetIngestService::new(chunker, embedder) {
        Ok(s) => s,
        Err(e) => {
            let _ = on_event.send(DatasetIngestEvent::Failed {
                import_id: import_id.clone(),
                error: format!("{e}"),
            });
            registry.unregister(&import_id);
            return;
        }
    };

    let request = IngestRequest {
        repo: config.repo,
        config: config.config,
        split: config.split,
        text_columns: config.text_columns,
        sample: config.sample,
    };

    let (progress_tx, progress_rx) = mpsc::channel::<ImporterProgress>(64);
    let (embedded_tx, embedded_rx) =
        mpsc::channel::<EmbeddedChunk>(DEFAULT_EMBEDDING_BATCH_SIZE * 4);

    // bridge — progress mpsc → Channel<DatasetIngestEvent>.
    let on_event_bridge = on_event.clone();
    let import_id_bridge = import_id.clone();
    let bridge = tauri::async_runtime::spawn(async move {
        run_event_bridge(progress_rx, on_event_bridge, import_id_bridge).await;
    });

    // writer — embedded mpsc → SQLCipher INSERT.
    let writer_handle = tauri::async_runtime::spawn(run_writer(
        Arc::clone(&store),
        dataset_id.clone(),
        embedded_rx,
        DEFAULT_WRITER_BATCH_SIZE,
    ));

    // service.run — progress_tx + embedded_tx 둘 다 drop 시점에 channels close.
    let service_res = service
        .run(request, progress_tx, cancel.clone(), embedded_tx)
        .await;

    // writer 자연 종료 (embedded_tx drop → channel close).
    let writer_res = writer_handle.await;
    let _ = bridge.await;

    let event = match (service_res, writer_res) {
        (Err(dataset_importer::DatasetImportError::Cancelled), _) => {
            DatasetIngestEvent::Cancelled {
                import_id: import_id.clone(),
            }
        }
        (Err(e), _) => DatasetIngestEvent::Failed {
            import_id: import_id.clone(),
            error: format!("{e}"),
        },
        (Ok(stats), Ok(Ok(inserted))) => {
            // Writing 완료 1회 emit (Done 직전).
            let _ = on_event.send(DatasetIngestEvent::Writing {
                import_id: import_id.clone(),
                inserted,
            });
            DatasetIngestEvent::Done {
                import_id: import_id.clone(),
                summary: DatasetImportSummary {
                    dataset_id: dataset_id.clone(),
                    rows_processed: stats.rows_processed,
                    chunks_generated: stats.chunks_generated,
                    chunks_embedded: stats.chunks_embedded,
                    chunks_inserted: inserted,
                },
            }
        }
        (Ok(_), Ok(Err(e))) => DatasetIngestEvent::Failed {
            import_id: import_id.clone(),
            error: format!("writer: {e}"),
        },
        (Ok(_), Err(join_err)) => DatasetIngestEvent::Failed {
            import_id: import_id.clone(),
            error: format!("writer join: {join_err}"),
        },
    };
    let _ = on_event.send(event);
    registry.unregister(&import_id);

    // service.run 결과의 IngestStats를 별도 사용 X — Done summary에 이미 포함.
    let _ = (IngestStats::default(),); // type 보존 (lint 회피).
}

/// progress mpsc → Channel<DatasetIngestEvent> 어댑터.
///
/// Stage::Done은 무시 (run_dataset_import의 final emit이 담당). Stage::Writing은 service에서
/// emit되지 않음 (writer가 별도 task) — run_dataset_import이 직접 emit.
async fn run_event_bridge(
    mut progress_rx: mpsc::Receiver<ImporterProgress>,
    on_event: Channel<DatasetIngestEvent>,
    import_id: String,
) {
    while let Some(p) = progress_rx.recv().await {
        let event = match p.stage {
            ImporterStage::Manifest => DatasetIngestEvent::Manifest {
                import_id: import_id.clone(),
                urls: p.total as u32,
            },
            ImporterStage::Downloading => DatasetIngestEvent::Downloading {
                import_id: import_id.clone(),
                urls_fetched: p.current as u32,
                urls_total: p.total as u32,
            },
            ImporterStage::Chunking => DatasetIngestEvent::Chunking {
                import_id: import_id.clone(),
                rows: p.current,
                chunks_generated: p.total,
                chunks_embedded: p.chunks_written,
            },
            ImporterStage::Embedding => DatasetIngestEvent::Embedding {
                import_id: import_id.clone(),
                chunks: p.chunks_written,
            },
            ImporterStage::Writing | ImporterStage::Done => continue,
        };
        if on_event.send(event).is_err() {
            break;
        }
    }
}
