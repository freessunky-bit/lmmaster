//! 설치 IPC 모듈 — Tauri command + Channel<InstallEvent> 와이어링.
//!
//! 정책 (Phase 1A.3.c 보강 리서치):
//! - `tauri::ipc::Channel<InstallEvent>` per-invocation stream — `Emitter::emit`보다 typed + ordered.
//! - InstallRegistry는 `app.manage(Arc<InstallRegistry>)`로 단일 instance 공유 — clone으로 defer 캡처.
//! - manifest 경로: `BaseDirectory::Resource`로 bundled `manifests/apps/<id>.json` 해결.
//!   dev에서 resource 경로가 없으면 `CARGO_MANIFEST_DIR`-relative 폴백.
//! - cache_dir은 `BaseDirectory::AppLocalData/cache/installer/`.
//! - `Channel::send`는 sync — 닫힘 감지 시 `InstallSinkClosed`로 변환.
//! - registry.finish는 `Drop` impl로 자동 — 어떤 종료 path든 누락 없음.

pub mod registry;

use std::path::PathBuf;
use std::sync::Arc;

use installer::{run_install, InstallEvent, InstallRunnerError, InstallSink, InstallSinkClosed};
use runtime_detector::manifest::AppManifest;
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use thiserror::Error;

use registry::InstallRegistry;

/// 사용자/UI에 노출할 IPC 에러. Serialize → invoke().catch에 전달.
#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum InstallApiError {
    #[error("동일한 앱이 이미 설치 중이에요 (id={id})")]
    AlreadyInstalling { id: String },

    #[error("매니페스트를 찾을 수 없어요: {message}")]
    ManifestNotFound { message: String },

    #[error("매니페스트 파싱 실패: {message}")]
    ManifestParse { message: String },

    #[error("캐시 디렉터리를 만들 수 없어요: {message}")]
    CacheDirCreate { message: String },

    #[error("설치 실행 중 오류 [{code}]: {message}")]
    Runner { code: String, message: String },

    /// Phase R-H hotfix (ADR-0064 §H) — install_app(id) IPC entry에 path traversal 차단.
    /// alpha-num + `-` + `_` 외 문자가 들어오거나 빈 문자열이면 거부.
    #[error("앱 식별자가 올바르지 않아요 (id={id})")]
    InvalidId { id: String },
}

/// Phase R-H hotfix (ADR-0064 §H) — install_app(id)의 안전한 식별자 검증.
///
/// 허용: ASCII alpha-num + `-` + `_` (현재 manifests/apps/*.json 명명 규칙과 정합).
/// 거부: 빈 문자열 / 점 / 슬래시 / 공백 / 제어 문자 / non-ASCII.
fn is_valid_app_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

impl InstallApiError {
    fn from_runner(e: InstallRunnerError) -> Self {
        let code = e.code().to_string();
        let message = e.to_string();
        Self::Runner { code, message }
    }
}

/// `Channel<InstallEvent>` → `InstallSink` 어댑터. 닫힘 → `InstallSinkClosed`.
struct ChannelInstallSink {
    channel: Channel<InstallEvent>,
}

impl InstallSink for ChannelInstallSink {
    fn emit(&self, event: InstallEvent) -> Result<(), InstallSinkClosed> {
        match self.channel.send(event) {
            Ok(()) => Ok(()),
            Err(e) => {
                tracing::debug!(error = %e, "install channel send failed (window closed?)");
                Err(InstallSinkClosed)
            }
        }
    }
}

/// id 등록 해제를 보장하는 Drop guard. Tauri command가 어떤 path로 빠져나가도 finish 호출.
struct InstallGuard {
    registry: Arc<InstallRegistry>,
    id: String,
}

impl Drop for InstallGuard {
    fn drop(&mut self) {
        self.registry.finish(&self.id);
    }
}

/// 매니페스트 디렉터리 해석. resource 경로 우선, 없으면 dev source-tree 폴백.
fn manifests_dir(app: &AppHandle) -> Result<PathBuf, InstallApiError> {
    // 1. Bundled resource (prod).
    if let Ok(p) = app
        .path()
        .resolve("manifests/apps", tauri::path::BaseDirectory::Resource)
    {
        if p.exists() {
            return Ok(p);
        }
    }
    // 2. Dev fallback: CARGO_MANIFEST_DIR(apps/desktop/src-tauri)에서 ../../../manifests/apps.
    let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dev_path = cargo_dir
        .join("..")
        .join("..")
        .join("..")
        .join("manifests")
        .join("apps");
    if dev_path.exists() {
        return Ok(dev_path);
    }
    Err(InstallApiError::ManifestNotFound {
        message: format!(
            "resource 또는 source-tree 양쪽에서 manifests/apps를 찾지 못했어요 (dev_path={})",
            dev_path.display()
        ),
    })
}

/// 캐시 디렉터리. 없으면 생성.
fn cache_dir(app: &AppHandle) -> Result<PathBuf, InstallApiError> {
    let base = app
        .path()
        .app_local_data_dir()
        .map_err(|e| InstallApiError::CacheDirCreate {
            message: format!("app_local_data_dir 해결 실패: {e}"),
        })?;
    let dir = base.join("cache").join("installer");
    std::fs::create_dir_all(&dir).map_err(|e| InstallApiError::CacheDirCreate {
        message: format!("create_dir_all({}): {e}", dir.display()),
    })?;
    Ok(dir)
}

