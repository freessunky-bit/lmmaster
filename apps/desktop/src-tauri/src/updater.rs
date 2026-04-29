//! Auto-updater IPC 모듈 — Phase 6'.b. Channel<UpdateEvent> + UpdaterRegistry + Poller wrapping.
//!
//! 정책 (ADR-0026, phase-6p-updater-pipelines-decision.md §4·§5, phase-5pb-4p5b-ipc-reinforcement.md §1):
//! - `tauri::ipc::Channel<UpdateEvent>` per-invocation stream — typed + ordered.
//! - 두 개의 별개 상태:
//!   * `UpdaterRegistry` — `check_for_update`로 시작한 단발 check task들 (check_id 키).
//!   * `PollerState` — 6h 자동 갱신 polling 1개 (전역 single-slot).
//! - 단발 check는 ad-hoc 다중 허용 (사용자가 Settings 들어가서 "지금 확인" 연타 가능). check_id uuid로 식별.
//! - 자동 폴러는 single-slot — 두 번째 start는 "이미 실행 중이에요" 거부 (idempotent).
//! - cancel 협력: `tokio_util::sync::CancellationToken`. cooperative — 다음 polling cycle 전에 깨어남.
//! - send 실패 = window 닫힘 → cancel 트리거 (workbench/knowledge `emit_or_cancel` 패턴).
//! - 한국어 해요체 에러 메시지.
//! - interval_secs 검증: 3600~86400 (1h~24h, ADR-0026 §2 허용 범위). 음수/0/초과 거부.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use auto_updater::{
    is_outdated, GitHubReleasesSource, Poller, ReleaseInfo, UpdateSource, UpdaterError,
};
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::State;
use thiserror::Error;
use tokio::sync::Mutex as AsyncMutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

// ───────────────────────────────────────────────────────────────────
// 정책 상수 — ADR-0026 §2 (1h~24h 허용)
// ───────────────────────────────────────────────────────────────────

/// 폴 간격 최저 — 1시간 (3600초). 그 미만은 GitHub API rate limit 위험.
pub const MIN_INTERVAL_SECS: u64 = 60 * 60;
/// 폴 간격 최고 — 24시간 (86400초). 그 이상은 보안 패치 전파 너무 늦음.
pub const MAX_INTERVAL_SECS: u64 = 24 * 60 * 60;

// ───────────────────────────────────────────────────────────────────
// DTO — frontend 미러 타입
// ───────────────────────────────────────────────────────────────────

/// `auto_updater::ReleaseInfo` DTO — frontend 직렬화 친화 (RFC3339 string).
///
/// `published_at`은 string으로 변환 — JS Date 호환 + 서버측 OffsetDateTime detail 노출 회피.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseInfoDto {
    /// `1.2.3` 또는 `v1.2.3`.
    pub version: String,
    /// RFC3339 ISO 시각 (예: `2026-04-01T12:34:56Z`).
    pub published_at_iso: String,
    pub url: String,
    pub notes: Option<String>,
}

impl ReleaseInfoDto {
    fn from_release(r: &ReleaseInfo) -> Self {
        let iso = r
            .published_at
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| String::new());
        Self {
            version: r.version.clone(),
            published_at_iso: iso,
            url: r.url.clone(),
            notes: r.notes.clone(),
        }
    }
}

/// 자동 폴러 상태 — `get_auto_update_status`가 반환.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PollerStatus {
    pub active: bool,
    pub repo: Option<String>,
    pub interval_secs: Option<u64>,
    /// 마지막 polling cycle이 끝난 시각 (RFC3339). 미실행 또는 첫 cycle 전이면 None.
    pub last_check_iso: Option<String>,
}

// ───────────────────────────────────────────────────────────────────
// Channel event — kebab-case tagged enum
// ───────────────────────────────────────────────────────────────────

/// Channel<UpdateEvent>로 frontend에 흘려보내는 event.
///
/// `#[serde(tag = "kind", rename_all = "kebab-case")]` — InstallEvent / WorkbenchEvent / IngestEvent와
/// 동일 셰입. frontend는 `event.kind`로 discriminated union narrow.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum UpdateEvent {
    /// check 시작 직후 1회.
    Started {
        check_id: String,
        current_version: String,
        repo: String,
    },
    /// 새 버전 감지.
    Outdated {
        check_id: String,
        current_version: String,
        latest: ReleaseInfoDto,
    },
    /// 최신 버전 사용 중.
    UpToDate {
        check_id: String,
        current_version: String,
    },
    /// 실패. message는 한국어 해요체.
    Failed { check_id: String, error: String },
    /// 사용자 cancel 또는 channel close.
    Cancelled { check_id: String },
}

