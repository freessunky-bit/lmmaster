//! `HardwareReport`와 그 sub-types.
//!
//! - 모든 type은 `Serialize + Deserialize` (frontend에 그대로 emit).
//! - kebab-case enum (`Status::NotInstalled` → `"not-installed"`)로 ko UI에 1:1 mapping.
//! - Recommender(Phase 2)와 installer(Phase 1A.3)이 소비하는 단일 진실원.

use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareReport {
    pub os: OsInfo,
    pub cpu: CpuInfo,
    pub mem: MemInfo,
    #[serde(default)]
    pub disks: Vec<DiskInfo>,
    #[serde(default)]
    pub gpus: Vec<GpuInfo>,
    pub runtimes: RuntimeInfo,
    pub probed_at: String, // RFC3339
    pub probe_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsInfo {
    pub family: OsFamily,
    pub version: String,
    pub arch: String, // x86_64 / aarch64
    pub kernel: String,
    /// macOS only — Rosetta-translated 여부.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rosetta: Option<bool>,
    /// Linux distro `ID` 값 (예: "ubuntu", "fedora").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distro: Option<String>,
    /// Linux distro `VERSION_ID` 값 (예: "24.04").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distro_version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OsFamily {
    Windows,
    Macos,
    Linux,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    pub brand: String,
    pub vendor_id: String,
    pub physical_cores: u32,
    pub logical_cores: u32,
    pub frequency_mhz: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub mount_point: String,
    pub kind: DiskKind,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiskKind {
    Ssd,
    Hdd,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub vendor: GpuVendor,
    pub model: String,
    /// VRAM bytes. `None`이면 측정 불가 (예: WMI 4GB clamp 회피, vendor SDK 부재).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vram_bytes: Option<u64>,
    /// PCI vendor + device ID (Apple Silicon은 `(0x106B, 0)` 등으로 정규화).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pci_id: Option<(u16, u16)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver_version: Option<String>,
    pub backend: GpuBackend,
    pub device_type: GpuDeviceType,
    /// Apple Metal capability tier ("Apple7"/"Apple8"/"Apple9"/"Mac2").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apple_family: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Apple,
    Microsoft, // WARP / Microsoft Basic Display
    Other,
}

impl GpuVendor {
    pub fn from_pci(vendor_pci: u32) -> Self {
        match vendor_pci {
            0x10DE => Self::Nvidia,
            0x1002 | 0x1022 => Self::Amd,
            0x8086 => Self::Intel,
            0x106B => Self::Apple,
            0x1414 => Self::Microsoft,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuBackend {
    Vulkan,
    Dx12,
    Metal,
    Gl,
    BrowserWebgpu,
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuDeviceType {
    DiscreteGpu,
    IntegratedGpu,
    VirtualGpu,
    Cpu,
    Other,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeInfo {
    /// CUDA toolkit (nvcc 포함). `["v12.4", "v13.0"]` 형태.
    #[serde(default)]
    pub cuda_toolkits: Vec<String>,
    /// CUDA runtime (nvcuda.dll / libcuda.so.1) 존재 여부.
    pub cuda_runtime: bool,
    /// Vulkan loader 존재 여부.
    pub vulkan: bool,
    /// macOS Metal API 사용 가능 여부.
    pub metal: bool,
    /// Windows DirectML.dll 존재 여부.
    pub directml: bool,
    /// Windows D3D12.dll 존재 여부.
    pub d3d12: bool,
    /// Linux ROCm 설치 여부 (`/opt/rocm`/sysfs 기반).
    pub rocm: bool,
    /// Windows WebView2 Evergreen runtime version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webview2: Option<String>,
    /// Windows VC++ 2022 redistributable version (예: "v14.42.34438").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcredist_2022: Option<String>,
    /// Linux glibc version (예: "2.39").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glibc: Option<String>,
    /// Linux libstdc++ runtime version (`/usr/lib/x86_64-linux-gnu/libstdc++.so.6` symlink target).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub libstdcpp: Option<String>,
    /// Vulkan probe 결과 — physical device 목록 + heap 기반 VRAM. None이면 loader 부재.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vulkan_devices: Option<crate::vulkan::VulkanProbe>,
}

/// Convenience: (probe_duration, started_at) → `(probed_at, probe_ms)` 필드.
pub(crate) fn timing(
    start: std::time::Instant,
    started_at: std::time::SystemTime,
) -> (String, u64) {
    let duration: Duration = start.elapsed();
    let dt = chrono_like_iso(started_at);
    (dt, duration.as_millis() as u64)
}

/// SystemTime → RFC3339 (chrono 의존 회피, time crate 미사용).
fn chrono_like_iso(t: std::time::SystemTime) -> String {
    let unix = t
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO);
    // Simple seconds-since-epoch ISO. Workspace의 time crate 활용도 가능하지만
    // 의존성 슬림화 위해 string 포맷.
    format!("{}.{:03}Z", unix.as_secs(), unix.subsec_millis())
}
