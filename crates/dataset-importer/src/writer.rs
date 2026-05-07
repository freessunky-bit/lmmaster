//! EmbeddedChunk → SQLCipher INSERT writer task — Phase 23'.c.2.d.2 (ADR-0063 §5).
//!
//! 정책:
//! - knowledge-stack의 `KnowledgeStore`에 *dataset_chunks* 테이블 INSERT.
//! - `rusqlite::Connection`은 `!Sync` — `Arc<std::sync::Mutex<KnowledgeStore>>`로 보호 +
//!   `tokio::task::spawn_blocking` + `lock().expect(...)`로 dedicated thread에서 실행.
//!   `KnowledgeStorePool`(`apps/desktop/src-tauri`)이 동일 std Mutex 사용 — 패턴 일치.
//! - batch INSERT (32 단위, transaction) — embedding_batch_size와 정렬.
//! - 채널 close 시 partial buffer flush + `update_dataset_total_chunks` counter 갱신.

#![allow(dead_code)]

use std::sync::{Arc, Mutex as StdMutex};

#[cfg(test)]
use knowledge_stack::AddDatasetInput;
use knowledge_stack::{DatasetChunkRecord, KnowledgeStore};
use tokio::sync::mpsc;

use crate::error::{DatasetImportError, DatasetImportResult};
use crate::service::EmbeddedChunk;

/// Writer batch 기본 — embedding_batch_size와 정렬 (`DEFAULT_EMBEDDING_BATCH_SIZE`).
pub const DEFAULT_WRITER_BATCH_SIZE: usize = 32;

/// EmbeddedChunk → KnowledgeStore writer.
///
/// `embedded_rx`에서 chunk를 받아 `batch_size` 단위로 INSERT (transaction). 채널 close 시
/// partial buffer flush + total_chunks counter 갱신 후 종료.
///
/// 반환: 총 INSERT 시도한 chunk 수 (`INSERT OR IGNORE`로 duplicate 카운트 포함).
pub async fn run_writer(
    store: Arc<StdMutex<KnowledgeStore>>,
    dataset_id: String,
    mut embedded_rx: mpsc::Receiver<EmbeddedChunk>,
    batch_size: usize,
) -> DatasetImportResult<u64> {
    let batch_size = batch_size.max(1);
    let mut buffer: Vec<DatasetChunkRecord> = Vec::with_capacity(batch_size);
    let mut total: u64 = 0;

    while let Some(chunk) = embedded_rx.recv().await {
        buffer.push(DatasetChunkRecord {
            row_index: chunk.row_index,
            chunk_index: chunk.chunk_index,
            content: chunk.text,
            embedding: chunk.embedding,
        });
        if buffer.len() >= batch_size {
            let drain = std::mem::take(&mut buffer);
            total += flush_batch(&store, &dataset_id, drain).await? as u64;
        }
    }
    if !buffer.is_empty() {
        let drain = std::mem::take(&mut buffer);
        total += flush_batch(&store, &dataset_id, drain).await? as u64;
    }

    // total_chunks counter 갱신 — list 응답에 즉시 반영.
    {
        let store = Arc::clone(&store);
        let id = dataset_id.clone();
        tokio::task::spawn_blocking(move || {
            let store = store.lock().expect("dataset store mutex poisoned");
            store.update_dataset_total_chunks(&id, total)
        })
        .await
        .map_err(|e| DatasetImportError::Internal(format!("counter join: {e}")))?
        .map_err(|e| DatasetImportError::Internal(format!("counter SQL: {e}")))?;
    }

    Ok(total)
}

