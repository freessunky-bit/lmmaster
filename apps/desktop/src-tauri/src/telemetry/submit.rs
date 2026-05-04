//! GlitchTip-compatible event submission scaffold — Phase 7'.b.
//!
//! 정책 (ADR-0027 §5, ADR-0041, phase-7p-release-prep-reinforcement.md §5):
//! - Sentry-compatible JSON shape (event_id / timestamp / level / message / extra).
//! - GlitchTip self-hosted DSN을 환경변수 `LMMASTER_GLITCHTIP_DSN`로 주입.
//! - DSN 미설정 시 비활성: queue에만 적재 (drop X) — 사용자가 endpoint 설정 후 v1.x에서 flush 가능.
//! - DSN 설정 시 backon 3회 retry POST. 실패한 이벤트는 queue에 남겨 다음 cycle에 재시도.
//! - queue cap 200, oldest drop. 24h retention은 timestamp 기반 expire.
//! - 외부 통신 0 정책 (ADR-0013) 예외: opt-in + 단일 도메인 (DSN 호스트만).
//! - Sentry SaaS 거부 — privacy thesis 위반.
//!
//! `TelemetryEvent` shape는 Sentry envelope의 최소 부분집합 (GlitchTip 0.5+ 호환).

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

/// 환경 변수명 — DSN 미설정 시 endpoint 비활성.
pub const GLITCHTIP_DSN_ENV_VAR: &str = "LMMASTER_GLITCHTIP_DSN";

/// 큐 cap — 200 이벤트. oldest drop.
pub const DEFAULT_QUEUE_CAP: usize = 200;

/// 24h retention — 그 이전 이벤트는 expire.
pub const RETENTION_SECS: i64 = 24 * 60 * 60;

/// retry — backon constant 3회 (즉시 / 1s / 5s).
pub const MAX_RETRIES: u32 = 3;

// ───────────────────────────────────────────────────────────────────
// Event DTO — Sentry envelope 호환
// ───────────────────────────────────────────────────────────────────

/// Sentry / GlitchTip의 `level` 필드.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventLevel {
    Info,
    Warning,
    Error,
}

/// Sentry envelope의 최소 부분집합.
///
/// GlitchTip 0.5+ store endpoint 호환:
///   POST `{base}/api/{project_id}/store/`
///   Body = JSON event (event_id / timestamp / level / message / platform / extra).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TelemetryEvent {
    /// 32-byte hex (UUID v4 dash 제거) — Sentry 규칙.
    pub event_id: String,
    /// RFC3339 ISO timestamp.
    pub timestamp: String,
    pub level: EventLevel,
    pub message: String,
    /// 플랫폼 — Sentry SDK가 보통 "rust" / "javascript" 등.
    pub platform: String,
    /// 사용자가 익명 UUID — TelemetryConfig::anon_id에서 주입.
    pub anon_id: Option<String>,
}

impl TelemetryEvent {
    /// 새 event 생성. timestamp는 호출 시점 UTC, event_id는 새 UUIDv4.
    pub fn new(level: EventLevel, message: String, anon_id: Option<String>) -> Self {
        let event_id = Uuid::new_v4().simple().to_string();
        let timestamp = OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default();
        Self {
            event_id,
            timestamp,
            level,
            message,
            platform: "rust".into(),
            anon_id,
        }
    }

    /// timestamp가 현재 시점 기준 24h를 넘었는지.
    fn is_expired(&self, now: OffsetDateTime) -> bool {
        let parsed = OffsetDateTime::parse(
            &self.timestamp,
            &time::format_description::well_known::Rfc3339,
        );
        match parsed {
            Ok(t) => (now - t).whole_seconds() > RETENTION_SECS,
            Err(_) => false, // parse 실패 = 보존 (의심스러우면 안 버림).
        }
    }
}

/// `submit` 결과 — 사용자/테스트 향 가시성.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum EventSubmitOutcome {
    /// DSN 미설정 → queue에 적재만.
    Queued,
    /// DSN 설정 + POST 성공.
    Sent,
    /// DSN 설정 + POST 실패 (3회 retry 후) → queue에 적재.
    Retained { reason: String },
}

