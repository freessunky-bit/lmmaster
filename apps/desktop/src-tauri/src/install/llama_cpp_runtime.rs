//! Phase 13'.h.2.f.1 — llama-server binary 자동 install.
//!
//! 정책:
//! - GPU 자동 감지 (`runtime_detector::probe_environment`) → 적합 ggerganov/llama.cpp Release asset 매핑.
//! - `installer::Downloader` 재사용 (atomic + retry + .partial resume + .no_proxy).
//! - `installer::extract` (Zip)로 풀고 binary 자동 search.
//! - 자동 settings.json 등록 + env 주입.
//! - 사용자 측 마찰 0 — Settings에서 button 1번만 누르면 OK.

use std::path::{Path, PathBuf};

use hardware_probe::GpuVendor;
use installer::downloader::{DownloadRequest, Downloader};
use installer::extract::{extract, ExtractFormat};
use installer::progress::DownloadEvent;
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

const RELEASES_API: &str = "https://api.github.com/repos/ggerganov/llama.cpp/releases/latest";

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum LlamaInstallError {
    #[error("최신 빌드 정보를 받지 못했어요: {message}")]
    ReleaseFetch { message: String },

    #[error("이 GPU/OS 조합에 맞는 빌드를 찾지 못했어요. 수동 등록을 사용해 주세요.")]
    NoMatchingAsset,

    #[error("빌드 다운로드에 실패했어요: {message}")]
    Download { message: String },

    #[error("압축 풀기에 실패했어요: {message}")]
    Extract { message: String },

    #[error("llama-server 실행 파일을 압축 안에서 찾지 못했어요. 다른 빌드를 시도해 주세요.")]
    BinaryNotFound,

    #[error("설정을 저장하지 못했어요: {message}")]
    SaveSettings { message: String },

    #[error("내부 오류: {message}")]
    Internal { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum LlamaInstallEvent {
    Status {
        status: String,
    },
    Progress {
        completed_bytes: u64,
        total_bytes: u64,
        speed_bps: u64,
    },
    Completed {
        binary_path: String,
    },
    Failed {
        message: String,
    },
}

#[derive(Debug, Deserialize)]
struct Release {
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    size: u64,
    browser_download_url: String,
}

/// 메인 IPC — 사용자가 Settings에서 "자동 셋업할게요" 버튼 누르면 호출.
#[tauri::command]
pub async fn install_llama_cpp_runtime(
    app: AppHandle,
    channel: Channel<LlamaInstallEvent>,
) -> Result<String, LlamaInstallError> {
    let cancel = CancellationToken::new();

    // 1. GPU 감지.
    let _ = channel.send(LlamaInstallEvent::Status {
        status: "GPU와 환경을 감지하고 있어요…".into(),
    });
    let env = runtime_detector::probe_environment().await;
    let gpu_vendor = env
        .hardware
        .gpus
        .iter()
        .map(|g| g.vendor)
        .find(|v| {
            matches!(
                v,
                GpuVendor::Nvidia | GpuVendor::Amd | GpuVendor::Intel | GpuVendor::Apple
            )
        })
        .unwrap_or(GpuVendor::Other);
    tracing::info!(?gpu_vendor, "llama-cpp 자동 install: GPU 감지 결과");

    // 2. cache 디렉터리.
    let local_data = app
        .path()
        .app_local_data_dir()
        .map_err(|e| LlamaInstallError::Internal {
            message: format!("app_local_data_dir 해결 실패: {e}"),
        })?;
    let cache_dir = local_data.join("runtimes").join("llama-cpp");
    std::fs::create_dir_all(&cache_dir).map_err(|e| LlamaInstallError::Internal {
        message: format!("디렉터리 생성 실패: {e}"),
    })?;
    // Phase 13'.h.2.f.2 — 옛 .zip 잔여물 일괄 정리 (v0.6.0 cudart-* 등).
    // 다음 install이 받을 zip과 충돌 위험 + 디스크 누적 회피.
    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        let mut cleaned = 0;
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().map(|e| e == "zip").unwrap_or(false)
                && std::fs::remove_file(&p).is_ok()
            {
                cleaned += 1;
            }
        }
        if cleaned > 0 {
            tracing::info!(cleaned, "옛 .zip 잔여물 정리 완료");
        }
    }

    // 3. GitHub API에서 최신 release fetch.
    let _ = channel.send(LlamaInstallEvent::Status {
        status: "GitHub에서 최신 빌드 정보를 받고 있어요…".into(),
    });
    let release = fetch_latest_release()
        .await
        .map_err(|message| LlamaInstallError::ReleaseFetch { message })?;

    // 4. asset selector.
    let asset =
        pick_asset(&release.assets, gpu_vendor).ok_or(LlamaInstallError::NoMatchingAsset)?;
    let _ = channel.send(LlamaInstallEvent::Status {
        status: format!(
            "선택: {} ({:.1} MB)",
            asset.name,
            asset.size as f64 / 1_048_576.0
        ),
    });

    // 5. 다운로드.
    let zip_path = cache_dir.join(&asset.name);
    let downloader = Downloader::new().map_err(|e| LlamaInstallError::Internal {
        message: format!("다운로더 초기화 실패: {e}"),
    })?;
    let req = DownloadRequest {
        url: asset.browser_download_url.clone(),
        final_path: zip_path.clone(),
        // GitHub Releases는 sha256 별도 제공 X — 검증 skip.
        expected_sha256: None,
        size_hint: Some(asset.size),
        max_retries: None,
        auth_header: None, // GitHub Releases는 공개 URL.
    };
    let channel_dl = channel.clone();
    let sink = move |ev: DownloadEvent| {
        if let DownloadEvent::Progress {
            downloaded,
            total,
            speed_bps,
        } = ev
        {
            let _ = channel_dl.send(LlamaInstallEvent::Progress {
                completed_bytes: downloaded,
                total_bytes: total.unwrap_or(0),
                speed_bps,
            });
        }
    };
    downloader
        .download(&req, &cancel, &sink)
        .await
        .map_err(|e| LlamaInstallError::Download {
            message: e.to_string(),
        })?;

    // 6. 압축 풀기. 이전 install이 있으면 재설치 위해 정리.
    let _ = channel.send(LlamaInstallEvent::Status {
        status: "압축을 풀고 있어요…".into(),
    });
    let extract_dir = cache_dir.join("bin");
    let _ = std::fs::remove_dir_all(&extract_dir);
    extract(&zip_path, &extract_dir, ExtractFormat::Zip, &cancel)
        .await
        .map_err(|e| LlamaInstallError::Extract {
            message: e.to_string(),
        })?;

    // 7. binary 자동 search (zip 안에 보통 한 단계 nested 디렉터리).
    let binary_filename = if cfg!(windows) {
        "llama-server.exe"
    } else {
        "llama-server"
    };
    let binary =
        find_in_dir(&extract_dir, binary_filename).ok_or(LlamaInstallError::BinaryNotFound)?;

    // 8. settings.json 저장 + env 주입.
    let mut s = crate::settings::UserSettings::load(&local_data);
    s.llama_server_path = Some(binary.display().to_string());
    s.save(&local_data)
        .map_err(|e| LlamaInstallError::SaveSettings {
            message: e.to_string(),
        })?;
    std::env::set_var("LMMASTER_LLAMA_SERVER_PATH", binary.display().to_string());

    // 9. zip 정리 (디스크 절약).
    let _ = std::fs::remove_file(&zip_path);

    let path_str = binary.display().to_string();
    tracing::info!(path = %path_str, "llama-cpp 자동 install 완료");
    let _ = channel.send(LlamaInstallEvent::Completed {
        binary_path: path_str.clone(),
    });
    Ok(path_str)
}

