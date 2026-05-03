//! macOS 전용 — sysctl + Apple Metal probe.
//!
//! 정책 (ADR-0021, Phase 1A.2.a §3):
//! - libc::sysctlbyname 직접 FFI (sysctl 0.5 crate 회피, 4 deps 절감).
//! - objc2-metal 0.3 + objc2 0.6 (구 metal-rs는 deprecated).
//! - system_profiler spawn 금지 — 250~400ms 지연.

#![cfg(target_os = "macos")]

use std::ffi::CString;

use crate::types::RuntimeInfo;

/// `hw.optional.arm64`, `sysctl.proc_translated`, `machdep.cpu.brand_string`,
/// `kern.osproductversion`, `kern.osrelease` 결과 묶음.
#[derive(Debug, Clone, Default)]
pub struct MacosArchInfo {
    pub native_arch: String,   // "arm64" / "x86_64"
    pub process_arch: String,  // 현재 프로세스 arch
    pub rosetta: bool,         // 현재 프로세스가 Rosetta-translated인가
    pub chip_brand: String,    // "Apple M2 Pro" 등
    pub macos_version: String, // "14.5"
    pub kernel: String,        // Darwin kernel
}

unsafe fn sysctl_string(name: &str) -> Option<String> {
    let c = CString::new(name).ok()?;
    let mut len: libc::size_t = 0;
    if libc::sysctlbyname(
        c.as_ptr(),
        std::ptr::null_mut(),
        &mut len,
        std::ptr::null_mut(),
        0,
    ) != 0
    {
        return None;
    }
    let mut buf = vec![0u8; len];
    if libc::sysctlbyname(
        c.as_ptr(),
        buf.as_mut_ptr() as *mut libc::c_void,
        &mut len,
        std::ptr::null_mut(),
        0,
    ) != 0
    {
        return None;
    }
    if let Some(&0) = buf.last() {
        buf.pop();
    }
    String::from_utf8(buf).ok()
}

unsafe fn sysctl_i32(name: &str) -> Option<i32> {
    let c = CString::new(name).ok()?;
    let mut v: i32 = 0;
    let mut len = std::mem::size_of::<i32>();
    if libc::sysctlbyname(
        c.as_ptr(),
        &mut v as *mut _ as *mut libc::c_void,
        &mut len,
        std::ptr::null_mut(),
        0,
    ) != 0
    {
        return None;
    }
    Some(v)
}

pub fn macos_arch_info() -> MacosArchInfo {
    let arm64 = unsafe { sysctl_i32("hw.optional.arm64") }.unwrap_or(0) == 1;
    let translated = unsafe { sysctl_i32("sysctl.proc_translated") }.unwrap_or(0) == 1;
    let chip = unsafe { sysctl_string("machdep.cpu.brand_string") }.unwrap_or_default();
    let osver = unsafe { sysctl_string("kern.osproductversion") }.unwrap_or_default();
    let kernel = unsafe { sysctl_string("kern.osrelease") }.unwrap_or_default();

    MacosArchInfo {
        native_arch: if arm64 {
            "arm64".into()
        } else {
            "x86_64".into()
        },
        process_arch: if translated {
            "x86_64".into()
        } else if arm64 {
            "arm64".into()
        } else {
            "x86_64".into()
        },
        rosetta: translated,
        chip_brand: chip,
        macos_version: osver,
        kernel,
    }
}

#[derive(Debug, Clone)]
pub struct MetalInfo {
    pub name: String,
    pub vram_bytes: u64, // recommendedMaxWorkingSetSize (UMA effective VRAM)
    pub tier: String,    // "Apple7"/"Apple8"/"Apple9"/"Mac2"
}

/// `MTLCreateSystemDefaultDevice` → name + recommendedMaxWorkingSetSize + tier.
/// 헤드리스/no-GPU 환경에선 None.
///
/// 정책 (2026-05-03 — clippy unused_unsafe fix):
/// - objc2-metal v0.2+에서 wrapper API가 safe로 expose됨. unsafe 블록 제거.
pub fn probe_metal() -> Option<MetalInfo> {
    use objc2_metal::{MTLCreateSystemDefaultDevice, MTLDevice, MTLGPUFamily};

    let device = MTLCreateSystemDefaultDevice()?;
    let name = device.name().to_string();
    let vram = device.recommendedMaxWorkingSetSize() as u64;

    let tier = [
        (MTLGPUFamily::Apple9, "Apple9"),
        (MTLGPUFamily::Apple8, "Apple8"),
        (MTLGPUFamily::Apple7, "Apple7"),
        (MTLGPUFamily::Mac2, "Mac2"),
    ]
    .iter()
    .find(|(f, _)| device.supportsFamily(*f))
    .map(|(_, s)| (*s).to_string())
    .unwrap_or_else(|| "Unknown".into());

    Some(MetalInfo {
        name,
        vram_bytes: vram,
        tier,
    })
}

pub fn probe_macos_runtime() -> RuntimeInfo {
    RuntimeInfo {
        cuda_toolkits: vec![],
        cuda_runtime: false,
        // MoltenVK가 별도 설치된 경우만 Vulkan 사용 가능. lib.rs의 ash probe가 권위 있는 결과로 덮어씀.
        vulkan: false,
        metal: probe_metal().is_some(),
        directml: false,
        d3d12: false,
        rocm: false,
        webview2: None,
        vcredist_2022: None,
        glibc: None,
        libstdcpp: None,
        vulkan_devices: None, // lib.rs probe()에서 ash 결과로 보강.
    }
}
