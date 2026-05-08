//! Phase 8'.c.4 (ADR-0066) Q1 helper — 모델 폴더 열기 IPC.
//!
//! 사용자가 LMmaster로 받은 GGUF 파일을 클라우드 GPU에 *손으로* 옮기거나 백업할 때
//! 폴더 위치를 OS 파일 탐색기로 한 번에 열어주는 도우미.
//!
//! 정책:
//! - 폴더가 없으면 생성 (mkdir -p) + 그 다음 열기 — 첫 사용자가 빈 상태에서 헤매지 않게.
//! - cross-platform: Windows = explorer, macOS = open, Linux = xdg-open.
//! - 실패 시 한국어 에러 메시지 (i18n은 frontend가 분기).

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ModelsDirError {
    #[error("폴더를 만들지 못했어요: {message}")]
    Create { message: String },

    #[error("폴더를 열지 못했어요: {message}")]
    Open { message: String },

    #[error("내부 오류: {message}")]
    Internal { message: String },
}

/// LMmaster 모델 폴더 경로 — `app_local_data_dir/models`.
fn models_dir(app: &AppHandle) -> Result<PathBuf, ModelsDirError> {
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| ModelsDirError::Internal {
            message: format!("app_local_data_dir 해결 실패: {e}"),
        })?;
    Ok(dir.join("models"))
}

/// 모델 폴더 경로 반환 — UI 표시용.
#[tauri::command]
pub fn get_models_dir(app: AppHandle) -> Result<String, ModelsDirError> {
    let p = models_dir(&app)?;
    Ok(p.display().to_string())
}

/// 모델 폴더를 OS 파일 탐색기로 열기. 없으면 생성.
#[tauri::command]
pub fn open_models_dir(app: AppHandle) -> Result<(), ModelsDirError> {
    let p = models_dir(&app)?;
    if !p.exists() {
        std::fs::create_dir_all(&p).map_err(|e| ModelsDirError::Create {
            message: e.to_string(),
        })?;
    }
    open_path(&p).map_err(|e| ModelsDirError::Open {
        message: e.to_string(),
    })?;
    tracing::info!(path = %p.display(), "모델 폴더 열기");
    Ok(())
}

#[cfg(target_os = "windows")]
fn open_path(p: &std::path::Path) -> std::io::Result<()> {
    std::process::Command::new("explorer.exe").arg(p).spawn()?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn open_path(p: &std::path::Path) -> std::io::Result<()> {
    std::process::Command::new("open").arg(p).spawn()?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn open_path(p: &std::path::Path) -> std::io::Result<()> {
    std::process::Command::new("xdg-open").arg(p).spawn()?;
    Ok(())
}
