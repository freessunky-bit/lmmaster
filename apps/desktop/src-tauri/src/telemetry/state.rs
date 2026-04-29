//! Telemetry config + opt-in toggle + 영속 — Phase 7'.a 원본 + Phase 7'.b queue 통합.
//!
//! 정책 (ADR-0027 §5, phase-7p-release-prep-reinforcement.md §5.2):
//! - 기본 비활성. 사용자가 명시적으로 opt-in 했을 때만 익명 사용 통계 전송.
//! - 영속 위치: `app_data_dir/telemetry/config.json`. 디스크 실패 시 메모리만 유지.
//! - 익명 UUID는 첫 opt-in 시 1회 생성 — 사용자 PC 단위 고정 식별자(개인 식별 X).
//! - 비활성 → 활성 토글 시 UUID 발급 + opted_in_at 기록. 활성 → 비활성 시 UUID 보존(재활성 시 재사용).
//! - Phase 7'.b: opt-in 시 EventQueue 통합 — submit_event는 endpoint 미설정 시 queue 적재만.
//! - 한국어 해요체 에러 메시지.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;
use thiserror::Error;
use time::OffsetDateTime;
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

use super::submit::{EventLevel, EventQueue, EventSubmitOutcome, TelemetryEvent};

const CONFIG_DIR_NAME: &str = "telemetry";
const CONFIG_FILE_NAME: &str = "config.json";

// ───────────────────────────────────────────────────────────────────
// DTO — frontend 미러
// ───────────────────────────────────────────────────────────────────

/// Telemetry config — 사용자가 보는 상태.
///
/// `enabled = false`(default)일 때는 익명 통계가 전혀 수집·전송되지 않아요.
/// 활성 시점에 `anon_id`(UUID v4) + `opted_in_at`(RFC3339)을 기록해요.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TelemetryConfig {
    /// 기본 false. 사용자가 opt-in해야 true.
    pub enabled: bool,
    /// 익명 UUID. opt-in 한 번이라도 했으면 보존(재활성 시 재사용).
    pub anon_id: Option<String>,
    /// 첫 opt-in 시각. RFC3339 ISO.
    pub opted_in_at: Option<String>,
}

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum TelemetryApiError {
    #[error("텔레메트리 설정 파일을 읽지 못했어요: {message}")]
    PersistFailed { message: String },
    #[error("텔레메트리 설정을 처리할 수 없어요: {message}")]
    Internal { message: String },
    #[error("텔레메트리가 비활성 상태여서 이벤트를 보낼 수 없어요. 먼저 옵트인해 주세요.")]
    NotEnabled,
    #[error("이벤트 메시지가 비어 있어요")]
    EmptyMessage,
}

// ───────────────────────────────────────────────────────────────────
// State
// ───────────────────────────────────────────────────────────────────

/// `app_data_dir`이 있으면 디스크 영속, 없으면 메모리만.
pub struct TelemetryState {
    inner: AsyncMutex<TelemetryConfig>,
    config_path: Option<PathBuf>,
    /// Phase 7'.b — 이벤트 큐. opt-in 시 panic / 명시 호출이 적재.
    event_queue: Arc<EventQueue>,
}

impl TelemetryState {
    pub fn new(app_data_dir: Option<PathBuf>) -> Self {
        let config_path = app_data_dir.map(|d| d.join(CONFIG_DIR_NAME).join(CONFIG_FILE_NAME));
        let initial = match config_path.as_deref() {
            Some(p) => load_from_disk(p).unwrap_or_default(),
            None => TelemetryConfig::default(),
        };
        Self {
            inner: AsyncMutex::new(initial),
            config_path,
            event_queue: Arc::new(EventQueue::new_default()),
        }
    }

    /// Phase 7'.b 테스트용: 외부에서 EventQueue를 주입.
    #[cfg(test)]
    pub fn with_queue(app_data_dir: Option<PathBuf>, queue: Arc<EventQueue>) -> Self {
        let config_path = app_data_dir.map(|d| d.join(CONFIG_DIR_NAME).join(CONFIG_FILE_NAME));
        let initial = match config_path.as_deref() {
            Some(p) => load_from_disk(p).unwrap_or_default(),
            None => TelemetryConfig::default(),
        };
        Self {
            inner: AsyncMutex::new(initial),
            config_path,
            event_queue: queue,
        }
    }

