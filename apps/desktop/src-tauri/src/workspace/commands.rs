//! Workspace fingerprint Tauri commands — Phase 3'.c.
//!
//! 정책 (ADR-0022 §8):
//! - get_workspace_fingerprint: 현재 host fingerprint 산출 + 분류 (저장된 fingerprint와 비교).
//! - check_workspace_repair: 저장 + tier별 액션 적용 (cache invalidate / runtimes invalidate).
//! - 첫 실행은 silent green.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use portable_workspace::{
    classify, evaluate_and_repair, load_fingerprint, save_fingerprint, ModelRecord, PortMap,
    RepairReport, RepairTier, RuntimeRecord, WorkspaceFingerprint, WorkspaceManifest,
};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use thiserror::Error;
use time::OffsetDateTime;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WorkspaceApiError {
    #[error("호스트 정보를 읽지 못했어요")]
    HostNotProbed,
    #[error("workspace 디스크 오류: {message}")]
    Disk { message: String },
    #[error("내부 오류: {message}")]
    Internal { message: String },
}

impl From<portable_workspace::RepairError> for WorkspaceApiError {
    fn from(e: portable_workspace::RepairError) -> Self {
        Self::Disk {
            message: e.to_string(),
        }
    }
}

/// Workspace 루트 — app_data_dir/workspace. AppHandle State로 한 번 계산하고 process 동안 재사용.
#[derive(Default)]
pub struct WorkspaceRoot {
    inner: Mutex<Option<PathBuf>>,
}

impl WorkspaceRoot {
    pub fn get_or_init(&self, app: &AppHandle) -> Result<PathBuf, WorkspaceApiError> {
        let mut g = self.inner.lock().expect("WorkspaceRoot poisoned");
        if let Some(p) = g.as_ref() {
            return Ok(p.clone());
        }
        let base = app
            .path()
            .app_data_dir()
            .map_err(|e| WorkspaceApiError::Internal {
                message: format!("app_data_dir: {e}"),
            })?;
        let root = base.join("workspace");
        std::fs::create_dir_all(&root).map_err(|e| WorkspaceApiError::Disk {
            message: e.to_string(),
        })?;
        *g = Some(root.clone());
        Ok(root)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceStatus {
    pub fingerprint: WorkspaceFingerprint,
    pub previous: Option<WorkspaceFingerprint>,
    pub tier: RepairTier,
    pub workspace_root: String,
}

/// 현재 host fingerprint + 저장된 것과 비교한 tier (액션은 안 적용 — 사용자 확인용).
#[tauri::command]
pub async fn get_workspace_fingerprint(
    app: AppHandle,
    root: tauri::State<'_, Arc<WorkspaceRoot>>,
) -> Result<WorkspaceStatus, WorkspaceApiError> {
    let workspace_root = root.get_or_init(&app)?;
    let env = runtime_detector::probe_environment().await;
    let host = host_fingerprint_from_report(&env).ok_or(WorkspaceApiError::HostNotProbed)?;
    let current = WorkspaceFingerprint::from_host(&host);
    let prev = load_fingerprint(&workspace_root)?;
    let tier = match prev.as_ref() {
        Some(p) => classify(p, &current),
        None => RepairTier::Green,
    };
    Ok(WorkspaceStatus {
        fingerprint: current,
        previous: prev,
        tier,
        workspace_root: workspace_root.display().to_string(),
    })
}

/// 실제 repair 적용 — cache invalidate + manifest 갱신 + fingerprint 저장.
/// manifest는 disk에 없으면 빈 manifest 생성 후 적용.
#[tauri::command]
pub async fn check_workspace_repair(
    app: AppHandle,
    root: tauri::State<'_, Arc<WorkspaceRoot>>,
) -> Result<RepairReport, WorkspaceApiError> {
    let workspace_root = root.get_or_init(&app)?;
    let env = runtime_detector::probe_environment().await;
    let host = host_fingerprint_from_report(&env).ok_or(WorkspaceApiError::HostNotProbed)?;
    let current = WorkspaceFingerprint::from_host(&host);

    let manifest_path = workspace_root.join("manifest.json");
    let mut manifest: WorkspaceManifest = if manifest_path.exists() {
        let body =
            std::fs::read_to_string(&manifest_path).map_err(|e| WorkspaceApiError::Disk {
                message: e.to_string(),
            })?;
        serde_json::from_str(&body).map_err(|e| WorkspaceApiError::Internal {
            message: format!("manifest parse: {e}"),
        })?
    } else {
        empty_manifest(&host)
    };

    // 첫 실행은 evaluate_and_repair 안에서 save만.
    let report = evaluate_and_repair(&workspace_root, &current, &mut manifest)?;

    // manifest 갱신 저장.
    let body =
        serde_json::to_string_pretty(&manifest).map_err(|e| WorkspaceApiError::Internal {
            message: e.to_string(),
        })?;
    std::fs::write(&manifest_path, body).map_err(|e| WorkspaceApiError::Disk {
        message: e.to_string(),
    })?;

    // fingerprint 자체는 evaluate_and_repair가 이미 저장 (첫 실행) 또는 apply_repair 끝.
    // green tier (이전 fingerprint와 동일)면 저장 변경 없으니 명시 save.
    save_fingerprint(&workspace_root, &current)?;

    // Phase 13'.b — green이 아닐 때 (실제 repair 발생) JSONL에 기록.
    // Diagnostics 페이지의 "복구 이력" 카드가 read.
    if !matches!(report.tier, RepairTier::Green) {
        let _ = append_repair_log_entry(&app, &report);
    }

    Ok(report)
}

/// Phase 13'.b — repair history 한 entry. JSONL append 형식.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct RepairHistoryEntry {
    /// RFC3339.
    pub at: String,
    /// "yellow" | "red".
    pub tier: String,
    /// 무효화된 캐시 종류 개수.
    pub invalidated_caches: u32,
    /// 사용자 향 한 문장 메모.
    pub note: String,
}

fn append_repair_log_entry(app: &AppHandle, report: &RepairReport) -> Result<(), std::io::Error> {
    let path = repair_log_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let entry = RepairHistoryEntry {
        at: OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
        tier: format!("{:?}", report.tier).to_lowercase(),
        invalidated_caches: report.invalidated_caches.len() as u32,
        note: format!("{} caches invalidated", report.invalidated_caches.len()),
    };
    let line = serde_json::to_string(&entry)
        .map_err(|e| std::io::Error::other(format!("repair history serialize: {e}")))?;
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    writeln!(file, "{line}")?;
    Ok(())
}

fn repair_log_path(app: &AppHandle) -> Result<std::path::PathBuf, std::io::Error> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| std::io::Error::other(format!("app_data_dir: {e}")))?;
    Ok(base.join("workspace").join("repair-log.jsonl"))
}

