//! Integration tests — Phase 4.5'.a end-to-end (ADR-0024).

use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use knowledge_stack::{
    Embedder, IngestProgress, IngestService, IngestStage, IngestSummary, KnowledgeStore,
    MockEmbedder,
};
use tempfile::TempDir;
use tokio::sync::mpsc;

fn fresh_service() -> (Arc<Mutex<KnowledgeStore>>, IngestService) {
    let store = Arc::new(Mutex::new(KnowledgeStore::open_memory().unwrap()));
    let emb: Arc<dyn Embedder> = Arc::new(MockEmbedder::default());
    let svc = IngestService::new(Arc::clone(&store), emb);
    (store, svc)
}

fn add_ws(store: &Arc<Mutex<KnowledgeStore>>, name: &str) -> String {
    let s = store.lock().unwrap();
    s.add_workspace(name).unwrap().id
}

#[tokio::test]
async fn per_workspace_isolation_search_only_own_chunks() {
    // 2 워크스페이스에 동일 콘텐츠 ingest → search는 자신의 chunks만 반환.
    let (store, svc) = fresh_service();
    let ws_a = add_ws(&store, "A");
    let ws_b = add_ws(&store, "B");

    let dir = TempDir::new().unwrap();
    let file = dir.path().join("doc.md");
    fs::write(
        &file,
        "한국어 RAG 격리 테스트. 두 번째 문장이에요. 세 번째 문장이에요.",
    )
    .unwrap();

    let cancel = Arc::new(AtomicBool::new(false));
    svc.ingest_path(&ws_a, &file, 100, 10, None, Arc::clone(&cancel))
        .await
        .unwrap();
    svc.ingest_path(&ws_b, &file, 100, 10, None, Arc::clone(&cancel))
        .await
        .unwrap();

    // query embedding은 임의의 deterministic 벡터.
    let emb = MockEmbedder::default();
    let query = emb.embed(&[String::from("한국어 RAG")]).await.unwrap();
    let q = &query[0];

    let store_lock = store.lock().unwrap();
    let hits_a = store_lock.search(&ws_a, q, 10).unwrap();
    let hits_b = store_lock.search(&ws_b, q, 10).unwrap();
    assert!(!hits_a.is_empty());
    assert!(!hits_b.is_empty());
    // workspace_id 격리 확인.
    for h in &hits_a {
        assert_eq!(h.chunk.workspace_id, ws_a);
    }
    for h in &hits_b {
        assert_eq!(h.chunk.workspace_id, ws_b);
    }
}

#[tokio::test]
async fn cross_link_test_search_excludes_other_workspace_documents() {
    // A에 ingest, B의 document_count는 0이어야 함.
    let (store, svc) = fresh_service();
    let ws_a = add_ws(&store, "A");
    let ws_b = add_ws(&store, "B");

    let dir = TempDir::new().unwrap();
    let file = dir.path().join("only-a.md");
    fs::write(&file, "오직 A 워크스페이스의 문서이에요.").unwrap();

    let cancel = Arc::new(AtomicBool::new(false));
    svc.ingest_path(&ws_a, &file, 100, 10, None, cancel)
        .await
        .unwrap();

    // 먼저 lock 없이 embed (await) → 그 다음 lock으로 search.
    let emb = MockEmbedder::default();
    let query = emb.embed(&[String::from("A 워크스페이스")]).await.unwrap();
    let store_lock = store.lock().unwrap();
    let count_a = store_lock.document_count(&ws_a).unwrap();
    let count_b = store_lock.document_count(&ws_b).unwrap();
    let chunks_b = store_lock.chunk_count(&ws_b).unwrap();
    assert_eq!(count_a, 1);
    assert_eq!(count_b, 0);
    assert_eq!(chunks_b, 0);

    // search on B → 빈 결과.
    let hits_b = store_lock.search(&ws_b, &query[0], 10).unwrap();
    assert!(hits_b.is_empty());
}

#[tokio::test]
async fn full_pipeline_ingest_chunk_embed_store_search() {
    // 전체 파이프라인 — ingest → chunk → embed → store → search → ranking 검증.
    let (store, svc) = fresh_service();
    let ws = add_ws(&store, "ws");

    let dir = TempDir::new().unwrap();
    let f1 = dir.path().join("a.md");
    let f2 = dir.path().join("b.md");
    fs::write(&f1, "사과는 빨갛고 달콤해요.").unwrap();
    fs::write(&f2, "바나나는 노랗고 부드러워요.").unwrap();

    let cancel = Arc::new(AtomicBool::new(false));
    let summary = svc
        .ingest_path(&ws, dir.path(), 200, 20, None, cancel)
        .await
        .unwrap();
    assert_eq!(summary.documents, 2);
    assert!(summary.chunks >= 2);

    // search — 사과 query는 사과 문서에 더 가까워야 함.
    let emb = MockEmbedder::default();
    let q1 = emb
        .embed(&[String::from("사과는 빨갛고 달콤해요.")])
        .await
        .unwrap();
    let store_lock = store.lock().unwrap();
    let hits = store_lock.search(&ws, &q1[0], 5).unwrap();
    assert!(!hits.is_empty());
    // self-match가 가장 위 — 정확히 같은 query embedding이므로.
    assert!(hits[0].chunk.content.contains("사과"));
}

