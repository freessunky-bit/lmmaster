//! Workspace export/import IPC — Phase 11'.
//!
//! 정책 (ADR-0009 + ADR-0039):
//! - `tauri::ipc::Channel<ExportEvent>` / `Channel<ImportEvent>` per-invocation stream.
//! - Registry: `PortableRegistry`로 export_id / import_id ↔ CancellationToken 매핑.
//! - 동시 다중 export 허용 (사용자가 여러 archive를 한 번에 만들 수 있음).
//! - Drop guard로 `cancel_*` 호출 누락 시에도 registry 정리.
//! - 한국어 해요체 에러 메시지.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use portable_workspace::{
    export_workspace, import_workspace, verify_archive, ArchivePreview, ConflictPolicy,
    ExportError, ExportEvent, ExportOptions, ExportSink, ExportSummary, ImportError, ImportEvent,
    ImportOptions, ImportSink, ImportSummary,
};
use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use super::commands::WorkspaceRoot;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PortableApiError {
    #[error("이미 진행 중인 작업이 있어요 ({id})")]
    AlreadyRunning { id: String },
    #[error("작업을 찾을 수 없어요 ({id})")]
    UnknownJob { id: String },
    #[error("내보내기에 실패했어요: {message}")]
    ExportFailed { message: String },
    #[error("가져오기에 실패했어요: {message}")]
    ImportFailed { message: String },
    #[error("아카이브를 읽지 못했어요: {message}")]
    VerifyFailed { message: String },
    #[error("workspace 디스크 오류: {message}")]
    Disk { message: String },
    /// Phase R-A (ADR-0052) — path boundary 위반 (workspace 외부 또는 traversal).
    #[error("workspace 밖 경로에는 가져올 수 없어요: {reason}")]
    PathDenied { reason: String },
}

impl From<ExportError> for PortableApiError {
    fn from(e: ExportError) -> Self {
        Self::ExportFailed {
            message: e.to_string(),
        }
    }
}

impl From<ImportError> for PortableApiError {
    fn from(e: ImportError) -> Self {
        Self::ImportFailed {
            message: e.to_string(),
        }
    }
}

/// id ↔ CancellationToken — export / import 양쪽 공유 (id namespace 분리).
#[derive(Default)]
pub struct PortableRegistry {
    inner: Mutex<HashMap<String, CancellationToken>>,
}

impl PortableRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self) -> (String, CancellationToken) {
        let id = Uuid::new_v4().to_string();
        let tok = CancellationToken::new();
        self.inner
            .lock()
            .expect("PortableRegistry poisoned")
            .insert(id.clone(), tok.clone());
        (id, tok)
    }

    pub fn finish(&self, id: &str) {
        self.inner
            .lock()
            .expect("PortableRegistry poisoned")
            .remove(id);
    }

    pub fn cancel(&self, id: &str) -> bool {
        let g = self.inner.lock().expect("PortableRegistry poisoned");
        if let Some(tok) = g.get(id) {
            tok.cancel();
            true
        } else {
            false
        }
    }

    pub fn cancel_all(&self) {
        let g = self.inner.lock().expect("PortableRegistry poisoned");
        for tok in g.values() {
            tok.cancel();
        }
    }

    pub fn cancel_all_blocking(&self) {
        if let Ok(g) = self.inner.try_lock() {
            for tok in g.values() {
                tok.cancel();
            }
        }
    }
}

/// Drop 시 registry.finish — 어떤 path로 빠져나가도 누수 없음.
struct PortableGuard {
    registry: Arc<PortableRegistry>,
    id: String,
}

impl Drop for PortableGuard {
    fn drop(&mut self) {
        self.registry.finish(&self.id);
    }
}

/// Channel<ExportEvent> 어댑터.
struct ChannelExportSink {
    channel: Channel<ExportEvent>,
}

impl ExportSink for ChannelExportSink {
    fn emit(&self, event: ExportEvent) {
        if let Err(e) = self.channel.send(event) {
            tracing::debug!(error = %e, "export channel send failed (window closed?)");
        }
    }
}

