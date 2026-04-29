//! Fingerprint mismatch 시 repair flow (ADR-0022 §8).
//!
//! 정책:
//! - **green**: silent — 아무 동작 없음.
//! - **yellow**: bench/scan 캐시 invalidate. 모델 항상 보존. 사용자에게 toast 안내.
//! - **red**: 위 + manifest.runtimes_installed[] invalidate (런타임 바이너리는 OS-bound).
//!   사용자에게 modal 안내 — "다른 OS에서 가져온 워크스페이스".

use std::path::{Path, PathBuf};

use thiserror::Error;
use time::OffsetDateTime;

use crate::fingerprint::{classify, RepairTier, WorkspaceFingerprint};
use crate::manifest::WorkspaceManifest;

#[derive(Debug, Error)]
pub enum RepairError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("workspace fingerprint 파일이 손상됐어요: {0}")]
    Corrupted(String),
}

/// 디스크 경로 helper — `workspace/fingerprint.json`.
pub fn fingerprint_path(workspace_root: &Path) -> PathBuf {
    workspace_root.join("fingerprint.json")
}

/// 저장된 fingerprint 읽기. 없으면 None (첫 실행).
pub fn load_fingerprint(
    workspace_root: &Path,
) -> Result<Option<WorkspaceFingerprint>, RepairError> {
    let p = fingerprint_path(workspace_root);
    if !p.exists() {
        return Ok(None);
    }
    let body = std::fs::read_to_string(&p)?;
    let fp: WorkspaceFingerprint =
        serde_json::from_str(&body).map_err(|e| RepairError::Corrupted(e.to_string()))?;
    Ok(Some(fp))
}

/// 현재 fingerprint 저장 (atomic — temp + rename).
pub fn save_fingerprint(
    workspace_root: &Path,
    fp: &WorkspaceFingerprint,
) -> Result<(), RepairError> {
    std::fs::create_dir_all(workspace_root)?;
    let target = fingerprint_path(workspace_root);
    let tmp = target.with_extension("json.tmp");
    let body = serde_json::to_string_pretty(fp)?;
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, &target)?;
    Ok(())
}

/// Repair 적용 — invalidate 액션 + manifest 업데이트.
///
/// `cache/{bench, scan}/`은 yellow+red 모두 invalidate.
/// `runtimes/`는 red에서 manifest 측만 invalidate (실제 바이너리는 보존 — cross-compile 가능성).
/// `models/`는 모든 tier에서 보존 (GGUF는 OS-agnostic).
pub fn apply_repair(
    workspace_root: &Path,
    tier: RepairTier,
    manifest: &mut WorkspaceManifest,
    new_fp: &WorkspaceFingerprint,
) -> Result<RepairReport, RepairError> {
    let mut report = RepairReport {
        tier,
        invalidated_caches: Vec::new(),
        invalidated_runtimes: 0,
        models_preserved: manifest.models_installed.len(),
    };

    match tier {
        RepairTier::Green => {
            // 아무 동작 안 함.
        }
        RepairTier::Yellow | RepairTier::Red => {
            for sub in ["bench", "scan"] {
                let dir = workspace_root.join("cache").join(sub);
                if dir.exists() {
                    invalidate_dir_contents(&dir)?;
                    report.invalidated_caches.push(sub.to_string());
                }
            }
        }
    }

    if tier == RepairTier::Red {
        // manifest 측 runtimes_installed[] 비움 — 실제 디렉터리는 보존 (사용자 결정).
        report.invalidated_runtimes = manifest.runtimes_installed.len();
        manifest.runtimes_installed.clear();
    }

    // fingerprint + last_repaired_at 갱신.
    manifest.host_fingerprint = shared_types::HostFingerprint {
        os: new_fp.os.clone(),
        arch: new_fp.arch.clone(),
        cpu: manifest.host_fingerprint.cpu.clone(),
        ram_mb: manifest.host_fingerprint.ram_mb,
        gpu_vendor: manifest.host_fingerprint.gpu_vendor.clone(),
        gpu_model: manifest.host_fingerprint.gpu_model.clone(),
        vram_mb: manifest.host_fingerprint.vram_mb,
    };
    manifest.last_repaired_at = OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .ok();

    Ok(report)
}

