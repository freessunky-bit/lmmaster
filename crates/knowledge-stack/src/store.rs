//! Per-workspace SQLite knowledge store.
//!
//! 정책 (ADR-0024 §4):
//! - rusqlite bundled — 외부 네트워크 / 외부 데몬 없이 portable.
//! - 모든 query에 `WHERE workspace_id = ?` 강제 — schema-level 격리.
//! - search는 brute-force cosine (top-k heap) — ~10K chunks까지 단순함 우선.
//! - embedding BLOB serialize: little-endian f32 byte stream (dim은 호출자가 보장).

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::chunker::Chunk;
use crate::error::KnowledgeError;

/// Workspace row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceRow {
    pub id: String,
    pub name: String,
    pub created_at: String,
}

/// Document row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentRow {
    pub id: String,
    pub workspace_id: String,
    pub path: String,
    pub sha256: String,
    pub ingested_at: String,
}

/// Chunk row (DB-backed) — content + start/end + workspace_id + document_id.
#[derive(Debug, Clone, PartialEq)]
pub struct ChunkRow {
    pub id: String,
    pub document_id: String,
    pub workspace_id: String,
    pub content: String,
    pub start: usize,
    pub end: usize,
}

/// search 결과 단위 — (chunk + score [0, 1]).
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub chunk: ChunkRow,
    pub score: f32,
}

pub struct KnowledgeStore {
    conn: Connection,
}