// ───────────────────────────────────────────────────────────────────
// API error — invoke().reject로 frontend에 전달
// ───────────────────────────────────────────────────────────────────

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum UpdaterApiError {
    #[error("자동 갱신 확인이 이미 실행 중이에요")]
    PollerAlreadyRunning,

    #[error("폴 간격은 1시간(3600초)~24시간(86400초) 사이여야 해요. 입력값: {got}초")]
    IntervalOutOfRange { got: u64 },

    #[error("저장소 형식이 올바르지 않아요. 'owner/repo' 형식으로 적어 주세요.")]
    InvalidRepo,

    #[error("업데이트 확인을 시작하지 못했어요: {message}")]
    StartFailed { message: String },
}

// ───────────────────────────────────────────────────────────────────
// UpdaterRegistry — check_id ↔ ActiveCheck (단발 check 다중 허용)
// ───────────────────────────────────────────────────────────────────

struct ActiveCheck {
    cancel: CancellationToken,
    started_at: Instant,
    repo: String,
}

/// 단발 check task registry. uuid check_id로 식별. cancel는 idempotent.
#[derive(Default)]
pub struct UpdaterRegistry {
    inner: AsyncMutex<HashMap<String, ActiveCheck>>,
}

impl UpdaterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 새 check 등록. uuid 충돌 가능성 무시 (UUIDv4 collision 거의 0).
    pub async fn register(&self, check_id: &str, repo: &str) -> CancellationToken {
        let mut g = self.inner.lock().await;
        let tok = CancellationToken::new();
        g.insert(
            check_id.to_string(),
            ActiveCheck {
                cancel: tok.clone(),
                started_at: Instant::now(),
                repo: repo.to_string(),
            },
        );
        tok
    }

    pub async fn get(&self, check_id: &str) -> Option<(String, Duration)> {
        let g = self.inner.lock().await;
        g.get(check_id)
            .map(|c| (c.repo.clone(), c.started_at.elapsed()))
    }

    pub async fn remove(&self, check_id: &str) {
        let mut g = self.inner.lock().await;
        g.remove(check_id);
    }

    /// idempotent. 미존재 = no-op.
    pub async fn cancel(&self, check_id: &str) {
        let g = self.inner.lock().await;
        if let Some(c) = g.get(check_id) {
            c.cancel.cancel();
        }
    }

    pub async fn cancel_all(&self) {
        let g = self.inner.lock().await;
        for c in g.values() {
            c.cancel.cancel();
        }
    }

    pub async fn in_flight_count(&self) -> usize {
        self.inner.lock().await.len()
    }

    /// `RunEvent::ExitRequested` sync 컨텍스트용. try_lock 기반 best-effort.
    pub fn cancel_all_blocking(&self) {
        if let Ok(g) = self.inner.try_lock() {
            for c in g.values() {
                c.cancel.cancel();
            }
        }
    }
}

// ───────────────────────────────────────────────────────────────────
// PollerState — 자동 폴러 single-slot
// ───────────────────────────────────────────────────────────────────

/// 한 번에 하나의 polling 만 — 두 번째 start는 거부.
struct PollerHandle {
    cancel: CancellationToken,
    repo: String,
    interval_secs: u64,
    last_check: Arc<AsyncMutex<Option<String>>>,
}

#[derive(Default)]
pub struct PollerState {
    inner: AsyncMutex<Option<PollerHandle>>,
}

impl PollerState {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn is_active(&self) -> bool {
        self.inner.lock().await.is_some()
    }

    pub async fn snapshot(&self) -> PollerStatus {
        let g = self.inner.lock().await;
        match g.as_ref() {
            Some(h) => {
                let last = h.last_check.lock().await.clone();
                PollerStatus {
                    active: true,
                    repo: Some(h.repo.clone()),
                    interval_secs: Some(h.interval_secs),
                    last_check_iso: last,
                }
            }
            None => PollerStatus::default(),
        }
    }

    /// 새 폴러 등록. 이미 실행 중이면 Err.
    pub async fn install(
        &self,
        repo: String,
        interval_secs: u64,
        cancel: CancellationToken,
        last_check: Arc<AsyncMutex<Option<String>>>,
    ) -> Result<(), UpdaterApiError> {
        let mut g = self.inner.lock().await;
        if g.is_some() {
            return Err(UpdaterApiError::PollerAlreadyRunning);
        }
        *g = Some(PollerHandle {
            cancel,
            repo,
            interval_secs,
            last_check,
        });
        Ok(())
    }