/// Phase 13'.b — Diagnostics가 표시할 repair 이력 (가장 최근 N개, 최신 → 오래된).
#[tauri::command]
pub async fn get_repair_history(
    app: AppHandle,
    limit: Option<u32>,
) -> Result<Vec<RepairHistoryEntry>, WorkspaceApiError> {
    let n = limit.unwrap_or(10).min(100) as usize;
    let path = repair_log_path(&app).map_err(|e| WorkspaceApiError::Disk {
        message: e.to_string(),
    })?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let body = std::fs::read_to_string(&path).map_err(|e| WorkspaceApiError::Disk {
        message: e.to_string(),
    })?;
    let mut entries: Vec<RepairHistoryEntry> = body
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    // 마지막 entry가 가장 최신 (append-only) — 역순.
    entries.reverse();
    entries.truncate(n);
    Ok(entries)
}

fn empty_manifest(host: &shared_types::HostFingerprint) -> WorkspaceManifest {
    WorkspaceManifest {
        schema_version: 1,
        workspace_id: uuid::Uuid::new_v4().to_string(),
        host_fingerprint: host.clone(),
        runtimes_installed: Vec::<RuntimeRecord>::new(),
        models_installed: Vec::<ModelRecord>::new(),
        ports: PortMap::default(),
        created_at: OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
        last_repaired_at: None,
    }
}

fn host_fingerprint_from_report(
    report: &runtime_detector::EnvironmentReport,
) -> Option<shared_types::HostFingerprint> {
    let h = &report.hardware;
    let primary_gpu = h.gpus.first();
    Some(shared_types::HostFingerprint {
        os: format!("{:?}", h.os.family).to_lowercase(),
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
    fn host_not_probed_serializes_kebab() {
        let v = serde_json::to_value(WorkspaceApiError::HostNotProbed).unwrap();
        assert_eq!(v["kind"], "host-not-probed");
    }

    #[test]
    fn disk_error_serializes_kebab() {
        let v = serde_json::to_value(WorkspaceApiError::Disk {
            message: "io".into(),
        })
        .unwrap();
        assert_eq!(v["kind"], "disk");
    }
}
