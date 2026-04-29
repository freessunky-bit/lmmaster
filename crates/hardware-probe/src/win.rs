//! Windows 전용 — 레지스트리 + DLL probe.
//!
//! 정책 (ADR-0021, Phase 1A.2.a §3):
//! - winreg 0.55 + `KEY_READ | KEY_WOW64_64KEY` 강제 (32-bit redirection 회피).
//! - 모든 read는 `Option`/`Vec` 반환 — `io::Error` bubble 금지.
//! - DLL은 `libloading` 으로 즉시 drop (probe-only).

#![cfg(windows)]

use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY};
use winreg::RegKey;

use crate::types::RuntimeInfo;

const WV2_GUID: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
const NVIDIA_DISPLAY_CLASS: &str =
    r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";
const VCREDIST_2022_X64: &str = r"SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64";
const CUDA_TOOLKIT_ROOT: &str = r"SOFTWARE\NVIDIA Corporation\GPU Computing Toolkit\CUDA";

fn read_string(root: &RegKey, path: &str, value: &str) -> Option<String> {
    root.open_subkey_with_flags(path, KEY_READ | KEY_WOW64_64KEY)
        .and_then(|k| k.get_value::<String, _>(value))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s != "0.0.0.0")
}

/// WebView2 Evergreen runtime 버전. Win 11 통상 22H2 이후 자동 설치됨.
pub fn webview2_version() -> Option<String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    read_string(
        &hklm,
        &format!(
            r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{}",
            WV2_GUID
        ),
        "pv",
    )
    .or_else(|| {
        read_string(
            &hklm,
            &format!(r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{}", WV2_GUID),
            "pv",
        )
    })
    .or_else(|| {
        read_string(
            &hkcu,
            &format!(r"Software\Microsoft\EdgeUpdate\Clients\{}", WV2_GUID),
            "pv",
        )
    })
}

/// VC++ 2022 x64 redistributable 버전 (예: "v14.42.34438").
pub fn vcredist_2022_version() -> Option<String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let k = hklm
        .open_subkey_with_flags(VCREDIST_2022_X64, KEY_READ | KEY_WOW64_64KEY)
        .ok()?;
    let installed: u32 = k.get_value("Installed").ok()?;
    if installed != 1 {
        return None;
    }
    k.get_value::<String, _>("Version")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// NVIDIA driver의 user-visible 버전 (예: "551.86").
/// Class GUID 하위 `0000..0007` 키를 iterate하며 `MatchingDeviceId`가 PCI\VEN_10DE인 항목의
/// `DriverVersion` 을 user-visible 형태로 변환한다.
pub fn nvidia_driver_version() -> Option<String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let class = hklm
        .open_subkey_with_flags(NVIDIA_DISPLAY_CLASS, KEY_READ | KEY_WOW64_64KEY)
        .ok()?;
    for sub in class.enum_keys().flatten() {
        let Ok(node) = class.open_subkey(&sub) else {
            continue;
        };
        let dev_id: String = node.get_value("MatchingDeviceId").unwrap_or_default();
        if !dev_id.to_ascii_uppercase().starts_with("PCI\\VEN_10DE") {
            continue;
        }
        if let Ok(raw) = node.get_value::<String, _>("DriverVersion") {
            return Some(format_nvidia_driver(&raw).unwrap_or(raw));
        }
    }
    None
}

/// "32.0.15.5186" → "551.86". raw가 잘못된 형태면 None.
fn format_nvidia_driver(raw: &str) -> Option<String> {
    // 마지막 5자리(점 제외)를 추출.
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 5 {
        return None;
    }
    let last5 = &digits[digits.len() - 5..];
    Some(format!("{}.{}", &last5[..3], &last5[3..]))
}

#[derive(Debug, Clone)]
pub struct CudaToolkitInfo {
    pub version: String,
    /// CUDA 툴킷 설치 디렉터리. Phase 1A.2.b의 nvcc/lib 경로 해석에서 사용 예정.
    #[allow(dead_code)]
    pub install_dir: String,
}

pub fn cuda_toolkits() -> Vec<CudaToolkitInfo> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let Ok(root) = hklm.open_subkey_with_flags(CUDA_TOOLKIT_ROOT, KEY_READ | KEY_WOW64_64KEY)
    else {
        return Vec::new();
    };
    root.enum_keys()
        .flatten()
        .filter(|k| k.starts_with('v'))
        .filter_map(|ver| {
            let sub = root.open_subkey(&ver).ok()?;
            let install_dir: String = sub.get_value("InstallDir").ok()?;
            Some(CudaToolkitInfo {
                version: ver,
                install_dir,
            })
        })
        .collect()
}

/// DLL 존재 여부만 확인 (즉시 drop).
fn dll_present(name: &str) -> bool {
    // SAFETY: probe-only — symbol resolve / call 없음. Library는 즉시 drop.
    unsafe { libloading::Library::new(name).is_ok() }
}

pub fn d3d12_present() -> bool {
    dll_present("d3d12.dll")
}

pub fn directml_present() -> bool {
    dll_present("DirectML.dll")
}

pub fn nvcuda_present() -> bool {
    dll_present("nvcuda.dll")
}

/// Windows 환경 prereq 종합 — 레지스트리 + DLL probe 결과를 RuntimeInfo에 매핑.
pub fn probe_windows_runtime() -> RuntimeInfo {
    let cuda_toolkits: Vec<String> = cuda_toolkits().into_iter().map(|c| c.version).collect();
    RuntimeInfo {
        cuda_toolkits,
        cuda_runtime: nvcuda_present(),
        // dll 존재만 확인 (1차 cross-check). lib.rs의 ash probe가 더 권위 있는 결과로 덮어씀.
        vulkan: dll_present("vulkan-1.dll"),
        metal: false,
        directml: directml_present(),
        d3d12: d3d12_present(),
        rocm: false,
        webview2: webview2_version(),
        vcredist_2022: vcredist_2022_version(),
        glibc: None,
        libstdcpp: None,
        vulkan_devices: None, // lib.rs probe()에서 ash 결과로 보강.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webview2_version_returns_string_or_none() {
        // Win 11 22H2+ 에서는 거의 항상 Some. CI 환경(Win Server)에선 미설치 가능.
        let v = webview2_version();
        if let Some(ver) = &v {
            assert!(
                ver.split('.').count() >= 3,
                "expected dotted version, got {ver:?}"
            );
        }
    }

    #[test]
    fn d3d12_should_be_present_on_modern_windows() {
        // Win 10 1607+ 모든 SKU에 d3d12.dll 존재. 이 테스트는 Win 환경 invariant.
        assert!(d3d12_present(), "d3d12.dll not loadable on this Windows");
    }

    #[test]
    fn vcredist_2022_present_after_buildtools_install() {
        // 우리 Phase 0에서 VS Build Tools 14.42.x 를 설치했으므로 Some.
        let v = vcredist_2022_version();
        assert!(
            v.is_some(),
            "VC++ 2022 redist not detected — was BuildTools installed?"
        );
    }

    #[test]
    fn nvidia_driver_format_strips_dots_and_takes_last5() {
        assert_eq!(
            format_nvidia_driver("32.0.15.5186").as_deref(),
            Some("551.86")
        );
        assert_eq!(
            format_nvidia_driver("31.0.15.4577").as_deref(),
            Some("545.77")
        );
        assert_eq!(format_nvidia_driver("abc").as_deref(), None);
    }

    #[test]
    fn probe_windows_runtime_does_not_panic() {
        let r = probe_windows_runtime();
        // 핵심 invariant: d3d12은 모든 modern Win에서 true.
        assert!(r.d3d12);
    }
}
