//! 크로스플랫폼 OS / CPU / RAM / 디스크 probe — sysinfo 0.31 기반.

use sysinfo::{Disks, System};

use crate::types::{CpuInfo, DiskInfo, DiskKind, MemInfo, OsFamily, OsInfo};

/// 단일 SysInfo refresh 후 OS/CPU/RAM/Disk 4종을 한 번에 수집.
pub fn probe_sysinfo() -> (OsInfo, CpuInfo, MemInfo, Vec<DiskInfo>) {
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu_all();

    let os = probe_os(&sys);
    let cpu = probe_cpu(&sys);
    let mem = probe_mem(&sys);

    let disks = Disks::new_with_refreshed_list();
    let disks = probe_disks(&disks);

    (os, cpu, mem, disks)
}

fn probe_os(_sys: &System) -> OsInfo {
    let family = if cfg!(target_os = "windows") {
        OsFamily::Windows
    } else if cfg!(target_os = "macos") {
        OsFamily::Macos
    } else if cfg!(target_os = "linux") {
        OsFamily::Linux
    } else {
        OsFamily::Other
    };

    let version = System::os_version().unwrap_or_else(|| "unknown".into());
    let kernel = System::kernel_version().unwrap_or_else(|| "unknown".into());
    let arch = std::env::consts::ARCH.to_string();

    OsInfo {
        family,
        version,
        arch,
        kernel,
        rosetta: None, // mac 전용 — mac.rs에서 보강.
        distro: None,  // linux 전용 — linux.rs에서 보강.
        distro_version: None,
    }
}

fn probe_cpu(sys: &System) -> CpuInfo {
    let cpus = sys.cpus();
    let first = cpus.first();
    let brand = first.map(|c| c.brand().to_string()).unwrap_or_default();
    let vendor_id = first.map(|c| c.vendor_id().to_string()).unwrap_or_default();
    let frequency_mhz = first.map(|c| c.frequency()).unwrap_or(0);
    let logical_cores = cpus.len() as u32;
    let physical_cores = sys.physical_core_count().unwrap_or(logical_cores as usize) as u32;

    CpuInfo {
        brand,
        vendor_id,
        physical_cores,
        logical_cores,
        frequency_mhz,
    }
}

fn probe_mem(sys: &System) -> MemInfo {
    MemInfo {
        total_bytes: sys.total_memory(),
        available_bytes: sys.available_memory(),
    }
}

fn probe_disks(disks: &Disks) -> Vec<DiskInfo> {
    disks
        .list()
        .iter()
        .map(|d| DiskInfo {
            mount_point: d.mount_point().to_string_lossy().into_owned(),
            kind: match d.kind() {
                sysinfo::DiskKind::SSD => DiskKind::Ssd,
                sysinfo::DiskKind::HDD => DiskKind::Hdd,
                _ => DiskKind::Other,
            },
            total_bytes: d.total_space(),
            available_bytes: d.available_space(),
        })
        .collect()
}