    /// stop — idempotent. 미실행 = no-op.
    pub async fn stop(&self) {
        let mut g = self.inner.lock().await;
        if let Some(h) = g.take() {
            h.cancel.cancel();
        }
    }

    /// 폴러가 자체 종료한 후 cleanup용 (같은 cancel token이지만 entry만 제거).
    pub async fn clear(&self) {
        let mut g = self.inner.lock().await;
        *g = None;
    }

    /// `RunEvent::ExitRequested` sync 컨텍스트용. try_lock 기반 best-effort.
    pub fn stop_blocking(&self) {
        if let Ok(mut g) = self.inner.try_lock() {
            if let Some(h) = g.take() {
                h.cancel.cancel();
            }
        }
    }
}

// ───────────────────────────────────────────────────────────────────
// Channel forwarding helper
// ───────────────────────────────────────────────────────────────────

fn emit_or_cancel(channel: &Channel<UpdateEvent>, cancel: &CancellationToken, event: UpdateEvent) {
    if channel.send(event).is_err() {
        tracing::debug!("updater channel send failed; triggering cancel");
        cancel.cancel();
    }
}

// ───────────────────────────────────────────────────────────────────
// 핵심 로직 — repo validation + 단발 check
// ───────────────────────────────────────────────────────────────────

/// "owner/repo" 형식 간단 검증 — 빈 토큰 / 공백 / 슬래시 부재 거부.
/// GitHub repo는 영숫자/하이픈/언더스코어/마침표만 허용 (관대하게 매칭).
fn validate_repo(repo: &str) -> Result<(), UpdaterApiError> {
    let trimmed = repo.trim();
    let parts: Vec<&str> = trimmed.split('/').collect();
    if parts.len() != 2 {
        return Err(UpdaterApiError::InvalidRepo);
    }
    if parts[0].is_empty() || parts[1].is_empty() {
        return Err(UpdaterApiError::InvalidRepo);
    }
    if parts[0].chars().any(|c| c.is_whitespace()) || parts[1].chars().any(|c| c.is_whitespace()) {
        return Err(UpdaterApiError::InvalidRepo);
    }
    Ok(())
}

/// interval_secs 검증 (ADR-0026 §2): 1h~24h.
fn validate_interval(interval_secs: u64) -> Result<(), UpdaterApiError> {
    if !(MIN_INTERVAL_SECS..=MAX_INTERVAL_SECS).contains(&interval_secs) {
        return Err(UpdaterApiError::IntervalOutOfRange { got: interval_secs });
    }
    Ok(())
}

/// 단발 check — UpdateSource 호출 + is_outdated + 적절한 event emit.
///
/// cancel이 polling 도중 발생하면 Cancelled emit. UpdaterError::Cancelled는 source-level cancel
/// (현재 GitHubReleasesSource는 reqwest Drop으로 cancel). Cancelled 변환은 caller 책임.
pub async fn run_check_once<S>(
    source: &S,
    current_version: &str,
    check_id: &str,
    repo: &str,
    cancel: &CancellationToken,
    channel: &Channel<UpdateEvent>,
) where
    S: UpdateSource + ?Sized,
{
    // 1. Started.
    emit_or_cancel(
        channel,
        cancel,
        UpdateEvent::Started {
            check_id: check_id.to_string(),
            current_version: current_version.to_string(),
            repo: repo.to_string(),
        },
    );

    if cancel.is_cancelled() {
        emit_or_cancel(
            channel,
            cancel,
            UpdateEvent::Cancelled {
                check_id: check_id.to_string(),
            },
        );
        return;
    }

    // 2. source 호출 + cancel 동시 listen.
    let source_call = source.latest_version();
    let result = tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            emit_or_cancel(
                channel,
                cancel,
                UpdateEvent::Cancelled {
                    check_id: check_id.to_string(),
                },
            );
            return;
        }
        r = source_call => r,
    };

    // 3. 결과별 분기.
    match result {
        Ok(release) => match is_outdated(current_version, &release.version) {
            Ok(true) => {
                emit_or_cancel(
                    channel,
                    cancel,
                    UpdateEvent::Outdated {
                        check_id: check_id.to_string(),
                        current_version: current_version.to_string(),
                        latest: ReleaseInfoDto::from_release(&release),
                    },
                );
            }
            Ok(false) => {
                emit_or_cancel(
                    channel,
                    cancel,
                    UpdateEvent::UpToDate {
                        check_id: check_id.to_string(),
                        current_version: current_version.to_string(),
                    },
                );
            }
            Err(e) => {
                emit_or_cancel(
                    channel,
                    cancel,
                    UpdateEvent::Failed {
                        check_id: check_id.to_string(),
                        error: format!("{e}"),
                    },
                );
            }
        },
        Err(UpdaterError::Cancelled) => {
            emit_or_cancel(
                channel,
                cancel,
                UpdateEvent::Cancelled {
                    check_id: check_id.to_string(),
                },
            );
        }
        Err(e) => {
            emit_or_cancel(
                channel,
                cancel,
                UpdateEvent::Failed {
                    check_id: check_id.to_string(),
                    error: format!("{e}"),
                },
            );
        }
    }
}

