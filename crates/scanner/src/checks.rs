//! Deterministic 환경 체크 — `EnvironmentReport` → `Vec<CheckResult>`.
//!
//! 정책 (ADR-0013, ADR-0020):
//! - 모든 판단은 deterministic 로직. LLM은 자연어 풀어쓰기만.
//! - 적용 안 되는 체크 (Win 외 NVIDIA driver 등)는 None 반환 → 자동 skip.
//! - severity는 사실 기반: Error = 필수 누락, Warn = 사양 미달, Info = 정보성.

use serde::{Deserialize, Serialize};

use hardware_probe::{GpuVendor, OsFamily};
use runtime_detector::EnvironmentReport;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    /// kebab-case stable id — UI/i18n 키로 사용.
    pub id: String,
    pub severity: Severity,
    pub title_ko: String,
    pub detail_ko: String,
    /// 사용자에게 권장되는 다음 행동 (있으면).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommendation: Option<String>,
}

const GIB: u64 = 1024 * 1024 * 1024;

/// 모든 deterministic 체크 실행. 적용 안 되는 체크는 자동 skip.
pub fn run_all(env: &EnvironmentReport) -> Vec<CheckResult> {
    let mut out = Vec::with_capacity(12);

    if let Some(c) = check_ram(env) {
        out.push(c);
    }
    if let Some(c) = check_disk(env) {
        out.push(c);
    }
    if let Some(c) = check_gpu(env) {
        out.push(c);
    }
    if let Some(c) = check_webview2(env) {
        out.push(c);
    }
    if let Some(c) = check_vc_redist(env) {
        out.push(c);
    }
    if let Some(c) = check_nvidia_driver(env) {
        out.push(c);
    }
    if let Some(c) = check_cuda(env) {
        out.push(c);
    }
    if let Some(c) = check_vulkan(env) {
        out.push(c);
    }
    out.extend(check_runtimes(env));
    out
}

fn check_ram(env: &EnvironmentReport) -> Option<CheckResult> {
    let bytes = env.hardware.mem.total_bytes;
    let gib = bytes as f64 / GIB as f64;
    if bytes < 8 * GIB {
        Some(CheckResult {
            id: "ram-low".into(),
            severity: Severity::Warn,
            title_ko: format!("메모리가 부족해요 ({gib:.1}GB)"),
            detail_ko: "8GB 미만이라 큰 모델은 어려울 수 있어요. 3B 이하 작은 모델을 권장해요."
                .into(),
            recommendation: Some("EXAONE 1.2B을 받아볼까요?".into()),
        })
    } else if bytes < 16 * GIB {
        Some(CheckResult {
            id: "ram-ok-7b".into(),
            severity: Severity::Info,
            title_ko: format!("메모리 {gib:.1}GB"),
            detail_ko: "7B 이하 모델이 잘 돌아요.".into(),
            recommendation: None,
        })
    } else if bytes < 32 * GIB {
        Some(CheckResult {
            id: "ram-ok-13b".into(),
            severity: Severity::Info,
            title_ko: format!("메모리 {gib:.1}GB"),
            detail_ko: "13B 이하 모델이 잘 돌아요.".into(),
            recommendation: None,
        })
    } else {
        Some(CheckResult {
            id: "ram-ample".into(),
            severity: Severity::Info,
            title_ko: format!("메모리 {gib:.1}GB"),
            detail_ko: "30B+ 큰 모델도 돌릴 수 있어요.".into(),
            recommendation: None,
        })
    }
}

fn check_disk(env: &EnvironmentReport) -> Option<CheckResult> {
    // boot 디스크는 OS별로 다른데, 가장 작은 가용 용량을 기준으로 보수적으로 평가.
    let min_avail = env.hardware.disks.iter().map(|d| d.available_bytes).min()?;
    let gib = min_avail as f64 / GIB as f64;
    if min_avail < 20 * GIB {
        Some(CheckResult {
            id: "disk-low".into(),
            severity: Severity::Warn,
            title_ko: format!("여유 디스크가 적어요 ({gib:.1}GB)"),
            detail_ko: "20GB 미만이에요. 모델 다운로드 전에 정리해 주세요.".into(),
            recommendation: None,
        })
    } else if min_avail < 50 * GIB {
        Some(CheckResult {
            id: "disk-modest".into(),
            severity: Severity::Info,
            title_ko: format!("여유 디스크 {gib:.1}GB"),
            detail_ko: "큰 모델은 50GB 이상을 권장해요.".into(),
            recommendation: None,
        })
    } else {
        None
    }
}