async fn flush_batch(
    store: &Arc<StdMutex<KnowledgeStore>>,
    dataset_id: &str,
    batch: Vec<DatasetChunkRecord>,
) -> DatasetImportResult<usize> {
    let store = Arc::clone(store);
    let id = dataset_id.to_string();
    let inserted = tokio::task::spawn_blocking(move || {
        let mut store = store.lock().expect("dataset store mutex poisoned");
        store.add_dataset_chunks(&id, &batch)
    })
    .await
    .map_err(|e| DatasetImportError::Internal(format!("flush join: {e}")))?
    .map_err(|e| DatasetImportError::Internal(format!("flush SQL: {e}")))?;
    Ok(inserted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_store() -> (TempDir, Arc<StdMutex<KnowledgeStore>>) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("ds.db");
        let store = KnowledgeStore::open(&path).unwrap();
        (dir, Arc::new(StdMutex::new(store)))
    }

    fn input<'a>(repo: &'a str, sample: &'a str) -> AddDatasetInput<'a> {
        AddDatasetInput {
            repo,
            config: "default",
            split: "train",
            license: "MIT",
            minor_safety: true,
            sample_strategy: sample,
            embedding_dim: 384,
        }
    }

    #[tokio::test]
    async fn writer_inserts_and_updates_counter() {
        let (_dir, store) = make_store();
        let dataset_id = {
            let s = store.lock().expect("test store mutex poisoned");
            let row = s
                .add_dataset(input("test/repo", "{\"kind\":\"first\",\"n\":5}"))
                .unwrap();
            row.id
        };

        let (tx, rx) = mpsc::channel::<EmbeddedChunk>(16);
        let writer_handle = tokio::spawn(run_writer(
            Arc::clone(&store),
            dataset_id.clone(),
            rx,
            2, // batch_size = 2 → 2 full batch + 1 partial.
        ));

        for i in 0..5u64 {
            tx.send(EmbeddedChunk {
                row_index: i / 2,
                chunk_index: (i % 2) as u32,
                text: format!("chunk {i}"),
                embedding: vec![0.01_f32; 384],
            })
            .await
            .unwrap();
        }
        drop(tx);

        let total = writer_handle.await.unwrap().unwrap();
        assert_eq!(total, 5);

        let s = store.lock().expect("test store mutex poisoned");
        assert_eq!(s.dataset_chunks_count(&dataset_id).unwrap(), 5);
        let datasets = s.list_datasets().unwrap();
        let target = datasets.iter().find(|d| d.id == dataset_id).unwrap();
        assert_eq!(target.total_chunks, 5);
    }

    #[tokio::test]
    async fn writer_handles_empty_channel() {
        let (_dir, store) = make_store();
        let dataset_id = {
            let s = store.lock().expect("test store mutex poisoned");
            s.add_dataset(input("empty/repo", "{\"kind\":\"full\"}"))
                .unwrap()
                .id
        };

        let (tx, rx) = mpsc::channel::<EmbeddedChunk>(4);
        drop(tx); // 즉시 close.

        let total = run_writer(Arc::clone(&store), dataset_id.clone(), rx, 32)
            .await
            .unwrap();
        assert_eq!(total, 0);

        let s = store.lock().expect("test store mutex poisoned");
        assert_eq!(s.dataset_chunks_count(&dataset_id).unwrap(), 0);
        let datasets = s.list_datasets().unwrap();
        let target = datasets.iter().find(|d| d.id == dataset_id).unwrap();
        assert_eq!(target.total_chunks, 0);
    }

    #[tokio::test]
    async fn writer_idempotent_on_duplicate_keys() {
        let (_dir, store) = make_store();
        let dataset_id = {
            let s = store.lock().expect("test store mutex poisoned");
            s.add_dataset(input("dup/repo", "{\"kind\":\"full\"}"))
                .unwrap()
                .id
        };

        // 동일 (row_index, chunk_index) 4건 → 1건만 INSERT (PK 충돌 IGNORE).
        let (tx, rx) = mpsc::channel::<EmbeddedChunk>(8);
        let writer_handle = tokio::spawn(run_writer(Arc::clone(&store), dataset_id.clone(), rx, 2));
        for _ in 0..4 {
            tx.send(EmbeddedChunk {
                row_index: 0,
                chunk_index: 0,
                text: "duplicate".into(),
                embedding: vec![0.0; 384],
            })
            .await
            .unwrap();
        }
        drop(tx);
        let _total = writer_handle.await.unwrap().unwrap();

        // 실 row 수는 1.
        let s = store.lock().expect("test store mutex poisoned");
        assert_eq!(
            s.dataset_chunks_count(&dataset_id).unwrap(),
            1,
            "PK 중복은 IGNORE — 1 row만 보존"
        );
    }

    #[tokio::test]
    async fn add_dataset_idempotent_on_repeat() {
        let (_dir, store) = make_store();
        let s = store.lock().expect("test store mutex poisoned");
        let a = s.add_dataset(input("x/y", "{\"kind\":\"full\"}")).unwrap();
        let b = s.add_dataset(input("x/y", "{\"kind\":\"full\"}")).unwrap();
        assert_eq!(
            a.id, b.id,
            "동일 (repo, config, split, sample_strategy) → 같은 id"
        );
    }
}
