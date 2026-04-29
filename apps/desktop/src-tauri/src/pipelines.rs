//! Pipelines IPC — Phase 6'.c. Settings UI에서 게이트웨이 필터 토글 + 감사 로그 노출.
//!
//! 정책 (ADR-0025, phase-6p-updater-pipelines-decision.md §6):
//! - v1 시드 3종 Pipeline 토글: pii-redact / token-quota / observability.
//! - 설정은 `app_data_dir/pipelines/config.json` 영속. 디스크 실패 시 메모리만 보존.
//! - 감사 로그는 메모리 ring buffer cap 200 — 가장 오래된 entry 자동 drop.
//! - 본 v1은 IPC 경계까지만. 게이트웨이가 PipelineLayer로 emit하는 audit entry 연결은
//!   Phase 6'.d (record_audit helper만 노출 + 단위 테스트로 push→get 경로 검증).
//! - 한국어 해요체 에러 메시지. 외부 통신 0 — 모든 상태는 로컬.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use pipelines::AuditEntry;
use serde::{Deserialize, Serialize};
use tauri::async_runtime::JoinHandle;
use tauri::State;
use thiserror::Error;
use tokio::sync::{mpsc, Mutex as AsyncMutex};

// ───────────────────────────────────────────────────────────────────
// 정책 상수
// ───────────────────────────────────────────────────────────────────

/// v1 시드 3종 — backend가 검증하는 화이트리스트.
const PII_REDACT_ID: &str = "pii-redact";
const TOKEN_QUOTA_ID: &str = "token-quota";
const OBSERVABILITY_ID: &str = "observability";

/// 감사 로그 ring buffer cap. 200 초과 시 가장 오래된 entry 제거.
pub const AUDIT_LOG_CAP: usize = 200;

/// `get_audit_log` 기본 limit.
pub const AUDIT_LOG_DEFAULT_LIMIT: usize = 50;

/// `get_audit_log` 최대 limit — `AUDIT_LOG_CAP`과 동일.
pub const AUDIT_LOG_MAX_LIMIT: usize = AUDIT_LOG_CAP;

/// `with_audit_channel`이 만드는 mpsc 채널 capacity.
///
/// 정책: gateway burst를 흡수하기 위해 충분히 크게(256). 채널이 가득 차면 게이트웨이 측은
/// best-effort try_send → drop이라 절대 block 안 해요. cap 200 ring buffer로 자동 정리되니
/// 채널이 250개 이상의 backlog로 넘어가는 케이스는 실질적으로 burst 폭주 시그널.
pub const AUDIT_CHANNEL_CAPACITY: usize = 256;

/// 영속 파일 이름.
const CONFIG_FILE_NAME: &str = "config.json";
const CONFIG_DIR_NAME: &str = "pipelines";

// ───────────────────────────────────────────────────────────────────
// DTO — frontend 미러 타입
// ───────────────────────────────────────────────────────────────────

/// Pipeline 설명자 — frontend가 라벨/설명을 i18n으로 그릴 때 사용.
///
/// `display_name_ko` / `description_ko`는 backend가 fallback 한국어 문자열만 보장.
/// frontend는 i18n 키 우선 사용 (`screens.settings.pipelines.{id}.{name|desc}`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineDescriptor {
    pub id: String,
    pub display_name_ko: String,
    pub description_ko: String,
    pub enabled: bool,
}

/// 활성/비활성 토글 설정 — 영속 대상.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelinesConfig {
    pub pii_redact_enabled: bool,
    pub token_quota_enabled: bool,
    pub observability_enabled: bool,
}

impl Default for PipelinesConfig {
    fn default() -> Self {
        // 정책: 보안/관찰성 향상이 기본값. 사용자가 명시적으로 끄지 않는 한 모두 ON.
        Self {
            pii_redact_enabled: true,
            token_quota_enabled: true,
            observability_enabled: true,
        }
    }
}

/// 감사 로그 entry — frontend 미러. timestamp는 RFC3339 string.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEntryDto {
    pub pipeline_id: String,
    /// "passed" / "modified" / "blocked".
    pub action: String,
    /// RFC3339 ISO (예: `2026-04-28T01:23:45Z`).
    pub timestamp_iso: String,
    pub details: Option<String>,
}

impl AuditEntryDto {
    pub fn new(
        pipeline_id: impl Into<String>,
        action: impl Into<String>,
        details: Option<String>,
    ) -> Self {
        Self {
            pipeline_id: pipeline_id.into(),
            action: action.into(),
            timestamp_iso: now_iso(),
            details,
        }
    }