async fn fetch_latest_release() -> Result<Release, String> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .user_agent(format!("LMmaster/{}", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(RELEASES_API)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("HTTP {status}"));
    }
    resp.json::<Release>().await.map_err(|e| e.to_string())
}

fn pick_asset(assets: &[Asset], vendor: GpuVendor) -> Option<&Asset> {
    let os_keyword = if cfg!(windows) {
        "win"
    } else if cfg!(target_os = "linux") {
        "ubuntu"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        return None;
    };

    // 우선순위: vendor 별 최선 → vendor agnostic vulkan → cpu fallback.
    let preferred: Vec<&str> = match vendor {
        GpuVendor::Nvidia => vec!["cuda-12", "cuda12", "cuda"],
        GpuVendor::Amd | GpuVendor::Intel => vec!["vulkan"],
        GpuVendor::Apple => vec!["arm64", "metal"],
        _ => vec!["vulkan", "cpu"],
    };
    let fallback: Vec<&str> = vec!["vulkan", "cpu"];

    let candidates: Vec<&str> = preferred
        .iter()
        .chain(fallback.iter())
        .copied()
        .collect::<Vec<_>>();

    for keyword in candidates {
        if let Some(a) = assets.iter().find(|a| {
            // Phase 13'.h.2.f.1 fix — `cudart-...` zip은 CUDA 런타임 DLL만 담고 binary 없음.
            // ggerganov/llama.cpp 정상 binary asset은 항상 `llama-b<버전>-` prefix.
            a.name.starts_with("llama-b")
                && a.name.contains(os_keyword)
                && a.name.contains(keyword)
                && (a.name.ends_with(".zip") || a.name.ends_with(".tar.gz"))
                && (a.name.contains("x64") || a.name.contains("x86_64") || a.name.contains("arm64"))
        }) {
            return Some(a);
        }
    }
    None
}

