# Phase 1A.2.a 보강 리서치 — 종합 (구현 직전)

> 2026-04-26. ADR-0021의 hardware probe stack을 **실제 코드 결정**으로 좁힌다. 3영역 보강 리서치 결과.
> 출처는 §6 참조.

## 1. 결정 요약 표

| 항목 | 결정 | 비고 |
|---|---|---|
| GPU vendor-agnostic enum | **wgpu 29** `Instance::new(InstanceDescriptor) + enumerate_adapters().await` | wgpu adapter는 VRAM 미보고 — model/vendor/pci_id/backend만. |
| NVIDIA enrichment | **nvml-wrapper 0.11** `Nvml::init()` graceful fail 3종(LibraryNotFound/LibloadingError/DriverNotLoaded) | VRAM = `Device::memory_info().total`. driver = `Nvml::sys_driver_version()`. |
| AMD enrichment (Linux) | `/sys/class/drm/card*/device/{vendor,device,mem_info_vram_total}` 직접 읽기 | `amdgpu-sysfs` crate는 옵션. 우리 use case엔 직접 fs read가 단순. |
| Apple Metal | **objc2-metal 0.3 + objc2 0.6** (구 `metal-rs`는 deprecated) | `MTLCreateSystemDefaultDevice` → `recommendedMaxWorkingSetSize`(UMA VRAM) + `supportsFamily(MTLGPUFamily::Apple7..9)`. |
| Apple Silicon detect | **`libc::sysctlbyname` 직접 FFI** (sysctl 0.5 crate 거부) | 4 deps 회피, ~30µs. `hw.optional.arm64`, `sysctl.proc_translated`, `machdep.cpu.brand_string`, `kern.osproductversion`. |
| Win 레지스트리 | **winreg 0.55** + `KEY_READ \| KEY_WOW64_64KEY` flag 강제 | x64 binary가 32-bit 작성된 키(예: WebView2의 WOW6432Node) 안전 read. |
| WebView2 정확 path | `HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}` value `pv` (REG_SZ) | HKCU + HKLM(non-WOW6432) fallback. `""`/`"0.0.0.0"`은 미설치로 간주. |
| VC++ 2022 정확 path | `HKLM\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64` `Installed`(DWORD)=1 + `Version`(REG_SZ) | 14.0 키는 2015~2022 binary-compatible 공통. |
| NVIDIA driver 정확 path | `HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\<idx>\DriverVersion` | iterate `\0000..\0007`, `MatchingDeviceId`가 `PCI\VEN_10DE`로 시작하는 항목만. raw "32.0.15.5186" → user-visible "551.86" 변환. |
| CUDA toolkits | `HKLM\SOFTWARE\NVIDIA Corporation\GPU Computing Toolkit\CUDA` 하위 `v<X.Y>` 키 enum + `InstallDir` | 다중 toolkit 동시 보고. |
| DLL probe (Win/Linux) | **libloading 0.8.6** `Library::new(name).is_ok()` (probe-only, 즉시 drop) | d3d12.dll, DirectML.dll, nvcuda.dll/libcuda.so.1. |
| Linux glibc | **`gnu_get_libc_version` 직접 FFI** (glibc_version crate는 build.rs-only) | ~50ns, zero alloc. |
| Linux distro | `/etc/os-release` 직접 파싱 (~50 LOC) | os-release crate는 abandoned. etc-os-release 0.1 옵션. |
| WMI VRAM | **사용 금지** | `Win32_VideoController.AdapterRAM` UInt32 4GB clamp 버그. NVML/IOKit/sysfs로만. |
| 병렬화 | `tokio::join!` + `tokio::task::spawn_blocking` (sysinfo, nvml, registry는 동기 API) | 목표 cold < 200ms typical PC. |
| `metal-rs` vs `objc2-metal` | **objc2-metal 0.3 채택** | metal-rs는 gfx-rs/metal-rs#339에서 공식 deprecated. wgpu도 마이그레이션 중. |
| `glibc_version` crate | **불채택** (build.rs-only, ldd spawn 내부 사용) | direct FFI. |
| `os-release` crate | **불채택** (2020 마지막 release, lazy_static 의존) | inline parse. |

## 2. 신규 dependencies (workspace Cargo.toml)

```toml
# workspace.dependencies 추가
nvml-wrapper = "0.11"
wgpu = { version = "29", default-features = false, features = ["wgsl", "vulkan", "dx12", "metal", "gles", "naga"] }
libloading = "0.8"
libc = "0.2"

[target.'cfg(windows)'.dependencies]
winreg = "0.55"

[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.6"
objc2-metal = { version = "0.3", features = ["MTLDevice"] }
objc2-foundation = "0.3"
```

`sysinfo`은 0.31 → **0.34**로 bump (`SystemExt`/`DiskExt` trait이 0.32+에서 inherent method로 통합되며 deprecate, but 0.31에서도 동작 — bump는 후속).

## 3. 핵심 코드 패턴

### 3.1 NVML graceful fail
```rust
use nvml_wrapper::{Nvml, error::NvmlError};
match Nvml::init() {
    Ok(n) => Some(n),
    Err(NvmlError::LibloadingError(_) | NvmlError::LibraryNotFound | NvmlError::DriverNotLoaded) => None,
    Err(e) => { tracing::warn!(error=%e, "nvml init failed"); None }
}
```

