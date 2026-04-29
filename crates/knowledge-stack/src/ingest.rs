//! Ingest pipeline — Reading → Chunking → Embedding → Writing → Done.
//!
//! 정책 (ADR-0024 §5):
//! - 단계 진입 시 cancel_token 검사 (협력 cancel).
//! - tokio mpsc로 진행률 emit. caller가 receiver를 drop해도 ingest 안 멈추도록 try_send.
//! - .md / .txt 파일만 v1 fully 지원. 디렉터리는 재귀 walk.
//! - 빈 파일은 EmptyContent 에러.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

use crate::chunker::chunk_text;
use crate::embed::Embedder;
use crate::error::KnowledgeError;
use crate::store::KnowledgeStore;

/// Cancel token — Arc<AtomicBool> 기반 (workbench-core/quantize.rs와 동일 결).
pub type CancelToken = Arc<AtomicBool>;
/// Progress 송신 채널 — caller가 None이면 emit skip.
pub type ProgressTx = Option<mpsc::Sender<IngestProgress>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IngestStage {
    Reading,
    Chunking,
    Embedding,
    Writing,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IngestProgress {
    pub stage: IngestStage,
    pub processed: usize,
    pub total: usize,
    pub current_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IngestSummary {
    pub workspace_id: String,
    pub documents: usize,
    pub chunks: usize,
    pub skipped: usize,
}

/// IngestService — store + embedder를 hold + ingest_path API.
pub struct IngestService {
    store: Arc<Mutex<KnowledgeStore>>,
    embedder: Arc<dyn Embedder>,
}

impl IngestService {
    pub fn new(store: Arc<Mutex<KnowledgeStore>>, embedder: Arc<dyn Embedder>) -> Self {
        Self { store, embedder }
    }

    /// 파일 또는 디렉터리를 ingest. 디렉터리는 재귀 walk (.md / .txt).
    pub async fn ingest_path(
        &self,
        workspace_id: &str,
        path: &Path,
        target_chunk_size: usize,
        overlap: usize,
        progress_tx: ProgressTx,
        cancel: CancelToken,
    ) -> Result<IngestSummary, KnowledgeError> {
        // workspace 존재 확인.
        {
            let store = self
                .store
                .lock()
                .map_err(|_| KnowledgeError::EmbeddingFailed("store mutex poisoned".to_string()))?;
            if !store.has_workspace(workspace_id)? {
                return Err(KnowledgeError::WorkspaceNotFound(workspace_id.to_string()));
            }
        }

        let files = collect_files(path)?;
        let total = files.len();
        let mut documents = 0usize;
        let mut chunks_total = 0usize;
        let mut skipped = 0usize;

        for (idx, file) in files.into_iter().enumerate() {
            if cancel.load(Ordering::SeqCst) {
                return Err(KnowledgeError::Cancelled);
            }

            // Reading.
            send_progress(
                &progress_tx,
                IngestProgress {
                    stage: IngestStage::Reading,
                    processed: idx,
                    total,
                    current_path: Some(file.display().to_string()),
                },
            )
            .await;

            let raw = match std::fs::read_to_string(&file) {
                Ok(s) => s,
                Err(e) => {
                    return Err(KnowledgeError::Io {
                        path: file.clone(),
                        source: e,
                    });
                }
            };
            if raw.trim().is_empty() {
                skipped = skipped.saturating_add(1);
                continue;
            }

            // Chunking.
            if cancel.load(Ordering::SeqCst) {
                return Err(KnowledgeError::Cancelled);
            }
            send_progress(
                &progress_tx,
                IngestProgress {
                    stage: IngestStage::Chunking,
                    processed: idx,
                    total,
                    current_path: Some(file.display().to_string()),
                },
            )
            .await;
            let chunks = chunk_text(&raw, target_chunk_size, overlap);
            if chunks.is_empty() {
                skipped = skipped.saturating_add(1);
                continue;
            }

            // Embedding.
            if cancel.load(Ordering::SeqCst) {
                return Err(KnowledgeError::Cancelled);
            }
            send_progress(
                &progress_tx,
                IngestProgress {
                    stage: IngestStage::Embedding,
                    processed: idx,
                    total,
                    current_path: Some(file.display().to_string()),
                },
            )
            .await;
            let texts: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();
            let embeds = self.embedder.embed(&texts).await?;
            if embeds.len() != chunks.len() {
                return Err(KnowledgeError::EmbeddingFailed(format!(
                    "embedder가 {}개 청크 대신 {}개 벡터를 반환했어요",
                    chunks.len(),
                    embeds.len()
                )));
            }

            // Writing.
            if cancel.load(Ordering::SeqCst) {
                return Err(KnowledgeError::Cancelled);
            }
            send_progress(
                &progress_tx,
                IngestProgress {
                    stage: IngestStage::Writing,
                    processed: idx,
                    total,
                    current_path: Some(file.display().to_string()),
                },
            )
            .await;
            let sha = sha256_of(&raw);
            let path_str = file.display().to_string();
            let doc_id = {
                let mut store = self.store.lock().map_err(|_| {
                    KnowledgeError::EmbeddingFailed("store mutex poisoned".to_string())
                })?;
                let doc = store.add_document(workspace_id, &path_str, &sha)?;
                store.add_chunks(&doc.id, workspace_id, &chunks, &embeds)?;
                doc.id
            };
            tracing::debug!(
                workspace_id = workspace_id,
                doc_id = doc_id,
                chunks = chunks.len(),
                "ingested document"
            );
            documents = documents.saturating_add(1);
            chunks_total = chunks_total.saturating_add(chunks.len());
        }

        // Done.
        send_progress(
            &progress_tx,
            IngestProgress {
                stage: IngestStage::Done,
                processed: total,
                total,
                current_path: None,
            },
        )
        .await;

        Ok(IngestSummary {
            workspace_id: workspace_id.to_string(),
            documents,
            chunks: chunks_total,
            skipped,
        })
    }
}

/// 디렉터리 재귀 walk → .md / .txt 파일 list. 단일 파일이면 [file] 반환.
fn collect_files(path: &Path) -> Result<Vec<PathBuf>, KnowledgeError> {
    if !path.exists() {
        return Err(KnowledgeError::Io {
            path: path.to_path_buf(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "path not found"),
        });
    }
    let mut out = Vec::new();
    if path.is_file() {
        if is_supported(path) {
            out.push(path.to_path_buf());
        }
        return Ok(out);
    }
    // dir → walk.
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) => {
                return Err(KnowledgeError::Io {
                    path: dir,
                    source: e,
                });
            }
        };
        for entry in entries {
            let entry = entry.map_err(|e| KnowledgeError::Io {
                path: dir.clone(),
                source: e,
            })?;
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.is_file() && is_supported(&p) {
                out.push(p);
            }
        }
    }
    out.sort();
    Ok(out)
}