    pub fn event_queue(&self) -> Arc<EventQueue> {
        Arc::clone(&self.event_queue)
    }

    pub async fn snapshot(&self) -> TelemetryConfig {
        self.inner.lock().await.clone()
    }

    pub async fn set_enabled(&self, enabled: bool) -> Result<TelemetryConfig, TelemetryApiError> {
        let mut guard = self.inner.lock().await;
        guard.enabled = enabled;
        if enabled && guard.anon_id.is_none() {
            // 첫 opt-in — UUID + 시각 발급.
            guard.anon_id = Some(Uuid::new_v4().to_string());
            // 시각은 best-effort: format 실패해도 토글은 성공시킵니다.
            guard.opted_in_at = OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .ok();
        }
        let snapshot = guard.clone();
        // 영속 — 실패하면 PersistFailed로 사용자 향 메시지.
        if let Some(path) = self.config_path.as_deref() {
            if let Err(e) = persist_to_disk(path, &snapshot) {
                return Err(TelemetryApiError::PersistFailed {
                    message: format!("{e}"),
                });
            }
        }
        Ok(snapshot)
    }

    /// 이벤트를 큐에 적재 + (DSN 설정 시) GlitchTip POST 시도.
    ///
    /// opt-out 상태면 `Err(NotEnabled)`. opt-in 상태면 endpoint 미설정 시 queue retention,
    /// 설정 시 backon 3회 retry. retention/retry 동작은 `EventQueue::submit`에 위임.
    pub async fn submit_event(
        &self,
        level: EventLevel,
        message: String,
    ) -> Result<EventSubmitOutcome, TelemetryApiError> {
        if message.trim().is_empty() {
            return Err(TelemetryApiError::EmptyMessage);
        }
        let cfg = self.snapshot().await;
        if !cfg.enabled {
            return Err(TelemetryApiError::NotEnabled);
        }
        let event = TelemetryEvent::new(level, message, cfg.anon_id.clone());
        Ok(self.event_queue.submit(event).await)
    }
}

// ───────────────────────────────────────────────────────────────────
// disk helpers
// ───────────────────────────────────────────────────────────────────

fn load_from_disk(path: &Path) -> Option<TelemetryConfig> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice::<TelemetryConfig>(&bytes).ok()
}

fn persist_to_disk(path: &Path, cfg: &TelemetryConfig) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let serialized = serde_json::to_vec_pretty(cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    // atomic-ish: tmp 작성 후 rename.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &serialized)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

// ───────────────────────────────────────────────────────────────────
// IPC commands
// ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_telemetry_config(
    state: State<'_, Arc<TelemetryState>>,
) -> Result<TelemetryConfig, TelemetryApiError> {
    Ok(state.snapshot().await)
}

#[tauri::command]
pub async fn set_telemetry_enabled(
    enabled: bool,
    state: State<'_, Arc<TelemetryState>>,
) -> Result<TelemetryConfig, TelemetryApiError> {
    state.set_enabled(enabled).await
}

/// Phase 7'.b — frontend에서 명시적으로 이벤트 적재. 보통은 미사용 (panic hook이 자동 호출).
///
/// `level`은 `"info" | "warning" | "error"`. 그 외 입력은 `Internal`로 거부.
#[tauri::command]
pub async fn submit_telemetry_event(
    level: String,
    message: String,
    state: State<'_, Arc<TelemetryState>>,
) -> Result<EventSubmitOutcome, TelemetryApiError> {
    let lvl = match level.as_str() {
        "info" => EventLevel::Info,
        "warning" | "warn" => EventLevel::Warning,
        "error" => EventLevel::Error,
        other => {
            return Err(TelemetryApiError::Internal {
                message: format!("알 수 없는 level이에요: {other}"),
            });
        }
    };
    state.submit_event(lvl, message).await
}

