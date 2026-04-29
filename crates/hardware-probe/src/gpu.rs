//! GPU 탐지 — vendor-specific paths (no wgpu).
//!
//! 정책 (ADR-0021, Phase 1A.2.a 보강 리서치 §3):
//! - NVIDIA: nvml-wrapper (graceful fail)
//! - Windows non-NVIDIA: DXGI via windows-rs `IDXGIFactory1::EnumAdapters1` + `GetDesc1`
//!   (4GB clamp 버그 없는 정확한 `DedicatedVideoMemory`)
//! - Apple: objc2-metal (cfg=mac)
//! - Linux AMD: /sys/class/drm/card*/device/{vendor,device,mem_info_vram_total} (cfg=linux)
//! - 모든 probe는 graceful fail (driver/loader 부재 시 panic 금지).
//!
//! wgpu 대신 vendor-specific path를 쓰는 이유: wgpu-hal-29.0.1의 windows-core 0.61 vs
//! 다른 deps의 0.62 trait 충돌. wgpu 드롭으로 빌드 안정성 확보.

use crate::types::{GpuBackend, GpuDeviceType, GpuInfo, GpuVendor};

/// NVIDIA NVML로 GPU + VRAM + driver 수집. NVML 미동작 시 빈 Vec.
pub fn probe_nvidia_via_nvml() -> Vec<NvidiaEnrichment> {
    use nvml_wrapper::error::NvmlError;
    use nvml_wrapper::Nvml;

    let nvml = match Nvml::init() {
        Ok(n) => n,
        Err(NvmlError::LibloadingError(_)) | Err(NvmlError::DriverNotLoaded) => {
            tracing::debug!("NVML library or driver not present (no NVIDIA)");
            return vec![];
        }
        Err(e) => {
            tracing::warn!(error = %e, "NVML init failed");
            return vec![];
        }
    };

    let driver = nvml.sys_driver_version().ok();
    let count = nvml.device_count().unwrap_or(0);

    let mut enriched = Vec::with_capacity(count as usize);
    for i in 0..count {
        let device = match nvml.device_by_index(i) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(index = i, error = %e, "NVML device_by_index failed");
                continue;
            }
        };
        let name = device.name().unwrap_or_default();
        let vram = device.memory_info().ok().map(|m| m.total);
        enriched.push(NvidiaEnrichment {
            name,
            vram_bytes: vram,
            driver_version: driver.clone(),
        });
    }

    tracing::info!(devices = enriched.len(), "NVML probe complete");
    enriched
}

#[derive(Debug, Clone)]
pub struct NvidiaEnrichment {
    pub name: String,
    pub vram_bytes: Option<u64>,
    pub driver_version: Option<String>,
}

/// Windows DXGI adapter enumeration — non-NVIDIA(Intel iGPU, AMD, Microsoft WARP) 까지 커버.
/// `DedicatedVideoMemory`는 4GB clamp 버그 없는 정확한 VRAM.
#[cfg(windows)]
pub fn probe_windows_via_dxgi() -> Vec<DxgiAdapter> {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1};

    // SAFETY: DXGI는 thread-safe하지만 COM 호출이라 unsafe.
    let factory: IDXGIFactory1 = unsafe {
        match CreateDXGIFactory1() {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(error = ?e, "CreateDXGIFactory1 failed");
                return vec![];
            }
        }
    };

    let mut out = Vec::new();
    let mut i = 0u32;
    loop {
        let adapter: IDXGIAdapter1 = match unsafe { factory.EnumAdapters1(i) } {
            Ok(a) => a,
            Err(_) => break, // DXGI_ERROR_NOT_FOUND — enumeration end.
        };

        // windows-rs 0.62: GetDesc1는 인자 없이 Result<DXGI_ADAPTER_DESC1> 반환.
        if let Ok(desc) = unsafe { adapter.GetDesc1() } {
            let nul = desc
                .Description
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(desc.Description.len());
            let model = String::from_utf16_lossy(&desc.Description[..nul])
                .trim()
                .to_string();
            out.push(DxgiAdapter {
                vendor_pci: desc.VendorId as u16,
                device_pci: desc.DeviceId as u16,
                model,
                vram_bytes: desc.DedicatedVideoMemory as u64,
                shared_system_bytes: desc.SharedSystemMemory as u64,
                is_software: desc.VendorId == 0x1414,
            });
        }
        i += 1;
    }
    tracing::info!(adapters = out.len(), "DXGI probe complete");
    out
}

#[cfg(windows)]
#[derive(Debug, Clone)]
pub struct DxgiAdapter {
    pub vendor_pci: u16,
    pub device_pci: u16,
    pub model: String,
    pub vram_bytes: u64,
    pub shared_system_bytes: u64,
    pub is_software: bool,
}