fn is_supported(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase())
            .as_deref(),
        Some("md") | Some("txt") | Some("markdown")
    )
}

fn sha256_of(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

async fn send_progress(tx: &ProgressTx, msg: IngestProgress) {
    if let Some(s) = tx {
        // try_send: receiver가 늦어도 ingest는 멈추지 않음.
        let _ = s.send(msg).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::MockEmbedder;
    use std::fs;
    use std::sync::atomic::AtomicBool;
    use tempfile::TempDir;

    fn make_service() -> (
        Arc<Mutex<KnowledgeStore>>,
        Arc<dyn Embedder>,
        IngestService,
        String,
    ) {
        let store = Arc::new(Mutex::new(KnowledgeStore::open_memory().unwrap()));
        let embedder: Arc<dyn Embedder> = Arc::new(MockEmbedder::default());
        let svc = IngestService::new(Arc::clone(&store), Arc::clone(&embedder));
        let ws_id = {
            let s = store.lock().unwrap();
            s.add_workspace("ws").unwrap().id
        };
        (store, embedder, svc, ws_id)
    }

    #[tokio::test]
    async fn ingest_single_file() {
        let (_store, _emb, svc, ws_id) = make_service();
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("a.md");
        fs::write(
            &p,
            "안녕하세요. 첫 단락이에요.\n\n두 번째 단락입니다. 확인해주세요.",
        )
        .unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let summary = svc
            .ingest_path(&ws_id, &p, 200, 20, None, cancel)
            .await
            .unwrap();
        assert_eq!(summary.documents, 1);
        assert!(summary.chunks >= 1);
    }

    #[tokio::test]
    async fn ingest_directory_recursive() {
        let (_store, _emb, svc, ws_id) = make_service();
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(dir.path().join("a.md"), "첫 파일이에요.").unwrap();
        fs::write(sub.join("b.txt"), "두 번째 파일이에요.").unwrap();
        // .pdf는 v1 미지원 — skip.
        fs::write(sub.join("c.pdf"), "ignored binary").unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let summary = svc
            .ingest_path(&ws_id, dir.path(), 200, 20, None, cancel)
            .await
            .unwrap();
        assert_eq!(summary.documents, 2);
    }

    #[tokio::test]
    async fn cancellation_returns_cancelled() {
        let (_store, _emb, svc, ws_id) = make_service();
        let dir = TempDir::new().unwrap();
        for i in 0..5 {
            fs::write(
                dir.path().join(format!("f{i}.md")),
                format!("파일 {i} 내용."),
            )
            .unwrap();
        }
        let cancel = Arc::new(AtomicBool::new(true));
        let err = svc
            .ingest_path(&ws_id, dir.path(), 100, 10, None, cancel)
            .await
            .unwrap_err();
        assert!(matches!(err, KnowledgeError::Cancelled));
    }

    #[tokio::test]
    async fn progress_emits_stages() {
        let (_store, _emb, svc, ws_id) = make_service();
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.md"), "안녕하세요. 진행률 테스트.").unwrap();
        let (tx, mut rx) = mpsc::channel::<IngestProgress>(64);
        let cancel = Arc::new(AtomicBool::new(false));
        let _summary = svc
            .ingest_path(&ws_id, dir.path(), 200, 20, Some(tx), cancel)
            .await
            .unwrap();
        let mut stages = Vec::new();
        while let Ok(p) = rx.try_recv() {
            stages.push(p.stage);
        }
        assert!(stages.contains(&IngestStage::Reading));
        assert!(stages.contains(&IngestStage::Chunking));
        assert!(stages.contains(&IngestStage::Embedding));
        assert!(stages.contains(&IngestStage::Writing));
        assert!(stages.contains(&IngestStage::Done));
    }

    #[tokio::test]
    async fn missing_path_errors() {
        let (_store, _emb, svc, ws_id) = make_service();
        let cancel = Arc::new(AtomicBool::new(false));
        let err = svc
            .ingest_path(
                &ws_id,
                Path::new("/nonexistent/x.md"),
                100,
                10,
                None,
                cancel,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, KnowledgeError::Io { .. }));
    }

    #[tokio::test]
    async fn empty_file_skipped() {
        let (_store, _emb, svc, ws_id) = make_service();
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("empty.md"), "").unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let summary = svc
            .ingest_path(&ws_id, dir.path(), 100, 10, None, cancel)
            .await
            .unwrap();
        assert_eq!(summary.documents, 0);
        assert_eq!(summary.skipped, 1);
    }

    #[tokio::test]
    async fn unknown_workspace_errors() {
        let store = Arc::new(Mutex::new(KnowledgeStore::open_memory().unwrap()));
        let embedder: Arc<dyn Embedder> = Arc::new(MockEmbedder::default());
        let svc = IngestService::new(store, embedder);
        let cancel = Arc::new(AtomicBool::new(false));
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("x.md"), "x").unwrap();
        let err = svc
            .ingest_path("missing-ws", dir.path(), 100, 10, None, cancel)
            .await
            .unwrap_err();
        assert!(matches!(err, KnowledgeError::WorkspaceNotFound(_)));
    }
}