/// Channel<ImportEvent> 어댑터.
struct ChannelImportSink {
    channel: Channel<ImportEvent>,
}

impl ImportSink for ChannelImportSink {
    fn emit(&self, event: ImportEvent) {
        if let Err(e) = self.channel.send(event) {
            tracing::debug!(error = %e, "import channel send failed (window closed?)");
        }
    }
}

/// Frontend 입력 — `ExportOptions`의 wire 형태. `target_path`는 string.
#[derive(Debug, Clone, Deserialize)]
pub struct StartExportRequest {
    #[serde(default)]
    pub include_models: bool,
    #[serde(default)]
    pub include_keys: bool,
    pub key_passphrase: Option<String>,
    pub target_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StartImportRequest {
    pub source_path: String,
    pub target_workspace_root: Option<String>,
    pub key_passphrase: Option<String>,
    #[serde(default = "default_conflict_policy")]
    pub conflict_policy: ConflictPolicy,
    pub expected_sha256: Option<String>,
}

/// Phase R-A (ADR-0052) — Overwrite default는 디렉터리 trash 위험.
/// import는 기본 Rename(자동 conflict 회피)으로. 사용자가 Overwrite 명시 시에만 적용.
fn default_conflict_policy() -> ConflictPolicy {
    ConflictPolicy::Rename
}

/// Phase R-A (ADR-0052) — IPC raw path를 workspace boundary 안으로 강제.
/// 실패 시 PathDenied 반환 — frontend에서 한국어 에러 노출.
///
/// 검증 규칙:
/// 1. workspace_base 는 canonicalize 가능해야 함 (디렉터리 존재 X면 Disk 에러).
/// 2. requested 경로가 *절대 경로*면 workspace_base prefix 검증.
/// 3. requested 가 None / 빈 문자열이면 workspace_base 그대로 (active workspace 복원).
/// 4. requested 가 *상대 경로*면 workspace_base + requested join 후 prefix 재검증.
/// 5. `..` segment 는 join 후 canonicalize 결과로 차단.
pub(crate) fn resolve_import_target(
    workspace_base: &Path,
    requested: Option<&str>,
) -> Result<PathBuf, PortableApiError> {
    let base_canon = workspace_base
        .canonicalize()
        .map_err(|e| PortableApiError::Disk {
            message: format!("workspace 루트 정규화 실패: {e}"),
        })?;

    let candidate = match requested {
        None => return Ok(base_canon),
        Some(s) if s.trim().is_empty() => return Ok(base_canon),
        Some(s) => s,
    };

    // 제어 문자 / null byte 거부.
    if candidate.chars().any(|c| c == '\0' || c.is_control()) {
        return Err(PortableApiError::PathDenied {
            reason: "경로에 사용할 수 없는 문자가 들어 있어요".into(),
        });
    }

    let joined = base_canon.join(candidate);
    // 디렉터리가 아직 존재하지 않을 수 있으므로 부모를 canonicalize 후 마지막 segment join.
    let parent = joined.parent().unwrap_or(&base_canon);
    let parent_canon = parent.canonicalize().or_else(|_| {
        // 부모도 없으면 base 까지 거슬러 올라감.
        base_canon
            .canonicalize()
            .map_err(|e| PortableApiError::Disk {
                message: format!("부모 경로 정규화 실패: {e}"),
            })
    })?;

    let final_path = match joined.file_name() {
        Some(name) => parent_canon.join(name),
        None => parent_canon.clone(),
    };

    if !final_path.starts_with(&base_canon) {
        return Err(PortableApiError::PathDenied {
            reason: "workspace 디렉터리 밖으로 나가는 경로예요".into(),
        });
    }

    Ok(final_path)
}

/// 응답 — invoke().resolve로 frontend가 받는 메타. Done 이벤트와 redundant.
#[derive(Debug, Clone, Serialize)]
pub struct StartExportResponse {
    pub export_id: String,
    pub summary: ExportSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct StartImportResponse {
    pub import_id: String,
    pub summary: ImportSummary,
}

/// `start_workspace_export(req, channel)` — workspace 루트에서 export.
///
/// 1. registry.register → (export_id, cancel).
/// 2. PortableGuard 즉시 — Drop으로 finish.
/// 3. workspace_root 결정 (현재 active workspace).
/// 4. ChannelExportSink 어댑터 + export_workspace 호출.
/// 5. summary 반환.
#[tauri::command]
pub async fn start_workspace_export(
    app: AppHandle,
    workspace_root: State<'_, Arc<WorkspaceRoot>>,
    registry: State<'_, Arc<PortableRegistry>>,
    req: StartExportRequest,
    on_event: Channel<ExportEvent>,
) -> Result<StartExportResponse, PortableApiError> {
    let registry: Arc<PortableRegistry> = (*registry).clone();
    let (export_id, cancel) = registry.register();
    let _guard = PortableGuard {
        registry: registry.clone(),
        id: export_id.clone(),
    };

    let root = workspace_root
        .get_or_init(&app)
        .map_err(|e| PortableApiError::Disk {
            message: format!("{e}"),
        })?;
    let opts = ExportOptions {
        include_models: req.include_models,
        include_keys: req.include_keys,
        key_passphrase: req.key_passphrase,
        target_path: PathBuf::from(req.target_path),
    };
    let sink = Arc::new(ChannelExportSink { channel: on_event });
    let summary = export_workspace(&root, opts, sink, cancel).await?;
    Ok(StartExportResponse { export_id, summary })
}

#[tauri::command]
pub async fn cancel_workspace_export(
    registry: State<'_, Arc<PortableRegistry>>,
    export_id: String,
) -> Result<(), PortableApiError> {
    let r: Arc<PortableRegistry> = (*registry).clone();
    if !r.cancel(&export_id) {
        return Err(PortableApiError::UnknownJob { id: export_id });
    }
    Ok(())
}

/// `start_workspace_import(req, channel)` — archive에서 import.
///
/// `target_workspace_root`가 None이면 active workspace 루트로 import (=동일 PC 복원).
#[tauri::command]
pub async fn start_workspace_import(
    app: AppHandle,
    workspace_root: State<'_, Arc<WorkspaceRoot>>,
    registry: State<'_, Arc<PortableRegistry>>,
    req: StartImportRequest,
    on_event: Channel<ImportEvent>,
) -> Result<StartImportResponse, PortableApiError> {
    let registry: Arc<PortableRegistry> = (*registry).clone();
    let (import_id, cancel) = registry.register();
    let _guard = PortableGuard {
        registry: registry.clone(),
        id: import_id.clone(),
    };

    // Phase R-A (ADR-0052) — workspace boundary 강제. raw target_workspace_root 직접 사용 금지.
    let workspace_base = workspace_root
        .get_or_init(&app)
        .map_err(|e| PortableApiError::Disk {
            message: format!("{e}"),
        })?;
    let target = resolve_import_target(&workspace_base, req.target_workspace_root.as_deref())?;
    let opts = ImportOptions {
        source_path: PathBuf::from(req.source_path),
        target_workspace_root: target,
        key_passphrase: req.key_passphrase,
        conflict_policy: req.conflict_policy,
        expected_sha256: req.expected_sha256,
    };
    let sink = Arc::new(ChannelImportSink { channel: on_event });
    let summary = import_workspace(opts, sink, cancel).await?;
    Ok(StartImportResponse { import_id, summary })
}

#[tauri::command]
pub async fn cancel_workspace_import(
    registry: State<'_, Arc<PortableRegistry>>,
    import_id: String,
) -> Result<(), PortableApiError> {
    let r: Arc<PortableRegistry> = (*registry).clone();
    if !r.cancel(&import_id) {
        return Err(PortableApiError::UnknownJob { id: import_id });
    }
    Ok(())
}

/// import 전 archive 미리보기.
#[tauri::command]
pub async fn verify_workspace_archive(
    source_path: String,
) -> Result<ArchivePreview, PortableApiError> {
    let path = PathBuf::from(&source_path);
    verify_archive(&path)
        .await
        .map_err(|e| PortableApiError::VerifyFailed {
            message: e.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_registry_cancel_marks_token() {
        let r = PortableRegistry::new();
        let (id, tok) = r.register();
        assert!(r.cancel(&id));
        assert!(tok.is_cancelled());
    }

    #[test]
    fn portable_registry_finish_removes_entry() {
        let r = PortableRegistry::new();
        let (id, _tok) = r.register();
        r.finish(&id);
        assert!(!r.cancel(&id), "finish 후 cancel은 unknown");
    }

    #[test]
    fn portable_api_error_kebab_serialization() {
        let e = PortableApiError::AlreadyRunning { id: "x".into() };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "already-running");
        let e = PortableApiError::UnknownJob { id: "y".into() };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "unknown-job");
    }

    #[test]
    fn portable_guard_calls_finish_on_drop() {
        let r = Arc::new(PortableRegistry::new());
        let (id, _tok) = r.register();
        {
            let _g = PortableGuard {
                registry: r.clone(),
                id: id.clone(),
            };
        }
        // After drop, registry should be empty.
        assert!(!r.cancel(&id));
    }

    // ── Phase R-A (ADR-0052) — path boundary invariants ────────────────

    #[test]
    fn default_conflict_policy_is_rename() {
        // Overwrite default는 디렉터리 trash 위험 — Rename으로 안전 default.
        assert!(matches!(default_conflict_policy(), ConflictPolicy::Rename));
    }

    #[test]
    fn resolve_import_target_none_returns_workspace_base() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let resolved = resolve_import_target(tmp.path(), None).expect("None ok");
        assert_eq!(resolved, tmp.path().canonicalize().unwrap());
    }