impl KnowledgeStore {
    /// 파일 경로에서 store를 연다 (parent dir 자동 생성). 평문 모드.
    ///
    /// Phase 8'.0.b: WAL + busy_timeout(5s) + synchronous=NORMAL 안정성 PRAGMA.
    /// Phase R-B (ADR-0053) — SQLCipher 빌드에서도 평문 모드는 유지 (테스트 / dev / Linux headless).
    /// 암호화 모드는 `open_with_passphrase`.
    pub fn open(path: &Path) -> Result<Self, KnowledgeError> {
        if let Some(parent) = path.parent() {
            // best effort — 권한 부족이면 open 시 오류로 표면화.
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path).map_err(|e| KnowledgeError::DbOpen {
            path: path.to_path_buf(),
            source: e,
        })?;
        apply_stability_pragmas(&conn)?;
        let mut store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// SQLCipher passphrase를 적용해 store를 연다 (parent dir 자동 생성).
    ///
    /// Phase R-B (ADR-0053) — knowledge-stack도 SQLCipher 적용. key-manager 패턴(ADR-0035) 차용.
    ///
    /// 정책:
    /// - `sqlcipher` feature OFF 빌드(stock SQLite)에서는 `PRAGMA key`가 unknown pragma로 무시됨.
    ///   즉 평문 DB / 빈 DB는 정상 작동, 잘못된 키 검증은 OFF 모드에서 스킵 (production은 항상 ON).
    /// - 호출 순서: PRAGMA key 먼저 (모든 read/write 전), 그 다음 stability PRAGMA, 마지막에 schema init.
    /// - SQL injection 방지 — passphrase 내 `'`는 escape (`''`).
    pub fn open_with_passphrase(path: &Path, passphrase: &str) -> Result<Self, KnowledgeError> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path).map_err(|e| KnowledgeError::DbOpen {
            path: path.to_path_buf(),
            source: e,
        })?;
        apply_passphrase(&conn, passphrase)?;
        apply_stability_pragmas(&conn)?;
        let mut store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// 메모리 DB — 테스트 용도.
    pub fn open_memory() -> Result<Self, KnowledgeError> {
        let conn = Connection::open_in_memory().map_err(|e| KnowledgeError::DbOpen {
            path: std::path::PathBuf::from(":memory:"),
            source: e,
        })?;
        // in-memory에 WAL은 의미 없지만 busy_timeout만 일관 설정.
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        let mut store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// 현재 connection의 `journal_mode` PRAGMA — 검증용.
    pub fn journal_mode(&self) -> Result<String, KnowledgeError> {
        let mode: String = self
            .conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))?;
        Ok(mode)
    }

    /// 스키마 초기화 — idempotent.
    pub fn init_schema(&mut self) -> Result<(), KnowledgeError> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS workspaces (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                created_at  TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS documents (
                id            TEXT PRIMARY KEY,
                workspace_id  TEXT NOT NULL,
                path          TEXT NOT NULL,
                sha256        TEXT NOT NULL,
                ingested_at   TEXT NOT NULL,
                UNIQUE(workspace_id, sha256),
                FOREIGN KEY(workspace_id) REFERENCES workspaces(id)
            );
            CREATE TABLE IF NOT EXISTS chunks (
                id            TEXT PRIMARY KEY,
                document_id   TEXT NOT NULL,
                workspace_id  TEXT NOT NULL,
                content       TEXT NOT NULL,
                embedding     BLOB NOT NULL,
                start         INTEGER NOT NULL,
                end           INTEGER NOT NULL,
                FOREIGN KEY(document_id) REFERENCES documents(id),
                FOREIGN KEY(workspace_id) REFERENCES workspaces(id)
            );
            CREATE INDEX IF NOT EXISTS idx_documents_workspace ON documents(workspace_id);
            CREATE INDEX IF NOT EXISTS idx_chunks_workspace ON chunks(workspace_id);
            CREATE INDEX IF NOT EXISTS idx_chunks_doc ON chunks(document_id);
            "#,
        )?;
        Ok(())
    }

    /// 워크스페이스 추가. id는 자동 생성.
    pub fn add_workspace(&self, name: &str) -> Result<WorkspaceRow, KnowledgeError> {
        let id = Uuid::new_v4().to_string();
        let now = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .map_err(|e| KnowledgeError::EmbeddingFailed(format!("time format: {e}")))?;
        self.conn.execute(
            "INSERT INTO workspaces (id, name, created_at) VALUES (?1, ?2, ?3)",
            params![id, name, now],
        )?;
        Ok(WorkspaceRow {
            id,
            name: name.to_string(),
            created_at: now,
        })
    }

    /// 문서 추가. workspace_id 존재 검증 + 중복(workspace_id+sha256) 시 기존 row 반환.
    pub fn add_document(
        &self,
        workspace_id: &str,
        path: &str,
        sha256: &str,
    ) -> Result<DocumentRow, KnowledgeError> {
        // workspace 존재 확인.
        let exists: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM workspaces WHERE id = ?1",
            params![workspace_id],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Err(KnowledgeError::WorkspaceNotFound(workspace_id.to_string()));
        }
        // 중복 (workspace_id + sha256) → 기존 row 반환.
        let existing: Option<DocumentRow> = self
            .conn
            .query_row(
                "SELECT id, workspace_id, path, sha256, ingested_at FROM documents
                 WHERE workspace_id = ?1 AND sha256 = ?2",
                params![workspace_id, sha256],
                |r| {
                    Ok(DocumentRow {
                        id: r.get(0)?,
                        workspace_id: r.get(1)?,
                        path: r.get(2)?,
                        sha256: r.get(3)?,
                        ingested_at: r.get(4)?,
                    })
                },
            )
            .ok();
        if let Some(d) = existing {
            return Ok(d);
        }
        let id = Uuid::new_v4().to_string();
        let now = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .map_err(|e| KnowledgeError::EmbeddingFailed(format!("time format: {e}")))?;
        self.conn.execute(
            "INSERT INTO documents (id, workspace_id, path, sha256, ingested_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, workspace_id, path, sha256, now],
        )?;
        Ok(DocumentRow {
            id,
            workspace_id: workspace_id.to_string(),
            path: path.to_string(),
            sha256: sha256.to_string(),
            ingested_at: now,
        })
    }

    /// chunk + 임베딩을 일괄 저장. transaction으로 atomic.
    /// chunk row id는 (document_id + chunk.id) 복합 — 같은 chunk content가 여러 워크스페이스/문서에
    /// 들어가도 PK 충돌이 없고, 동일 (doc_id, chunk_id) 재투입은 INSERT OR IGNORE로 idempotent.
    pub fn add_chunks(
        &mut self,
        document_id: &str,
        workspace_id: &str,
        chunks: &[Chunk],
        embeddings: &[Vec<f32>],
    ) -> Result<(), KnowledgeError> {
        if chunks.len() != embeddings.len() {
            return Err(KnowledgeError::EmbeddingFailed(format!(
                "chunks({}) vs embeddings({}) 길이 불일치",
                chunks.len(),
                embeddings.len()
            )));
        }
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO chunks
                 (id, document_id, workspace_id, content, embedding, start, end)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for (chunk, emb) in chunks.iter().zip(embeddings.iter()) {
                let row_id = format!("{document_id}:{}", chunk.id);
                let blob = encode_embedding(emb);
                stmt.execute(params![
                    row_id,
                    document_id,
                    workspace_id,
                    chunk.content,
                    blob,
                    chunk.start as i64,
                    chunk.end as i64,
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// workspace_id + query_embedding으로 cosine top-k 반환.
    /// k=0이면 빈 Vec.
    pub fn search(
        &self,
        workspace_id: &str,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<SearchHit>, KnowledgeError> {
        if k == 0 || query.is_empty() {
            return Ok(Vec::new());
        }
        let mut stmt = self.conn.prepare(
            "SELECT id, document_id, workspace_id, content, embedding, start, end
             FROM chunks WHERE workspace_id = ?1",
        )?;
        let rows = stmt.query_map(params![workspace_id], |r| {
            let blob: Vec<u8> = r.get(4)?;
            Ok((
                ChunkRow {
                    id: r.get(0)?,
                    document_id: r.get(1)?,
                    workspace_id: r.get(2)?,
                    content: r.get(3)?,
                    start: r.get::<_, i64>(5)? as usize,
                    end: r.get::<_, i64>(6)? as usize,
                },
                blob,
            ))
        })?;

        // Min-heap of size k (push, pop smallest if exceed).
        let mut heap: BinaryHeap<RankedHit> = BinaryHeap::with_capacity(k + 1);
        let q_norm = l2_norm(query);
        for row in rows {
            let (chunk, blob) = row?;
            let emb = decode_embedding(&blob);
            let score = if emb.is_empty() || query.len() != emb.len() {
                0.0
            } else {
                cosine_normalized(query, &emb, q_norm)
            };
            heap.push(RankedHit {
                score,
                hit: SearchHit { chunk, score },
            });
            if heap.len() > k {
                // Drop smallest (Reverse heap → smallest at top).
                heap.pop();
            }
        }
        // Heap is min-heap of top-k; sort descending.
        let mut out: Vec<SearchHit> = heap.into_iter().map(|r| r.hit).collect();
        out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
        Ok(out)
    }

    /// 워크스페이스 ID 존재 확인 — 외부에서 호출.
    pub fn has_workspace(&self, workspace_id: &str) -> Result<bool, KnowledgeError> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM workspaces WHERE id = ?1",
            params![workspace_id],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// workspace의 document 수.
    pub fn document_count(&self, workspace_id: &str) -> Result<usize, KnowledgeError> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM documents WHERE workspace_id = ?1",
            params![workspace_id],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    /// workspace의 chunk 수.
    pub fn chunk_count(&self, workspace_id: &str) -> Result<usize, KnowledgeError> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM chunks WHERE workspace_id = ?1",
            params![workspace_id],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    /// 문서 경로 조회 — workspace_id 격리. 없거나 다른 workspace 소유면 `None`.
    ///
    /// 정책 (Phase 8'.a.1):
    /// - workspace_id 필터링은 schema-level 격리 정책의 일부 — 다른 workspace의 document_id로
    ///   조회해도 None 반환.
    /// - DB 오류는 `Err` 전파 (rusqlite Error → `KnowledgeError::Db`). row 부재는 `Ok(None)`.
    /// - `path` 컬럼은 documents 테이블에 NOT NULL — 빈 string 가능성은 caller 보호용 fallback에서 처리.
    pub fn get_document_path(
        &self,
        workspace_id: &str,
        document_id: &str,
    ) -> Result<Option<PathBuf>, KnowledgeError> {
        let mut stmt = self
            .conn
            .prepare("SELECT path FROM documents WHERE workspace_id = ?1 AND id = ?2 LIMIT 1")?;
        let mut rows = stmt.query(params![workspace_id, document_id])?;
        if let Some(row) = rows.next()? {
            let p: String = row.get(0)?;
            Ok(Some(PathBuf::from(p)))
        } else {
            Ok(None)
        }
    }
}

/// min-heap에 score 기준 정렬을 위한 wrapper.
struct RankedHit {
    score: f32,
    hit: SearchHit,
}

impl PartialEq for RankedHit {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
    }
}
impl Eq for RankedHit {}
impl PartialOrd for RankedHit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for RankedHit {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse: BinaryHeap는 max-heap이니, *-1 효과를 줘 min-heap처럼 사용.
        other
            .score
            .partial_cmp(&self.score)
            .unwrap_or(Ordering::Equal)
    }
}

/// WAL + busy_timeout + synchronous=NORMAL — 안정성 PRAGMA (Phase 8'.0.b).
///
/// `journal_mode = WAL`은 결과 row를 반환하므로 `query_row`로 명시 처리.
fn apply_stability_pragmas(conn: &Connection) -> Result<(), KnowledgeError> {
    let _: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
    conn.busy_timeout(std::time::Duration::from_millis(5000))?;
    conn.execute_batch("PRAGMA synchronous = NORMAL;")?;
    Ok(())
}

/// Phase R-B (ADR-0053) — SQLCipher PRAGMA key 적용. 모든 read/write보다 먼저 호출.
///
/// `sqlcipher` feature OFF 빌드(stock SQLite)에서는 unknown pragma로 무시. 키가 맞는지는
/// 직후 sqlite_master 조회로 검증 (잘못된 키면 NotADatabase 에러).
fn apply_passphrase(conn: &Connection, passphrase: &str) -> Result<(), KnowledgeError> {
    let escaped = passphrase.replace('\'', "''");
    conn.execute_batch(&format!("PRAGMA key = '{escaped}'"))?;
    let _: i64 = conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0))?;
    Ok(())
}

