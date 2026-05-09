//! HuggingFace Personal Access Token 설정 IPC.
//!
//! 정책:
//! - 토큰은 settings.json의 `hf_access_token`에 평문 저장 (로컬 파일 시스템 보호에 의존, v1).
//! - gated 모델(naver-hyperclovax 등) 다운로드 시 Authorization: Bearer {token} 헤더로 주입.
//! - 토큰 미설정 → 공개 모델만 다운 가능. 설정 시 gated 모델도 다운 가능.
//! - 토큰은 접두사만 UI에 노출 (`hf_...` 형식 → `hf_...` 8자 + "****" 마스크).

use serde::Serialize;
use tauri::{AppHandle, Manager};
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum HfTokenError {
    #[error("설정 저장 실패: {message}")]
    Save { message: String },
    #[error("내부 오류: {message}")]
    Internal { message: String },
}

fn dir(app: &AppHandle) -> Result<std::path::PathBuf, HfTokenError> {
    app.path()
        .app_local_data_dir()
        .map_err(|e| HfTokenError::Internal {
            message: format!("app_local_data_dir 해결 실패: {e}"),
        })
}

/// 저장된 HF 토큰 prefix만 반환. None이면 미설정.
/// 보안: 평문 전체 반환 안 함 — UI는 prefix로 등록 여부만 확인.
#[tauri::command]
pub fn get_hf_token_prefix(app: AppHandle) -> Result<Option<String>, HfTokenError> {
    let s = super::UserSettings::load(&dir(&app)?);
    Ok(s.hf_access_token.map(|t| {
        if t.len() > 10 {
            format!("{}****", &t[..10])
        } else {
            "****".to_string()
        }
    }))
}

/// HF 토큰 저장. 빈 문자열 전달 시 토큰 제거와 동일.
#[tauri::command]
pub fn set_hf_token(app: AppHandle, token: String) -> Result<(), HfTokenError> {
    let d = dir(&app)?;
    let mut s = super::UserSettings::load(&d);
    s.hf_access_token = if token.trim().is_empty() {
        None
    } else {
        Some(token.trim().to_string())
    };
    s.save(&d).map_err(|e| HfTokenError::Save {
        message: e.to_string(),
    })?;
    tracing::info!(
        has_token = s.hf_access_token.is_some(),
        "HuggingFace 토큰 갱신됨"
    );
    Ok(())
}

/// HF 토큰 제거.
#[tauri::command]
pub fn clear_hf_token(app: AppHandle) -> Result<(), HfTokenError> {
    let d = dir(&app)?;
    let mut s = super::UserSettings::load(&d);
    s.hf_access_token = None;
    s.save(&d).map_err(|e| HfTokenError::Save {
        message: e.to_string(),
    })?;
    tracing::info!("HuggingFace 토큰 제거됨");
    Ok(())
}

/// 내부 헬퍼 — model_pull이 Authorization 헤더 값을 얻을 때 사용.
/// "Bearer {token}" 형식. 토큰 미설정 시 None.
pub fn auth_header_value(app: &AppHandle) -> Option<String> {
    let d = app.path().app_local_data_dir().ok()?;
    let s = super::UserSettings::load(&d);
    s.hf_access_token
        .filter(|t| !t.is_empty())
        .map(|t| format!("Bearer {t}"))
}
