//! IPC commands — 프런트가 Tauri invoke로 호출한다.
//!
//! 정책: gateway HTTP API와 1:1 미러링하지 않는다.
//! GUI 전용 동작(상태 snapshot, lifecycle hint 등)만 노출.

use std::sync::{Arc, Mutex, RwLock};

use serde::Serialize;
use shared_types::ModelCategory;
use thiserror::Error;

use crate::gateway::{GatewayHandle, GatewayState};

#[tauri::command]
pub fn ping() -> &'static str {
    "pong"
}

#[tauri::command]
pub fn get_gateway_status(handle: tauri::State<'_, GatewayHandle>) -> GatewayState {
    handle.snapshot()
}

// ── Phase 1A.4.b — 환경 점검 ────────────────────────────────────────────

/// 환경 점검 IPC 에러. 현재 `probe_environment`는 graceful fail이므로 사용처는 미래 확장용.
#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum EnvApiError {
    /// 예외적인 panic 등을 감싸는 보호망.
    #[error("환경 점검 중 알 수 없는 오류가 났어요: {message}")]
    Internal { message: String },
}

/// 마법사 Step 2 / 진단 화면이 호출하는 통합 환경 점검.
/// hardware-probe + runtime-detector 결과를 한 번의 invoke로 반환한다.
#[tauri::command]
pub async fn detect_environment() -> Result<runtime_detector::EnvironmentReport, EnvApiError> {
    Ok(runtime_detector::probe_environment().await)
}

// ── Phase 1' — Self-scan ────────────────────────────────────────────

/// 마지막 자가 점검 결과 캐시 — broadcast subscriber가 갱신, get_last_scan이 읽음.
#[derive(Default)]
pub struct LastScanCache {
    inner: Mutex<Option<scanner::ScanSummary>>,
}

impl LastScanCache {
    pub fn set(&self, summary: scanner::ScanSummary) {
        let mut g = self.inner.lock().expect("LastScanCache poisoned");
        *g = Some(summary);
    }
    pub fn get(&self) -> Option<scanner::ScanSummary> {
        self.inner.lock().expect("LastScanCache poisoned").clone()
    }
}

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ScanApiError {
    #[error("이미 점검이 진행 중이에요")]
    AlreadyRunning,

    #[error("환경 점검 중 오류: {message}")]
    Internal { message: String },
}

impl From<scanner::ScannerError> for ScanApiError {
    fn from(e: scanner::ScannerError) -> Self {
        match e {
            scanner::ScannerError::AlreadyRunning => Self::AlreadyRunning,
            other => Self::Internal {
                message: other.to_string(),
            },
        }
    }
}

/// 즉시 자가 점검 실행 — 결과를 ScanSummary로 반환 + scan:summary event 자동 emit.
#[tauri::command]
pub async fn start_scan(
    scanner_state: tauri::State<'_, Arc<scanner::Scanner>>,
) -> Result<scanner::ScanSummary, ScanApiError> {
    let scanner = scanner_state.inner().clone();
    Ok(scanner.scan_now().await?)
}

/// 마지막 점검 결과 (캐시) — 프런트 첫 렌더 시 빠르게 표시. 점검이 한 번도 안 됐으면 None.
#[tauri::command]
pub fn get_last_scan(cache: tauri::State<'_, Arc<LastScanCache>>) -> Option<scanner::ScanSummary> {
    cache.get()
}