### 3.2 wgpu 29 enumeration (async)
```rust
let inst = wgpu::Instance::new(&wgpu::InstanceDescriptor {
    backends: wgpu::Backends::all(),
    ..Default::default()
});
let adapters = inst.enumerate_adapters(wgpu::Backends::all());
for a in adapters {
    let i = a.get_info();   // AdapterInfo { name, vendor, device, device_type, driver, driver_info, backend }
    // vendor PCI id: 0x10DE NVIDIA, 0x1002 AMD, 0x8086 Intel, 0x106B Apple
}
```
주의: `Instance::new`가 wgpu 29에서 `&InstanceDescriptor` (참조)를 받는다 — 28과 차이.

### 3.3 winreg WOW6432Node-safe read
```rust
use winreg::{RegKey, enums::*};
let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
let k = hklm.open_subkey_with_flags(
    r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
    KEY_READ | KEY_WOW64_64KEY,
)?;
let pv: String = k.get_value("pv")?;
```

### 3.4 sysctlbyname FFI (mac)
```rust
unsafe fn sysctl_string(name: &str) -> Option<String> {
    let c = std::ffi::CString::new(name).ok()?;
    let mut len: libc::size_t = 0;
    if libc::sysctlbyname(c.as_ptr(), std::ptr::null_mut(), &mut len, std::ptr::null_mut(), 0) != 0 { return None; }
    let mut buf = vec![0u8; len];
    if libc::sysctlbyname(c.as_ptr(), buf.as_mut_ptr() as _, &mut len, std::ptr::null_mut(), 0) != 0 { return None; }
    if let Some(&0) = buf.last() { buf.pop(); }
    String::from_utf8(buf).ok()
}
```

### 3.5 Apple Metal probe
```rust
use objc2_metal::{MTLCreateSystemDefaultDevice, MTLDevice, MTLGPUFamily};
let device = unsafe { MTLCreateSystemDefaultDevice() }?;   // Option<Retained<...>>
let name = unsafe { device.name() }.to_string();
let vram = unsafe { device.recommendedMaxWorkingSetSize() } as u64;
let apple9 = unsafe { device.supportsFamily(MTLGPUFamily::Apple9) };
```

## 4. 모듈 구조 결정

`crates/hardware-probe/src/`:
- `lib.rs` — 공개 API + async `probe()` entry + 인-소스 테스트
- `types.rs` — HardwareReport / OsInfo / CpuInfo / MemInfo / DiskInfo / GpuInfo / GpuVendor / BackendCaps / RuntimeInfo
- `sys.rs` — sysinfo wrapper (OS/CPU/RAM/disk; cross-platform)
- `gpu.rs` — wgpu adapter enum + nvml augment + (cfg) mac/linux 호출
- `win.rs` — `cfg(windows)` 레지스트리 + DLL probe
- `mac.rs` — `cfg(target_os = "macos")` sysctl + Metal
- `linux.rs` — `cfg(target_os = "linux")` /etc/os-release + glibc + libcuda

기존 stub 5개(os.rs/cpu.rs/memory.rs/disk.rs/capability.rs)는 sys.rs로 통합되어 **삭제**.

## 5. 성능 예산 (실측 기대값)

- sysinfo(OS/CPU/RAM/disk): 10ms
- wgpu instance + enumerate: 80~150ms cold (백엔드 4종 병렬)
- nvml init + per-GPU: 40~120ms (NVIDIA 있을 때만)
- Win 레지스트리 5개 read: <10ms
- DLL probe 3개: 1~5ms each
- macOS sysctl 5개 + Metal: 10~25ms cold
- Linux /etc/os-release + dlopen: 1~5ms

전체 cold target: **< 500ms** Win 일반 PC, **< 25ms** mac, **< 10ms** Linux.

## 6. 출처 (검증)

- nvml-wrapper: docs.rs/nvml-wrapper, github.com/rust-nvml/nvml-wrapper, NvmlError variants
- wgpu 29: docs.rs/crate/wgpu/latest, gfx-rs/wgpu Discussions #5442 (adapter order)
- objc2-metal: lib.rs/crates/objc2-metal, github.com/madsmtm/objc2, Apple developer.apple.com/documentation/metal/{recommendedmaxworkingsetsize,supportsfamily}
- gfx-rs/metal-rs#339 (deprecation in favor of objc2-metal)
- winreg: docs.rs/winreg/0.55.0, github.com/gentoo90/winreg-rs
- WebView2 path: learn.microsoft.com/microsoft-edge/webview2/concepts/distribution
- VC++ redist: learn.microsoft.com/visualstudio/releases/2022/redistribution, learn.microsoft.com/cpp/windows/latest-supported-vc-redist
- NVIDIA driver path: forums.developer.nvidia.com/t/checking-graphics-driver-version-using-registry-keys/61862
- CUDA path: docs.nvidia.com/cuda/cuda-installation-guide-microsoft-windows
- libloading: docs.rs/libloading
- AMD sysfs: lunnova.dev/articles/linux-get-gpu-vram-size, github.com/Umio-Yasuno/libdrm-amdgpu-sys-rs
- glibc FFI: pop-os/os-release(unmaintained), aristocratos/btop#678
- spacedrive (Rust+macOS reference)
