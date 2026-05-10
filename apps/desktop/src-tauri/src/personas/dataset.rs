//! Personas-Korea 데이터셋 자동 다운로드.
//!
//! 정책:
//! - HuggingFace `nvidia/Nemotron-Personas-Korea` 의 main 브랜치 .parquet 파일 모두 다운.
//! - 파일 목록은 `https://huggingface.co/api/datasets/{repo}/tree/main` API로 동적 조회.
//! - 캐시 위치: `app_local_data_dir()/personas/<filename>`.
//! - 진행률은 Tauri Channel<PersonasDatasetEvent>로 스트림 (모델 다운과 동일 패턴).
//! - 다운로드는 `installer::Downloader` 재사용 → resume + sha256(없음) + retry + atomic rename.

use std::path::PathBuf;

use installer::downloader::{DownloadRequest, Downloader};
use installer::progress::DownloadEvent;
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

const HF_REPO: &str = "nvidia/Nemotron-Personas-Korea";

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PersonasDatasetError {
    #[error("내부 오류: {message}")]
    Internal { message: String },
    #[error("데이터셋 목록 조회 실패: {message}")]
    ListFailed { message: String },
    #[error("다운로드 실패: {message}")]
    DownloadFailed { message: String },
}

/// 데이터셋 다운로드 진행 이벤트 — frontend Channel<PersonasDatasetEvent> 수신.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PersonasDatasetEvent {
    /// 단계 안내 ("파일 목록 가져오는 중", "{name} 받기 시작" 등).
    Status {
        status: String,
        file_index: usize,
        file_total: usize,
    },
    /// 현재 파일 다운 진행률.
    Progress {
        completed_bytes: u64,
        total_bytes: u64,
        speed_bps: u64,
    },
    /// 모든 파일 다운 완료.
    Completed { file_count: usize, total_bytes: u64 },
    /// 실패 — 사용자에게 메시지 노출.
    Failed { message: String },
}

/// 사용자 향 데이터셋 상태.
#[derive(Debug, Clone, Serialize)]
pub struct PersonasDatasetStatus {
    /// 데이터셋이 사용 가능한 상태인지 (.parquet 파일 1개 이상 + 합산 100MB+).
    pub installed: bool,
    pub size_bytes: u64,
    pub file_count: usize,
}

fn personas_dir(app: &AppHandle) -> Result<PathBuf, PersonasDatasetError> {
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| PersonasDatasetError::Internal {
            message: format!("app_local_data_dir 실패: {e}"),
        })?
        .join("personas");
    Ok(dir)
}

/// HF 데이터셋 트리 API 응답 (필요한 필드만).
#[derive(Deserialize)]
struct HfTreeEntry {
    #[serde(rename = "type")]
    kind: String,
    path: String,
    #[serde(default)]
    size: u64,
}

/// HF API로 main 브랜치의 .parquet 파일 목록 조회.
async fn list_dataset_parquets() -> Result<Vec<(String, u64)>, PersonasDatasetError> {
    let url = format!("https://huggingface.co/api/datasets/{HF_REPO}/tree/main?recursive=true");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| PersonasDatasetError::Internal {
            message: format!("HTTP 클라이언트 생성 실패: {e}"),
        })?;
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| PersonasDatasetError::ListFailed {
            message: format!("HF API 요청 실패: {e}"),
        })?;
    if !resp.status().is_success() {
        return Err(PersonasDatasetError::ListFailed {
            message: format!("HF API HTTP {}", resp.status()),
        });
    }
    let entries: Vec<HfTreeEntry> =
        resp.json()
            .await
            .map_err(|e| PersonasDatasetError::ListFailed {
                message: format!("HF API 응답 파싱 실패: {e}"),
            })?;

    let parquets: Vec<(String, u64)> = entries
        .into_iter()
        .filter(|e| e.kind == "file" && e.path.to_lowercase().ends_with(".parquet"))
        .map(|e| (e.path, e.size))
        .collect();

    if parquets.is_empty() {
        return Err(PersonasDatasetError::ListFailed {
            message: "데이터셋 .parquet 파일을 찾지 못했어요. HF 측 구조 변경 가능성".into(),
        });
    }
    Ok(parquets)
}