/// 디렉터리 *내부* entry만 삭제 (디렉터리 자체 보존).
fn invalidate_dir_contents(dir: &Path) -> Result<(), RepairError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            std::fs::remove_dir_all(&path)?;
        } else {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RepairReport {
    pub tier: RepairTier,
    pub invalidated_caches: Vec<String>,
    pub invalidated_runtimes: usize,
    pub models_preserved: usize,
}

/// 메인 entry — 현재 host fingerprint를 받아서 tier 계산 + repair 적용 + 저장.
pub fn evaluate_and_repair(
    workspace_root: &Path,
    current: &WorkspaceFingerprint,
    manifest: &mut WorkspaceManifest,
) -> Result<RepairReport, RepairError> {
    let prev = load_fingerprint(workspace_root)?;
    let tier = match prev {
        Some(p) => classify(&p, current),
        None => {
            // 첫 실행 — green으로 간주, 단순 저장.
            save_fingerprint(workspace_root, current)?;
            return Ok(RepairReport {
                tier: RepairTier::Green,
                invalidated_caches: Vec::new(),
                invalidated_runtimes: 0,
                models_preserved: manifest.models_installed.len(),
            });
        }
    };
    let report = apply_repair(workspace_root, tier, manifest, current)?;
    save_fingerprint(workspace_root, current)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fingerprint::GpuClass;
    use crate::manifest::{ModelRecord, PortMap, RuntimeRecord};
    use shared_types::{HostFingerprint, RuntimeKind};
    use tempfile::tempdir;

    fn make_fp(os: &str, arch: &str, gpu: GpuClass, vram: u64, ram: u64) -> WorkspaceFingerprint {
        WorkspaceFingerprint::from_host(&HostFingerprint {
            os: os.into(),
            arch: arch.into(),
            cpu: "test".into(),
            ram_mb: ram,
            gpu_vendor: match gpu {
                GpuClass::Nvidia => Some("nvidia".into()),
                GpuClass::Amd => Some("amd".into()),
                GpuClass::Intel => Some("intel".into()),
                GpuClass::Apple => Some("apple".into()),
                GpuClass::Other => Some("matrox".into()),
                GpuClass::None => None,
            },
            gpu_model: None,
            vram_mb: if vram > 0 { Some(vram) } else { None },
        })
    }

    fn make_manifest() -> WorkspaceManifest {
        WorkspaceManifest {
            schema_version: 1,
            workspace_id: "ws-1".into(),
            host_fingerprint: HostFingerprint {
                os: "windows".into(),
                arch: "x86_64".into(),
                cpu: "test".into(),
                ram_mb: 65536,
                gpu_vendor: Some("nvidia".into()),
                gpu_model: Some("RTX 4090".into()),
                vram_mb: Some(24576),
            },
            runtimes_installed: vec![RuntimeRecord {
                id: "ollama".into(),
                kind: RuntimeKind::Ollama,
                version: "0.4.0".into(),
                build_target: "win-x64".into(),
                install_dir_rel: "runtimes/ollama".into(),
            }],
            models_installed: vec![ModelRecord {
                id: "exaone".into(),
                runtime_id: "ollama".into(),
                quantization: "Q4_K_M".into(),
                file_rel_path: "models/exaone.gguf".into(),
                sha256: "0".repeat(64),
                size_bytes: 800_000_000,
            }],
            ports: PortMap::default(),
            created_at: "2026-04-27T00:00:00Z".into(),
            last_repaired_at: None,
        }
    }

    #[test]
    fn first_run_saves_and_returns_green() {
        let tmp = tempdir().unwrap();
        let fp = make_fp("windows", "x86_64", GpuClass::Nvidia, 24576, 65536);
        let mut manifest = make_manifest();
        let report = evaluate_and_repair(tmp.path(), &fp, &mut manifest).unwrap();
        assert_eq!(report.tier, RepairTier::Green);
        assert_eq!(report.invalidated_caches.len(), 0);
        assert!(fingerprint_path(tmp.path()).exists());
    }

    #[test]
    fn green_on_same_host_reload() {
        let tmp = tempdir().unwrap();
        let fp = make_fp("windows", "x86_64", GpuClass::Nvidia, 24576, 65536);
        let mut manifest = make_manifest();
        evaluate_and_repair(tmp.path(), &fp, &mut manifest).unwrap();

        // 두 번째 실행 — 같은 host.
        let report = evaluate_and_repair(tmp.path(), &fp, &mut manifest).unwrap();
        assert_eq!(report.tier, RepairTier::Green);
        assert_eq!(report.invalidated_caches.len(), 0);
        assert_eq!(manifest.runtimes_installed.len(), 1);
    }

    #[test]
    fn yellow_on_gpu_class_change_invalidates_caches_preserves_models_and_runtimes() {
        let tmp = tempdir().unwrap();
        let initial = make_fp("windows", "x86_64", GpuClass::Nvidia, 24576, 65536);
        let mut manifest = make_manifest();
        evaluate_and_repair(tmp.path(), &initial, &mut manifest).unwrap();

        // bench/scan cache 디렉터리 + 파일 생성 (invalidate 검증용).
        for sub in ["bench", "scan"] {
            let dir = tmp.path().join("cache").join(sub);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("dummy.json"), b"{}").unwrap();
        }

        // GPU 교체.
        let after = make_fp("windows", "x86_64", GpuClass::Amd, 24576, 65536);
        let report = evaluate_and_repair(tmp.path(), &after, &mut manifest).unwrap();
        assert_eq!(report.tier, RepairTier::Yellow);
        assert!(report.invalidated_caches.contains(&"bench".to_string()));
        assert!(report.invalidated_caches.contains(&"scan".to_string()));
        // 모델 + 런타임은 보존.
        assert_eq!(manifest.runtimes_installed.len(), 1);
        assert_eq!(manifest.models_installed.len(), 1);
        // cache 디렉터리는 비어있어야.
        assert!(tmp.path().join("cache/bench").exists());
        assert!(std::fs::read_dir(tmp.path().join("cache/bench"))
            .unwrap()
            .next()
            .is_none());
    }

    #[test]
    fn red_on_os_change_invalidates_runtimes_preserves_models() {
        let tmp = tempdir().unwrap();
        let win = make_fp("windows", "x86_64", GpuClass::Nvidia, 24576, 65536);
        let mut manifest = make_manifest();
        evaluate_and_repair(tmp.path(), &win, &mut manifest).unwrap();

        // OS 교체 (USB로 이동한 시나리오).
        let mac = make_fp("macos", "x86_64", GpuClass::Apple, 0, 65536);
        let report = evaluate_and_repair(tmp.path(), &mac, &mut manifest).unwrap();
        assert_eq!(report.tier, RepairTier::Red);
        assert_eq!(report.invalidated_runtimes, 1);
        // manifest 측 runtimes는 비워짐.
        assert!(manifest.runtimes_installed.is_empty());
        // 모델은 보존.
        assert_eq!(manifest.models_installed.len(), 1);
    }

    #[test]
    fn red_on_arch_change() {
        let tmp = tempdir().unwrap();
        let x86 = make_fp("macos", "x86_64", GpuClass::Apple, 0, 16384);
        let mut manifest = make_manifest();
        evaluate_and_repair(tmp.path(), &x86, &mut manifest).unwrap();
        let arm = make_fp("macos", "aarch64", GpuClass::Apple, 0, 16384);
        let report = evaluate_and_repair(tmp.path(), &arm, &mut manifest).unwrap();
        assert_eq!(report.tier, RepairTier::Red);
    }

    #[test]
    fn corrupted_fingerprint_returns_corrupted_error() {
        let tmp = tempdir().unwrap();
        let fp_path = fingerprint_path(tmp.path());
        std::fs::write(&fp_path, "{not json").unwrap();
        let r = load_fingerprint(tmp.path());
        assert!(matches!(r, Err(RepairError::Corrupted(_))));
    }

    #[test]
    fn save_then_load_round_trip() {
        let tmp = tempdir().unwrap();
        let fp = make_fp("windows", "x86_64", GpuClass::Nvidia, 24576, 65536);
        save_fingerprint(tmp.path(), &fp).unwrap();
        let loaded = load_fingerprint(tmp.path()).unwrap().unwrap();
        assert_eq!(fp, loaded);
    }

    #[test]
    fn last_repaired_at_set_on_yellow() {
        let tmp = tempdir().unwrap();
        let initial = make_fp("windows", "x86_64", GpuClass::Nvidia, 24576, 65536);
        let mut manifest = make_manifest();
        evaluate_and_repair(tmp.path(), &initial, &mut manifest).unwrap();
        assert!(manifest.last_repaired_at.is_none());

        let after = make_fp("windows", "x86_64", GpuClass::Amd, 24576, 65536);
        evaluate_and_repair(tmp.path(), &after, &mut manifest).unwrap();
        assert!(manifest.last_repaired_at.is_some());
    }
}
