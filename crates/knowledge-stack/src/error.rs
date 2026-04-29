//! Knowledge Stack 에러 타입.
//!
//! 정책 (ADR-0024, phase-4p5-rag-decision.md):
//! - 모든 사용자 향 메시지는 한국어 해요체 (CLAUDE.md §4.1).
//! - thiserror 기반 — `Display`만으로 IPC 응답에 그대로 사용 가능해야 함.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum KnowledgeError {
    #[error("지식 저장소를 열지 못했어요: {path}")]
    DbOpen {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },

    #[error("지식 저장소 질의에 실패했어요: {0}")]
    DbQuery(#[from] rusqlite::Error),

    #[error("파일을 읽지 못했어요: {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("내용이 비어 있어 인덱싱할 수 없어요")]
    EmptyContent,

    #[error("임베딩 생성에 실패했어요: {0}")]
    EmbeddingFailed(String),

    #[error("워크스페이스를 찾지 못했어요: {0}")]
    WorkspaceNotFound(String),

    #[error("사용자가 작업을 취소했어요")]
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_open_message_includes_korean_and_path() {
        let path = PathBuf::from("/tmp/missing.db");
        let source = rusqlite::Error::InvalidPath(path.clone());
        let err = KnowledgeError::DbOpen {
            path: path.clone(),
            source,
        };
        let msg = format!("{err}");
        assert!(msg.contains("지식 저장소"));
        assert!(msg.contains("열지 못했어요"));
        assert!(msg.contains("/tmp/missing.db") || msg.contains("missing.db"));
    }

    #[test]
    fn db_query_message_korean() {
        // rusqlite::Error variant 하나 골라서 from 변환.
        let bad: rusqlite::Error = rusqlite::Error::QueryReturnedNoRows;
        let err: KnowledgeError = bad.into();
        let msg = format!("{err}");
        assert!(msg.contains("질의에 실패했어요"));
    }

    #[test]
    fn io_message_includes_korean_and_path() {
        let path = PathBuf::from("/tmp/file.txt");
        let source = io::Error::new(io::ErrorKind::NotFound, "missing");
        let err = KnowledgeError::Io { path, source };
        let msg = format!("{err}");
        assert!(msg.contains("파일을 읽지 못했어요"));
        assert!(msg.contains("/tmp/file.txt") || msg.contains("file.txt"));
    }

    #[test]
    fn empty_content_message_korean() {
        let err = KnowledgeError::EmptyContent;
        let msg = format!("{err}");
        assert!(msg.contains("내용이 비어 있어"));
        assert!(msg.contains("인덱싱"));
    }

    #[test]
    fn embedding_failed_includes_detail() {
        let err = KnowledgeError::EmbeddingFailed("dim mismatch".into());
        let msg = format!("{err}");
        assert!(msg.contains("임베딩 생성에 실패"));
        assert!(msg.contains("dim mismatch"));
    }

    #[test]
    fn workspace_not_found_includes_id() {
        let err = KnowledgeError::WorkspaceNotFound("ws-123".into());
        let msg = format!("{err}");
        assert!(msg.contains("워크스페이스를 찾지 못했어요"));
        assert!(msg.contains("ws-123"));
    }

    #[test]
    fn cancelled_message_korean() {
        let err = KnowledgeError::Cancelled;
        let msg = format!("{err}");
        assert!(msg.contains("취소"));
    }

    #[test]
    fn debug_does_not_panic() {
        // 모든 variant에 대해 Debug 동작 확인.
        let path = PathBuf::from("/x");
        let dbg_list = [
            format!(
                "{:?}",
                KnowledgeError::DbOpen {
                    path: path.clone(),
                    source: rusqlite::Error::QueryReturnedNoRows,
                }
            ),
            format!(
                "{:?}",
                KnowledgeError::DbQuery(rusqlite::Error::QueryReturnedNoRows)
            ),
            format!(
                "{:?}",
                KnowledgeError::Io {
                    path,
                    source: io::Error::other("x"),
                }
            ),
            format!("{:?}", KnowledgeError::EmptyContent),
            format!("{:?}", KnowledgeError::EmbeddingFailed("x".into())),
            format!("{:?}", KnowledgeError::WorkspaceNotFound("x".into())),
            format!("{:?}", KnowledgeError::Cancelled),
        ];
        for s in dbg_list {
            assert!(!s.is_empty());
        }
    }
}
