//! Linux 전용 — `/etc/os-release` + glibc + libcuda + AMD sysfs.
//!
//! 정책 (ADR-0021, Phase 1A.2.a §3):
//! - glibc_version crate는 build.rs-only — 직접 FFI 사용.
//! - os-release crate는 abandoned — inline parse.
//! - AMD sysfs `/sys/class/drm/card*/device/{vendor,device,mem_info_vram_total}`.

#![cfg(target_os = "linux")]

use crate::types::RuntimeInfo;

#[derive(Debug, Clone, Default)]
pub struct LinuxDistroInfo {
    pub id: String,
    pub version_id: String,
    pub kernel: String,
    pub glibc: Option<String>,
    pub libcuda_present: bool,
}

extern "C" {
    fn gnu_get_libc_version() -> *const libc::c_char;
}

pub fn glibc_version() -> Option<String> {
    unsafe {
        let p = gnu_get_libc_version();
        if p.is_null() {
            None
        } else {
            std::ffi::CStr::from_ptr(p).to_str().ok().map(str::to_owned)
        }
    }
}

pub fn libcuda_present() -> bool {
    // SAFETY: probe-only — symbol resolve 없음.
    unsafe { libloading::Library::new("libcuda.so.1").is_ok() }
}

/// `/usr/lib/<arch>/libstdc++.so.6` 심볼릭 링크의 target에서 버전 추출.
/// 예: `libstdc++.so.6.0.33` → `"6.0.33"`.
pub fn libstdcpp_version() -> Option<String> {
    let candidates = [
        "/usr/lib/x86_64-linux-gnu/libstdc++.so.6",
        "/usr/lib/aarch64-linux-gnu/libstdc++.so.6",
        "/usr/lib64/libstdc++.so.6",
        "/usr/lib/libstdc++.so.6",
    ];
    for path in candidates.iter() {
        if let Ok(target) = std::fs::read_link(path) {
            let s = target.to_string_lossy();
            if let Some(v) = s.strip_prefix("libstdc++.so.") {
                return Some(v.to_string());
            }
        }
    }
    None
}

pub fn rocm_present() -> bool {
    std::path::Path::new("/opt/rocm").exists()
        || std::path::Path::new("/sys/module/amdgpu").exists()
}

pub fn vulkan_present() -> bool {
    // SAFETY: probe-only.
    unsafe { libloading::Library::new("libvulkan.so.1").is_ok() }
}

pub fn linux_distro_info() -> LinuxDistroInfo {
    let raw = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let mut id = String::new();
    let mut version_id = String::new();
    for line in raw.lines() {
        if let Some(v) = line.strip_prefix("ID=") {
            id = v.trim_matches('"').to_string();
        } else if let Some(v) = line.strip_prefix("VERSION_ID=") {
            version_id = v.trim_matches('"').to_string();
        }
    }
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .unwrap_or_default()
        .trim()
        .to_string();
    LinuxDistroInfo {
        id,
        version_id,
        kernel,
        glibc: glibc_version(),
        libcuda_present: libcuda_present(),
    }
}

/// AMD GPU sysfs probe — `/sys/class/drm/card*/device/...` 패턴.
#[derive(Debug, Clone)]
pub struct AmdSysfsRow {
    pub vendor_pci: u16,
    pub device_pci: u16,
    pub vram_bytes: Option<u64>,
}

pub fn probe_amd_sysfs() -> Vec<AmdSysfsRow> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir("/sys/class/drm") {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !name.starts_with("card") || name.contains('-') {
            continue;
        }
        let device_dir = path.join("device");
        let vendor = match read_hex(&device_dir.join("vendor")) {
            Some(v) => v,
            None => continue,
        };
        if vendor != 0x1002 {
            continue; // AMD only
        }
        let device = read_hex(&device_dir.join("device")).unwrap_or(0);
        let vram = std::fs::read_to_string(device_dir.join("mem_info_vram_total"))
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok());
        out.push(AmdSysfsRow {
            vendor_pci: vendor as u16,
            device_pci: device as u16,
            vram_bytes: vram,
        });
    }
    out
}

fn read_hex(path: &std::path::Path) -> Option<u32> {
    let s = std::fs::read_to_string(path).ok()?;
    let s = s.trim().trim_start_matches("0x");
    u32::from_str_radix(s, 16).ok()
}

pub fn probe_linux_runtime() -> RuntimeInfo {
    let distro = linux_distro_info();
    RuntimeInfo {
        cuda_toolkits: vec![],
        cuda_runtime: distro.libcuda_present,
        vulkan: vulkan_present(),
        metal: false,
        directml: false,
        d3d12: false,
        rocm: rocm_present(),
        webview2: None,
        vcredist_2022: None,
        glibc: distro.glibc,
        libstdcpp: libstdcpp_version(),
        vulkan_devices: None, // lib.rs probe()에서 ash 결과로 보강.
    }
}