fn check_gpu(env: &EnvironmentReport) -> Option<CheckResult> {
    if env.hardware.gpus.is_empty() {
        return Some(CheckResult {
            id: "gpu-cpu-only".into(),
            severity: Severity::Info,
            title_ko: "GPU를 찾지 못했어요".into(),
            detail_ko: "CPU 전용으로 동작해요. 작은 모델만 권장해요.".into(),
            recommendation: None,
        });
    }
    // VRAM 가장 큰 GPU 기준.
    let primary = env
        .hardware
        .gpus
        .iter()
        .max_by_key(|g| g.vram_bytes.unwrap_or(0))?;
    let vram = primary.vram_bytes.unwrap_or(0);
    let vram_gib = vram as f64 / GIB as f64;
    let vendor_label = match primary.vendor {
        GpuVendor::Nvidia => "NVIDIA",
        GpuVendor::Amd => "AMD",
        GpuVendor::Intel => "Intel",
        GpuVendor::Apple => "Apple",
        GpuVendor::Microsoft => "Microsoft",
        GpuVendor::Other => "기타",
    };

    if matches!(primary.vendor, GpuVendor::Nvidia) {
        if vram >= 8 * GIB {
            Some(CheckResult {
                id: "gpu-nv-good".into(),
                severity: Severity::Info,
                title_ko: format!("{vendor_label} GPU ({vram_gib:.1}GB VRAM)"),
                detail_ko: "GPU 가속이 잘 동작해요.".into(),
                recommendation: None,
            })
        } else if vram >= 4 * GIB {
            Some(CheckResult {
                id: "gpu-nv-mid".into(),
                severity: Severity::Warn,
                title_ko: format!("{vendor_label} GPU ({vram_gib:.1}GB VRAM)"),
                detail_ko: "VRAM이 8GB 미만이라 7B 이하 모델을 권장해요.".into(),
                recommendation: None,
            })
        } else {
            Some(CheckResult {
                id: "gpu-nv-low".into(),
                severity: Severity::Warn,
                title_ko: format!("{vendor_label} GPU ({vram_gib:.1}GB VRAM)"),
                detail_ko: "VRAM이 4GB 미만이라 3B 이하 모델만 권장해요.".into(),
                recommendation: None,
            })
        }
    } else if matches!(primary.vendor, GpuVendor::Apple) {
        Some(CheckResult {
            id: "gpu-apple".into(),
            severity: Severity::Info,
            title_ko: "Apple GPU".into(),
            detail_ko: "Metal 가속을 사용해요.".into(),
            recommendation: None,
        })
    } else {
        Some(CheckResult {
            id: "gpu-other".into(),
            severity: Severity::Info,
            title_ko: format!("{vendor_label} GPU"),
            detail_ko: "CPU/Vulkan/DirectML로 추론해요.".into(),
            recommendation: None,
        })
    }
}

fn check_webview2(env: &EnvironmentReport) -> Option<CheckResult> {
    if env.hardware.os.family != OsFamily::Windows {
        return None;
    }
    if env.hardware.runtimes.webview2.is_none() {
        Some(CheckResult {
            id: "webview2-missing".into(),
            severity: Severity::Error,
            title_ko: "WebView2가 설치되어 있지 않아요".into(),
            detail_ko: "LMmaster 동작에 필요해요. Microsoft 공식 사이트에서 받아 주세요.".into(),
            recommendation: Some("WebView2 런타임 설치".into()),
        })
    } else {
        None
    }
}

fn check_vc_redist(env: &EnvironmentReport) -> Option<CheckResult> {
    if env.hardware.os.family != OsFamily::Windows {
        return None;
    }
    if env.hardware.runtimes.vcredist_2022.is_none() {
        Some(CheckResult {
            id: "vc-redist-missing".into(),
            severity: Severity::Error,
            title_ko: "Visual C++ 2022 재배포 패키지가 없어요".into(),
            detail_ko: "Ollama / LM Studio 일부 빌드에 필요해요.".into(),
            recommendation: Some("VC++ 2022 재배포 설치".into()),
        })
    } else {
        None
    }
}