    /// `pipelines::AuditEntry` → `AuditEntryDto` 변환 (Phase 6'.d).
    ///
    /// 매핑:
    /// - `pipeline_id` 그대로.
    /// - `action` (passed/modified/blocked) 그대로.
    /// - `timestamp` → `timestamp_iso` (RFC3339 문자열).
    /// - `details` 그대로 Option<String>.
    pub fn from_audit_entry(entry: AuditEntry) -> Self {
        Self {
            pipeline_id: entry.pipeline_id.clone(),
            action: entry.action.clone(),
            timestamp_iso: entry.timestamp_iso(),
            details: entry.details.clone(),
        }
    }
}

impl From<AuditEntry> for AuditEntryDto {
    fn from(entry: AuditEntry) -> Self {
        Self::from_audit_entry(entry)
    }
}

fn now_iso() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

// ───────────────────────────────────────────────────────────────────
// API error
// ───────────────────────────────────────────────────────────────────

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PipelinesApiError {
    #[error("알 수 없는 필터예요: {pipeline_id}. 허용 ID는 pii-redact / token-quota / observability 예요.")]
    UnknownPipeline { pipeline_id: String },

    #[error("필터 설정을 저장하지 못했어요: {message}")]
    PersistFailed { message: String },
}

// ───────────────────────────────────────────────────────────────────
// Ring buffer — VecDeque cap 보장
// ───────────────────────────────────────────────────────────────────

/// 메모리 ring buffer — cap 초과 시 oldest 자동 drop.
///
/// `VecDeque::push_back` + `pop_front` 조합으로 cap 유지.
#[derive(Debug, Clone)]
pub struct RingBuffer<T> {
    inner: VecDeque<T>,
    cap: usize,
}

impl<T> RingBuffer<T> {
    pub fn new(cap: usize) -> Self {
        Self {
            inner: VecDeque::with_capacity(cap.max(1)),
            cap: cap.max(1),
        }
    }

    pub fn push(&mut self, value: T) {
        if self.inner.len() >= self.cap {
            self.inner.pop_front();
        }
        self.inner.push_back(value);
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// 마지막 N개를 시간 역순(최신부터) snapshot. 최대 `cap`까지만 반환.
    pub fn last_n(&self, n: usize) -> Vec<T>
    where
        T: Clone,
    {
        let take = n.min(self.cap).min(self.inner.len());
        if take == 0 {
            return Vec::new();
        }
        // VecDeque는 oldest→newest 순. 역순으로 take하기 위해 iter().rev() 사용.
        self.inner.iter().rev().take(take).cloned().collect()
    }
}

// ───────────────────────────────────────────────────────────────────
// 영속 helpers — JSON read/write
// ───────────────────────────────────────────────────────────────────

fn config_path(base: &Path) -> PathBuf {
    base.join(CONFIG_DIR_NAME).join(CONFIG_FILE_NAME)
}

fn read_config_from_disk(base: &Path) -> Option<PipelinesConfig> {
    let path = config_path(base);
    let contents = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str::<PipelinesConfig>(&contents) {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "pipelines config 파싱 실패 — 기본값으로");
            None
        }
    }
}