// ───────────────────────────────────────────────────────────────────
// 단위 테스트
// ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_state(dir: &TempDir) -> TelemetryState {
        TelemetryState::new(Some(dir.path().to_path_buf()))
    }

    #[tokio::test]
    async fn default_is_disabled_without_uuid() {
        let dir = TempDir::new().unwrap();
        let state = make_state(&dir);
        let cfg = state.snapshot().await;
        assert!(!cfg.enabled);
        assert!(cfg.anon_id.is_none());
        assert!(cfg.opted_in_at.is_none());
    }

    #[tokio::test]
    async fn opt_in_assigns_uuid_and_timestamp() {
        let dir = TempDir::new().unwrap();
        let state = make_state(&dir);
        let cfg = state.set_enabled(true).await.expect("set_enabled");
        assert!(cfg.enabled);
        let id = cfg.anon_id.expect("uuid issued");
        assert_eq!(id.len(), 36, "UUID v4 형식");
        assert!(cfg.opted_in_at.is_some());
    }

    #[tokio::test]
    async fn opt_in_then_out_preserves_uuid() {
        let dir = TempDir::new().unwrap();
        let state = make_state(&dir);
        let on = state.set_enabled(true).await.unwrap();
        let id = on.anon_id.clone().unwrap();
        let off = state.set_enabled(false).await.unwrap();
        assert!(!off.enabled);
        assert_eq!(off.anon_id, Some(id), "UUID는 보존돼요 (재활성 시 재사용)");
    }

    #[tokio::test]
    async fn re_enable_does_not_regenerate_uuid() {
        let dir = TempDir::new().unwrap();
        let state = make_state(&dir);
        let first = state.set_enabled(true).await.unwrap();
        let id = first.anon_id.clone().unwrap();
        let _off = state.set_enabled(false).await.unwrap();
        let again = state.set_enabled(true).await.unwrap();
        assert_eq!(again.anon_id, Some(id));
    }

    #[tokio::test]
    async fn persists_to_disk_round_trip() {
        let dir = TempDir::new().unwrap();
        let state = make_state(&dir);
        state.set_enabled(true).await.unwrap();
        let path = dir.path().join(CONFIG_DIR_NAME).join(CONFIG_FILE_NAME);
        assert!(path.exists(), "config.json 파일 생성");
        // 새 state로 다시 로드 — 같은 UUID여야 해요.
        let state2 = TelemetryState::new(Some(dir.path().to_path_buf()));
        let snap2 = state2.snapshot().await;
        assert!(snap2.enabled);
        assert!(snap2.anon_id.is_some());
    }

    #[tokio::test]
    async fn corrupt_file_falls_back_to_default() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(CONFIG_DIR_NAME).join(CONFIG_FILE_NAME);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"this is not json").unwrap();
        let state = TelemetryState::new(Some(dir.path().to_path_buf()));
        let cfg = state.snapshot().await;
        assert_eq!(cfg, TelemetryConfig::default());
    }

    #[tokio::test]
    async fn memory_only_when_no_dir() {
        let state = TelemetryState::new(None);
        let cfg = state.set_enabled(true).await.unwrap();
        assert!(cfg.enabled);
        assert!(cfg.anon_id.is_some());
    }

    #[tokio::test]
    async fn snapshot_does_not_mutate_state() {
        let dir = TempDir::new().unwrap();
        let state = make_state(&dir);
        let before = state.snapshot().await;
        let after = state.snapshot().await;
        assert_eq!(before, after);
    }

    // ── Phase 7'.b — submit_event 통합 ────────────────────────────────────

    #[tokio::test]
    async fn submit_event_rejects_when_not_opted_in() {
        let state = TelemetryState::new(None);
        let err = state
            .submit_event(EventLevel::Error, "panic at foo".into())
            .await
            .unwrap_err();
        assert!(matches!(err, TelemetryApiError::NotEnabled));
    }

    #[tokio::test]
    async fn submit_event_rejects_empty_message() {
        let state = TelemetryState::new(None);
        let _ = state.set_enabled(true).await.unwrap();
        let err = state
            .submit_event(EventLevel::Info, "   ".into())
            .await
            .unwrap_err();
        assert!(matches!(err, TelemetryApiError::EmptyMessage));
    }

    #[tokio::test]
    async fn submit_event_when_no_dsn_queues_only() {
        // DSN env var 영향을 받지 않도록 EventQueue를 직접 주입 (DSN None).
        let queue = Arc::new(EventQueue::new(None, 200));
        let state = TelemetryState::with_queue(None, Arc::clone(&queue));
        let _ = state.set_enabled(true).await.unwrap();
        let outcome = state
            .submit_event(EventLevel::Info, "hello".into())
            .await
            .unwrap();
        assert!(matches!(outcome, EventSubmitOutcome::Queued));
        assert_eq!(queue.len().await, 1);
    }
}
