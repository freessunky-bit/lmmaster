//! Phase 4.h — Korean preset Tauri commands.
//!
//! 정책:
//! - PresetCache는 `app.manage(Arc<PresetCache>)`로 단일 instance 공유.
//! - manifest 경로: `BaseDirectory::Resource`로 bundled `manifests/presets/` 해결.
//!   dev에서 resource 경로가 없으면 `CARGO_MANIFEST_DIR` 부모 ancestors 폴백.
//! - 첫 호출 시 lazy load → Mutex<Option<Vec<Preset>>> 캐시.
//! - get_presets(category?) / get_preset(id) 두 가지 read-only IPC.
//! - 카테고리 필터는 `PresetCategory` enum (kebab-case serde).
//! - 모든 에러는 `PresetApiError` (kebab-case tag) → frontend invoke().catch에 직렬화.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use preset_registry::{load_all, Preset, PresetCategory, PresetError};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use thiserror::Error;

/// Lazy-load preset 캐시 — `app.manage(Arc<PresetCache>)`로 setup에서 등록.
#[derive(Default)]
pub struct PresetCache {
    inner: Mutex<Option<Vec<Preset>>>,
}

impl PresetCache {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    /// 캐시에 적재됐으면 clone 반환, 아니면 디스크에서 로드 + 캐시.
    pub fn get_or_load(&self, app: &AppHandle) -> Result<Vec<Preset>, PresetApiError> {
        // 빠른 경로: 이미 적재됐으면 clone.
        {
            let guard = self.inner.lock().map_err(|e| PresetApiError::LoadFailed {
                message: format!("PresetCache lock poisoned: {e}"),
            })?;
            if let Some(presets) = guard.as_ref() {
                return Ok(presets.clone());
            }
        }

        // 미적재: 디렉터리 해결 + 로드.
        let dir = presets_dir(app)?;
        let presets = load_all(&dir)?;

        // 적재 후 clone 반환.
        let mut guard = self.inner.lock().map_err(|e| PresetApiError::LoadFailed {
            message: format!("PresetCache lock poisoned (write): {e}"),
        })?;
        *guard = Some(presets.clone());
        Ok(presets)
    }
}

/// 사용자/UI에 노출할 IPC 에러.
#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PresetApiError {
    #[error("preset 디렉터리를 찾을 수 없어요")]
    NotFound,
    #[error("preset 로드 실패: {message}")]
    LoadFailed { message: String },
}

impl From<PresetError> for PresetApiError {
    fn from(e: PresetError) -> Self {
        Self::LoadFailed {
            message: e.to_string(),
        }
    }
}

/// preset 디렉터리 해석.
///
/// 1. resource_dir/manifests/presets — 프로덕션 빌드.
/// 2. CARGO_MANIFEST_DIR(apps/desktop/src-tauri)에서 ../../../manifests/presets — dev 폴백.
/// 3. cwd ancestors에서 manifests/presets 탐색 — 추가 폴백.
fn presets_dir(app: &AppHandle) -> Result<PathBuf, PresetApiError> {
    // 1. Bundled resource (prod).
    if let Ok(p) = app
        .path()
        .resolve("manifests/presets", tauri::path::BaseDirectory::Resource)
    {
        if p.exists() {
            return Ok(p);
        }
    }

    // 2. Dev fallback: CARGO_MANIFEST_DIR(apps/desktop/src-tauri)에서 ../../../manifests/presets.
    let cargo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dev_path = cargo_dir
        .join("..")
        .join("..")
        .join("..")
        .join("manifests")
        .join("presets");
    if dev_path.exists() {
        return Ok(dev_path);
    }

    // 3. cwd ancestors fallback (워크스페이스 root).
    if let Ok(cwd) = std::env::current_dir() {
        for ancestor in cwd.ancestors() {
            let candidate = ancestor.join("manifests").join("presets");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    Err(PresetApiError::NotFound)
}

/// 모든 preset 또는 카테고리 필터된 preset 목록.
///
/// `category=None`이면 전체. 결과는 id 알파벳 순 (preset-registry::load_all이 정렬 보장).
#[tauri::command]
pub fn get_presets(
    cache: tauri::State<'_, Arc<PresetCache>>,
    app: AppHandle,
    category: Option<PresetCategory>,
) -> Result<Vec<Preset>, PresetApiError> {
    let presets = cache.get_or_load(&app)?;
    let filtered = match category {
        Some(c) => presets.into_iter().filter(|p| p.category == c).collect(),
        None => presets,
    };
    Ok(filtered)
}

/// id로 단일 preset 조회. 없으면 None (404 아님 — UI 빈 상태로 처리).
#[tauri::command]
pub fn get_preset(
    cache: tauri::State<'_, Arc<PresetCache>>,
    app: AppHandle,
    id: String,
) -> Result<Option<Preset>, PresetApiError> {
    let presets = cache.get_or_load(&app)?;
    Ok(presets.into_iter().find(|p| p.id == id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_api_error_serializes_with_kind_tag() {
        let e = PresetApiError::NotFound;
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "not-found");

        let e2 = PresetApiError::LoadFailed {
            message: "boom".into(),
        };
        let v2 = serde_json::to_value(&e2).unwrap();
        assert_eq!(v2["kind"], "load-failed");
        assert_eq!(v2["message"], "boom");
    }

    #[test]
    fn preset_api_error_from_preset_error_carries_message() {
        let inner = PresetError::IdMismatch {
            id: "wrong/x".into(),
            category: PresetCategory::Coding,
        };
        let api: PresetApiError = inner.into();
        match api {
            PresetApiError::LoadFailed { message } => {
                assert!(message.contains("wrong/x"));
            }
            _ => panic!("expected LoadFailed"),
        }
    }

    #[test]
    fn preset_cache_starts_empty() {
        let cache = PresetCache::new();
        let guard = cache.inner.lock().unwrap();
        assert!(guard.is_none());
    }
}
