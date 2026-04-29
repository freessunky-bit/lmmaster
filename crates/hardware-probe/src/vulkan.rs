//! Vulkan capability probe via `ash 0.38` `loaded` feature.
//!
//! 정책 (Phase 1A.2.b 보강 리서치):
//! - `Entry::load()`로 loader 동적 로드 — vulkan-1.dll(Win) / libvulkan.so.1(Linux) /
//!   libvulkan.1.dylib(MoltenVK, mac). 미존재 시 None 반환 (panic 금지).
//! - validation layer / surface extension 사용 안 함 — render 안 하고 enum + properties만.
//! - VRAM = `physical_device_memory_properties.memory_heaps` 중 DEVICE_LOCAL flag 합산.
//!   NVML/DXGI/Metal로 측정 못 한 vendor에서 cross-check 또는 fallback.
//! - ash 0.38의 `Instance`는 `Drop` 구현 안 함 — 명시적 `destroy_instance` 호출 필수.

use ash::{vk, Entry, Instance};
use serde::{Deserialize, Serialize};

use crate::types::GpuDeviceType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulkanProbe {
    /// instance API version (예: "1.3.280")
    pub api_version: String,
    pub devices: Vec<VulkanDevice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulkanDevice {
    pub name: String,
    pub vendor_pci: u16,
    pub device_pci: u16,
    pub device_type: GpuDeviceType,
    pub api_version: String,
    /// 원시 driver_version u32. 인코딩은 vendor-specific (예: NVIDIA = (major<<22)|(minor<<14)).
    pub driver_version_raw: u32,
    pub vram_bytes: u64,
}

/// Vulkan loader가 부재하거나 instance 생성에 실패하면 None.
pub fn probe_vulkan() -> Option<VulkanProbe> {
    // SAFETY: Entry::load는 libloading 호출. unsafe로 표시되어 있지만 안전한 사용 패턴.
    let entry: Entry = match unsafe { Entry::load() } {
        Ok(e) => e,
        Err(err) => {
            tracing::debug!(?err, "Vulkan loader not present");
            return None;
        }
    };

    // application/engine 이름은 진단용. 1.0 API version 요청 (가장 permissive).
    let app_name = c"lmmaster-probe";
    let app_info = vk::ApplicationInfo::default()
        .application_name(app_name)
        .application_version(0)
        .engine_name(app_name)
        .engine_version(0)
        .api_version(vk::make_api_version(0, 1, 0, 0));

    // 확장/레이어 모두 비활성 — overhead 회피.
    let create_info = vk::InstanceCreateInfo::default().application_info(&app_info);

    // SAFETY: create_info는 stack 할당. 호출 후 즉시 instance ownership으로 이전.
    let instance: Instance = match unsafe { entry.create_instance(&create_info, None) } {
        Ok(i) => i,
        Err(err) => {
            tracing::warn!(?err, "vkCreateInstance failed");
            return None;
        }
    };

    let probe = collect_devices(&entry, &instance);

    // ash 0.38 Instance는 Drop 구현 안 함 — 명시적 정리.
    unsafe { instance.destroy_instance(None) };

    Some(probe)
}

fn collect_devices(entry: &Entry, instance: &Instance) -> VulkanProbe {
    // instance API version: try_enumerate_instance_version은 1.0 loader에서 None 반환.
    let inst_api = match unsafe { entry.try_enumerate_instance_version() } {
        Ok(Some(v)) => v,
        _ => vk::API_VERSION_1_0,
    };

    let phys = match unsafe { instance.enumerate_physical_devices() } {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!(?err, "enumerate_physical_devices failed");
            Vec::new()
        }
    };

    let mut devices = Vec::with_capacity(phys.len());
    for pd in phys {
        // SAFETY: pd는 enumerate_physical_devices의 반환값.
        let props = unsafe { instance.get_physical_device_properties(pd) };
        let mem = unsafe { instance.get_physical_device_memory_properties(pd) };

        let vram: u64 = mem.memory_heaps[..mem.memory_heap_count as usize]
            .iter()
            .filter(|h| h.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL))
            .map(|h| h.size)
            .sum();

        devices.push(VulkanDevice {
            name: cstr_array_to_string(&props.device_name),
            vendor_pci: props.vendor_id as u16,
            device_pci: props.device_id as u16,
            device_type: map_device_type(props.device_type),
            api_version: format_api(props.api_version),
            driver_version_raw: props.driver_version,
            vram_bytes: vram,
        });
    }

    VulkanProbe {
        api_version: format_api(inst_api),
        devices,
    }
}

