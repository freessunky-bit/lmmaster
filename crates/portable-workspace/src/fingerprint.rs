//! Workspace fingerprint — host 식별자 + 3-tier repair 분류 (ADR-0022 §8).
//!
//! 정책:
//! - fingerprint = sha256(os + arch + gpu_class + vram_bucket + ram_bucket).
//! - GPU class: vendor 단위 (NVIDIA / AMD / Intel / Apple / None) — 정확한 모델 차이는 무시.
//! - RAM/VRAM bucket: 16GB 단위 round-down — 미세 변화는 무시 (예: OS 업데이트 후 가용 RAM 변동).
//! - tier 분류:
//!   - **green**: 모든 필드 일치 → silent.
//!   - **yellow**: os+arch 일치 / gpu_class 또는 bucket 변동 → bench/scan 캐시 invalidate, 모델 보존.
//!   - **red**: os 또는 arch 불일치 → 런타임 manifest invalidate (cross-OS GGUF는 보존, 런타임 바이너리는 OS-bound).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shared_types::HostFingerprint;

const RAM_BUCKET_MB: u64 = 16 * 1024; // 16 GB.
const VRAM_BUCKET_MB: u64 = 8 * 1024; // 8 GB.

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceFingerprint {
    pub os: String,
    pub arch: String,
    /// GPU vendor class — 모델별 차이는 무시.
    pub gpu_class: GpuClass,
    pub vram_bucket_mb: u64,
    pub ram_bucket_mb: u64,
    /// sha256(canonical fields) — 16자 truncate.
    pub fingerprint_hash: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GpuClass {
    Nvidia,
    Amd,
    Intel,
    Apple,
    None,
    Other,
}

impl GpuClass {
    pub fn from_vendor(v: Option<&str>) -> Self {
        match v.map(|s| s.to_ascii_lowercase()).as_deref() {
            Some("nvidia") => Self::Nvidia,
            Some("amd") => Self::Amd,
            Some("intel") => Self::Intel,
            Some("apple") => Self::Apple,
            None => Self::None,
            Some("microsoft") | Some("other") => Self::Other,
            Some(_) => Self::Other,
        }
    }
}

impl WorkspaceFingerprint {
    pub fn from_host(host: &HostFingerprint) -> Self {
        let gpu_class = GpuClass::from_vendor(host.gpu_vendor.as_deref());
        let vram_bucket_mb = host
            .vram_mb
            .map(|v| (v / VRAM_BUCKET_MB) * VRAM_BUCKET_MB)
            .unwrap_or(0);
        let ram_bucket_mb = (host.ram_mb / RAM_BUCKET_MB) * RAM_BUCKET_MB;
        let mut fp = Self {
            os: host.os.clone(),
            arch: host.arch.clone(),
            gpu_class,
            vram_bucket_mb,
            ram_bucket_mb,
            fingerprint_hash: String::new(),
        };
        fp.fingerprint_hash = fp.compute_hash();
        fp
    }

    fn compute_hash(&self) -> String {
        let canonical = format!(
            "{}|{}|{:?}|{}|{}",
            self.os, self.arch, self.gpu_class, self.vram_bucket_mb, self.ram_bucket_mb
        );
        let digest = Sha256::digest(canonical.as_bytes());
        hex::encode(&digest[..8]) // 16-hex 자.
    }
}

/// Repair tier — 분류 정책 (ADR-0022 §8).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RepairTier {
    /// 모든 필드 일치 — silent.
    Green,
    /// os+arch 일치 / gpu_class 또는 bucket 변동 — bench/scan 캐시 invalidate.
    Yellow,
    /// os 또는 arch 불일치 — 런타임 manifest invalidate (모델 파일 자체는 보존).
    Red,
}

