//! Trends 요약 IPC — Phase 22'.e.2 (ADR-0060 §6).
//!
//! 정책:
//! - `summarize_trends(items, force_refresh)` Tauri command — cache hit이면 즉시 반환,
//!   miss이면 Mock Summarizer로 생성 + cache put.
//! - 실 ollama / lm-studio adapter Summarizer impl은 22'.e.3에서 (현재는 MockSummarizer).
//! - SQLite 캐시: `app_data_dir/trends-summary.db`, schema_version 1, TTL 30일.
//! - 4B+ 모델 게이트 — caller (frontend Trends.tsx)가 사전 검사. 본 IPC는 검증 X (단순함).

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use trend_summarizer::{
    cache_key, summarize_bundle, MockSummarizer, SummarizerError, SummaryInput, TrendsSummary,
};

// ───────────────────────────────────────────────────────────────────
// Tauri error type
// ───────────────────────────────────────────────────────────────────

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum TrendsApiError {
    #[error("요약 캐시 저장소를 열 수 없어요: {message}")]
    StoreOpen { message: String },

    #[error("요약 캐시 작업이 실패했어요: {message}")]
    StoreFailed { message: String },

    #[error("로컬 LLM 요약에 실패했어요: {message}")]
    SummaryFailed { message: String },

    #[error("내부 에러가 발생했어요: {message}")]
    Internal { message: String },
}

impl From<SummarizerError> for TrendsApiError {
    fn from(err: SummarizerError) -> Self {
        Self::SummaryFailed {
            message: err.to_string(),
        }
    }
}

impl From<rusqlite::Error> for TrendsApiError {
    fn from(err: rusqlite::Error) -> Self {
        Self::StoreFailed {
            message: err.to_string(),
        }
    }
}

// ───────────────────────────────────────────────────────────────────
// SQLite 캐시 저장소
// ───────────────────────────────────────────────────────────────────

/// 요약 캐시 저장소 — `app_data_dir/trends-summary.db`.
pub struct TrendsSummaryStore {
    conn: StdMutex<Connection>,
}

impl TrendsSummaryStore {
    /// 파일 경로에서 store 열기. parent 디렉터리 자동 생성.
    pub fn open(path: &Path) -> Result<Self, TrendsApiError> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path).map_err(|e| TrendsApiError::StoreOpen {
            message: format!("{}: {e}", path.display()),
        })?;
        // 안정성 PRAGMA — knowledge-stack 패턴 정합.
        conn.busy_timeout(std::time::Duration::from_millis(5000))?;
        let _: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
        conn.execute_batch("PRAGMA synchronous = NORMAL")?;

        // schema.
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS summary (
                cache_key      TEXT PRIMARY KEY,
                model_kind     TEXT NOT NULL,
                schema_version INTEGER NOT NULL,
                sections_json  TEXT NOT NULL,
                created_at     TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_summary_created ON summary(created_at);
            "#,
        )?;
        Ok(Self {
            conn: StdMutex::new(conn),
        })
    }

    /// `cache_key`로 캐시된 요약 조회. 30일 TTL — 오래된 entry는 None 반환.
    pub fn get(&self, cache_key: &str) -> Result<Option<TrendsSummary>, TrendsApiError> {
        let conn = self.conn.lock().map_err(|_| TrendsApiError::Internal {
            message: "store mutex poisoned".into(),
        })?;
        let row: Option<(String, u32, String, String)> = conn
            .query_row(
                "SELECT model_kind, schema_version, sections_json, created_at
                 FROM summary WHERE cache_key = ?1",
                params![cache_key],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .ok();
        let Some((model_kind, schema_version, sections_json, created_at)) = row else {
            return Ok(None);
        };

        // 30일 TTL — created_at 이후 30일이 지났으면 stale.
        if is_stale(&created_at, 30) {
            return Ok(None);
        }

        let sections =
            serde_json::from_str(&sections_json).map_err(|e| TrendsApiError::StoreFailed {
                message: format!("sections_json parse: {e}"),
            })?;

        Ok(Some(TrendsSummary {
            schema_version,
            sections,
            model_kind,
            cache_key: cache_key.to_string(),
        }))
    }

    /// 요약 저장. 동일 cache_key 중복은 INSERT OR REPLACE.
    pub fn put(&self, summary: &TrendsSummary) -> Result<(), TrendsApiError> {
        let conn = self.conn.lock().map_err(|_| TrendsApiError::Internal {
            message: "store mutex poisoned".into(),
        })?;
        let now =
            OffsetDateTime::now_utc()
                .format(&Rfc3339)
                .map_err(|e| TrendsApiError::Internal {
                    message: format!("time format: {e}"),
                })?;
        let sections_json =
            serde_json::to_string(&summary.sections).map_err(|e| TrendsApiError::StoreFailed {
                message: format!("sections serialize: {e}"),
            })?;
        conn.execute(
            "INSERT OR REPLACE INTO summary
             (cache_key, model_kind, schema_version, sections_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                summary.cache_key,
                summary.model_kind,
                summary.schema_version,
                sections_json,
                now
            ],
        )?;
        Ok(())
    }
}

