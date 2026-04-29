//! crate: hardware-probe — OS / CPU / RAM / GPU / 디스크 / 환경 prereq 통합 probe.
//!
//! 정책 (ADR-0014, ADR-0021):
//! - probe는 deterministic. 같은 PC = 같은 결과.
//! - 모든 probe는 graceful fail (driver/loader 부재 시 panic 금지, None/empty 반환).
//! - WMI VRAM 사용 금지 (4GB clamp 버그) — NVML/Metal/sysfs/wgpu로만.
//! - 성능 예산: cold < 500ms (Win 일반 PC), < 25ms (mac), < 10ms (Linux).
//!
//! Phase 1A.2.a 책임 영역 (이 sub-phase):
//! - sysinfo 기반 OS/CPU/RAM/Disk
//! - wgpu vendor-agnostic GPU enum + nvml-wrapper NVIDIA enrichment
//! - Apple Metal (cfg=mac) + AMD sysfs (cfg=linux)
//! - Win 레지스트리 (WebView2/VC++/NVIDIA driver/CUDA toolkit) + DLL probe (D3D12/DirectML/nvcuda)
//! - 통합 `HardwareReport` 산출
//!
//! Phase 1A.2.b 합류 예정 (다음 sub-phase):
//! - ash 기반 Vulkan capability detection
//! - Linux libstdc++ GLIBCXX symbol probe
//! - Pinokio detect 메소드 evaluator (registry.read / fs.exists / shell.which / http.get 통합)

// 모듈은 모두 pub — Tauri command 또는 다른 워크스페이스 crate에서 직접 호출 가능.
// (lib 내부에서만 일부 함수가 호출되더라도 외부 consumer가 있으므로 dead_code 아님.)
pub mod gpu;
#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod mac;
pub mod sys;
mod types;
pub mod vulkan;
#[cfg(windows)]
pub mod win;

pub use types::*;

use std::time::{Instant, SystemTime};

/// 통합 hardware probe. cold-cache 환경에서 가능한 한 병렬화.
pub async fn probe() -> HardwareReport {
    let start = Instant::now();
    let started_at = SystemTime::now();

    let sysinfo_fut = tokio::task::spawn_blocking(sys::probe_sysinfo);
    let gpu_fut = gpu::probe_all_gpus();
    let vulkan_fut = tokio::task::spawn_blocking(vulkan::probe_vulkan);
    let runtimes_fut = tokio::task::spawn_blocking(probe_runtimes_sync);

    let (sysinfo_res, gpus, vulkan_res, runtimes_res) =
        tokio::join!(sysinfo_fut, gpu_fut, vulkan_fut, runtimes_fut);
    let mut gpus = gpus;

    let (mut os, cpu, mem, disks) = sysinfo_res.unwrap_or_else(|e| {
        tracing::error!(error = %e, "sysinfo probe panicked — using empty defaults");
        (
            OsInfo {
                family: OsFamily::Other,
                version: String::new(),
                arch: std::env::consts::ARCH.into(),
                kernel: String::new(),
                rosetta: None,
                distro: None,
                distro_version: None,
            },
            CpuInfo {
                brand: String::new(),
                vendor_id: String::new(),
                physical_cores: 0,
                logical_cores: 0,
                frequency_mhz: 0,
            },
            MemInfo {
                total_bytes: 0,
                available_bytes: 0,
            },
            vec![],
        )
    });
    let mut runtimes = runtimes_res.unwrap_or_default();

    // Vulkan probe 결과 통합. None이면 vulkan loader 부재 — runtimes.vulkan_devices=None,
    // 기존 dll_present 기반 `runtimes.vulkan` bool은 유지 (사소한 cross-check).
    let vulkan_probe = vulkan_res.unwrap_or(None);
    if vulkan_probe.is_some() {
        runtimes.vulkan = true;
    }
    if let Some(vp) = &vulkan_probe {
        // GpuInfo의 VRAM이 비어 있으면 Vulkan heap 합산값으로 보강 (Intel iGPU/AMD/일부 NVIDIA).
        for vd in &vp.devices {
            if let Some(g) = gpus
                .iter_mut()
                .find(|g| g.pci_id == Some((vd.vendor_pci, vd.device_pci)))
            {
                if g.vram_bytes.is_none() && vd.vram_bytes > 0 {
                    g.vram_bytes = Some(vd.vram_bytes);
                }
            }
        }
    }
    runtimes.vulkan_devices = vulkan_probe;

    // 플랫폼별 OS 보강.
    #[cfg(target_os = "macos")]
    {
        let m = mac::macos_arch_info();
        os.rosetta = Some(m.rosetta);
        if !m.macos_version.is_empty() {
            os.version = m.macos_version;
        }
        if !m.kernel.is_empty() {
            os.kernel = m.kernel;
        }
    }
    #[cfg(target_os = "linux")]
    {
        let d = linux::linux_distro_info();
        if !d.id.is_empty() {
            os.distro = Some(d.id);
        }
        if !d.version_id.is_empty() {
            os.distro_version = Some(d.version_id);
        }
        if !d.kernel.is_empty() {
            os.kernel = d.kernel;
        }
    }
    #[cfg(target_os = "windows")]
    {
        let _ = &mut os; // Win 측 OS는 sysinfo가 충분.
    }

    let (probed_at, probe_ms) = types::timing(start, started_at);

    HardwareReport {
        os,
        cpu,
        mem,
        disks,
        gpus,
        runtimes,
        probed_at,
        probe_ms,
    }
}