// ───────────────────────────────────────────────────────────────────
// EventQueue — Mutex<VecDeque<TelemetryEvent>>, cap 200, oldest drop
// ───────────────────────────────────────────────────────────────────

pub struct EventQueue {
    inner: AsyncMutex<VecDeque<TelemetryEvent>>,
    /// DSN endpoint. None이면 retention only.
    dsn: Option<String>,
    /// 큐 cap. 초과 시 oldest drop.
    cap: usize,
}

impl EventQueue {
    /// 환경변수 + 기본 cap으로 새 큐 생성.
    pub fn new_default() -> Self {
        let dsn = std::env::var(GLITCHTIP_DSN_ENV_VAR).ok();
        Self::new(dsn, DEFAULT_QUEUE_CAP)
    }

    pub fn new(dsn: Option<String>, cap: usize) -> Self {
        Self {
            inner: AsyncMutex::new(VecDeque::with_capacity(cap.max(1))),
            dsn,
            cap: cap.max(1),
        }
    }

    pub fn dsn_configured(&self) -> bool {
        self.dsn.is_some()
    }

    /// 현재 적재된 이벤트 개수.
    pub async fn len(&self) -> usize {
        self.inner.lock().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.inner.lock().await.is_empty()
    }

    /// 24h 초과 이벤트 제거. 호출 측이 명시적으로 invoke (보통 submit / drain 직전).
    pub async fn evict_expired(&self) -> usize {
        let now = OffsetDateTime::now_utc();
        let mut g = self.inner.lock().await;
        let before = g.len();
        g.retain(|e| !e.is_expired(now));
        before - g.len()
    }

    /// 새 이벤트 적재. cap 초과 시 oldest drop.
    async fn enqueue(&self, event: TelemetryEvent) {
        let mut g = self.inner.lock().await;
        while g.len() >= self.cap {
            g.pop_front();
        }
        g.push_back(event);
    }

    /// `submit` — DSN 미설정이면 queue에만 적재(Queued).
    /// 설정이면 backon 3회 retry POST. 성공 시 Sent, 실패 시 queue 적재(Retained).
    pub async fn submit(&self, event: TelemetryEvent) -> EventSubmitOutcome {
        // 24h 초과 이벤트 사전 청소 — 큐가 cap에 가깝지 않도록.
        let _ = self.evict_expired().await;
        match self.dsn.clone() {
            None => {
                self.enqueue(event).await;
                EventSubmitOutcome::Queued
            }
            Some(dsn) => match try_post(&dsn, &event).await {
                Ok(()) => EventSubmitOutcome::Sent,
                Err(reason) => {
                    self.enqueue(event).await;
                    EventSubmitOutcome::Retained { reason }
                }
            },
        }
    }

    /// 모든 이벤트를 꺼내고 (drain). 외부 호출 (e.g. shutdown flush)에서 사용.
    /// queue retention 디버깅용 — v1.x에서 startup flush가 사용 예정.
    pub async fn drain(&self) -> Vec<TelemetryEvent> {
        let mut g = self.inner.lock().await;
        g.drain(..).collect()
    }
}

// ───────────────────────────────────────────────────────────────────
// HTTP POST — backon 3회 retry
// ───────────────────────────────────────────────────────────────────

