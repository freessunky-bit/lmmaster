//! Gateway 메트릭 수집 — Phase 13'.b. Diagnostics 페이지의 latency sparkline + 최근 요청 로그.
//!
//! 정책:
//! - 메모리 ring buffer만 — 앱 재시작 시 손실 OK (실시간 진단용, 영속 가치 낮음).
//! - 동시 안전: `RwLock` — Tauri state로 manage, IPC 호출은 read-heavy.
//! - latency: 최근 60s 동안의 모든 요청 ms를 보관 (가변 길이, 실측 100~1000 entry 예상).
//!   - sparkline UI는 30 bucket으로 평균 산출 → diag-latency-sparkline 컴포넌트.
//! - 최근 요청: `(ts, method, path, status, ms)` 튜플 last 50 entry. ring buffer.
//! - PII: query string은 caller가 strip 책임 — path만 보관 (`/v1/chat/completions`).
//! - opt-out: 향후 Settings → 진단 → "메트릭 수집 끄기" 토글 v1.x.

use std::collections::VecDeque;
use std::sync::RwLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

/// 최근 요청 한 건의 메타 — Diagnostics가 표시할 정보만.
#[derive(Debug, Clone, Serialize)]
pub struct RequestRecord {
    /// UNIX epoch ms.
    pub ts_ms: u64,
    pub method: String,
    /// query string 제외 — `/v1/chat/completions`.
    pub path: String,
    pub status: u16,
    pub ms: u32,
}

/// latency sample — 메모리 ring buffer.
#[derive(Debug, Clone, Copy)]
struct LatencySample {
    at: Instant,
    ms: u32,
}

const LATENCY_RETENTION_SECS: u64 = 60;
const RECENT_REQUESTS_CAPACITY: usize = 50;

/// 게이트웨이 메트릭 — Tauri state로 manage. middleware가 push, IPC가 read.
pub struct GatewayMetrics {
    /// 최근 60s 모든 요청 latency. 60초 지난 entry는 push 시 자동 evict.
    latency: RwLock<VecDeque<LatencySample>>,
    /// 최근 N개 요청 메타 (capacity 50).
    recent: RwLock<VecDeque<RequestRecord>>,
}

impl GatewayMetrics {
    pub fn new() -> Self {
        Self {
            latency: RwLock::new(VecDeque::new()),
            recent: RwLock::new(VecDeque::new()),
        }
    }

    /// 미들웨어가 호출 — 단일 요청 결과를 메트릭에 누적.
    pub fn record(&self, method: &str, path: &str, status: u16, ms: u32) {
        let now = Instant::now();
        let ts_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        // 1) latency ring — 60s evict.
        if let Ok(mut g) = self.latency.write() {
            // 앞에서 만료 entry 제거.
            while let Some(s) = g.front() {
                if now.duration_since(s.at) > Duration::from_secs(LATENCY_RETENTION_SECS) {
                    g.pop_front();
                } else {
                    break;
                }
            }
            g.push_back(LatencySample { at: now, ms });
        }

        // 2) 최근 N개 요청 메타.
        if let Ok(mut g) = self.recent.write() {
            if g.len() >= RECENT_REQUESTS_CAPACITY {
                g.pop_front();
            }
            g.push_back(RequestRecord {
                ts_ms,
                method: method.to_string(),
                path: path.to_string(),
                status,
                ms,
            });
        }
    }

    /// IPC용 sparkline — 최근 60s를 30 bucket(2초 간격)으로 평균. 빈 bucket은 0.
    pub fn latency_sparkline(&self) -> Vec<u32> {
        const BUCKET_COUNT: usize = 30;
        let now = Instant::now();
        let bucket_secs = LATENCY_RETENTION_SECS as f64 / BUCKET_COUNT as f64;
        let mut sums = [0u64; BUCKET_COUNT];
        let mut counts = [0u32; BUCKET_COUNT];

        if let Ok(g) = self.latency.read() {
            for s in g.iter() {
                let age_secs = now.duration_since(s.at).as_secs_f64();
                if age_secs >= LATENCY_RETENTION_SECS as f64 {
                    continue;
                }
                // 0 = 가장 최근, 29 = 가장 오래된.
                let idx_from_end = (age_secs / bucket_secs).floor() as usize;
                if idx_from_end >= BUCKET_COUNT {
                    continue;
                }
                let idx = BUCKET_COUNT - 1 - idx_from_end;
                sums[idx] += s.ms as u64;
                counts[idx] += 1;
            }
        }

        sums.iter()
            .zip(counts.iter())
            .map(|(s, c)| if *c == 0 { 0 } else { (*s / *c as u64) as u32 })
            .collect()
    }