fn check_nvidia_driver(env: &EnvironmentReport) -> Option<CheckResult> {
    // GpuInfo.driver_version에서 NVIDIA 드라이버 버전 추출.
    let nvidia_driver = env
        .hardware
        .gpus
        .iter()
        .find(|g| matches!(g.vendor, GpuVendor::Nvidia))
        .and_then(|g| g.driver_version.as_deref());
    let has_nvidia = env
        .hardware
        .gpus
        .iter()
        .any(|g| matches!(g.vendor, GpuVendor::Nvidia));
    if !has_nvidia {
        return None;
    }
    match nvidia_driver {
        None => Some(CheckResult {
            id: "nvidia-driver-missing".into(),
            severity: Severity::Warn,
            title_ko: "NVIDIA 드라이버를 확인하지 못했어요".into(),
            detail_ko: "최신 GeForce / Studio 드라이버 설치를 권장해요.".into(),
            recommendation: Some("NVIDIA 드라이버 설치".into()),
        }),
        Some(v) => {
            // 첫 숫자 파싱 — "551.86" → 551.
            let major: Option<u32> = v.split('.').next().and_then(|s| s.parse().ok());
            if let Some(m) = major {
                if env.hardware.os.family == OsFamily::Windows && m < 530 {
                    return Some(CheckResult {
                        id: "nvidia-driver-old".into(),
                        severity: Severity::Warn,
                        title_ko: format!("NVIDIA 드라이버 {v} (오래된 버전)"),
                        detail_ko: "CUDA 12 호환 위해 530 이상을 권장해요.".into(),
                        recommendation: Some("NVIDIA 드라이버 업데이트".into()),
                    });
                }
            }
            Some(CheckResult {
                id: "nvidia-driver-ok".into(),
                severity: Severity::Info,
                title_ko: format!("NVIDIA 드라이버 {v}"),
                detail_ko: "정상 동작해요.".into(),
                recommendation: None,
            })
        }
    }
}

fn check_cuda(env: &EnvironmentReport) -> Option<CheckResult> {
    let has_nvidia = env
        .hardware
        .gpus
        .iter()
        .any(|g| matches!(g.vendor, GpuVendor::Nvidia));
    if !has_nvidia {
        return None;
    }
    if env.hardware.runtimes.cuda_toolkits.is_empty() && !env.hardware.runtimes.cuda_runtime {
        Some(CheckResult {
            id: "cuda-missing".into(),
            severity: Severity::Info,
            title_ko: "CUDA가 설치되어 있지 않아요".into(),
            detail_ko: "GPU 가속에 영향이 있을 수 있어요. CUDA 12 권장.".into(),
            recommendation: None,
        })
    } else {
        None
    }
}

fn check_vulkan(env: &EnvironmentReport) -> Option<CheckResult> {
    if env.hardware.runtimes.vulkan {
        return None;
    }
    Some(CheckResult {
        id: "vulkan-missing".into(),
        severity: Severity::Info,
        title_ko: "Vulkan을 찾지 못했어요".into(),
        detail_ko: "DirectML / CUDA / CPU로 동작해요.".into(),
        recommendation: None,
    })
}