/// `created_at` (RFC3339)이 `days` 일 이전이면 true.
fn is_stale(created_at: &str, days: i64) -> bool {
    let parsed = OffsetDateTime::parse(created_at, &Rfc3339);
    let Ok(parsed) = parsed else {
        return true; // parse 실패 = stale 취급.
    };
    let age = OffsetDateTime::now_utc() - parsed;
    age > time::Duration::days(days)
}

// ───────────────────────────────────────────────────────────────────
// Tauri 시작 시 store provision
// ───────────────────────────────────────────────────────────────────

/// `app_data_dir/trends-summary.db` 경로 결정.
fn resolve_summary_db_path(app: &AppHandle) -> Result<PathBuf, TrendsApiError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| TrendsApiError::StoreOpen {
            message: format!("app_data_dir: {e}"),
        })?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| TrendsApiError::StoreOpen {
            message: format!("mkdir: {e}"),
        })?;
    }
    Ok(dir.join("trends-summary.db"))
}

/// setup에서 호출 — Arc<TrendsSummaryStore>를 manage.
pub fn provision_trends_summary_store(
    app: &AppHandle,
) -> Result<Arc<TrendsSummaryStore>, TrendsApiError> {
    let path = resolve_summary_db_path(app)?;
    let store = TrendsSummaryStore::open(&path)?;
    Ok(Arc::new(store))
}

// ───────────────────────────────────────────────────────────────────
// Tauri command
// ───────────────────────────────────────────────────────────────────

/// 트렌드 항목 요약 — cache hit이면 즉시, miss이면 LLM 호출.
///
/// 정책 (Phase 22'.e.2):
/// - `force_refresh = true`이면 cache 무시하고 새로 생성.
/// - 본 cut은 *MockSummarizer* 사용 (.e.3에서 ollama/lm-studio adapter inject).
/// - SQLite cache miss → MockSummarizer.complete() → cache put → 반환.
#[tauri::command]
pub async fn summarize_trends(
    items: Vec<SummaryInput>,
    force_refresh: bool,
    store: State<'_, Arc<TrendsSummaryStore>>,
) -> Result<TrendsSummary, TrendsApiError> {
    let summarizer = MockSummarizer::new();
    let model_kind = summarizer.model_kind.clone();
    let key = cache_key(&items, &model_kind);

    // cache hit & !force_refresh → 즉시 반환.
    if !force_refresh {
        let store_clone = Arc::clone(&store);
        let key_clone = key.clone();
        let cached = tokio::task::spawn_blocking(move || store_clone.get(&key_clone))
            .await
            .map_err(|e| TrendsApiError::Internal {
                message: format!("cache get join: {e}"),
            })??;
        if let Some(s) = cached {
            tracing::info!(cache_key = %key, "trends summary cache hit");
            return Ok(s);
        }
    }

    // miss → 새로 생성.
    let summary = summarize_bundle(&items, &summarizer).await?;

    // cache put — blocking.
    let store_clone = Arc::clone(&store);
    let summary_clone = summary.clone();
    tokio::task::spawn_blocking(move || store_clone.put(&summary_clone))
        .await
        .map_err(|e| TrendsApiError::Internal {
            message: format!("cache put join: {e}"),
        })??;

    Ok(summary)
}

// 단위 테스트는 lmmaster-desktop crate에서 webview DLL 의존으로 실행 불가 (기존 정책).
// store CRUD invariant은 cargo check + workspace clippy로 컴파일 검증.
// SQLite + serde + TrendsSummary round-trip은 trend-summarizer crate의 18 invariant로 검증.
