//! Phase 13'.h.2.e.1 — LlamaCpp binary path settings IPC.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use thiserror::Error;

use crate::path_tokens::PathTokenRegistry;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum LlamaServerSettingsError {
    #[error("파일 선택 토큰이 만료됐거나 잘못됐어요. 다시 선택해 주세요.")]
    InvalidToken,

    #[error("설정 검증 실패: {message}")]
    Validation { message: String },

    #[error("설정 저장 실패: {message}")]
    Save { message: String },

    #[error("내부 오류: {message}")]
    Internal { message: String },
}

/// 현재 저장된 binary path 반환 — `null`이면 미설정.
///
/// 정책 (v0.6.4 fix — Settings 재진입 시 idle 회귀 회피):
/// 1. settings.json `llama_server_path` 우선.
/// 2. 1번이 None이면 `LMMASTER_LLAMA_SERVER_PATH` env 변수 fallback.
/// 3. 둘 다 없으면 None — 사용자가 미등록 상태.
///
/// env 변수는 `apply_startup_env` 또는 `set_llama_server_path`가 set한 값이라
/// settings.json read가 어떤 이유로 fail해도 *같은 process 내에선 saved 상태 유지*.
#[tauri::command]
pub fn get_llama_server_path(app: AppHandle) -> Result<Option<String>, LlamaServerSettingsError> {
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| LlamaServerSettingsError::Internal {
            message: format!("app_local_data_dir 해결 실패: {e}"),
        })?;
    let s = super::UserSettings::load(&dir);
    if let Some(p) = s.llama_server_path {
        if !p.is_empty() {
            return Ok(Some(p));
        }
    }
    // Fallback: env 변수.
    if let Ok(env_path) = std::env::var("LMMASTER_LLAMA_SERVER_PATH") {
        if !env_path.is_empty() {
            return Ok(Some(env_path));
        }
    }
    Ok(None)
}

/// 사용자 file picker로 받은 token을 raw path로 resolve + 검증 + settings.json 저장 + env 주입.
#[tauri::command]
pub async fn set_llama_server_path(
    app: AppHandle,
    path_tokens: State<'_, Arc<PathTokenRegistry>>,
    path_token: String,
) -> Result<(), LlamaServerSettingsError> {
    let resolved: PathBuf = path_tokens
        .resolve(&path_token)
        .await
        .map_err(|_| LlamaServerSettingsError::InvalidToken)?;

    let validated = super::validate_binary_path(&resolved)
        .map_err(|message| LlamaServerSettingsError::Validation { message })?;

    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| LlamaServerSettingsError::Internal {
            message: format!("app_local_data_dir 해결 실패: {e}"),
        })?;

    let mut s = super::UserSettings::load(&dir);
    s.llama_server_path = Some(validated.display().to_string());
    s.save(&dir).map_err(|e| LlamaServerSettingsError::Save {
        message: e.to_string(),
    })?;

    // 즉시 env 주입 — 사용자가 Settings 저장 후 바로 chat 가능.
    std::env::set_var(
        "LMMASTER_LLAMA_SERVER_PATH",
        validated.display().to_string(),
    );
    tracing::info!(path = %validated.display(), "llama-server path 등록 + env 주입");

    Ok(())
}

/// 저장된 path 삭제 — 사용자가 "초기화"를 눌렀을 때.
#[tauri::command]
pub fn clear_llama_server_path(app: AppHandle) -> Result<(), LlamaServerSettingsError> {
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| LlamaServerSettingsError::Internal {
            message: format!("app_local_data_dir 해결 실패: {e}"),
        })?;
    let mut s = super::UserSettings::load(&dir);
    s.llama_server_path = None;
    s.save(&dir).map_err(|e| LlamaServerSettingsError::Save {
        message: e.to_string(),
    })?;
    std::env::remove_var("LMMASTER_LLAMA_SERVER_PATH");
    tracing::info!("llama-server path 초기화 + env 제거");
    Ok(())
}
