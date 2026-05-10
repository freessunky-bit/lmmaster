//! Phase 13'.h.2.e.1 (ADR-0051 후속) — 사용자 settings persistence.
//!
//! 정책:
//! - `app_local_data_dir()/settings.json`에 단순 JSON 저장 (Tauri tauri-plugin-store 도입 부담 회피).
//! - 첫 진입 시 `llama_server_path`만. 후속 settings 추가는 같은 파일에 키 추가.
//! - App startup 시 read + `LMMASTER_LLAMA_SERVER_PATH` env 주입 (chat::start_chat 분기와 자동 호환).
//! - 사용자 `Settings` UI에서 file picker → IPC `set_llama_server_path(path_token)` →
//!   backend가 path_token resolve + raw path 저장 + 즉시 std::env::set_var.
//!
//! Phase 8'.c.4 (ADR-0066) — `gateway_allow_external` 추가. 게이트웨이가 0.0.0.0 바인딩하도록
//! startup env 주입. 변경 후 게이트웨이 재시작 필요 (자동 hot-restart는 v1.x 이월).

pub mod gateway;
pub mod hf_token;
pub mod llama_server;
pub mod models;
pub mod remote_endpoints;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const SETTINGS_FILENAME: &str = "settings.json";

/// 원격 LMmaster 게이트웨이 연결 정보 — 사용자 B가 사용자 A의 모델을 쓸 때.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteEndpoint {
    pub id: String,
    /// 사용자가 붙이는 별명 (예: "진우 PC").
    pub alias: String,
    /// 원격 게이트웨이 base URL — "/v1" 포함 (예: "http://192.168.1.10:14964/v1").
    pub base_url: String,
    /// 사용자 A가 발급한 LAN API 키.
    pub api_key: String,
    pub created_at: String,
}

/// 사용자 settings 스키마. `serde(default)`로 부분 기록 호환 — 누락 키는 None.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserSettings {
    /// llama-server binary 절대 경로 — Phase 13'.h.2.e.1.
    /// None이면 chat::start_chat이 `LlamaServerNotConfigured`로 한국어 안내.
    #[serde(default)]
    pub llama_server_path: Option<String>,

    /// 게이트웨이 0.0.0.0 바인딩 허용 — Phase 8'.c.4 (ADR-0066).
    /// false (default) → 127.0.0.1 only. true → 사내망의 다른 기기에서도 호출 가능.
    /// 변경 후 게이트웨이 재시작 필요 (자동 hot-restart는 v1.x).
    #[serde(default)]
    pub gateway_allow_external: bool,

    /// 원격 LMmaster 게이트웨이 연결 목록 — 사용자가 수동 등록한 외부 API 엔드포인트.
    /// 빈 vec이 default. 기존 settings.json 호환 (serde default).
    #[serde(default)]
    pub remote_endpoints: Vec<RemoteEndpoint>,

    /// HuggingFace Personal Access Token — gated 모델 다운로드에 필요.
    /// None이면 공개 모델만 다운 가능. settings.json에 평문 저장 (로컬 파일 시스템 보호에 의존).
    #[serde(default)]
    pub hf_access_token: Option<String>,
}

impl UserSettings {
    /// app_local_data_dir 안의 settings.json read. 없으면 default.
    pub fn load(app_local_data_dir: &Path) -> Self {
        let p = app_local_data_dir.join(SETTINGS_FILENAME);
        if !p.exists() {
            return Self::default();
        }
        match std::fs::read_to_string(&p) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                tracing::warn!(path = %p.display(), error = %e, "settings.json parse 실패 — default로 폴백");
                Self::default()
            }),
            Err(e) => {
                tracing::warn!(path = %p.display(), error = %e, "settings.json read 실패");
                Self::default()
            }
        }
    }

    /// settings.json atomic write (write to .tmp + rename).
    pub fn save(&self, app_local_data_dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(app_local_data_dir)?;
        let final_path = app_local_data_dir.join(SETTINGS_FILENAME);
        let tmp_path = app_local_data_dir.join(format!("{SETTINGS_FILENAME}.tmp"));
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(&tmp_path, json)?;
        std::fs::rename(&tmp_path, &final_path)?;
        Ok(())
    }
}