fn probe_runtimes_sync() -> RuntimeInfo {
    #[cfg(windows)]
    {
        win::probe_windows_runtime()
    }
    #[cfg(target_os = "macos")]
    {
        mac::probe_macos_runtime()
    }
    #[cfg(target_os = "linux")]
    {
        linux::probe_linux_runtime()
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        RuntimeInfo::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn probe_returns_populated_report() {
        let r = probe().await;
        assert!(r.probe_ms > 0, "probe_ms must be > 0");
        assert!(!r.probed_at.is_empty(), "probed_at must be set");
        assert!(matches!(
            r.os.family,
            OsFamily::Windows | OsFamily::Macos | OsFamily::Linux
        ));
        assert!(
            !r.cpu.brand.is_empty() || r.cpu.logical_cores > 0,
            "CPU info empty"
        );
        assert!(r.mem.total_bytes > 0, "memory total_bytes must be > 0");
    }

    #[tokio::test]
    async fn probe_serializes_to_json() {
        let r = probe().await;
        let json = serde_json::to_value(&r).expect("serialize");
        assert!(json["os"]["family"].is_string());
        assert!(json["cpu"]["logical_cores"].is_number());
        assert!(json["mem"]["total_bytes"].is_number());
        assert!(json["runtimes"].is_object());
    }

    #[tokio::test]
    async fn probe_runs_within_budget() {
        let _warm_up = probe().await;
        let start = std::time::Instant::now();
        let _ = probe().await;
        let dur = start.elapsed();
        assert!(
            dur.as_millis() < 2000,
            "warm probe took {dur:?}, exceeding 2s cap"
        );
    }

    #[test]
    fn gpu_vendor_pci_mapping() {
        assert_eq!(GpuVendor::from_pci(0x10DE), GpuVendor::Nvidia);
        assert_eq!(GpuVendor::from_pci(0x1002), GpuVendor::Amd);
        assert_eq!(GpuVendor::from_pci(0x8086), GpuVendor::Intel);
        assert_eq!(GpuVendor::from_pci(0x106B), GpuVendor::Apple);
        assert_eq!(GpuVendor::from_pci(0xDEAD), GpuVendor::Other);
    }

    #[test]
    fn types_round_trip_serde() {
        let info = OsInfo {
            family: OsFamily::Windows,
            version: "11".into(),
            arch: "x86_64".into(),
            kernel: "10.0.26200".into(),
            rosetta: None,
            distro: None,
            distro_version: None,
        };
        let s = serde_json::to_string(&info).unwrap();
        assert!(s.contains("\"family\":\"windows\""));
        assert!(!s.contains("rosetta"));
    }
}