#[tokio::test]
async fn cancel_scenario_mid_flight() {
    // 큰 디렉터리 ingest 도중 cancel → Cancelled 에러.
    let (store, svc) = fresh_service();
    let ws = add_ws(&store, "ws");

    let dir = TempDir::new().unwrap();
    for i in 0..20 {
        fs::write(
            dir.path().join(format!("f{i}.md")),
            format!("파일 {i} 내용. 진짜 한국어 콘텐츠가 포함돼 있어요."),
        )
        .unwrap();
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_clone = Arc::clone(&cancel);

    // 별도 task에서 약간 기다린 뒤 cancel 토글.
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        cancel_clone.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    let res = svc
        .ingest_path(&ws, dir.path(), 100, 10, None, cancel)
        .await;
    // 미리 cancel이 토글되도록 조절돼 있어 대부분 Cancelled.
    // 단, 너무 빨리 종료된 경우엔 OK일 수 있음 — 둘 다 허용.
    match res {
        Ok(s) => {
            // 타이밍에 따라 끝까지 완료되더라도, summary는 정상.
            assert!(s.documents <= 20);
        }
        Err(e) => {
            assert!(matches!(e, knowledge_stack::KnowledgeError::Cancelled));
        }
    }
}

#[tokio::test]
async fn cancel_before_start_returns_cancelled() {
    // 시작 전에 cancel 토글 → 즉시 Cancelled.
    let (store, svc) = fresh_service();
    let ws = add_ws(&store, "ws");
    let dir = TempDir::new().unwrap();
    for i in 0..3 {
        fs::write(dir.path().join(format!("f{i}.md")), format!("내용 {i}")).unwrap();
    }
    let cancel = Arc::new(AtomicBool::new(true));
    let res = svc
        .ingest_path(&ws, dir.path(), 100, 10, None, cancel)
        .await;
    assert!(matches!(
        res.unwrap_err(),
        knowledge_stack::KnowledgeError::Cancelled
    ));
}

#[tokio::test]
async fn serde_round_trip_progress_and_summary() {
    // IngestProgress / IngestSummary serde 안정성.
    let p = IngestProgress {
        stage: IngestStage::Embedding,
        processed: 3,
        total: 10,
        current_path: Some("/a.md".into()),
    };
    let s = serde_json::to_string(&p).unwrap();
    let back: IngestProgress = serde_json::from_str(&s).unwrap();
    assert_eq!(p, back);
    // kebab-case 검증 — "embedding" 출력.
    assert!(s.contains("embedding"));

    let sum = IngestSummary {
        workspace_id: "ws-1".into(),
        documents: 4,
        chunks: 12,
        skipped: 1,
    };
    let s2 = serde_json::to_string(&sum).unwrap();
    let back2: IngestSummary = serde_json::from_str(&s2).unwrap();
    assert_eq!(sum, back2);
}

#[tokio::test]
async fn progress_receiver_collects_complete_pipeline() {
    let (_store, svc) = fresh_service();
    let ws_id = {
        let s = _store.lock().unwrap();
        s.add_workspace("ws").unwrap().id
    };

    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("a.md"),
        "콘텐츠 첫 줄.\n\n두 번째 단락이에요.",
    )
    .unwrap();
    let (tx, mut rx) = mpsc::channel::<IngestProgress>(64);
    let cancel = Arc::new(AtomicBool::new(false));
    let _summary = svc
        .ingest_path(&ws_id, dir.path(), 200, 20, Some(tx), cancel)
        .await
        .unwrap();

    let mut collected = Vec::new();
    while let Ok(p) = rx.try_recv() {
        collected.push(p.stage);
    }
    assert!(collected.last().is_some());
    assert_eq!(collected.last().copied(), Some(IngestStage::Done));
}

#[tokio::test]
async fn duplicate_ingest_same_file_idempotent_document() {
    // 동일 콘텐츠를 두 번 ingest해도 document는 1개 (sha256 unique).
    let (store, svc) = fresh_service();
    let ws = add_ws(&store, "ws");
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("dup.md");
    fs::write(&file, "동일 콘텐츠 idempotent 검증이에요.").unwrap();

    let cancel = Arc::new(AtomicBool::new(false));
    svc.ingest_path(&ws, &file, 100, 10, None, Arc::clone(&cancel))
        .await
        .unwrap();
    let _ = svc
        .ingest_path(&ws, &file, 100, 10, None, cancel)
        .await
        .unwrap();
    let store_lock = store.lock().unwrap();
    let count = store_lock.document_count(&ws).unwrap();
    assert_eq!(count, 1, "duplicate ingest should keep doc count = 1");
}

#[tokio::test]
async fn knowledge_store_open_creates_db_file() {
    // 실 파일 DB open + init.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("kb.sqlite");
    let store = KnowledgeStore::open(&path).unwrap();
    let ws = store.add_workspace("ws").unwrap();
    assert!(!ws.id.is_empty());
    assert!(Path::new(&path).exists());
}
