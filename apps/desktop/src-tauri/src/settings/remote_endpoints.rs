//! 원격 LMmaster 게이트웨이 연결 관리 IPC.
//!
//! 정책:
//! - 연결 정보(alias + base_url + api_key)는 settings.json의 `remote_endpoints` 배열에 저장.
//! - `test_remote_endpoint` — `/models` 엔드포인트 호출로 연결 유효성 + 모델 목록 확인.
//! - `list_remote_models` — 저장된 모든 연결에서 모델 목록 조회 (채팅 드롭다운용).
//! - API 키는 평문 저장 (로컬 settings.json, 사용자 파일 시스템 보호에 의존 — v1 정책).

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

use super::RemoteEndpoint;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RemoteEndpointError {
    #[error("설정 저장 실패: {message}")]
    Save { message: String },
    #[error("연결 테스트 실패: {message}")]
    TestFailed { message: String },
    #[error("연결을 찾을 수 없어요 (id={id})")]
    NotFound { id: String },
    #[error("내부 오류: {message}")]
    Internal { message: String },
}

/// 원격 서버에서 조회한 모델 1개.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteModelInfo {
    /// "remote::{endpoint_id}::{model_id}" 형태 — Chat.tsx 드롭다운 runtimeId.
    pub runtime_id: String,
    pub endpoint_id: String,
    pub endpoint_alias: String,
    pub model_id: String,
    /// 드롭다운 표시명: "{alias} · {model_id}".
    pub display_name: String,
}

// ── 내부 helper ──────────────────────────────────────────────────────

fn dir(app: &AppHandle) -> Result<std::path::PathBuf, RemoteEndpointError> {
    app.path()
        .app_local_data_dir()
        .map_err(|e| RemoteEndpointError::Internal {
            message: format!("app_local_data_dir 해결 실패: {e}"),
        })
}

fn load(app: &AppHandle) -> Result<super::UserSettings, RemoteEndpointError> {
    Ok(super::UserSettings::load(&dir(app)?))
}

fn save(app: &AppHandle, s: &super::UserSettings) -> Result<(), RemoteEndpointError> {
    s.save(&dir(app)?).map_err(|e| RemoteEndpointError::Save {
        message: e.to_string(),
    })
}

// ── OpenAI /v1/models 응답 ──────────────────────────────────────────

#[derive(Deserialize)]
struct ModelListResponse {
    data: Vec<ModelItem>,
}

#[derive(Deserialize)]
struct ModelItem {
    id: String,
}

async fn fetch_models(
    base_url: &str,
    api_key: &str,
) -> Result<Vec<String>, RemoteEndpointError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| RemoteEndpointError::Internal {
            message: e.to_string(),
        })?;

    // base_url은 "/v1" 포함 (예: "http://192.168.1.10:14964/v1").
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|e| RemoteEndpointError::TestFailed {
            message: format!("연결 실패: {e}"),
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let msg = match status.as_u16() {
            401 | 403 => "API 키가 올바르지 않아요. 키를 다시 확인해 주세요.".to_string(),
            _ => format!("HTTP {status}: {body}"),
        };
        return Err(RemoteEndpointError::TestFailed { message: msg });
    }

    let list = resp
        .json::<ModelListResponse>()
        .await
        .map_err(|e| RemoteEndpointError::TestFailed {
            message: format!("응답 파싱 실패: {e}"),
        })?;

    Ok(list.data.into_iter().map(|m| m.id).collect())
}

// ── IPC commands ──────────────────────────────────────────────────────

/// 저장된 원격 연결 목록 반환.
#[tauri::command]
pub fn list_remote_endpoints(app: AppHandle) -> Result<Vec<RemoteEndpoint>, RemoteEndpointError> {
    Ok(load(&app)?.remote_endpoints)
}

/// 원격 연결 추가 + 저장. 저장 후 생성된 RemoteEndpoint 반환.
#[tauri::command]
pub fn add_remote_endpoint(
    app: AppHandle,
    alias: String,
    base_url: String,
    api_key: String,
) -> Result<RemoteEndpoint, RemoteEndpointError> {
    let mut s = load(&app)?;
    let ep = RemoteEndpoint {
        id: Uuid::new_v4().to_string(),
        alias,
        base_url,
        api_key,
        created_at: OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| String::new()),
    };
    s.remote_endpoints.push(ep.clone());
    save(&app, &s)?;
    tracing::info!(id = %ep.id, alias = %ep.alias, "원격 엔드포인트 추가됨");
    Ok(ep)
}

/// 원격 연결 삭제. 존재하지 않으면 NotFound.
#[tauri::command]
pub fn remove_remote_endpoint(app: AppHandle, id: String) -> Result<(), RemoteEndpointError> {
    let mut s = load(&app)?;
    let before = s.remote_endpoints.len();
    s.remote_endpoints.retain(|e| e.id != id);
    if s.remote_endpoints.len() == before {
        return Err(RemoteEndpointError::NotFound { id });
    }
    save(&app, &s)?;
    tracing::info!(%id, "원격 엔드포인트 삭제됨");
    Ok(())
}

/// 연결 테스트 — /v1/models 호출 + 모델 목록 반환.
/// 성공: 사용 가능한 model_id 목록. 실패: TestFailed 에러.
#[tauri::command]
pub async fn test_remote_endpoint(
    base_url: String,
    api_key: String,
) -> Result<Vec<String>, RemoteEndpointError> {
    fetch_models(&base_url, &api_key).await
}

/// 저장된 모든 원격 연결에서 모델 목록 조회 — Chat.tsx 드롭다운용.
/// 연결 실패한 엔드포인트는 조용히 건너뜀 (best-effort).
#[tauri::command]
pub async fn list_all_remote_models(app: AppHandle) -> Vec<RemoteModelInfo> {
    let endpoints = match list_remote_endpoints(app) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let mut result = Vec::new();
    for ep in endpoints {
        match fetch_models(&ep.base_url, &ep.api_key).await {
            Ok(models) => {
                for model_id in models {
                    result.push(RemoteModelInfo {
                        runtime_id: format!("remote::{}::{}", ep.id, model_id),
                        endpoint_id: ep.id.clone(),
                        endpoint_alias: ep.alias.clone(),
                        model_id: model_id.clone(),
                        display_name: format!("{} · {}", ep.alias, model_id),
                    });
                }
            }
            Err(e) => {
                tracing::warn!(
                    endpoint_id = %ep.id,
                    alias = %ep.alias,
                    error = %e,
                    "원격 모델 조회 실패 — 건너뜀"
                );
            }
        }
    }
    result
}