/// GlitchTip / Sentry store endpoint POST 시도.
///
/// - DSN 형식: `https://<key>@host[:port]/<project_id>` (Sentry 표준).
/// - URL 변환: `https://host[:port]/api/<project_id>/store/`.
/// - Header: `X-Sentry-Auth: Sentry sentry_key=<key>, sentry_version=7, sentry_client=lmmaster/0.1`.
/// - Body: JSON `TelemetryEvent`.
/// - timeout 5s, backon constant 3회.
async fn try_post(dsn: &str, event: &TelemetryEvent) -> Result<(), String> {
    let parsed = parse_dsn(dsn).map_err(|e| format!("DSN 파싱 실패: {e}"))?;
    let body = serde_json::to_vec(event).map_err(|e| format!("이벤트 직렬화 실패: {e}"))?;

    // Phase R-C (ADR-0055) — .no_proxy() 강제. GlitchTip 외부 통신은 self-hosted 도메인만.
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("HTTP 클라이언트 생성 실패: {e}"))?;

    let auth_header = format!(
        "Sentry sentry_version=7, sentry_client=lmmaster/0.1, sentry_key={}",
        parsed.public_key
    );

    let mut last_err: Option<String> = None;
    for attempt in 0..MAX_RETRIES {
        let resp = client
            .post(&parsed.store_url)
            .header("Content-Type", "application/json")
            .header("X-Sentry-Auth", &auth_header)
            .body(body.clone())
            .send()
            .await;
        match resp {
            Ok(r) if r.status().is_success() => return Ok(()),
            Ok(r) => {
                let status = r.status();
                last_err = Some(format!("HTTP {status} (attempt {})", attempt + 1));
                if !status.is_server_error() {
                    // 4xx — retry 무의미.
                    break;
                }
            }
            Err(e) => {
                last_err = Some(format!("network error (attempt {}): {e}", attempt + 1));
            }
        }
        // backoff: 0s / 1s / 5s.
        let delay = match attempt {
            0 => std::time::Duration::from_millis(0),
            1 => std::time::Duration::from_secs(1),
            _ => std::time::Duration::from_secs(5),
        };
        if delay > std::time::Duration::ZERO {
            tokio::time::sleep(delay).await;
        }
    }
    Err(last_err.unwrap_or_else(|| "알 수 없는 전송 실패".into()))
}

struct ParsedDsn {
    public_key: String,
    store_url: String,
}

/// `https://<key>@host[:port]/<project_id>` → `(public_key, store_url)`.
fn parse_dsn(dsn: &str) -> Result<ParsedDsn, String> {
    let dsn = dsn.trim();
    if dsn.is_empty() {
        return Err("DSN이 비었어요".into());
    }
    let (scheme_marker, rest) = dsn.split_once("://").ok_or("scheme이 없어요")?;
    if scheme_marker != "https" && scheme_marker != "http" {
        return Err(format!("scheme이 http(s)가 아니에요: {scheme_marker}"));
    }
    let (key_part, host_path) = rest.split_once('@').ok_or("key가 없어요")?;
    if key_part.is_empty() {
        return Err("key가 비었어요".into());
    }
    let (host_port, project_id) = host_path.rsplit_once('/').ok_or("project_id가 없어요")?;
    if project_id.is_empty() || host_port.is_empty() {
        return Err("host 또는 project_id가 비었어요".into());
    }
    let store_url = format!("{scheme_marker}://{host_port}/api/{project_id}/store/");
    Ok(ParsedDsn {
        public_key: key_part.to_string(),
        store_url,
    })
}