// ───────────────────────────────────────────────────────────────────
// Tauri commands
// ───────────────────────────────────────────────────────────────────

/// 단발 update check 시작. check_id를 즉시 반환하고, source 호출은 백그라운드 task.
/// 진행 이벤트는 `on_event` Channel로 흘려보낸다.
#[tauri::command]
pub async fn check_for_update(
    repo: String,
    current_version: String,
    on_event: Channel<UpdateEvent>,
    registry: State<'_, Arc<UpdaterRegistry>>,
) -> Result<String, UpdaterApiError> {
    validate_repo(&repo)?;
    if current_version.trim().is_empty() {
        return Err(UpdaterApiError::StartFailed {
            message: "현재 버전이 비어 있어요".to_string(),
        });
    }

    let check_id = Uuid::new_v4().to_string();
    let cancel = registry.register(&check_id, &repo).await;
    let registry_arc: Arc<UpdaterRegistry> = registry.inner().clone();
    let id_for_return = check_id.clone();

    let source = GitHubReleasesSource::new(repo.clone());
    let cancel_clone = cancel.clone();
    let check_id_for_task = check_id.clone();
    let repo_for_task = repo.clone();

    // Tauri 2 정책: tauri::async_runtime::spawn 사용.
    tauri::async_runtime::spawn(async move {
        run_check_once(
            &source,
            &current_version,
            &check_id_for_task,
            &repo_for_task,
            &cancel_clone,
            &on_event,
        )
        .await;
        registry_arc.remove(&check_id_for_task).await;
    });

    Ok(id_for_return)
}

/// 단발 check cancel — idempotent. 미존재 = no-op.
#[tauri::command]
pub async fn cancel_update_check(
    check_id: String,
    registry: State<'_, Arc<UpdaterRegistry>>,
) -> Result<(), UpdaterApiError> {
    registry.cancel(&check_id).await;
    Ok(())
}

/// 자동 폴러 시작. single-slot — 이미 실행 중이면 거부. interval_secs 1h~24h 강제.
///
/// outdated 감지 시 callback이 Channel로 Outdated event emit. Poller는 dedup invariant 보유 — 같은
/// 버전을 두 번 emit 안 함.
#[tauri::command]
pub async fn start_auto_update_poller(
    repo: String,
    current_version: String,
    interval_secs: u64,
    on_event: Channel<UpdateEvent>,
    poller_state: State<'_, Arc<PollerState>>,
) -> Result<(), UpdaterApiError> {
    validate_repo(&repo)?;
    validate_interval(interval_secs)?;
    if current_version.trim().is_empty() {
        return Err(UpdaterApiError::StartFailed {
            message: "현재 버전이 비어 있어요".to_string(),
        });
    }

    let cancel = CancellationToken::new();
    let last_check: Arc<AsyncMutex<Option<String>>> = Arc::new(AsyncMutex::new(None));

    // single-slot 등록 — 실패하면 이미 다른 폴러가 실행 중.
    poller_state
        .install(
            repo.clone(),
            interval_secs,
            cancel.clone(),
            Arc::clone(&last_check),
        )
        .await?;

    let source: Arc<dyn UpdateSource> = Arc::new(GitHubReleasesSource::new(repo.clone()));
    let poller = Poller::with_interval(
        source,
        current_version.clone(),
        Duration::from_secs(interval_secs),
    );

    let channel = on_event.clone();
    let cancel_for_task = cancel.clone();
    let poller_state_arc: Arc<PollerState> = poller_state.inner().clone();
    let current_version_for_task = current_version.clone();
    let last_check_for_task = Arc::clone(&last_check);

    tauri::async_runtime::spawn(async move {
        // Phase 8'.a.3 — last_check_iso 갱신은 매 cycle source 호출 *성공* 시 (outdated/uptodate 모두).
        //                source 실패는 갱신 안 함 (실패 = "확인 못 함"). Poller가 cycle hook을 호출.
        // dedup은 Poller가 보유. last_notified는 메모리에만 — 앱 재시작 시 리셋 (ADR-0026 감내).
        let channel_for_cb = channel.clone();
        let cancel_for_cb = cancel_for_task.clone();
        let current_for_cb = current_version_for_task.clone();
        let on_update = move |release: ReleaseInfo| {
            // 자동 폴러는 check_id를 매 emit마다 새로 발급 (각 outdated 감지 = 별개 사용자 알림).
            let cid = Uuid::new_v4().to_string();
            emit_or_cancel(
                &channel_for_cb,
                &cancel_for_cb,
                UpdateEvent::Outdated {
                    check_id: cid,
                    current_version: current_for_cb.clone(),
                    latest: ReleaseInfoDto::from_release(&release),
                },
            );
        };

        // cycle 성공 hook — source 호출이 OK일 때마다 last_check_iso 갱신.
        let last_for_cycle = Arc::clone(&last_check_for_task);
        let on_cycle_success = move |_version: &str| {
            let now_iso = time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default();
            let last_clone = Arc::clone(&last_for_cycle);
            // tokio AsyncMutex — blocking_lock 금지(tokio runtime 안). spawn으로 비차단.
            tauri::async_runtime::spawn(async move {
                let mut g = last_clone.lock().await;
                *g = Some(now_iso);
            });
        };

        poller
            .run_with_lifecycle(on_update, on_cycle_success, cancel_for_task)
            .await;

        // 폴러 종료 시 state cleanup.
        poller_state_arc.clear().await;
    });

    tracing::info!(repo = %repo, interval_secs, "자동 갱신 폴러를 시작했어요");
    Ok(())
}