// ── Phase 2'.a — Catalog + Recommender ────────────────────────────────────

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CatalogApiError {
    #[error("카탈로그가 아직 로드되지 않았어요")]
    NotLoaded,

    #[error("호스트 점검 결과를 찾을 수 없어요")]
    HostNotProbed,

    #[error("카탈로그 조회 중 오류: {message}")]
    Internal { message: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogView {
    pub entries: Vec<model_registry::ModelEntry>,
    pub recommendation: Option<model_registry::Recommendation>,
}

/// Hot-swappable catalog — Phase 1' 자동 갱신 통합용.
///
/// 정책 (Phase 1' decision §2):
/// - bundled snapshot 1차 로드는 lib.rs setup()이 처리.
/// - 자동/수동 갱신 시 `reload_from_bundled`로 디스크 재스캔.
/// - 읽기 다중 동시 OK — RwLock으로 reader concurrency 보존.
pub struct CatalogState {
    inner: RwLock<Arc<model_registry::Catalog>>,
}

impl CatalogState {
    pub fn new(catalog: Arc<model_registry::Catalog>) -> Self {
        Self {
            inner: RwLock::new(catalog),
        }
    }

    pub fn snapshot(&self) -> Arc<model_registry::Catalog> {
        self.inner
            .read()
            .expect("CatalogState read lock poisoned")
            .clone()
    }

    /// 새 카탈로그로 교체 — atomic swap.
    pub fn swap(&self, next: Arc<model_registry::Catalog>) {
        *self
            .inner
            .write()
            .expect("CatalogState write lock poisoned") = next;
    }

    /// bundled snapshot 디렉터리에서 재로드. registry_fetcher가 매니페스트를 새로
    /// 받았을 때 호출. dev/prod 모두 같은 lookup 알고리즘 (lib.rs `load_bundled_catalog` 참조).
    pub fn reload_from_bundled(&self, app: &tauri::AppHandle) -> Result<(), anyhow::Error> {
        let catalog = crate::load_bundled_catalog(app)?;
        self.swap(Arc::new(catalog));
        Ok(())
    }

    /// 원격에서 받은 catalog bundle (`ModelManifest` JSON) body를 decode해 hot-swap.
    ///
    /// 정책 (Phase 13'.a — live model catalog refresh):
    /// - 받은 body는 단일 `ModelManifest` (모든 모델 entries 합본).
    /// - schema_version != 1이면 거부 (다음 메이저 릴리스가 schema 깨면 stale fallback 유지).
    /// - 빈 entries는 거부 (실수로 빈 bundle 푸시 방지).
    /// - 성공 시 entry 개수 반환 (UI hint chip용).
    pub fn swap_from_bundle_body(&self, body: &[u8]) -> Result<usize, anyhow::Error> {
        let manifest: model_registry::ModelManifest = serde_json::from_slice(body)
            .map_err(|e| anyhow::anyhow!("catalog bundle JSON parse 실패: {e}"))?;
        if manifest.schema_version != 1 {
            return Err(anyhow::anyhow!(
                "catalog bundle schema_version={} 지원 안 함 (앱 업데이트 필요)",
                manifest.schema_version
            ));
        }
        if manifest.entries.is_empty() {
            return Err(anyhow::anyhow!(
                "catalog bundle entries가 비었어요 — stale fallback 유지"
            ));
        }
        let count = manifest.entries.len();
        let catalog = model_registry::Catalog::from_entries(manifest.entries);
        self.swap(Arc::new(catalog));
        Ok(count)
    }
}

// ── Phase 13'.b — Diagnostics 실 데이터 IPC ──────────────────────

/// 게이트웨이 60s latency sparkline (30 bucket 평균 ms).
///
/// 정책 (Phase 13'.b):
/// - middleware가 record한 metrics를 ring buffer에서 산출. 빈 bucket은 0.
/// - Diagnostics 페이지의 LatencySparkline 차트가 5초 polling.
#[tauri::command]
pub fn get_gateway_latency_sparkline(
    metrics: tauri::State<'_, Arc<core_gateway::GatewayMetrics>>,
) -> Vec<u32> {
    metrics.latency_sparkline()
}

/// 최근 N개 게이트웨이 요청 메타. 최근 → 오래된 순서.
#[tauri::command]
pub fn get_gateway_recent_requests(
    metrics: tauri::State<'_, Arc<core_gateway::GatewayMetrics>>,
    limit: Option<u32>,
) -> Vec<core_gateway::RequestRecord> {
    metrics.recent_requests(limit.unwrap_or(5) as usize)
}

/// 메모리 percentiles snapshot (p50 / p95 / count) — Diagnostics 카드 sub-metric.
#[tauri::command]
pub fn get_gateway_percentiles(
    metrics: tauri::State<'_, Arc<core_gateway::GatewayMetrics>>,
) -> core_gateway::Percentiles {
    metrics.percentiles()
}

/// 카탈로그 — entries 필터(category Optional). 추천은 별도 호출.
///
/// Phase 13'.e.2: 각 entry에 HF metadata (`hf_meta`) 머지 — 큐레이션 시점엔 비어있는데
/// 백엔드 cron이 채운 cache 값을 응답에 포함. 큐레이터가 manifest에 직접 작성하지 않아도
/// downloads/likes/lastModified가 UI에 자동 노출됨.
#[tauri::command]
pub fn get_catalog(
    catalog: tauri::State<'_, Arc<CatalogState>>,
    hf_cache: tauri::State<'_, Arc<crate::hf_meta::HfMetaCache>>,
    category: Option<ModelCategory>,
) -> Result<CatalogView, CatalogApiError> {
    let snap = catalog.snapshot();
    let mut entries: Vec<model_registry::ModelEntry> =
        snap.filter(category).into_iter().cloned().collect();
    // hf_meta가 manifest에 명시되지 않은 entry는 cache에서 가져와 머지.
    for e in &mut entries {
        if e.hf_meta.is_none() {
            if let Some(meta) = hf_cache.get(&e.id) {
                e.hf_meta = Some(meta);
            }
        }
    }
    Ok(CatalogView {
        entries,
        recommendation: None,
    })
}

/// 카테고리 + (선택) 의도 기반 추천 — deterministic. host fingerprint 미보장 시 HostNotProbed.
///
/// Phase 11'.b (ADR-0048): `intent`가 `Some`이면 `domain_scores[intent]`가 ranking에 가중,
/// `None`이면 기존 카테고리 기반 추천 (backward compat).
#[tauri::command]
pub async fn get_recommendation(
    catalog: tauri::State<'_, Arc<CatalogState>>,
    category: ModelCategory,
    intent: Option<String>,
) -> Result<model_registry::Recommendation, CatalogApiError> {
    let report = runtime_detector::probe_environment().await;
    let host = host_fingerprint_from_report(&report).ok_or(CatalogApiError::HostNotProbed)?;
    Ok(catalog
        .snapshot()
        .recommend_with_intent(&host, category, intent.as_ref()))
}

/// runtime-detector EnvironmentReport → shared_types::HostFingerprint 변환.
fn host_fingerprint_from_report(
    report: &runtime_detector::EnvironmentReport,
) -> Option<shared_types::HostFingerprint> {
    let h = &report.hardware;
    let os_label = format!("{:?}", h.os.family).to_lowercase();
    let primary_gpu = h.gpus.first();
    Some(shared_types::HostFingerprint {
        os: os_label,
        arch: h.os.arch.clone(),
        cpu: h.cpu.brand.clone(),
        ram_mb: h.mem.total_bytes / (1024 * 1024),
        gpu_vendor: primary_gpu.map(|g| format!("{:?}", g.vendor).to_lowercase()),
        gpu_model: primary_gpu.map(|g| g.model.clone()),
        vram_mb: primary_gpu.and_then(|g| g.vram_bytes.map(|b| b / (1024 * 1024))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_api_error_serializes_with_kind_tag() {
        let e = EnvApiError::Internal {
            message: "boom".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "internal");
        assert_eq!(v["message"], "boom");
    }

    #[test]
    fn scan_api_error_already_running_serializes() {
        let e = ScanApiError::AlreadyRunning;
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "already-running");
    }

    #[test]
    fn last_scan_cache_round_trip() {
        let cache = LastScanCache::default();
        assert!(cache.get().is_none());
        let dummy = scanner::ScanSummary {
            started_at: std::time::SystemTime::UNIX_EPOCH,
            checks: vec![],
            summary_korean: "정상이에요".into(),
            summary_source: scanner::SummarySource::Deterministic,
            model_used: None,
            took_ms: 10,
        };
        cache.set(dummy.clone());
        let got = cache.get().expect("cached");
        assert_eq!(got.summary_korean, "정상이에요");
    }

    // ── Phase 13'.a — CatalogState::swap_from_bundle_body invariants ──────

    fn empty_catalog_state() -> CatalogState {
        CatalogState::new(Arc::new(model_registry::Catalog::default()))
    }

    fn make_test_bundle(entries_json: &str) -> Vec<u8> {
        format!(
            r#"{{"schema_version":1,"generated_at":"2026-04-30T00:00:00Z","entries":[{}]}}"#,
            entries_json
        )
        .into_bytes()
    }

    fn entry_json(id: &str) -> String {
        format!(
            r#"{{
                "id": "{id}",
                "display_name": "{id}",
                "category": "agent-general",
                "model_family": "test",
                "source": {{"type": "direct-url", "url": "https://x"}},
                "runner_compatibility": ["llama-cpp"],
                "quantization_options": [],
                "min_vram_mb": null,
                "rec_vram_mb": null,
                "min_ram_mb": 1024,
                "rec_ram_mb": 2048,
                "install_size_mb": 100,
                "tool_support": false,
                "vision_support": false,
                "structured_output_support": false,
                "license": "MIT",
                "maturity": "stable",
                "portable_suitability": 5,
                "on_device_suitability": 5,
                "fine_tune_suitability": 5
            }}"#
        )
    }

    #[test]
    fn swap_from_bundle_body_accepts_valid_entries() {
        let state = empty_catalog_state();
        let body = make_test_bundle(&format!("{},{}", entry_json("alpha"), entry_json("beta")));
        let count = state.swap_from_bundle_body(&body).expect("valid bundle");
        assert_eq!(count, 2);
        let snap = state.snapshot();
        assert_eq!(snap.entries().len(), 2);
        assert!(snap.entries().iter().any(|e| e.id == "alpha"));
    }

    #[test]
    fn swap_from_bundle_body_rejects_wrong_schema() {
        let state = empty_catalog_state();
        let body = format!(
            r#"{{"schema_version":2,"generated_at":"2026-04-30T00:00:00Z","entries":[{}]}}"#,
            entry_json("alpha")
        );
        let err = state
            .swap_from_bundle_body(body.as_bytes())
            .expect_err("schema_version=2 should reject");
        assert!(err.to_string().contains("schema_version=2"));
    }

    #[test]
    fn swap_from_bundle_body_rejects_empty_entries() {
        let state = empty_catalog_state();
        let body = make_test_bundle("");
        let err = state
            .swap_from_bundle_body(&body)
            .expect_err("empty entries should reject");
        assert!(err.to_string().contains("비었어요"));
    }

    #[test]
    fn swap_from_bundle_body_rejects_invalid_json() {
        let state = empty_catalog_state();
        let err = state
            .swap_from_bundle_body(b"not json")
            .expect_err("invalid JSON should reject");
        assert!(err.to_string().contains("parse"));
    }
}