fn encode_embedding(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for &x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

fn decode_embedding(bytes: &[u8]) -> Vec<f32> {
    let mut out = Vec::with_capacity(bytes.len() / 4);
    let mut i = 0;
    while i + 4 <= bytes.len() {
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&bytes[i..i + 4]);
        out.push(f32::from_le_bytes(buf));
        i += 4;
    }
    out
}

fn l2_norm(v: &[f32]) -> f32 {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n < f32::EPSILON {
        1.0
    } else {
        n
    }
}

/// cosine similarity (with caller-provided q_norm). 두 벡터 차원이 같다는 사전 가정.
/// 결과는 [-1, 1]에서 [0, 1]로 매핑 ((x + 1) / 2).
fn cosine_normalized(a: &[f32], b: &[f32], a_norm: f32) -> f32 {
    let b_norm = l2_norm(b);
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let denom = a_norm * b_norm;
    if denom < f32::EPSILON {
        return 0.5;
    }
    let cos = dot / denom;
    let mapped = (cos + 1.0) / 2.0;
    mapped.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunker::Chunk;

    fn mk_chunk(id: &str, content: &str) -> Chunk {
        Chunk {
            id: id.into(),
            content: content.into(),
            start: 0,
            end: content.chars().count(),
        }
    }

    fn mk_emb(seed: f32) -> Vec<f32> {
        (0..8).map(|i| seed + i as f32).collect()
    }

    #[test]
    fn open_and_init_creates_schema() {
        let store = KnowledgeStore::open_memory().unwrap();
        // workspaces 테이블 존재 확인.
        let n: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='workspaces'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn add_workspace_document_chunks_search() {
        let mut store = KnowledgeStore::open_memory().unwrap();
        let ws = store.add_workspace("ws-1").unwrap();
        let doc = store.add_document(&ws.id, "/tmp/a.md", "sha-aaa").unwrap();
        let chunks = vec![
            mk_chunk("c1", "안녕 첫 chunk"),
            mk_chunk("c2", "두 번째 chunk"),
        ];
        let embeds = vec![mk_emb(1.0), mk_emb(2.0)];
        store.add_chunks(&doc.id, &ws.id, &chunks, &embeds).unwrap();
        let hits = store.search(&ws.id, &mk_emb(1.0), 2).unwrap();
        assert_eq!(hits.len(), 2);
        // top-1은 self-match (cosine=1).
        assert_eq!(hits[0].chunk.content, "안녕 첫 chunk");
    }

    #[test]
    fn search_returns_top_k_only() {
        let mut store = KnowledgeStore::open_memory().unwrap();
        let ws = store.add_workspace("ws-1").unwrap();
        let doc = store.add_document(&ws.id, "/tmp/a.md", "sha-aaa").unwrap();
        let chunks: Vec<Chunk> = (0..10)
            .map(|i| mk_chunk(&format!("c{i}"), &format!("chunk {i}")))
            .collect();
        let embeds: Vec<Vec<f32>> = (0..10).map(|i| mk_emb(i as f32)).collect();
        store.add_chunks(&doc.id, &ws.id, &chunks, &embeds).unwrap();
        let hits = store.search(&ws.id, &mk_emb(0.0), 3).unwrap();
        assert_eq!(hits.len(), 3);
    }

    #[test]
    fn search_filters_by_workspace_id() {
        let mut store = KnowledgeStore::open_memory().unwrap();
        let ws_a = store.add_workspace("A").unwrap();
        let ws_b = store.add_workspace("B").unwrap();
        let doc_a = store.add_document(&ws_a.id, "a.md", "sha-a").unwrap();
        let doc_b = store.add_document(&ws_b.id, "b.md", "sha-b").unwrap();
        store
            .add_chunks(
                &doc_a.id,
                &ws_a.id,
                &[mk_chunk("c-a", "A workspace chunk")],
                &[mk_emb(1.0)],
            )
            .unwrap();
        store
            .add_chunks(
                &doc_b.id,
                &ws_b.id,
                &[mk_chunk("c-b", "B workspace chunk")],
                &[mk_emb(1.0)],
            )
            .unwrap();
        let hits_a = store.search(&ws_a.id, &mk_emb(1.0), 5).unwrap();
        assert_eq!(hits_a.len(), 1);
        assert_eq!(hits_a[0].chunk.content, "A workspace chunk");
        let hits_b = store.search(&ws_b.id, &mk_emb(1.0), 5).unwrap();
        assert_eq!(hits_b.len(), 1);
        assert_eq!(hits_b[0].chunk.content, "B workspace chunk");
    }

    #[test]
    fn search_score_in_range() {
        let mut store = KnowledgeStore::open_memory().unwrap();
        let ws = store.add_workspace("ws").unwrap();
        let doc = store.add_document(&ws.id, "/x", "sha-x").unwrap();
        store
            .add_chunks(&doc.id, &ws.id, &[mk_chunk("c", "x")], &[mk_emb(1.0)])
            .unwrap();
        let hits = store.search(&ws.id, &mk_emb(1.0), 1).unwrap();
        assert!(hits[0].score >= 0.0 && hits[0].score <= 1.0);
    }

    #[test]
    fn duplicate_document_returns_existing() {
        let store = KnowledgeStore::open_memory().unwrap();
        let ws = store.add_workspace("ws").unwrap();
        let d1 = store.add_document(&ws.id, "/a", "sha-1").unwrap();
        let d2 = store.add_document(&ws.id, "/a", "sha-1").unwrap();
        assert_eq!(d1.id, d2.id);
    }

    #[test]
    fn search_with_k_zero_returns_empty() {
        let mut store = KnowledgeStore::open_memory().unwrap();
        let ws = store.add_workspace("ws").unwrap();
        let doc = store.add_document(&ws.id, "/x", "sha-x").unwrap();
        store
            .add_chunks(&doc.id, &ws.id, &[mk_chunk("c", "x")], &[mk_emb(1.0)])
            .unwrap();
        let hits = store.search(&ws.id, &mk_emb(1.0), 0).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn add_document_unknown_workspace_errors() {
        let store = KnowledgeStore::open_memory().unwrap();
        let err = store.add_document("missing-ws", "/x", "sha").unwrap_err();
        assert!(matches!(err, KnowledgeError::WorkspaceNotFound(_)));
    }

    #[test]
    fn embedding_length_mismatch_errors() {
        let mut store = KnowledgeStore::open_memory().unwrap();
        let ws = store.add_workspace("ws").unwrap();
        let doc = store.add_document(&ws.id, "/a", "sha").unwrap();
        let chunks = vec![mk_chunk("c1", "a"), mk_chunk("c2", "b")];
        let embeds = vec![mk_emb(1.0)]; // 1 vs 2.
        let err = store
            .add_chunks(&doc.id, &ws.id, &chunks, &embeds)
            .unwrap_err();
        assert!(matches!(err, KnowledgeError::EmbeddingFailed(_)));
    }

    #[test]
    fn cosine_self_match_is_one() {
        let v = vec![1.0_f32, 2.0, 3.0];
        let n = l2_norm(&v);
        let s = cosine_normalized(&v, &v, n);
        assert!((s - 1.0).abs() < 1e-5);
    }

    #[test]
    fn embedding_round_trip() {
        let v = vec![0.1f32, -0.2, 0.3, -0.4];
        let blob = encode_embedding(&v);
        let back = decode_embedding(&blob);
        assert_eq!(v, back);
    }

    #[test]
    fn open_file_uses_wal_journal() {
        // file-backed DB는 WAL 모드 활성. Phase 8'.0.b 안정성 검증.
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("knowledge.db");
        let store = KnowledgeStore::open(&path).unwrap();
        let mode = store.journal_mode().unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
    }

    // ── Phase R-B (ADR-0053) — SQLCipher path ──────────────────────────

    #[test]
    fn open_with_passphrase_creates_db_when_feature_gated() {
        // sqlcipher feature ON: 새 암호화 DB 생성 + schema 초기화.
        // sqlcipher feature OFF (stock SQLite): PRAGMA key 무시, 평문 DB 생성. 둘 다 OK.
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("kdb.db");
        let store = KnowledgeStore::open_with_passphrase(&path, "passphrase-aaaaaaaaaaaaaaaaa")
            .expect("open_with_passphrase should create new DB");
        let mode = store.journal_mode().unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
    }

    /// SQLCipher 활성 빌드에서만 — stock SQLite는 PRAGMA key를 무시해 잘못된 passphrase여도 통과.
    #[cfg(feature = "sqlcipher")]
    #[test]
    fn open_wrong_passphrase_fails_on_existing_encrypted_db() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("kdb.db");
        // 1) 첫 open — passphrase A로 schema + workspace 1건.
        {
            let mut s = KnowledgeStore::open_with_passphrase(&path, "passphrase-aaaaaaaaaaaaaaaaa")
                .unwrap();
            s.add_workspace("ws-encrypted").unwrap();
        }
        // 2) 잘못된 passphrase로 open 시도 — 에러여야.
        let wrong = KnowledgeStore::open_with_passphrase(&path, "passphrase-bbbbbbbbbbbbbbbbb");
        assert!(wrong.is_err(), "wrong passphrase는 NotADatabase로 실패해야");
    }

    // ── Phase 8'.a.1 — get_document_path ────────────────────────────

    #[test]
    fn get_document_path_returns_path_for_existing_document() {
        let store = KnowledgeStore::open_memory().unwrap();
        let ws = store.add_workspace("ws").unwrap();
        let doc = store
            .add_document(&ws.id, "/tmp/한국어 문서.md", "sha-1")
            .unwrap();
        let path = store.get_document_path(&ws.id, &doc.id).unwrap();
        assert_eq!(path, Some(PathBuf::from("/tmp/한국어 문서.md")));
    }

    #[test]
    fn get_document_path_returns_none_for_missing_document_id() {
        let store = KnowledgeStore::open_memory().unwrap();
        let ws = store.add_workspace("ws").unwrap();
        // workspace는 있지만 doc id는 없음 — None.
        let path = store.get_document_path(&ws.id, "missing-doc-id").unwrap();
        assert!(path.is_none());
    }

    #[test]
    fn get_document_path_filters_cross_workspace() {
        let store = KnowledgeStore::open_memory().unwrap();
        let ws_a = store.add_workspace("A").unwrap();
        let ws_b = store.add_workspace("B").unwrap();
        let doc_a = store.add_document(&ws_a.id, "/a.md", "sha-a").unwrap();
        // ws_a의 doc을 ws_b id로 조회 → None (격리).
        let path = store.get_document_path(&ws_b.id, &doc_a.id).unwrap();
        assert!(
            path.is_none(),
            "다른 workspace의 document_id는 보이면 안 돼요"
        );
        // ws_a로 조회 시는 정상 반환.
        let path_ok = store.get_document_path(&ws_a.id, &doc_a.id).unwrap();
        assert_eq!(path_ok, Some(PathBuf::from("/a.md")));
    }
}