/// `install_app(id, channel)` Tauri command.
///
/// 1. registry.try_start(id) → cancel token (중복 시 `AlreadyInstalling`).
/// 2. InstallGuard 즉시 생성 — Drop으로 finish 보장.
/// 3. manifest 로드 + cache_dir 보장.
/// 4. `ChannelInstallSink`로 InstallEvent 스트리밍 — sink 닫힘 시 cancel + 종료.
/// 5. ActionOutcome 반환 (`InstallEvent::Finished`로도 emit됨).
#[tauri::command]
pub async fn install_app(
    app: AppHandle,
    registry: State<'_, Arc<InstallRegistry>>,
    id: String,
    channel: Channel<InstallEvent>,
) -> Result<installer::ActionOutcome, InstallApiError> {
    let registry: Arc<InstallRegistry> = (*registry).clone();

    // 0. Phase R-H hotfix (ADR-0064 §H) — id 검증을 가장 먼저.
    //    `../etc` 같은 traversal id가 try_start나 InstallGuard에 흘러들어가지 않도록.
    if !is_valid_app_id(&id) {
        return Err(InstallApiError::InvalidId { id: id.clone() });
    }

    // 1. 중복 검증 + cancel token 발급.
    let cancel = registry
        .try_start(&id)
        .map_err(|_| InstallApiError::AlreadyInstalling { id: id.clone() })?;

    // 2. RAII: 어떤 path로 빠져나가도 finish 호출.
    let _guard = InstallGuard {
        registry: registry.clone(),
        id: id.clone(),
    };

    // 3. 매니페스트 로드 — canonicalize prefix check (Phase R-H, ADR-0064 §H).
    //    exists() + read 분리 패턴은 TOCTOU 표면이라 단일 read fail path로 단순화.
    let manifests = manifests_dir(&app)?;
    let manifest_file = manifests.join(format!("{id}.json"));
    let canonical_manifests =
        manifests
            .canonicalize()
            .map_err(|e| InstallApiError::ManifestNotFound {
                message: format!("manifests_dir canonicalize 실패: {e}"),
            })?;
    let canonical_file =
        manifest_file
            .canonicalize()
            .map_err(|e| InstallApiError::ManifestNotFound {
                message: format!("manifest 파일 없음: {} ({e})", manifest_file.display()),
            })?;
    if !canonical_file.starts_with(&canonical_manifests) {
        // 정상 id 검증이 통과했어도 symlink escape 등을 마지막 방어선으로 차단.
        return Err(InstallApiError::InvalidId { id: id.clone() });
    }
    let manifest_text = std::fs::read_to_string(&canonical_file).map_err(|e| {
        InstallApiError::ManifestNotFound {
            message: format!("read {}: {e}", canonical_file.display()),
        }
    })?;
    let manifest: AppManifest =
        serde_json::from_str(&manifest_text).map_err(|e| InstallApiError::ManifestParse {
            message: format!("{}: {e}", canonical_file.display()),
        })?;

    // 4. 캐시 디렉터리.
    let cache = cache_dir(&app)?;

    // 5. 실행.
    let sink: Arc<ChannelInstallSink> = Arc::new(ChannelInstallSink { channel });
    let outcome = run_install(&manifest, &cache, &cancel, sink)
        .await
        .map_err(InstallApiError::from_runner)?;

    Ok(outcome)
}

/// `cancel_install(id)` — 미존재면 no-op (idempotent).
#[tauri::command]
pub fn cancel_install(registry: State<'_, Arc<InstallRegistry>>, id: String) {
    registry.cancel(&id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_api_error_serializes_with_kind_tag() {
        let e = InstallApiError::AlreadyInstalling {
            id: "ollama".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "already-installing");
        assert_eq!(v["id"], "ollama");
    }

    #[test]
    fn install_api_error_runner_carries_code_and_message() {
        let e = InstallApiError::Runner {
            code: "download-failed".into(),
            message: "타임아웃".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "runner");
        assert_eq!(v["code"], "download-failed");
        assert_eq!(v["message"], "타임아웃");
    }

    #[test]
    fn install_guard_calls_finish_on_drop() {
        let registry = Arc::new(InstallRegistry::new());
        let _ = registry.try_start("ollama").unwrap();
        assert_eq!(registry.in_flight_count(), 1);
        {
            let _g = InstallGuard {
                registry: registry.clone(),
                id: "ollama".into(),
            };
        } // Drop here.
        assert_eq!(registry.in_flight_count(), 0);
    }

    // ── Phase R-H hotfix (ADR-0064 §H) — install_app id validation ───

    #[test]
    fn is_valid_app_id_accepts_alpha_num_dash_underscore() {
        assert!(is_valid_app_id("ollama"));
        assert!(is_valid_app_id("lm-studio"));
        assert!(is_valid_app_id("LM_Studio_42"));
        assert!(is_valid_app_id("123"));
    }

    #[test]
    fn is_valid_app_id_rejects_empty() {
        assert!(!is_valid_app_id(""));
    }

    #[test]
    fn is_valid_app_id_rejects_path_traversal() {
        assert!(!is_valid_app_id("../etc"));
        assert!(!is_valid_app_id("foo/bar"));
        assert!(!is_valid_app_id("foo\\bar"));
    }

    #[test]
    fn is_valid_app_id_rejects_dot_extension() {
        // .json은 caller가 붙이므로 id에 .은 들어오면 안 됨.
        assert!(!is_valid_app_id("ollama.json"));
        assert!(!is_valid_app_id("foo.bar"));
    }

    #[test]
    fn is_valid_app_id_rejects_control_and_unicode() {
        assert!(!is_valid_app_id("ollama\0evil"));
        assert!(!is_valid_app_id("ollama\nfoo"));
        assert!(!is_valid_app_id("올라마")); // non-ASCII
        assert!(!is_valid_app_id("ollama foo")); // 공백
    }

    #[test]
    fn install_api_error_invalid_id_kebab_serialization() {
        let e = InstallApiError::InvalidId {
            id: "../etc".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "invalid-id");
        assert_eq!(v["id"], "../etc");
    }
}