/// startup hook — settings.json 읽고 env 주입. 실패해도 앱은 계속.
///
/// 주입하는 env:
/// - `LMMASTER_LLAMA_SERVER_PATH` — Phase 13'.h.2.e.1.
/// - `LMMASTER_GATEWAY_ALLOW_EXTERNAL` — Phase 8'.c.4 (ADR-0066). "1" / "0".
pub fn apply_startup_env(app_local_data_dir: &Path) {
    let settings = UserSettings::load(app_local_data_dir);
    if let Some(path) = settings.llama_server_path.as_deref() {
        if !path.is_empty() {
            std::env::set_var("LMMASTER_LLAMA_SERVER_PATH", path);
            tracing::info!(path = %path, "LMMASTER_LLAMA_SERVER_PATH env 주입 (settings.json)");
        }
    }
    // Phase 8'.c.4 — bool → "1" / "0" 문자열로 표현. gateway::run이 read.
    let allow_external_value = if settings.gateway_allow_external {
        "1"
    } else {
        "0"
    };
    std::env::set_var("LMMASTER_GATEWAY_ALLOW_EXTERNAL", allow_external_value);
    tracing::info!(
        allow_external = settings.gateway_allow_external,
        "LMMASTER_GATEWAY_ALLOW_EXTERNAL env 주입 (settings.json)"
    );
}

/// path가 *파일이 존재*하는 절대 경로인지 검증. 검증만 — 실행 검증은 향후 sub-phase.
pub fn validate_binary_path(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err("절대 경로여야 해요.".to_string());
    }
    if !path.exists() {
        return Err(format!("파일이 없어요: {}", path.display()));
    }
    if !path.is_file() {
        return Err(format!("파일이 아니에요: {}", path.display()));
    }
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_default_when_missing() {
        let tmp = TempDir::new().unwrap();
        let s = UserSettings::load(tmp.path());
        assert!(s.llama_server_path.is_none());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let s = UserSettings {
            llama_server_path: Some("C:\\bin\\llama-server.exe".into()),
            ..Default::default()
        };
        s.save(tmp.path()).unwrap();
        let back = UserSettings::load(tmp.path());
        assert_eq!(
            back.llama_server_path.as_deref(),
            Some("C:\\bin\\llama-server.exe")
        );
        assert!(!back.gateway_allow_external);
    }

    /// Phase 8'.c.4 — gateway_allow_external 필드 round-trip + 기존 키 호환.
    #[test]
    fn gateway_allow_external_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let s = UserSettings {
            llama_server_path: None,
            gateway_allow_external: true,
            remote_endpoints: vec![],
            hf_access_token: None,
        };
        s.save(tmp.path()).unwrap();
        let back = UserSettings::load(tmp.path());
        assert!(back.gateway_allow_external);
        assert!(back.llama_server_path.is_none());
    }

    /// 기존 settings.json (gateway_allow_external 필드 없음) 호환 — `serde(default)`로 false.
    #[test]
    fn legacy_settings_json_defaults_gateway_false() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join(SETTINGS_FILENAME);
        std::fs::write(&p, r#"{"llama_server_path":"/usr/bin/llama-server"}"#).unwrap();
        let back = UserSettings::load(tmp.path());
        assert_eq!(
            back.llama_server_path.as_deref(),
            Some("/usr/bin/llama-server")
        );
        assert!(!back.gateway_allow_external);
    }

    #[test]
    fn save_atomic_write() {
        let tmp = TempDir::new().unwrap();
        let s = UserSettings {
            llama_server_path: Some("/usr/bin/llama-server".into()),
            ..Default::default()
        };
        s.save(tmp.path()).unwrap();
        let final_path = tmp.path().join(SETTINGS_FILENAME);
        assert!(final_path.exists());
        let tmp_path = tmp.path().join(format!("{SETTINGS_FILENAME}.tmp"));
        assert!(!tmp_path.exists(), "tmp 파일은 rename 후 사라져야 해요");
    }

    #[test]
    fn parse_failure_falls_back_to_default() {
        let tmp = TempDir::new().unwrap();
        let bad = tmp.path().join(SETTINGS_FILENAME);
        std::fs::write(&bad, "not json").unwrap();
        let s = UserSettings::load(tmp.path());
        assert!(s.llama_server_path.is_none());
    }

    #[test]
    fn validate_binary_path_rejects_relative() {
        let err = validate_binary_path(Path::new("relative.exe")).unwrap_err();
        assert!(err.contains("절대 경로"));
    }

    #[test]
    fn validate_binary_path_rejects_missing() {
        let err = validate_binary_path(Path::new("/nope/missing.exe")).unwrap_err();
        assert!(err.contains("파일이 없어요"));
    }
}
