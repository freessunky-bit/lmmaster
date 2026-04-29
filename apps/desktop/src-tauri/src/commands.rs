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
}

/// 카탈로그 — entries 필터(category Optional). 추천은 별도 호출.
#[tauri::command]
pub fn get_catalog(
    catalog: tauri::State<'_, Arc<CatalogState>>,
    category: Option<ModelCategory>,
) -> Result<CatalogView, CatalogApiError> {
    let snap = catalog.snapshot();
    let entries: Vec<model_registry::ModelEntry> =
        snap.filter(category).into_iter().cloned().collect();
    Ok(CatalogView {
        entries,
        recommendation: None,
    })
}

/// 카테고리 별 추천 — deterministic. host fingerprint 미보장 시 HostNotProbed.
#[tauri::command]
pub async fn get_recommendation(
    catalog: tauri::State<'_, Arc<CatalogState>>,
    category: ModelCategory,
) -> Result<model_registry::Recommendation, CatalogApiError> {
    let report = runtime_detector::probe_environment().await;
    let host = host_fingerprint_from_report(&report).ok_or(CatalogApiError::HostNotProbed)?;
    Ok(catalog.snapshot().recommend(&host, category))
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
}