fn write_config_to_disk(base: &Path, config: &PipelinesConfig) -> Result<(), std::io::Error> {
    let dir = base.join(CONFIG_DIR_NAME);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(CONFIG_FILE_NAME);
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    // atomic rename (Windows에서도 동작 — 같은 디렉터리).
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

// ───────────────────────────────────────────────────────────────────
// PipelinesState — 단일 source of truth
// ───────────────────────────────────────────────────────────────────

/// Tauri State에 등록 — 모든 IPC command가 공유.
///
/// `app_data_dir`가 None이면 메모리 전용 모드 (write_config_to_disk skip).
///
/// Phase 6'.d — audit_task는 game gateway → AuditEntry → AuditEntryDto 변환 receiver task.
/// `with_audit_channel`을 호출하면 spawn되며, 다시 호출하면 이전 task abort 후 새 task spawn.
pub struct PipelinesState {
    config: Arc<AsyncMutex<PipelinesConfig>>,
    audit_log: Arc<AsyncMutex<RingBuffer<AuditEntryDto>>>,
    /// 영속 디렉터리. None = 메모리 전용 fallback.
    app_data_dir: Option<PathBuf>,
    /// audit receiver task — `with_audit_channel`이 spawn. 재호출 시 이전 task abort.
    audit_task: Arc<std::sync::Mutex<Option<JoinHandle<()>>>>,
}

impl PipelinesState {
    /// 새 state — `app_data_dir`가 있으면 disk에서 config 로드 시도, 없거나 실패하면 default.
    pub fn new(app_data_dir: Option<PathBuf>) -> Self {
        let initial = app_data_dir
            .as_deref()
            .and_then(read_config_from_disk)
            .unwrap_or_default();
        Self {
            config: Arc::new(AsyncMutex::new(initial)),
            audit_log: Arc::new(AsyncMutex::new(RingBuffer::new(AUDIT_LOG_CAP))),
            app_data_dir,
            audit_task: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// 메모리 전용 (테스트용 + disk 폴백).
    pub fn in_memory() -> Self {
        Self::new(None)
    }

    /// 현재 config snapshot.
    pub async fn snapshot_config(&self) -> PipelinesConfig {
        *self.config.lock().await
    }

    /// 감사 entry 추가 — gateway가 호출 (Phase 6'.d 연결 예정).
    /// v1은 unit test에서 직접 호출 + IPC `get_audit_log` 동작 검증.
    pub async fn record_audit(&self, entry: AuditEntryDto) {
        let mut g = self.audit_log.lock().await;
        g.push(entry);
    }

    /// 게이트웨이에서 보낼 `AuditEntry`를 수신해 ring buffer에 push하는 receiver task를 spawn.
    ///
    /// 정책 (Phase 6'.d):
    /// - 반환된 `Sender`는 `core_gateway::with_pipelines_audited` 빌더에 주입.
    /// - capacity는 `AUDIT_CHANNEL_CAPACITY` (256) — gateway burst 흡수.
    /// - 재호출 시 이전 receiver task `abort()` 후 새 task spawn (idempotent).
    /// - Sender drop / 모든 sender drop → receiver의 `recv()`가 None → task 자연 종료.
    pub fn with_audit_channel(self: &Arc<Self>) -> mpsc::Sender<AuditEntry> {
        let (tx, mut rx) = mpsc::channel::<AuditEntry>(AUDIT_CHANNEL_CAPACITY);

        // 이전 task abort.
        if let Ok(mut g) = self.audit_task.lock() {
            if let Some(prev) = g.take() {
                prev.abort();
            }
        }

        let state = Arc::clone(self);
        // Tauri builder.setup()는 tokio 런타임 외부에서 호출되므로
        // tokio::spawn은 panic. tauri::async_runtime::spawn은 Tauri의 자체
        // 멀티스레드 런타임을 사용해서 setup() context에서도 안전.
        let handle = tauri::async_runtime::spawn(async move {
            while let Some(entry) = rx.recv().await {
                let dto = AuditEntryDto::from(entry);
                state.record_audit(dto).await;
            }
            tracing::debug!(
                target: "lmmaster.pipelines",
                "audit channel receiver task exiting (sender dropped)"
            );
        });

        if let Ok(mut g) = self.audit_task.lock() {
            *g = Some(handle);
        }

        tx
    }

    /// limit clamping — `(0, AUDIT_LOG_MAX_LIMIT]` 범위. 0이면 default.
    pub fn clamp_limit(limit: usize) -> usize {
        if limit == 0 {
            return AUDIT_LOG_DEFAULT_LIMIT;
        }
        limit.min(AUDIT_LOG_MAX_LIMIT)
    }

    /// 현재 enabled 매핑을 i18n 비의존 한국어 fallback과 함께 반환.
    /// frontend는 i18n 키로 라벨을 갱신 — id + enabled만 사용해도 충분.
    async fn descriptors(&self) -> Vec<PipelineDescriptor> {
        let cfg = self.snapshot_config().await;
        vec![
            PipelineDescriptor {
                id: PII_REDACT_ID.to_string(),
                display_name_ko: "개인정보 보호 필터".to_string(),
                description_ko: "주민등록번호·휴대폰·신용카드·이메일을 자동으로 가려요."
                    .to_string(),
                enabled: cfg.pii_redact_enabled,
            },
            PipelineDescriptor {
                id: TOKEN_QUOTA_ID.to_string(),
                display_name_ko: "토큰 한도 관리".to_string(),
                description_ko: "키별 토큰 한도를 추적하고 초과 요청을 막아 드려요.".to_string(),
                enabled: cfg.token_quota_enabled,
            },
            PipelineDescriptor {
                id: OBSERVABILITY_ID.to_string(),
                display_name_ko: "관찰성 로그".to_string(),
                description_ko: "요청·응답 메타를 진단 로그에 남겨드려요.".to_string(),
                enabled: cfg.observability_enabled,
            },
        ]
    }

    /// 토글 적용 + 디스크 영속.
    async fn apply_set(&self, pipeline_id: &str, enabled: bool) -> Result<(), PipelinesApiError> {
        let mut g = self.config.lock().await;
        match pipeline_id {
            PII_REDACT_ID => g.pii_redact_enabled = enabled,
            TOKEN_QUOTA_ID => g.token_quota_enabled = enabled,
            OBSERVABILITY_ID => g.observability_enabled = enabled,
            other => {
                return Err(PipelinesApiError::UnknownPipeline {
                    pipeline_id: other.to_string(),
                });
            }
        }
        let snapshot = *g;
        drop(g); // disk write 동안 락 보유 X.

        if let Some(base) = self.app_data_dir.as_deref() {
            if let Err(e) = write_config_to_disk(base, &snapshot) {
                // 영속 실패는 사용자에게 알리되 메모리 상태는 유지.
                tracing::warn!(error = %e, "pipelines config 영속 실패 — 메모리만 갱신");
                return Err(PipelinesApiError::PersistFailed {
                    message: format!("{e}"),
                });
            }
        }
        Ok(())
    }
}

impl Drop for PipelinesState {
    /// state가 drop되면 receiver task도 abort — 누수 방지.
    fn drop(&mut self) {
        if let Ok(mut g) = self.audit_task.lock() {
            if let Some(handle) = g.take() {
                handle.abort();
            }
        }
    }
}

// ───────────────────────────────────────────────────────────────────
// Tauri commands
// ───────────────────────────────────────────────────────────────────

/// 3종 시드 Pipeline 설명자 + enabled 상태.
#[tauri::command]
pub async fn list_pipelines(
    state: State<'_, Arc<PipelinesState>>,
) -> Result<Vec<PipelineDescriptor>, PipelinesApiError> {
    Ok(state.descriptors().await)
}

/// `pipeline_id` 토글. 알 수 없는 id는 한국어 에러.
#[tauri::command]
pub async fn set_pipeline_enabled(
    pipeline_id: String,
    enabled: bool,
    state: State<'_, Arc<PipelinesState>>,
) -> Result<(), PipelinesApiError> {
    state.apply_set(&pipeline_id, enabled).await
}

/// 현재 영속 설정 snapshot.
#[tauri::command]
pub async fn get_pipelines_config(
    state: State<'_, Arc<PipelinesState>>,
) -> Result<PipelinesConfig, PipelinesApiError> {
    Ok(state.snapshot_config().await)
}

/// 마지막 N개 감사 entry — 시간 역순(최신부터). limit 기본 50, 최대 200.
#[tauri::command]
pub async fn get_audit_log(
    limit: Option<usize>,
    state: State<'_, Arc<PipelinesState>>,
) -> Result<Vec<AuditEntryDto>, PipelinesApiError> {
    let n = PipelinesState::clamp_limit(limit.unwrap_or(AUDIT_LOG_DEFAULT_LIMIT));
    let g = state.audit_log.lock().await;
    Ok(g.last_n(n))
}

/// ring buffer 비우기.
#[tauri::command]
pub async fn clear_audit_log(
    state: State<'_, Arc<PipelinesState>>,
) -> Result<(), PipelinesApiError> {
    let mut g = state.audit_log.lock().await;
    g.clear();
    Ok(())
}

// ───────────────────────────────────────────────────────────────────
// Tests
// ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_dir() -> TempDir {
        TempDir::new().expect("tempdir")
    }

    // ── Config defaults + serde ──────────────────────────────────────

    #[test]
    fn pipelines_config_default_all_enabled() {
        let cfg = PipelinesConfig::default();
        assert!(cfg.pii_redact_enabled);
        assert!(cfg.token_quota_enabled);
        assert!(cfg.observability_enabled);
    }

    #[test]
    fn pipelines_config_serde_round_trip() {
        let cfg = PipelinesConfig {
            pii_redact_enabled: false,
            token_quota_enabled: true,
            observability_enabled: false,
        };
        let json = serde_json::to_string(&cfg).expect("serialize");
        let back: PipelinesConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cfg, back);
    }

    // ── Disk persist round-trip + missing-dir fallback ───────────────

    #[test]
    fn write_then_read_config_round_trip() {
        let td = temp_dir();
        let cfg = PipelinesConfig {
            pii_redact_enabled: false,
            token_quota_enabled: true,
            observability_enabled: true,
        };
        write_config_to_disk(td.path(), &cfg).unwrap();
        let back = read_config_from_disk(td.path()).expect("read");
        assert_eq!(cfg, back);
    }

    #[test]
    fn read_missing_config_returns_none() {
        let td = temp_dir();
        // 파일 자체가 없음.
        assert!(read_config_from_disk(td.path()).is_none());
    }

    #[test]
    fn read_corrupt_json_returns_none_and_does_not_panic() {
        let td = temp_dir();
        let dir = td.path().join(CONFIG_DIR_NAME);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(CONFIG_FILE_NAME), "{not valid json").unwrap();
        assert!(read_config_from_disk(td.path()).is_none());
    }

    #[tokio::test]
    async fn state_new_loads_from_disk_when_present() {
        let td = temp_dir();
        let stored = PipelinesConfig {
            pii_redact_enabled: false,
            token_quota_enabled: false,
            observability_enabled: true,
        };
        write_config_to_disk(td.path(), &stored).unwrap();

        let s = PipelinesState::new(Some(td.path().to_path_buf()));
        let cfg = s.snapshot_config().await;
        assert_eq!(cfg, stored);
    }

    #[tokio::test]
    async fn state_new_falls_back_to_default_when_disk_missing() {
        let td = temp_dir();
        let s = PipelinesState::new(Some(td.path().join("non-existent").to_path_buf()));
        let cfg = s.snapshot_config().await;
        assert_eq!(cfg, PipelinesConfig::default());
    }

    #[tokio::test]
    async fn in_memory_state_uses_default_config() {
        let s = PipelinesState::in_memory();
        let cfg = s.snapshot_config().await;
        assert_eq!(cfg, PipelinesConfig::default());
    }

    // ── set_pipeline_enabled — valid + invalid + persistence ─────────

    #[tokio::test]
    async fn apply_set_valid_pipeline_id_updates_config() {
        let td = temp_dir();
        let s = PipelinesState::new(Some(td.path().to_path_buf()));
        s.apply_set(PII_REDACT_ID, false).await.unwrap();
        let cfg = s.snapshot_config().await;
        assert!(!cfg.pii_redact_enabled);
        assert!(cfg.token_quota_enabled);
        assert!(cfg.observability_enabled);
    }

    #[tokio::test]
    async fn apply_set_invalid_pipeline_id_returns_korean_error() {
        let s = PipelinesState::in_memory();
        let err = s.apply_set("unknown-filter", true).await.unwrap_err();
        match err {
            PipelinesApiError::UnknownPipeline { pipeline_id } => {
                assert_eq!(pipeline_id, "unknown-filter");
                let msg = format!(
                    "{}",
                    PipelinesApiError::UnknownPipeline {
                        pipeline_id: "unknown-filter".into()
                    }
                );
                assert!(msg.contains("알 수 없는"));
                assert!(msg.contains("unknown-filter"));
            }
            other => panic!("expected UnknownPipeline, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn apply_set_persists_to_disk() {
        let td = temp_dir();
        let s = PipelinesState::new(Some(td.path().to_path_buf()));
        s.apply_set(TOKEN_QUOTA_ID, false).await.unwrap();

        // 같은 디렉터리로 새 state 생성 — disk에서 로드.
        let s2 = PipelinesState::new(Some(td.path().to_path_buf()));
        let cfg = s2.snapshot_config().await;
        assert!(!cfg.token_quota_enabled);
        assert!(cfg.pii_redact_enabled);
    }

    #[tokio::test]
    async fn apply_set_in_memory_state_is_ok_without_disk() {
        let s = PipelinesState::in_memory();
        // 디스크 영속 skip — 에러 없음.
        s.apply_set(OBSERVABILITY_ID, false).await.unwrap();
        let cfg = s.snapshot_config().await;
        assert!(!cfg.observability_enabled);
    }

    // ── descriptors — 3종 시드 + i18n-friendly ───────────────────────

    #[tokio::test]
    async fn descriptors_returns_three_seeds_with_default_enabled() {
        let s = PipelinesState::in_memory();
        let descs = s.descriptors().await;
        assert_eq!(descs.len(), 3);
        let ids: Vec<&str> = descs.iter().map(|d| d.id.as_str()).collect();
        assert!(ids.contains(&PII_REDACT_ID));
        assert!(ids.contains(&TOKEN_QUOTA_ID));
        assert!(ids.contains(&OBSERVABILITY_ID));
        assert!(descs.iter().all(|d| d.enabled));
        // 한국어 fallback 라벨 모두 비어있지 않음.
        assert!(descs.iter().all(|d| !d.display_name_ko.is_empty()));
        assert!(descs.iter().all(|d| !d.description_ko.is_empty()));
    }

    #[tokio::test]
    async fn descriptors_reflects_current_enabled_state() {
        let s = PipelinesState::in_memory();
        s.apply_set(PII_REDACT_ID, false).await.unwrap();
        let descs = s.descriptors().await;
        let pii = descs.iter().find(|d| d.id == PII_REDACT_ID).unwrap();
        assert!(!pii.enabled);
    }

    // ── Audit log ring buffer ────────────────────────────────────────

    #[test]
    fn ring_buffer_under_cap_keeps_all_entries() {
        let mut rb: RingBuffer<i32> = RingBuffer::new(5);
        for i in 0..3 {
            rb.push(i);
        }
        assert_eq!(rb.len(), 3);
        let snap = rb.last_n(10);
        // last_n은 newest→oldest.
        assert_eq!(snap, vec![2, 1, 0]);
    }

    #[test]
    fn ring_buffer_over_cap_drops_oldest() {
        let mut rb: RingBuffer<i32> = RingBuffer::new(3);
        for i in 0..5 {
            rb.push(i);
        }
        assert_eq!(rb.len(), 3);
        let snap = rb.last_n(3);
        assert_eq!(snap, vec![4, 3, 2]); // oldest 0,1 dropped.
    }

    #[test]
    fn ring_buffer_clear_empties() {
        let mut rb: RingBuffer<i32> = RingBuffer::new(5);
        rb.push(1);
        rb.push(2);
        assert!(!rb.is_empty());
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
        assert!(rb.last_n(5).is_empty());
    }

    #[test]
    fn ring_buffer_last_n_clamps_to_len() {
        let mut rb: RingBuffer<i32> = RingBuffer::new(10);
        rb.push(7);
        let snap = rb.last_n(5);
        assert_eq!(snap, vec![7]);
    }

    #[test]
    fn ring_buffer_last_n_zero_returns_empty() {
        let mut rb: RingBuffer<i32> = RingBuffer::new(10);
        rb.push(1);
        assert!(rb.last_n(0).is_empty());
    }

    // ── record_audit + get_audit_log + clear ─────────────────────────

    #[tokio::test]
    async fn record_audit_then_get_returns_entry() {
        let s = PipelinesState::in_memory();
        let entry = AuditEntryDto::new(PII_REDACT_ID, "modified", Some("redacted 1".into()));
        s.record_audit(entry.clone()).await;
        let g = s.audit_log.lock().await;
        let snap = g.last_n(10);
        assert_eq!(snap.len(), 1);
        assert_eq!(snap[0].pipeline_id, entry.pipeline_id);
        assert_eq!(snap[0].action, entry.action);
        assert_eq!(snap[0].details, entry.details);
    }

    #[tokio::test]
    async fn record_audit_over_cap_drops_oldest() {
        let s = PipelinesState::in_memory();
        for i in 0..AUDIT_LOG_CAP + 50 {
            s.record_audit(AuditEntryDto::new(
                "pipeline",
                "passed",
                Some(format!("{i}")),
            ))
            .await;
        }
        let g = s.audit_log.lock().await;
        assert_eq!(g.len(), AUDIT_LOG_CAP);
        // newest의 details = "{cap+49}" 형식.
        let snap = g.last_n(1);
        let last = &snap[0];
        let expected_last = AUDIT_LOG_CAP + 49;
        assert_eq!(
            last.details.as_deref(),
            Some(format!("{expected_last}").as_str())
        );
    }

    #[tokio::test]
    async fn clear_audit_log_empties_buffer() {
        let s = PipelinesState::in_memory();
        s.record_audit(AuditEntryDto::new("pipeline", "passed", None))
            .await;
        // 직접 buffer clear.
        let mut g = s.audit_log.lock().await;
        g.clear();
        drop(g);
        let g2 = s.audit_log.lock().await;
        assert!(g2.is_empty());
    }

    // ── clamp_limit ──────────────────────────────────────────────────

    #[test]
    fn clamp_limit_zero_returns_default() {
        assert_eq!(PipelinesState::clamp_limit(0), AUDIT_LOG_DEFAULT_LIMIT);
    }

    #[test]
    fn clamp_limit_under_max_returns_input() {
        assert_eq!(PipelinesState::clamp_limit(10), 10);
        assert_eq!(PipelinesState::clamp_limit(150), 150);
    }

    #[test]
    fn clamp_limit_over_max_clamps() {
        assert_eq!(PipelinesState::clamp_limit(500), AUDIT_LOG_MAX_LIMIT);
        assert_eq!(PipelinesState::clamp_limit(usize::MAX), AUDIT_LOG_MAX_LIMIT);
    }

    // ── AuditEntryDto serde ──────────────────────────────────────────

    #[test]
    fn audit_entry_dto_serde_round_trip() {
        let entry = AuditEntryDto {
            pipeline_id: "pii-redact".into(),
            action: "modified".into(),
            timestamp_iso: "2026-04-28T01:23:45Z".into(),
            details: Some("redacted 1 PII".into()),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: AuditEntryDto = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, back);
    }

    #[test]
    fn audit_entry_dto_new_populates_iso_timestamp() {
        let entry = AuditEntryDto::new("a", "passed", None);
        // "T" + "Z" 또는 offset 패턴이 RFC3339.
        assert!(entry.timestamp_iso.contains('T'));
        // Empty string은 format 실패 시에만 — 테스트 환경에선 정상 format 기대.
        assert!(!entry.timestamp_iso.is_empty());
    }

    // ── Concurrent Mutex correctness ─────────────────────────────────

    #[tokio::test]
    async fn concurrent_record_audit_preserves_count() {
        let s = Arc::new(PipelinesState::in_memory());
        let mut handles = Vec::new();
        for i in 0..50 {
            let s_clone = Arc::clone(&s);
            handles.push(tokio::spawn(async move {
                s_clone
                    .record_audit(AuditEntryDto::new("p", "passed", Some(format!("{i}"))))
                    .await;
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        let g = s.audit_log.lock().await;
        assert_eq!(g.len(), 50);
    }

    #[tokio::test]
    async fn concurrent_set_get_does_not_deadlock() {
        let s = Arc::new(PipelinesState::in_memory());
        let mut handles = Vec::new();
        for i in 0..20 {
            let s_clone = Arc::clone(&s);
            handles.push(tokio::spawn(async move {
                let on = i % 2 == 0;
                s_clone.apply_set(PII_REDACT_ID, on).await.unwrap();
                let _ = s_clone.snapshot_config().await;
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        // 마지막 상태가 deterministic하지 않아도 panic/deadlock 없음 = OK.
    }

    // ── API error serde — kebab-case tag ─────────────────────────────

    #[test]
    fn api_error_unknown_pipeline_kebab_tag() {
        let e = PipelinesApiError::UnknownPipeline {
            pipeline_id: "x".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "unknown-pipeline");
    }

    #[test]
    fn api_error_persist_failed_kebab_tag() {
        let e = PipelinesApiError::PersistFailed {
            message: "disk full".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "persist-failed");
        let msg = format!("{e}");
        assert!(msg.contains("저장하지 못했어요"));
        assert!(msg.contains("disk full"));
    }

    // ── Atomic write — temp file rename ──────────────────────────────

    #[test]
    fn write_config_creates_dir_if_missing() {
        let td = temp_dir();
        let nested = td.path().join("a").join("b");
        // a/b 자체가 존재 안 함 — write가 만들어야.
        let cfg = PipelinesConfig::default();
        write_config_to_disk(&nested, &cfg).unwrap();
        let path = config_path(&nested);
        assert!(path.exists());
    }

    // ───────────────────────────────────────────────────────────────────
    // Phase 6'.d — audit channel + AuditEntry → DTO 변환
    // ───────────────────────────────────────────────────────────────────

    /// `pipelines::AuditEntry` → `AuditEntryDto` 변환이 모든 필드를 보존해야 해요.
    #[test]
    fn audit_entry_to_dto_preserves_all_fields() {
        let entry = AuditEntry::modified("pii-redact", "redacted 1");
        let entry_iso = entry.timestamp_iso();
        let dto = AuditEntryDto::from(entry);
        assert_eq!(dto.pipeline_id, "pii-redact");
        assert_eq!(dto.action, "modified");
        assert_eq!(dto.timestamp_iso, entry_iso);
        assert_eq!(dto.details.as_deref(), Some("redacted 1"));
    }

    #[test]
    fn audit_entry_passed_to_dto_no_details() {
        let entry = AuditEntry::passed("observability");
        let dto = AuditEntryDto::from_audit_entry(entry);
        assert_eq!(dto.pipeline_id, "observability");
        assert_eq!(dto.action, "passed");
        assert!(dto.details.is_none());
        // RFC3339 마커.
        assert!(dto.timestamp_iso.contains('T'));
    }

    #[test]
    fn audit_entry_blocked_to_dto_keeps_details() {
        let entry = AuditEntry::blocked("token-quota", "budget exceeded");
        let dto = AuditEntryDto::from_audit_entry(entry);
        assert_eq!(dto.pipeline_id, "token-quota");
        assert_eq!(dto.action, "blocked");
        assert_eq!(dto.details.as_deref(), Some("budget exceeded"));
    }

    /// `with_audit_channel`이 spawn한 task가 sender로 받은 entry를 ring buffer에 push.
    #[tokio::test]
    async fn with_audit_channel_routes_entries_to_ring_buffer() {
        let s: Arc<PipelinesState> = Arc::new(PipelinesState::in_memory());
        let tx = s.with_audit_channel();

        // entry 1개 보내기.
        let entry = AuditEntry::modified("pii-redact", "redacted");
        tx.send(entry).await.expect("send");

        // task가 받아 record_audit 호출할 시간을 잠깐 줘야 해요. yield + 짧은 sleep.
        for _ in 0..50 {
            tokio::task::yield_now().await;
            let g = s.audit_log.lock().await;
            if !g.is_empty() {
                let snap = g.last_n(10);
                assert_eq!(snap[0].pipeline_id, "pii-redact");
                assert_eq!(snap[0].action, "modified");
                return;
            }
            drop(g);
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("audit_log이 timeout 안에 채워지지 않았어요");
    }

    /// 재호출 시 이전 task abort + 새 task가 정상 동작.
    #[tokio::test]
    async fn with_audit_channel_replaces_previous_task() {
        let s: Arc<PipelinesState> = Arc::new(PipelinesState::in_memory());
        let tx1 = s.with_audit_channel();
        // 첫 sender에 entry 1개.
        tx1.send(AuditEntry::passed("first")).await.unwrap();
        // 처리 시간.
        for _ in 0..30 {
            tokio::task::yield_now().await;
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            let g = s.audit_log.lock().await;
            if g.len() == 1 {
                break;
            }
        }

        // 두 번째 호출 — 이전 task abort 예상.
        let tx2 = s.with_audit_channel();
        // tx1로 send는 이전 receiver abort 후라 처리 안 됨.
        // tx1은 capacity 256이라 버퍼링은 가능하지만 receiver가 죽었으니 ring buffer엔 안 들어와요.
        tx2.send(AuditEntry::passed("second")).await.unwrap();

        // tx2 entry가 ring buffer에 들어왔는지 확인.
        for _ in 0..50 {
            tokio::task::yield_now().await;
            let g = s.audit_log.lock().await;
            if g.len() >= 2 {
                let snap = g.last_n(10);
                let ids: Vec<&str> = snap.iter().map(|e| e.pipeline_id.as_str()).collect();
                assert!(ids.contains(&"second"));
                return;
            }
            drop(g);
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("두 번째 sender의 entry가 ring buffer에 들어오지 않았어요");
    }

    /// 모든 sender drop → receiver task가 자연 종료 (no panic).
    #[tokio::test]
    async fn audit_channel_sender_drop_terminates_receiver_task() {
        let s: Arc<PipelinesState> = Arc::new(PipelinesState::in_memory());
        let tx = s.with_audit_channel();

        // task handle 추출 (이전 단계에서 저장됨).
        let handle = {
            let mut g = s.audit_task.lock().unwrap();
            g.take().expect("task should exist")
        };

        // sender drop → receiver의 recv()가 None → task 종료.
        drop(tx);

        // task가 자연 종료되는지 확인 (timeout 짧게).
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), handle).await;
        assert!(
            result.is_ok(),
            "sender drop 후 receiver task가 종료되지 않았어요"
        );
        // tauri::async_runtime::JoinHandle은 abort/panic도 모두 Err로 반환 —
        // 정상 종료(Ok) 또는 abort(Err) 둘 다 허용. panic은 별개로 stderr에 표시됨.
        let _join_res = result.unwrap();
    }

    /// 다중 entry — 보낸 순서대로 ring buffer에 push.
    #[tokio::test]
    async fn audit_channel_preserves_order_of_multiple_entries() {
        let s: Arc<PipelinesState> = Arc::new(PipelinesState::in_memory());
        let tx = s.with_audit_channel();

        for i in 0..10 {
            tx.send(AuditEntry::modified("p", format!("entry-{i}")))
                .await
                .unwrap();
        }

        // 처리 대기.
        for _ in 0..100 {
            tokio::task::yield_now().await;
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            let g = s.audit_log.lock().await;
            if g.len() >= 10 {
                // last_n은 newest→oldest. entry-9가 newest여야.
                let snap = g.last_n(10);
                assert_eq!(snap.len(), 10);
                assert_eq!(snap[0].details.as_deref(), Some("entry-9"));
                assert_eq!(snap[9].details.as_deref(), Some("entry-0"));
                return;
            }
        }
        panic!("10개 entry가 timeout 안에 처리되지 않았어요");
    }

    /// concurrent push (channel + record_audit) — Mutex 정합성.
    #[tokio::test]
    async fn audit_channel_concurrent_with_direct_record_keeps_count() {
        let s: Arc<PipelinesState> = Arc::new(PipelinesState::in_memory());
        let tx = s.with_audit_channel();

        let mut handles = Vec::new();
        for i in 0..20 {
            let tx_clone = tx.clone();
            handles.push(tokio::spawn(async move {
                tx_clone
                    .send(AuditEntry::passed(format!("ch-{i}")))
                    .await
                    .unwrap();
            }));
        }
        for i in 0..20 {
            let s_clone = Arc::clone(&s);
            handles.push(tokio::spawn(async move {
                s_clone
                    .record_audit(AuditEntryDto::new("direct", "passed", Some(format!("{i}"))))
                    .await;
            }));
        }
        for h in handles {
            h.await.unwrap();
        }

        // 채널 entry 처리 + 직접 record_audit 모두 합쳐서 40개.
        for _ in 0..200 {
            tokio::task::yield_now().await;
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            let g = s.audit_log.lock().await;
            if g.len() == 40 {
                return;
            }
        }
        let g = s.audit_log.lock().await;
        panic!(
            "concurrent push의 누적 카운트가 40개에 못 미쳤어요: 실제={}",
            g.len()
        );
    }
}