    #[test]
    fn resolve_import_target_empty_string_returns_workspace_base() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let resolved = resolve_import_target(tmp.path(), Some("   ")).expect("empty ok");
        assert_eq!(resolved, tmp.path().canonicalize().unwrap());
    }

    #[test]
    fn resolve_import_target_accepts_subdir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let resolved = resolve_import_target(tmp.path(), Some("sub")).expect("subdir ok");
        let canon_base = tmp.path().canonicalize().unwrap();
        assert!(resolved.starts_with(&canon_base));
        assert!(resolved.ends_with("sub"));
    }

    #[test]
    fn resolve_import_target_rejects_parent_traversal() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // base 안에 sub 만들고 sub 기준으로 ../.. 시도.
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        // resolve_import_target는 sub를 base로 쓰면 .. 가 base 위로 새는 케이스를 잡아야 함.
        let err =
            resolve_import_target(&sub, Some("../../etc/passwd")).expect_err("traversal must fail");
        assert!(matches!(err, PortableApiError::PathDenied { .. }));
    }

    #[test]
    fn resolve_import_target_rejects_absolute_outside() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("tempdir2");
        // 절대 경로는 PathBuf::join 시 RHS가 그대로 대체 → boundary 검증에서 거부돼야 함.
        let err = resolve_import_target(tmp.path(), Some(outside.path().to_str().unwrap()))
            .expect_err("outside must fail");
        assert!(matches!(err, PortableApiError::PathDenied { .. }));
    }

    #[test]
    fn resolve_import_target_rejects_control_chars() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let err =
            resolve_import_target(tmp.path(), Some("sub\0null")).expect_err("null byte must fail");
        assert!(matches!(err, PortableApiError::PathDenied { .. }));
        let err = resolve_import_target(tmp.path(), Some("sub\nline"))
            .expect_err("control char must fail");
        assert!(matches!(err, PortableApiError::PathDenied { .. }));
    }

    #[test]
    fn portable_api_error_path_denied_kebab_serialization() {
        let e = PortableApiError::PathDenied {
            reason: "test".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "path-denied");
        assert!(e.to_string().contains("workspace 밖 경로"));
    }
}