/// 두 fingerprint를 비교해 tier 산출.
pub fn classify(prev: &WorkspaceFingerprint, current: &WorkspaceFingerprint) -> RepairTier {
    if prev.os != current.os || prev.arch != current.arch {
        return RepairTier::Red;
    }
    if prev.fingerprint_hash == current.fingerprint_hash {
        return RepairTier::Green;
    }
    // os+arch 일치 + 해시는 다름 → gpu_class / bucket 변동.
    RepairTier::Yellow
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(
        os: &str,
        arch: &str,
        gpu: Option<&str>,
        vram: Option<u64>,
        ram: u64,
    ) -> HostFingerprint {
        HostFingerprint {
            os: os.into(),
            arch: arch.into(),
            cpu: "test".into(),
            ram_mb: ram,
            gpu_vendor: gpu.map(|s| s.to_string()),
            gpu_model: gpu.map(|s| format!("{s} GPU")),
            vram_mb: vram,
        }
    }

    #[test]
    fn fingerprint_hash_is_16_hex() {
        let fp = WorkspaceFingerprint::from_host(&host(
            "windows",
            "x86_64",
            Some("nvidia"),
            Some(24576),
            65536,
        ));
        assert_eq!(fp.fingerprint_hash.len(), 16);
        assert!(fp.fingerprint_hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn same_host_same_hash() {
        let h = host("windows", "x86_64", Some("nvidia"), Some(24576), 65536);
        let a = WorkspaceFingerprint::from_host(&h);
        let b = WorkspaceFingerprint::from_host(&h);
        assert_eq!(a, b);
    }

    #[test]
    fn ram_within_bucket_collapses() {
        // 32 GB와 33 GB는 같은 16GB bucket(32 GB).
        let a = WorkspaceFingerprint::from_host(&host(
            "windows",
            "x86_64",
            Some("nvidia"),
            Some(24576),
            32 * 1024,
        ));
        let b = WorkspaceFingerprint::from_host(&host(
            "windows",
            "x86_64",
            Some("nvidia"),
            Some(24576),
            33 * 1024,
        ));
        assert_eq!(a, b);
    }

    #[test]
    fn vram_above_bucket_changes_hash() {
        // 24 GB bucket (16 GB rounddown to 16) vs 8 GB bucket (8 GB).
        let a = WorkspaceFingerprint::from_host(&host(
            "windows",
            "x86_64",
            Some("nvidia"),
            Some(24576),
            65536,
        ));
        let b = WorkspaceFingerprint::from_host(&host(
            "windows",
            "x86_64",
            Some("nvidia"),
            Some(8192),
            65536,
        ));
        assert_ne!(a, b);
    }

    #[test]
    fn classify_green_when_identical() {
        let h = host("windows", "x86_64", Some("nvidia"), Some(24576), 65536);
        let a = WorkspaceFingerprint::from_host(&h);
        let b = WorkspaceFingerprint::from_host(&h);
        assert_eq!(classify(&a, &b), RepairTier::Green);
    }

    #[test]
    fn classify_yellow_on_gpu_class_change() {
        let nvidia = WorkspaceFingerprint::from_host(&host(
            "windows",
            "x86_64",
            Some("nvidia"),
            Some(24576),
            65536,
        ));
        let amd = WorkspaceFingerprint::from_host(&host(
            "windows",
            "x86_64",
            Some("amd"),
            Some(24576),
            65536,
        ));
        assert_eq!(classify(&nvidia, &amd), RepairTier::Yellow);
    }

    #[test]
    fn classify_yellow_on_vram_bucket_change() {
        let big = WorkspaceFingerprint::from_host(&host(
            "windows",
            "x86_64",
            Some("nvidia"),
            Some(24576),
            65536,
        ));
        let small = WorkspaceFingerprint::from_host(&host(
            "windows",
            "x86_64",
            Some("nvidia"),
            Some(8192),
            65536,
        ));
        assert_eq!(classify(&big, &small), RepairTier::Yellow);
    }

    #[test]
    fn classify_red_on_os_change() {
        let win = WorkspaceFingerprint::from_host(&host(
            "windows",
            "x86_64",
            Some("nvidia"),
            Some(24576),
            65536,
        ));
        let mac = WorkspaceFingerprint::from_host(&host(
            "macos",
            "x86_64",
            Some("nvidia"),
            Some(24576),
            65536,
        ));
        assert_eq!(classify(&win, &mac), RepairTier::Red);
    }

    #[test]
    fn classify_red_on_arch_change() {
        let x86 = WorkspaceFingerprint::from_host(&host(
            "macos",
            "x86_64",
            Some("apple"),
            Some(24576),
            65536,
        ));
        let arm = WorkspaceFingerprint::from_host(&host(
            "macos",
            "aarch64",
            Some("apple"),
            Some(24576),
            65536,
        ));
        assert_eq!(classify(&x86, &arm), RepairTier::Red);
    }

    #[test]
    fn gpu_class_none_when_no_gpu() {
        let cpu_only = WorkspaceFingerprint::from_host(&host("linux", "x86_64", None, None, 16384));
        assert_eq!(cpu_only.gpu_class, GpuClass::None);
    }

    #[test]
    fn gpu_class_other_for_unknown_vendor() {
        let weird = WorkspaceFingerprint::from_host(&host(
            "linux",
            "x86_64",
            Some("matrox"),
            Some(512),
            16384,
        ));
        assert_eq!(weird.gpu_class, GpuClass::Other);
    }
}