/// 현재 캐시된 데이터셋 상태 조회.
#[tauri::command]
pub fn get_personas_dataset_status(
    app: AppHandle,
) -> Result<PersonasDatasetStatus, PersonasDatasetError> {
    let dir = personas_dir(&app)?;
    if !dir.exists() {
        return Ok(PersonasDatasetStatus {
            installed: false,
            size_bytes: 0,
            file_count: 0,
        });
    }
    let mut total_size: u64 = 0;
    let mut file_count: usize = 0;
    let entries = std::fs::read_dir(&dir).map_err(|e| PersonasDatasetError::Internal {
        message: format!("read_dir 실패: {e}"),
    })?;
    for entry in entries.flatten() {
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                let name = entry
                    .file_name()
                    .into_string()
                    .unwrap_or_default()
                    .to_lowercase();
                if name.ends_with(".parquet") {
                    total_size += meta.len();
                    file_count += 1;
                }
            }
        }
    }
    // 100MB+ 또는 파일 1개 이상 — 후자는 작은 분할도 사용 가능.
    let installed = file_count >= 1 && total_size > 50_000_000;
    Ok(PersonasDatasetStatus {
        installed,
        size_bytes: total_size,
        file_count,
    })
}

/// Personas-Korea 데이터셋 자동 다운로드.
/// 파일 목록 조회 → 누락된 .parquet 파일을 순차적으로 다운로드.
#[tauri::command]
pub async fn download_personas_dataset(
    app: AppHandle,
    channel: Channel<PersonasDatasetEvent>,
) -> Result<(), PersonasDatasetError> {
    let dir = personas_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| PersonasDatasetError::Internal {
        message: format!("personas 디렉터리 생성 실패: {e}"),
    })?;

    let _ = channel.send(PersonasDatasetEvent::Status {
        status: "데이터셋 파일 목록을 가져오고 있어요".into(),
        file_index: 0,
        file_total: 0,
    });

    let files = list_dataset_parquets().await.inspect_err(|e| {
        let _ = channel.send(PersonasDatasetEvent::Failed {
            message: e.to_string(),
        });
    })?;
    let total_files = files.len();
    let total_bytes: u64 = files.iter().map(|(_, s)| *s).sum();

    let cancel = CancellationToken::new();
    let downloader = Downloader::new().map_err(|e| PersonasDatasetError::Internal {
        message: format!("다운로더 초기화 실패: {e}"),
    })?;

    let mut completed_files: usize = 0;
    for (idx, (path, size)) in files.iter().enumerate() {
        let filename = path.rsplit('/').next().unwrap_or(path).to_string();
        let final_path = dir.join(&filename);

        // 이미 동일 파일 + 크기 일치 시 skip (resume 효과).
        if final_path.exists() {
            if let Ok(meta) = std::fs::metadata(&final_path) {
                if meta.len() == *size && *size > 0 {
                    completed_files += 1;
                    let _ = channel.send(PersonasDatasetEvent::Status {
                        status: format!("{filename} (이미 받음)"),
                        file_index: idx + 1,
                        file_total: total_files,
                    });
                    continue;
                }
            }
        }

        let _ = channel.send(PersonasDatasetEvent::Status {
            status: format!("{filename} 받는 중"),
            file_index: idx + 1,
            file_total: total_files,
        });

        let url = format!("https://huggingface.co/datasets/{HF_REPO}/resolve/main/{path}");
        let req = DownloadRequest {
            url,
            final_path,
            expected_sha256: None, // HF API의 oid는 git LFS 해시라 sha256과 다름 — 검증 skip.
            size_hint: if *size > 0 { Some(*size) } else { None },
            max_retries: None,
            auth_header: None,
        };

        // ProgressSink는 closure로 — Channel send.
        let channel_for_sink = channel.clone();
        let sink = move |event: DownloadEvent| {
            if let DownloadEvent::Progress {
                downloaded,
                total,
                speed_bps,
            } = event
            {
                let _ = channel_for_sink.send(PersonasDatasetEvent::Progress {
                    completed_bytes: downloaded,
                    total_bytes: total.unwrap_or(0),
                    speed_bps,
                });
            }
        };

        downloader
            .download(&req, &cancel, &sink)
            .await
            .map_err(|e| {
                let msg = format!("{filename}: {e}");
                let _ = channel.send(PersonasDatasetEvent::Failed {
                    message: msg.clone(),
                });
                PersonasDatasetError::DownloadFailed { message: msg }
            })?;
        completed_files += 1;
    }

    let _ = channel.send(PersonasDatasetEvent::Completed {
        file_count: completed_files,
        total_bytes,
    });
    tracing::info!(
        files = completed_files,
        bytes = total_bytes,
        "Personas-Korea 데이터셋 다운로드 완료"
    );
    Ok(())
}
