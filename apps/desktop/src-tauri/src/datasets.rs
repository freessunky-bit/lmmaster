//! Dataset 카탈로그 IPC — Phase 23'.c.2.d.3.1.
//!
//! 정책:
//! - dataset store는 *workspace 횡단 전역* — `app_data_dir/datasets.db` 고정 경로.
//! - 기존 `KnowledgeStorePool` 재사용 (passphrase 자동 적용 — Phase R-B/#38).
//! - `tokio::task::spawn_blocking` + `StdMutex.lock()`으로 rusqlite Connection !Sync 우회.
//! - 한국어 해요체 에러 메시지 (#[error] Display).
//!
//! 본 sub-phase 범위 (.d.3.1):
//! - `list_datasets` / `delete_dataset` 2 commands.
//! - `dataset_import_start` / `dataset_import_cancel`은 .d.3.2 (Channel<DatasetIngestEvent> +
//!   service.run + run_writer 백그라운드 task).

use std::sync::Arc;

use knowledge_stack::DatasetRow;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use thiserror::Error;

use crate::knowledge::KnowledgeStorePool;

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

// 단위 테스트는 lmmaster-desktop crate에서 webview DLL 의존으로 실행 불가 (기존 정책).
// DatasetRow CRUD는 knowledge-stack에서 70+ tests로 이미 검증됨 (`store::tests`).
// IPC handler 자체의 컴파일 + 시그니처는 cargo check + clippy --workspace로 검증.