fn check_runtimes(env: &EnvironmentReport) -> Vec<CheckResult> {
    use runtime_detector::Status;
    use shared_types::RuntimeKind;
    let mut out = Vec::new();
    for r in &env.runtimes {
        let label = match r.runtime {
            RuntimeKind::Ollama => "Ollama",
            RuntimeKind::LmStudio => "LM Studio",
            RuntimeKind::LlamaCpp => "llama.cpp",
            RuntimeKind::KoboldCpp => "KoboldCpp",
            RuntimeKind::Vllm => "vLLM",
        };
        let id_prefix = match r.runtime {
            RuntimeKind::Ollama => "ollama",
            RuntimeKind::LmStudio => "lm-studio",
            RuntimeKind::LlamaCpp => "llama-cpp",
            RuntimeKind::KoboldCpp => "kobold-cpp",
            RuntimeKind::Vllm => "vllm",
        };
        let (severity, title, detail) = match r.status {
            Status::Running => (
                Severity::Info,
                format!("{label} 사용 중"),
                "지금 바로 모델을 불러올 수 있어요.".to_string(),
            ),
            Status::Installed => (
                Severity::Info,
                format!("{label} 설치됨"),
                "실행해 두면 더 빨라요.".to_string(),
            ),
            Status::NotInstalled => (
                Severity::Info,
                format!("{label} 설치 안 됨"),
                "필요하면 설치 센터에서 받을 수 있어요.".to_string(),
            ),
            Status::Error => (
                Severity::Warn,
                format!("{label} 점검 실패"),
                r.error.clone().unwrap_or_else(|| "알 수 없는 오류".into()),
            ),
        };
        out.push(CheckResult {
            id: format!("runtime-{id_prefix}"),
            severity,
            title_ko: title,
            detail_ko: detail,
            recommendation: None,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use hardware_probe::{
        CpuInfo, DiskInfo, DiskKind, HardwareReport, MemInfo, OsFamily, OsInfo, RuntimeInfo,
    };
    use runtime_detector::{DetectResult, EnvironmentReport, Status};
    use shared_types::RuntimeKind;

    fn base_env() -> EnvironmentReport {
        EnvironmentReport {
            hardware: HardwareReport {
                os: OsInfo {
                    family: OsFamily::Windows,
                    version: "11".into(),
                    arch: "x86_64".into(),
                    kernel: "10.0".into(),
                    rosetta: None,
                    distro: None,
                    distro_version: None,
                },
                cpu: CpuInfo {
                    brand: "Intel".into(),
                    vendor_id: "GenuineIntel".into(),
                    physical_cores: 8,
                    logical_cores: 16,
                    frequency_mhz: 3000,
                },
                mem: MemInfo {
                    total_bytes: 16 * GIB,
                    available_bytes: 8 * GIB,
                },
                disks: vec![DiskInfo {
                    mount_point: "C:".into(),
                    kind: DiskKind::Ssd,
                    total_bytes: 500 * GIB,
                    available_bytes: 250 * GIB,
                }],
                gpus: vec![],
                runtimes: RuntimeInfo {
                    cuda_toolkits: vec![],
                    cuda_runtime: false,
                    vulkan: true,
                    metal: false,
                    directml: true,
                    d3d12: true,
                    rocm: false,
                    webview2: Some("128.0.2739.42".into()),
                    vcredist_2022: Some("14.40.33810".into()),
                    glibc: None,
                    libstdcpp: None,
                    vulkan_devices: None,
                },
                probed_at: "2026-04-27T00:00:00Z".into(),
                probe_ms: 100,
            },
            runtimes: vec![DetectResult {
                runtime: RuntimeKind::Ollama,
                status: Status::Running,
                version: Some("0.4.0".into()),
                endpoint: Some("http://127.0.0.1:11434".into()),
                error: None,
            }],
        }
    }

    #[test]
    fn ram_warn_below_8gb() {
        let mut env = base_env();
        env.hardware.mem.total_bytes = 4 * GIB;
        let checks = run_all(&env);
        let ram = checks.iter().find(|c| c.id == "ram-low").unwrap();
        assert_eq!(ram.severity, Severity::Warn);
    }

    #[test]
    fn ram_info_at_16gb() {
        let env = base_env();
        let checks = run_all(&env);
        let ram = checks.iter().find(|c| c.id == "ram-ok-13b").unwrap();
        assert_eq!(ram.severity, Severity::Info);
    }

    #[test]
    fn disk_warn_below_20gb() {
        let mut env = base_env();
        env.hardware.disks[0].available_bytes = 10 * GIB;
        let checks = run_all(&env);
        let d = checks.iter().find(|c| c.id == "disk-low").unwrap();
        assert_eq!(d.severity, Severity::Warn);
    }

    #[test]
    fn webview2_missing_is_error_on_windows() {
        let mut env = base_env();
        env.hardware.runtimes.webview2 = None;
        let checks = run_all(&env);
        let c = checks.iter().find(|c| c.id == "webview2-missing").unwrap();
        assert_eq!(c.severity, Severity::Error);
    }

    #[test]
    fn webview2_check_skipped_on_macos() {
        let mut env = base_env();
        env.hardware.os.family = OsFamily::Macos;
        env.hardware.runtimes.webview2 = None;
        let checks = run_all(&env);
        assert!(checks.iter().all(|c| c.id != "webview2-missing"));
    }

    #[test]
    fn ollama_running_emits_info() {
        let env = base_env();
        let checks = run_all(&env);
        let c = checks.iter().find(|c| c.id == "runtime-ollama").unwrap();
        assert_eq!(c.severity, Severity::Info);
        assert!(c.title_ko.contains("Ollama"));
    }

    #[test]
    fn no_gpu_yields_cpu_only_info() {
        let env = base_env();
        let checks = run_all(&env);
        let c = checks.iter().find(|c| c.id == "gpu-cpu-only").unwrap();
        assert_eq!(c.severity, Severity::Info);
    }
}