/// 통합 GPU probe — NVML + (cfg) DXGI/Metal/sysfs.
pub async fn probe_all_gpus() -> Vec<GpuInfo> {
    let nvml_fut = tokio::task::spawn_blocking(probe_nvidia_via_nvml);

    #[cfg(windows)]
    let dxgi_fut = tokio::task::spawn_blocking(probe_windows_via_dxgi);

    let nvml_results = nvml_fut.await.unwrap_or_default();

    let mut gpus: Vec<GpuInfo> = nvml_results
        .into_iter()
        .map(|n| GpuInfo {
            vendor: GpuVendor::Nvidia,
            model: n.name,
            vram_bytes: n.vram_bytes,
            pci_id: None, // NVML은 PCI ID 직접 노출 안 함 — DXGI matching 시 추가.
            driver_version: n.driver_version,
            backend: GpuBackend::Vulkan, // NVIDIA는 Vulkan/CUDA/DX12 모두 지원 — Vulkan 표시.
            device_type: GpuDeviceType::DiscreteGpu,
            apple_family: None,
        })
        .collect();

    #[cfg(windows)]
    {
        let dxgi_results = dxgi_fut.await.unwrap_or_default();

        // NVML이 비었거나 driver_version을 못 채웠을 때 레지스트리 fallback.
        let registry_driver = crate::win::nvidia_driver_version();

        for adapter in dxgi_results {
            let vendor = GpuVendor::from_pci(adapter.vendor_pci as u32);
            if matches!(vendor, GpuVendor::Nvidia) {
                // NVIDIA: NVML 항목 PCI ID enrich + driver_version registry fallback.
                if let Some(g) = gpus
                    .iter_mut()
                    .find(|g| matches!(g.vendor, GpuVendor::Nvidia) && g.pci_id.is_none())
                {
                    g.pci_id = Some((adapter.vendor_pci, adapter.device_pci));
                    if g.vram_bytes.is_none() {
                        g.vram_bytes = Some(adapter.vram_bytes);
                    }
                    if g.driver_version.is_none() {
                        g.driver_version = registry_driver.clone();
                    }
                } else if !gpus.iter().any(|g| {
                    matches!(g.vendor, GpuVendor::Nvidia)
                        && g.pci_id == Some((adapter.vendor_pci, adapter.device_pci))
                }) {
                    // NVML이 부재(driver만 설치된 환경 등). DXGI + 레지스트리 driver로 NVIDIA entry 신규 생성.
                    gpus.push(GpuInfo {
                        vendor: GpuVendor::Nvidia,
                        model: adapter.model.clone(),
                        vram_bytes: Some(adapter.vram_bytes),
                        pci_id: Some((adapter.vendor_pci, adapter.device_pci)),
                        driver_version: registry_driver.clone(),
                        backend: GpuBackend::Dx12,
                        device_type: GpuDeviceType::DiscreteGpu,
                        apple_family: None,
                    });
                }
                continue;
            }
            let device_type = if adapter.is_software {
                GpuDeviceType::Cpu
            } else if matches!(vendor, GpuVendor::Intel) {
                GpuDeviceType::IntegratedGpu
            } else {
                GpuDeviceType::DiscreteGpu
            };
            gpus.push(GpuInfo {
                vendor,
                model: adapter.model,
                vram_bytes: Some(adapter.vram_bytes),
                pci_id: Some((adapter.vendor_pci, adapter.device_pci)),
                driver_version: None,
                backend: GpuBackend::Dx12,
                device_type,
                apple_family: None,
            });
        }
    }

    // Apple Metal enrichment (cfg).
    #[cfg(target_os = "macos")]
    {
        if let Some(metal) = crate::mac::probe_metal() {
            gpus.push(GpuInfo {
                vendor: GpuVendor::Apple,
                model: metal.name,
                vram_bytes: Some(metal.vram_bytes),
                pci_id: Some((0x106B, 0)),
                driver_version: None,
                backend: GpuBackend::Metal,
                device_type: GpuDeviceType::IntegratedGpu,
                apple_family: Some(metal.tier),
            });
        }
    }

    // Linux AMD sysfs enrichment (cfg).
    #[cfg(target_os = "linux")]
    {
        let amd_extras = crate::linux::probe_amd_sysfs();
        for amd in amd_extras {
            // 같은 device가 NVML/기타 경로에 없을 때만 추가.
            if !gpus.iter().any(|g| {
                matches!(g.vendor, GpuVendor::Amd)
                    && g.pci_id.map(|(_, d)| d) == Some(amd.device_pci)
            }) {
                gpus.push(GpuInfo {
                    vendor: GpuVendor::Amd,
                    model: format!("AMD {:04x}:{:04x}", amd.vendor_pci, amd.device_pci),
                    vram_bytes: amd.vram_bytes,
                    pci_id: Some((amd.vendor_pci, amd.device_pci)),
                    driver_version: None,
                    backend: GpuBackend::Vulkan,
                    device_type: GpuDeviceType::DiscreteGpu,
                    apple_family: None,
                });
            }
        }
    }

    gpus
}