fn find_in_dir(dir: &Path, filename: &str) -> Option<PathBuf> {
    walkdir::WalkDir::new(dir)
        .max_depth(5)
        .into_iter()
        .filter_map(|e| e.ok())
        .find(|e| e.file_type().is_file() && e.file_name() == filename)
        .map(|e| e.path().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(name: &str) -> Asset {
        Asset {
            name: name.into(),
            size: 100_000_000,
            browser_download_url: format!(
                "https://github.com/ggerganov/llama.cpp/releases/download/b6000/{name}"
            ),
        }
    }

    #[test]
    fn pick_nvidia_prefers_cuda() {
        let assets = vec![
            asset("llama-b6000-bin-win-cpu-x64.zip"),
            asset("llama-b6000-bin-win-vulkan-x64.zip"),
            asset("llama-b6000-bin-win-cuda-12.4-x64.zip"),
        ];
        let picked = pick_asset(&assets, GpuVendor::Nvidia).unwrap();
        assert!(picked.name.contains("cuda"), "got: {}", picked.name);
    }

    #[test]
    fn pick_amd_prefers_vulkan() {
        let assets = vec![
            asset("llama-b6000-bin-win-cpu-x64.zip"),
            asset("llama-b6000-bin-win-vulkan-x64.zip"),
        ];
        let picked = pick_asset(&assets, GpuVendor::Amd).unwrap();
        assert!(picked.name.contains("vulkan"));
    }

    #[test]
    fn pick_unknown_falls_back_to_cpu() {
        let assets = vec![asset("llama-b6000-bin-win-cpu-x64.zip")];
        let picked = pick_asset(&assets, GpuVendor::Other).unwrap();
        assert!(picked.name.contains("cpu"));
    }

    #[test]
    fn pick_returns_none_on_empty() {
        let assets: Vec<Asset> = vec![];
        let picked = pick_asset(&assets, GpuVendor::Nvidia);
        assert!(picked.is_none());
    }

    #[test]
    fn pick_skips_unrelated_assets() {
        let assets = vec![
            asset("llama-b6000-bin-macos-arm64.zip"),
            asset("source.tar.gz"),
        ];
        let picked = pick_asset(&assets, GpuVendor::Nvidia);
        // Windows에서 macOS asset은 매칭 X.
        if cfg!(windows) {
            assert!(picked.is_none() || !picked.unwrap().name.contains("macos"));
        }
    }

    #[test]
    fn pick_skips_cudart_runtime_zip() {
        // 회귀 가드 — cudart-*.zip은 CUDA 런타임 DLL만 담고 llama-server.exe 없음.
        // pick_asset은 항상 `llama-b<버전>-` prefix asset만 매칭해야 함.
        let assets = vec![
            asset("cudart-llama-bin-win-cuda-12.4-x64.zip"),
            asset("llama-b9072-bin-win-cuda-12.4-x64.zip"),
        ];
        let picked = pick_asset(&assets, GpuVendor::Nvidia).unwrap();
        assert!(
            picked.name.starts_with("llama-b"),
            "picked: {}",
            picked.name
        );
        assert!(!picked.name.starts_with("cudart"));
    }
}