/// 자동 폴러 중단 — idempotent. 미실행 = no-op.
#[tauri::command]
pub async fn stop_auto_update_poller(
    poller_state: State<'_, Arc<PollerState>>,
) -> Result<(), UpdaterApiError> {
    poller_state.stop().await;
    Ok(())
}

/// 자동 폴러 상태 조회.
#[tauri::command]
pub async fn get_auto_update_status(
    poller_state: State<'_, Arc<PollerState>>,
) -> Result<PollerStatus, UpdaterApiError> {
    Ok(poller_state.snapshot().await)
}

// ───────────────────────────────────────────────────────────────────
// 종료 helper — RunEvent::ExitRequested에서 호출 (sync).
// ───────────────────────────────────────────────────────────────────

pub fn cancel_all_blocking(registry: &UpdaterRegistry) {
    registry.cancel_all_blocking();
}

pub fn stop_poller_blocking(state: &PollerState) {
    state.stop_blocking();
}

// ───────────────────────────────────────────────────────────────────
// Tests
// ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use auto_updater::MockSource;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn release(version: &str) -> ReleaseInfo {
        ReleaseInfo {
            version: version.to_string(),
            published_at: time::OffsetDateTime::UNIX_EPOCH,
            url: format!("https://example.com/{version}"),
            notes: Some("changelog".into()),
        }
    }

    fn counting_channel() -> (
        Channel<UpdateEvent>,
        Arc<AtomicUsize>,
        Arc<AsyncMutex<Vec<UpdateEvent>>>,
    ) {
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();
        let events: Arc<AsyncMutex<Vec<UpdateEvent>>> = Arc::new(AsyncMutex::new(Vec::new()));
        let events_clone = Arc::clone(&events);
        let ch: Channel<UpdateEvent> = Channel::new(
            move |body: tauri::ipc::InvokeResponseBody| -> tauri::Result<()> {
                count_clone.fetch_add(1, Ordering::SeqCst);
                // body는 InvokeResponseBody — JSON 본문만 추출 가능. 파싱 시도.
                let json_text = match &body {
                    tauri::ipc::InvokeResponseBody::Json(s) => s.clone(),
                    _ => String::new(),
                };
                if !json_text.is_empty() {
                    if let Ok(ev) = serde_json::from_str::<serde_json::Value>(&json_text) {
                        let kind = ev.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                        // 더 정확한 enum 재구성은 필요 시 — 여기서는 카운트 + raw value 보관.
                        let synthetic = match kind {
                            "started" => UpdateEvent::Started {
                                check_id: ev
                                    .get("check_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                                current_version: ev
                                    .get("current_version")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                                repo: ev
                                    .get("repo")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                            },
                            "outdated" => UpdateEvent::Outdated {
                                check_id: ev
                                    .get("check_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                                current_version: ev
                                    .get("current_version")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                                latest: serde_json::from_value(
                                    ev.get("latest").cloned().unwrap_or(serde_json::Value::Null),
                                )
                                .unwrap_or(ReleaseInfoDto {
                                    version: String::new(),
                                    published_at_iso: String::new(),
                                    url: String::new(),
                                    notes: None,
                                }),
                            },
                            "up-to-date" => UpdateEvent::UpToDate {
                                check_id: ev
                                    .get("check_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                                current_version: ev
                                    .get("current_version")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                            },
                            "failed" => UpdateEvent::Failed {
                                check_id: ev
                                    .get("check_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                                error: ev
                                    .get("error")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                            },
                            "cancelled" => UpdateEvent::Cancelled {
                                check_id: ev
                                    .get("check_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                            },
                            _ => return Ok(()),
                        };
                        let events_inner = Arc::clone(&events_clone);
                        tauri::async_runtime::spawn(async move {
                            events_inner.lock().await.push(synthetic);
                        });
                    }
                }
                Ok(())
            },
        );
        (ch, count, events)
    }

    // ── Registry CRUD ────────────────────────────────────────────────

    #[tokio::test]
    async fn registry_register_and_get() {
        let r = UpdaterRegistry::new();
        let _ = r.register("c1", "owner/repo").await;
        let entry = r.get("c1").await;
        assert!(entry.is_some());
        let (repo, _elapsed) = entry.unwrap();
        assert_eq!(repo, "owner/repo");
    }

    #[tokio::test]
    async fn registry_remove_idempotent() {
        let r = UpdaterRegistry::new();
        let _ = r.register("c1", "o/r").await;
        r.remove("c1").await;
        r.remove("c1").await; // 두 번째 — 패닉 안 함.
        assert_eq!(r.in_flight_count().await, 0);
    }

    #[tokio::test]
    async fn registry_cancel_marks_token() {
        let r = UpdaterRegistry::new();
        let tok = r.register("c1", "o/r").await;
        r.cancel("c1").await;
        assert!(tok.is_cancelled());
    }

    #[tokio::test]
    async fn registry_cancel_unknown_is_noop() {
        let r = UpdaterRegistry::new();
        r.cancel("missing").await; // 패닉 안 함.
        assert_eq!(r.in_flight_count().await, 0);
    }

    #[tokio::test]
    async fn registry_cancel_all_marks_every_token() {
        let r = UpdaterRegistry::new();
        let t1 = r.register("c1", "o/r").await;
        let t2 = r.register("c2", "o/r").await;
        r.cancel_all().await;
        assert!(t1.is_cancelled());
        assert!(t2.is_cancelled());
    }

    #[test]
    fn registry_cancel_all_blocking_does_not_panic_on_empty() {
        let r = UpdaterRegistry::new();
        r.cancel_all_blocking();
    }

    // ── Event serde kebab-case ──────────────────────────────────────

    #[test]
    fn event_started_serializes_kebab() {
        let ev = UpdateEvent::Started {
            check_id: "c1".into(),
            current_version: "0.1.0".into(),
            repo: "o/r".into(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "started");
        assert_eq!(v["check_id"], "c1");
        assert_eq!(v["current_version"], "0.1.0");
        assert_eq!(v["repo"], "o/r");
    }

    #[test]
    fn event_outdated_serializes_with_release() {
        let ev = UpdateEvent::Outdated {
            check_id: "c1".into(),
            current_version: "0.1.0".into(),
            latest: ReleaseInfoDto {
                version: "0.2.0".into(),
                published_at_iso: "2026-04-28T00:00:00Z".into(),
                url: "https://example.com/r".into(),
                notes: None,
            },
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "outdated");
        assert_eq!(v["latest"]["version"], "0.2.0");
        assert_eq!(v["latest"]["published_at_iso"], "2026-04-28T00:00:00Z");
    }

    #[test]
    fn event_up_to_date_uses_kebab() {
        let ev = UpdateEvent::UpToDate {
            check_id: "c1".into(),
            current_version: "0.1.0".into(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "up-to-date");
    }

    #[test]
    fn event_failed_includes_error() {
        let ev = UpdateEvent::Failed {
            check_id: "c1".into(),
            error: "안 됐어요".into(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "failed");
        assert!(v["error"].as_str().unwrap().contains("안 됐어요"));
    }

    #[test]
    fn event_cancelled_kind_only() {
        let ev = UpdateEvent::Cancelled {
            check_id: "c1".into(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "cancelled");
    }

    #[test]
    fn api_error_poller_already_running_kebab() {
        let e = UpdaterApiError::PollerAlreadyRunning;
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "poller-already-running");
        assert!(format!("{e}").contains("자동 갱신"));
    }

    #[test]
    fn api_error_interval_out_of_range_kebab() {
        let e = UpdaterApiError::IntervalOutOfRange { got: 60 };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "interval-out-of-range");
        assert!(format!("{e}").contains("1시간"));
        assert!(format!("{e}").contains("24시간"));
    }

    #[test]
    fn api_error_invalid_repo_kebab() {
        let e = UpdaterApiError::InvalidRepo;
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "invalid-repo");
        assert!(format!("{e}").contains("owner/repo"));
    }

    // ── ReleaseInfoDto ──────────────────────────────────────────────

    #[test]
    fn release_info_dto_round_trip() {
        let r = release("1.0.0");
        let dto = ReleaseInfoDto::from_release(&r);
        assert_eq!(dto.version, "1.0.0");
        assert!(!dto.published_at_iso.is_empty());
        let json = serde_json::to_string(&dto).unwrap();
        let back: ReleaseInfoDto = serde_json::from_str(&json).unwrap();
        assert_eq!(dto, back);
    }

    // ── Validation ──────────────────────────────────────────────────

    #[test]
    fn validate_repo_accepts_owner_slash_repo() {
        assert!(validate_repo("anthropics/lmmaster").is_ok());
        assert!(validate_repo("foo-bar/baz_qux.gh").is_ok());
    }

    #[test]
    fn validate_repo_rejects_empty_or_no_slash() {
        assert!(validate_repo("").is_err());
        assert!(validate_repo("noslash").is_err());
        assert!(validate_repo("/").is_err());
        assert!(validate_repo("a/").is_err());
        assert!(validate_repo("/b").is_err());
        assert!(validate_repo("a/b/c").is_err()); // 두 슬래시.
    }

    #[test]
    fn validate_repo_rejects_whitespace() {
        assert!(validate_repo("a /b").is_err());
        assert!(validate_repo("a/b ").is_ok()); // trim 후 OK.
        assert!(validate_repo("a /b ").is_err());
    }

    #[test]
    fn validate_interval_accepts_in_range() {
        assert!(validate_interval(MIN_INTERVAL_SECS).is_ok());
        assert!(validate_interval(MAX_INTERVAL_SECS).is_ok());
        assert!(validate_interval(6 * 60 * 60).is_ok()); // 6h.
    }

    #[test]
    fn validate_interval_rejects_below_min() {
        assert!(validate_interval(0).is_err());
        assert!(validate_interval(60).is_err());
        assert!(validate_interval(MIN_INTERVAL_SECS - 1).is_err());
    }

    #[test]
    fn validate_interval_rejects_above_max() {
        assert!(validate_interval(MAX_INTERVAL_SECS + 1).is_err());
        assert!(validate_interval(7 * 24 * 60 * 60).is_err()); // 1주.
    }

    // ── run_check_once with MockSource ───────────────────────────────

    #[tokio::test]
    async fn run_check_once_outdated_emits_outdated_event() {
        let mock = MockSource::with_release(release("1.1.0"));
        let cancel = CancellationToken::new();
        let (ch, count, events) = counting_channel();
        run_check_once(&mock, "1.0.0", "c1", "o/r", &cancel, &ch).await;
        // Started + Outdated.
        let n = count.load(Ordering::SeqCst);
        assert!(n >= 2, "expected ≥2 events, got {n}");
        // events에 Outdated가 포함되도록.
        // bridge spawn가 비동기라 약간 yield.
        for _ in 0..32 {
            tokio::task::yield_now().await;
        }
        let kinds: Vec<&'static str> = events
            .lock()
            .await
            .iter()
            .map(|e| match e {
                UpdateEvent::Started { .. } => "started",
                UpdateEvent::Outdated { .. } => "outdated",
                UpdateEvent::UpToDate { .. } => "up-to-date",
                UpdateEvent::Failed { .. } => "failed",
                UpdateEvent::Cancelled { .. } => "cancelled",
            })
            .collect();
        assert!(kinds.contains(&"outdated"), "kinds={kinds:?}");
    }

    #[tokio::test]
    async fn run_check_once_same_version_emits_up_to_date() {
        let mock = MockSource::with_release(release("1.0.0"));
        let cancel = CancellationToken::new();
        let (ch, count, events) = counting_channel();
        run_check_once(&mock, "1.0.0", "c1", "o/r", &cancel, &ch).await;
        let n = count.load(Ordering::SeqCst);
        assert!(n >= 2, "expected ≥2 events, got {n}");
        for _ in 0..32 {
            tokio::task::yield_now().await;
        }
        let has_up = events
            .lock()
            .await
            .iter()
            .any(|e| matches!(e, UpdateEvent::UpToDate { .. }));
        assert!(has_up, "UpToDate event 누락");
    }

    #[tokio::test]
    async fn run_check_once_no_releases_emits_failed_with_korean() {
        let mock = MockSource::new(); // None → NoReleases.
        let cancel = CancellationToken::new();
        let (ch, count, events) = counting_channel();
        run_check_once(&mock, "1.0.0", "c1", "o/r", &cancel, &ch).await;
        let n = count.load(Ordering::SeqCst);
        assert!(n >= 2, "expected ≥2 events, got {n}");
        for _ in 0..32 {
            tokio::task::yield_now().await;
        }
        let mut found_korean = false;
        for ev in events.lock().await.iter() {
            if let UpdateEvent::Failed { error, .. } = ev {
                if error.contains("릴리스") {
                    found_korean = true;
                    break;
                }
            }
        }
        assert!(found_korean, "한국어 Failed event 누락");
    }

    #[tokio::test]
    async fn run_check_once_invalid_version_emits_failed() {
        let mock = MockSource::with_release(release("not.a.version"));
        let cancel = CancellationToken::new();
        let (ch, _count, events) = counting_channel();
        run_check_once(&mock, "1.0.0", "c1", "o/r", &cancel, &ch).await;
        for _ in 0..32 {
            tokio::task::yield_now().await;
        }
        let has_failed = events
            .lock()
            .await
            .iter()
            .any(|e| matches!(e, UpdateEvent::Failed { .. }));
        assert!(has_failed, "Failed event 누락");
    }

    #[tokio::test]
    async fn run_check_once_pre_cancel_emits_cancelled() {
        let mock = MockSource::with_release(release("1.1.0"));
        let cancel = CancellationToken::new();
        cancel.cancel();
        let (ch, _count, events) = counting_channel();
        run_check_once(&mock, "1.0.0", "c1", "o/r", &cancel, &ch).await;
        for _ in 0..32 {
            tokio::task::yield_now().await;
        }
        let has_cancel = events
            .lock()
            .await
            .iter()
            .any(|e| matches!(e, UpdateEvent::Cancelled { .. }));
        assert!(has_cancel, "Cancelled event 누락");
    }

    // ── PollerState ──────────────────────────────────────────────────

    #[tokio::test]
    async fn poller_state_idle_status_is_inactive() {
        let s = PollerState::new();
        let snap = s.snapshot().await;
        assert!(!snap.active);
        assert!(snap.repo.is_none());
        assert!(snap.interval_secs.is_none());
        assert!(snap.last_check_iso.is_none());
    }

    #[tokio::test]
    async fn poller_state_install_then_status_is_active() {
        let s = PollerState::new();
        let cancel = CancellationToken::new();
        let last = Arc::new(AsyncMutex::new(None));
        s.install(
            "owner/repo".into(),
            6 * 60 * 60,
            cancel.clone(),
            Arc::clone(&last),
        )
        .await
        .unwrap();
        let snap = s.snapshot().await;
        assert!(snap.active);
        assert_eq!(snap.repo.as_deref(), Some("owner/repo"));
        assert_eq!(snap.interval_secs, Some(6 * 60 * 60));
    }

    #[tokio::test]
    async fn poller_state_install_twice_rejects() {
        let s = PollerState::new();
        let cancel = CancellationToken::new();
        let last = Arc::new(AsyncMutex::new(None));
        s.install("o/r".into(), 3600, cancel.clone(), Arc::clone(&last))
            .await
            .unwrap();
        let err = s
            .install("o/r2".into(), 3600, cancel, Arc::clone(&last))
            .await
            .unwrap_err();
        assert!(matches!(err, UpdaterApiError::PollerAlreadyRunning));
    }

    #[tokio::test]
    async fn poller_state_stop_cancels_and_clears() {
        let s = PollerState::new();
        let cancel = CancellationToken::new();
        let last = Arc::new(AsyncMutex::new(None));
        s.install("o/r".into(), 3600, cancel.clone(), last)
            .await
            .unwrap();
        s.stop().await;
        assert!(cancel.is_cancelled());
        assert!(!s.snapshot().await.active);
    }

    #[tokio::test]
    async fn poller_state_stop_idempotent_when_idle() {
        let s = PollerState::new();
        s.stop().await; // idle → no-op, panic 안 함.
        s.stop().await;
        assert!(!s.snapshot().await.active);
    }

    #[test]
    fn poller_state_stop_blocking_does_not_panic_on_empty() {
        let s = PollerState::new();
        s.stop_blocking();
    }
}