fn map_device_type(t: vk::PhysicalDeviceType) -> GpuDeviceType {
    match t {
        vk::PhysicalDeviceType::DISCRETE_GPU => GpuDeviceType::DiscreteGpu,
        vk::PhysicalDeviceType::INTEGRATED_GPU => GpuDeviceType::IntegratedGpu,
        vk::PhysicalDeviceType::VIRTUAL_GPU => GpuDeviceType::VirtualGpu,
        vk::PhysicalDeviceType::CPU => GpuDeviceType::Cpu,
        _ => GpuDeviceType::Other,
    }
}

fn format_api(v: u32) -> String {
    format!(
        "{}.{}.{}",
        vk::api_version_major(v),
        vk::api_version_minor(v),
        vk::api_version_patch(v),
    )
}

/// Vulkan 구조체의 `[c_char; N]` 또는 `[i8; N]` 디바이스 이름을 String으로.
fn cstr_array_to_string(buf: &[i8]) -> String {
    // SAFETY: buf는 stack/struct에 인접한 바이트 시퀀스. bit-level 재해석.
    let bytes: &[u8] = unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, buf.len()) };
    let nul = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..nul]).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_does_not_panic_regardless_of_loader_presence() {
        // Vulkan loader 유무와 무관하게 panic 금지 — None 또는 valid VulkanProbe.
        let r = probe_vulkan();
        if let Some(p) = r {
            assert!(
                !p.api_version.is_empty(),
                "api_version must be set when probe is Some"
            );
            // device count 0도 valid (CPU/iGPU 없는 경우 거의 없지만 가능).
        }
    }

    #[test]
    fn format_api_examples() {
        let v = vk::make_api_version(0, 1, 3, 280);
        assert_eq!(format_api(v), "1.3.280");
        assert_eq!(format_api(vk::API_VERSION_1_0), "1.0.0");
    }

    #[test]
    fn cstr_array_handles_no_nul_terminator() {
        let buf: [i8; 4] = [b'a' as i8, b'b' as i8, b'c' as i8, b'd' as i8];
        assert_eq!(cstr_array_to_string(&buf), "abcd");
    }

    #[test]
    fn cstr_array_strips_at_nul() {
        let mut buf: [i8; 8] = [0; 8];
        buf[0] = b'h' as i8;
        buf[1] = b'i' as i8;
        // buf[2] = 0 (already)
        assert_eq!(cstr_array_to_string(&buf), "hi");
    }

    #[test]
    fn vulkan_probe_serializes_to_clean_json() {
        let probe = VulkanProbe {
            api_version: "1.3.280".into(),
            devices: vec![VulkanDevice {
                name: "Test GPU".into(),
                vendor_pci: 0x10DE,
                device_pci: 0x2782,
                device_type: GpuDeviceType::DiscreteGpu,
                api_version: "1.3.280".into(),
                driver_version_raw: 0x12345678,
                vram_bytes: 8 * 1024 * 1024 * 1024,
            }],
        };
        let json = serde_json::to_value(&probe).unwrap();
        assert_eq!(json["api_version"], "1.3.280");
        assert_eq!(json["devices"][0]["device_type"], "discrete-gpu");
        assert_eq!(json["devices"][0]["vram_bytes"], 8 * 1024 * 1024 * 1024_u64);
    }
}