// ───────────────────────────────────────────────────────────────────
// 단위 테스트
// ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_event_new_has_uuid_and_timestamp() {
        let ev = TelemetryEvent::new(EventLevel::Error, "boom".into(), None);
        assert!(!ev.event_id.is_empty());
        assert_eq!(ev.event_id.len(), 32, "Sentry-style 32-char hex");
        assert!(!ev.timestamp.is_empty());
        assert_eq!(ev.level, EventLevel::Error);
        assert_eq!(ev.message, "boom");
        assert_eq!(ev.platform, "rust");
        assert!(ev.anon_id.is_none());
    }

    #[test]
    fn event_level_serializes_lowercase() {
        let v = serde_json::to_value(EventLevel::Warning).unwrap();
        assert_eq!(v, serde_json::json!("warning"));
        let v = serde_json::to_value(EventLevel::Error).unwrap();
        assert_eq!(v, serde_json::json!("error"));
    }

    #[test]
    fn parse_dsn_extracts_key_and_store_url() {
        let p = parse_dsn("https://abc123@telemetry.example.com/42").unwrap();
        assert_eq!(p.public_key, "abc123");
        assert_eq!(p.store_url, "https://telemetry.example.com/api/42/store/");
    }

    #[test]
    fn parse_dsn_rejects_invalid_inputs() {
        assert!(parse_dsn("").is_err());
        assert!(parse_dsn("not-a-url").is_err());
        assert!(parse_dsn("ftp://abc@host/1").is_err());
        assert!(parse_dsn("https://host/1").is_err()); // key 없음.
        assert!(parse_dsn("https://abc@host").is_err()); // project_id 없음.
        assert!(parse_dsn("https://@host/1").is_err()); // key 비어 있음.
    }

    #[tokio::test]
    async fn submit_when_no_dsn_queues_only() {
        let q = EventQueue::new(None, 10);
        let ev = TelemetryEvent::new(EventLevel::Info, "hi".into(), None);
        let outcome = q.submit(ev.clone()).await;
        assert!(matches!(outcome, EventSubmitOutcome::Queued));
        assert_eq!(q.len().await, 1);
        let drained = q.drain().await;
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].message, "hi");
    }

    #[tokio::test]
    async fn queue_cap_drops_oldest() {
        let q = EventQueue::new(None, 3);
        for i in 0..5 {
            let ev = TelemetryEvent::new(EventLevel::Info, format!("m{i}"), None);
            let _ = q.submit(ev).await;
        }
        assert_eq!(q.len().await, 3, "cap=3 — 5개 들어가도 최대 3개");
        let kept: Vec<String> = q.drain().await.into_iter().map(|e| e.message).collect();
        // oldest drop이면 마지막 3개 (m2, m3, m4)가 남아 있어야 함.
        assert_eq!(kept, vec!["m2".to_string(), "m3".into(), "m4".into()]);
    }

    #[tokio::test]
    async fn evict_expired_removes_old_events() {
        let q = EventQueue::new(None, 10);
        // 25h 전 timestamp를 강제 주입.
        let old_ts = (OffsetDateTime::now_utc() - time::Duration::hours(25))
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        let mut old = TelemetryEvent::new(EventLevel::Info, "old".into(), None);
        old.timestamp = old_ts;
        q.enqueue(old).await;
        let fresh = TelemetryEvent::new(EventLevel::Info, "fresh".into(), None);
        q.enqueue(fresh).await;
        assert_eq!(q.len().await, 2);
        let evicted = q.evict_expired().await;
        assert_eq!(evicted, 1);
        assert_eq!(q.len().await, 1);
        let kept = q.drain().await;
        assert_eq!(kept[0].message, "fresh");
    }

    #[tokio::test]
    async fn dsn_configured_reports_correctly() {
        let q1 = EventQueue::new(None, 10);
        assert!(!q1.dsn_configured());
        let q2 = EventQueue::new(Some("https://k@h/1".into()), 10);
        assert!(q2.dsn_configured());
    }

    #[tokio::test]
    async fn submit_with_unreachable_dsn_retains_event() {
        // 127.0.0.1:1 — 즉시 connect refused (포트 1 = TCPMUX, 일반적으로 closed).
        let q = EventQueue::new(Some("https://abc@127.0.0.1:1/42".into()), 10);
        let ev = TelemetryEvent::new(EventLevel::Error, "panic".into(), None);
        let outcome = q.submit(ev).await;
        match outcome {
            EventSubmitOutcome::Retained { .. } => {}
            other => panic!("expected Retained, got {other:?}"),
        }
        assert_eq!(q.len().await, 1, "실패한 이벤트는 큐에 보존");
    }

    #[test]
    fn is_expired_returns_true_for_old_timestamps() {
        let now = OffsetDateTime::now_utc();
        let mut ev = TelemetryEvent::new(EventLevel::Info, "x".into(), None);
        ev.timestamp = (now - time::Duration::hours(25))
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        assert!(ev.is_expired(now));
    }

    #[test]
    fn is_expired_returns_false_for_fresh() {
        let now = OffsetDateTime::now_utc();
        let ev = TelemetryEvent::new(EventLevel::Info, "x".into(), None);
        assert!(!ev.is_expired(now));
    }

    #[test]
    fn telemetry_event_serializes_with_expected_fields() {
        let ev = TelemetryEvent {
            event_id: "deadbeef".into(),
            timestamp: "2026-04-28T00:00:00Z".into(),
            level: EventLevel::Error,
            message: "panic at foo:42".into(),
            platform: "rust".into(),
            anon_id: Some("anon-uuid".into()),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["event_id"], "deadbeef");
        assert_eq!(v["level"], "error");
        assert_eq!(v["platform"], "rust");
        assert_eq!(v["anon_id"], "anon-uuid");
    }
}