    /// IPC용 recent — last N개. UI는 보통 5개 표시. 최근 → 오래된 순서.
    pub fn recent_requests(&self, limit: usize) -> Vec<RequestRecord> {
        if let Ok(g) = self.recent.read() {
            let take = limit.min(g.len());
            g.iter().rev().take(take).cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// p50 / p95 — 메모리 안에서 정렬 후 산출. Diagnostics 카드 메트릭.
    pub fn percentiles(&self) -> Percentiles {
        let mut samples: Vec<u32> = if let Ok(g) = self.latency.read() {
            g.iter().map(|s| s.ms).collect()
        } else {
            Vec::new()
        };
        if samples.is_empty() {
            return Percentiles::default();
        }
        samples.sort_unstable();
        let p = |q: f64| -> u32 {
            let idx = ((samples.len() as f64) * q).min(samples.len() as f64 - 1.0) as usize;
            samples[idx]
        };
        Percentiles {
            p50_ms: p(0.50),
            p95_ms: p(0.95),
            count: samples.len() as u32,
        }
    }
}

impl Default for GatewayMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct Percentiles {
    pub p50_ms: u32,
    pub p95_ms: u32,
    pub count: u32,
}

// ── Tower middleware — every request의 latency + 메타를 GatewayMetrics에 record ──

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

/// `axum::middleware::from_fn_with_state(metrics, record_metrics)` 형태로 mount.
///
/// 정책 (research §1, §3, §4):
/// - `next.run(req)` 직전·직후로 elapsed 측정 — TraceLayer와 분리해 lock contention 회피.
/// - path만 저장(query string drop) — PII 누수 방지.
/// - SSE는 `next.run` 반환 시점이 stream 시작 시점 — `total_ms`는 첫 chunk까지로 의미 한정 (v1).
///   `time_to_first_byte_ms` 별도 필드는 v1.x.
pub async fn record_metrics(
    State(metrics): State<Arc<GatewayMetrics>>,
    req: Request,
    next: Next,
) -> Response {
    let started = Instant::now();
    let method = req.method().as_str().to_string();
    // query string drop — `.path()`가 path-only 반환.
    let path = req.uri().path().to_string();

    let response = next.run(req).await;
    let ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;
    let status = response.status().as_u16();

    metrics.record(&method, &path, status, ms);

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_appends_latency_and_recent() {
        let m = GatewayMetrics::new();
        m.record("POST", "/v1/chat/completions", 200, 412);
        m.record("GET", "/v1/models", 200, 14);

        let recent = m.recent_requests(10);
        assert_eq!(recent.len(), 2);
        // recent[0]은 가장 최근 (역순 반환).
        assert_eq!(recent[0].path, "/v1/models");
        assert_eq!(recent[1].path, "/v1/chat/completions");
    }

    #[test]
    fn sparkline_returns_30_buckets_with_zero_for_empty() {
        let m = GatewayMetrics::new();
        let s = m.latency_sparkline();
        assert_eq!(s.len(), 30);
        assert!(s.iter().all(|v| *v == 0));
    }

    #[test]
    fn sparkline_recent_sample_ends_up_in_last_bucket() {
        let m = GatewayMetrics::new();
        m.record("GET", "/health", 200, 100);
        let s = m.latency_sparkline();
        assert_eq!(s.len(), 30);
        // 방금 기록한 sample은 마지막 bucket에.
        assert_eq!(s[29], 100);
        assert!(s[..29].iter().all(|v| *v == 0));
    }

    #[test]
    fn percentiles_compute_correctly() {
        let m = GatewayMetrics::new();
        for ms in [10u32, 20, 30, 40, 50, 60, 70, 80, 90, 100] {
            m.record("GET", "/x", 200, ms);
        }
        let p = m.percentiles();
        assert_eq!(p.count, 10);
        // 정렬 후 idx 5 = 60 (10*0.5=5), idx 9 = 100 (10*0.95=9.5→9).
        assert_eq!(p.p50_ms, 60);
        assert_eq!(p.p95_ms, 100);
    }

    #[test]
    fn recent_evicts_when_full() {
        let m = GatewayMetrics::new();
        for i in 0..(RECENT_REQUESTS_CAPACITY + 5) {
            m.record("GET", "/x", 200, i as u32);
        }
        let all = m.recent_requests(usize::MAX);
        assert_eq!(all.len(), RECENT_REQUESTS_CAPACITY);
        // 가장 오래된 것은 evict — 최신 5번째 항목이 순서대로 보임.
        // 제일 최근(역순 [0])은 last record (RECENT_REQUESTS_CAPACITY + 5 - 1)의 ms.
        assert_eq!(all[0].ms as usize, RECENT_REQUESTS_CAPACITY + 4);
    }

    #[test]
    fn record_path_storage_is_verbatim() {
        // 본 모듈은 path를 그대로 저장 — caller(미들웨어)가 query 제거 책임.
        let m = GatewayMetrics::new();
        m.record("GET", "/v1/models", 200, 5);
        let recent = m.recent_requests(1);
        assert_eq!(recent[0].path, "/v1/models");
    }
}
