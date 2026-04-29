//! 사용자 직접 만든 (Workbench output) 모델을 영속화하는 custom-model 레지스트리.
//!
//! 정책 (Phase 5'.d):
//! - 카탈로그(manifest)와 분리 — Workbench로 만든 모델은 별도 JSON에 누적.
//! - 저장 경로: `{registry_dir}/custom-models.json`. registry_dir 미설정 시 in-memory.
//! - id 유일성: caller(Workbench)가 uuid로 보장. 중복 시 덮어쓰기 (재실행 케이스 — 같은 base+timestamp 거의 0).
//! - serde-friendly. tagged enum 없음 — 단순 struct.
//! - 모든 IO 에러 메시지 한국어 (`#[error]`).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModelRegistryError {
    #[error("custom-models.json을 읽지 못했어요: {0}")]
    ReadFailed(String),

    #[error("custom-models.json을 저장하지 못했어요: {0}")]
    WriteFailed(String),

    #[error("custom-models.json 형식이 올바르지 않아요: {0}")]
    ParseFailed(String),

    #[error("등록할 모델 정보가 비어 있어요: {field}")]
    InvalidModel { field: String },
}

/// Workbench output을 카탈로그에 영속화하는 단일 record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CustomModel {
    /// uuid — Workbench run_id를 그대로 쓰거나 별도 발급.
    pub id: String,
    /// 베이스 모델 식별자 (예: `Qwen2.5-3B`).
    pub base_model: String,
    /// 양자화 유형 (예: `Q4_K_M`).
    pub quant_type: String,
    /// LoRA adapter 출력 경로.
    pub lora_adapter: Option<String>,
    /// Modelfile 본문 (사용자가 미리보기에서 그대로 복사 가능).
    pub modelfile: String,
    /// RFC3339 생성 시각.
    pub created_at: String,
    /// baseline 10 case 점수 (passed/total).
    pub eval_passed: usize,
    pub eval_total: usize,
    /// Modelfile/adapter 등 결과물 경로 모음.
    pub artifact_paths: Vec<String>,
}

/// 디스크 또는 메모리 기반 사용자 모델 레지스트리.
///
/// - `with_dir` 호출 시 `{dir}/custom-models.json`을 매 read/write마다 직렬화.
/// - dir 미설정(`in_memory`) 시 메모리에 보관 — 테스트/UI dev 모드에서 사용.
#[derive(Debug)]
pub struct ModelRegistry {
    storage_dir: Option<PathBuf>,
    in_memory: Mutex<Vec<CustomModel>>,
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl ModelRegistry {
    /// 메모리만 — Tauri State 미설정 환경/단위 테스트용.
    pub fn in_memory() -> Self {
        Self {
            storage_dir: None,
            in_memory: Mutex::new(Vec::new()),
        }
    }

    /// 디스크 영속화. dir이 없으면 첫 register 호출 시 자동 생성.
    pub fn with_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            storage_dir: Some(dir.into()),
            in_memory: Mutex::new(Vec::new()),
        }
    }

    /// 파일 경로 — `{dir}/custom-models.json`.
    fn file_path(&self) -> Option<PathBuf> {
        self.storage_dir
            .as_ref()
            .map(|d| d.join("custom-models.json"))
    }

    /// 디스크에서 현재 list를 불러옴. 파일 없으면 빈 vec.
    fn load_disk(path: &Path) -> Result<Vec<CustomModel>, ModelRegistryError> {
        if !path.exists() {
            return Ok(Vec::new());
        }
        let body = std::fs::read_to_string(path)
            .map_err(|e| ModelRegistryError::ReadFailed(format!("{e}")))?;
        if body.trim().is_empty() {
            return Ok(Vec::new());
        }
        serde_json::from_str::<Vec<CustomModel>>(&body)
            .map_err(|e| ModelRegistryError::ParseFailed(format!("{e}")))
    }

    /// 디스크에 list를 통째 저장. 부모 디렉터리 자동 생성.
    fn save_disk(path: &Path, models: &[CustomModel]) -> Result<(), ModelRegistryError> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| ModelRegistryError::WriteFailed(format!("{e}")))?;
            }
        }
        let body = serde_json::to_string_pretty(models)
            .map_err(|e| ModelRegistryError::WriteFailed(format!("{e}")))?;
        std::fs::write(path, body).map_err(|e| ModelRegistryError::WriteFailed(format!("{e}")))?;
        Ok(())
    }

    /// 새 모델 등록. 기존 동일 id가 있으면 덮어쓰기. 반환값은 등록된 model id.
    pub fn register(&self, model: CustomModel) -> Result<String, ModelRegistryError> {
        if model.id.trim().is_empty() {
            return Err(ModelRegistryError::InvalidModel { field: "id".into() });
        }
        if model.base_model.trim().is_empty() {
            return Err(ModelRegistryError::InvalidModel {
                field: "base_model".into(),
            });
        }
        if model.modelfile.trim().is_empty() {
            return Err(ModelRegistryError::InvalidModel {
                field: "modelfile".into(),
            });
        }

        let id = model.id.clone();

        // disk 모드면 disk → upsert → save.
        if let Some(path) = self.file_path() {
            let mut current = Self::load_disk(&path)?;
            if let Some(idx) = current.iter().position(|m| m.id == model.id) {
                current[idx] = model;
            } else {
                current.push(model);
            }
            Self::save_disk(&path, &current)?;
        } else {
            // in-memory upsert.
            let mut g = self
                .in_memory
                .lock()
                .map_err(|e| ModelRegistryError::WriteFailed(format!("lock poisoned: {e}")))?;
            if let Some(idx) = g.iter().position(|m| m.id == id) {
                g[idx] = model;
            } else {
                g.push(model);
            }
        }
        Ok(id)
    }

    /// 전체 목록. disk 모드면 매 호출마다 read.
    pub fn list(&self) -> Result<Vec<CustomModel>, ModelRegistryError> {
        if let Some(path) = self.file_path() {
            return Self::load_disk(&path);
        }
        let g = self
            .in_memory
            .lock()
            .map_err(|e| ModelRegistryError::ReadFailed(format!("lock poisoned: {e}")))?;
        Ok(g.clone())
    }

    /// 단건 lookup. 없으면 None.
    pub fn get(&self, id: &str) -> Result<Option<CustomModel>, ModelRegistryError> {
        let list = self.list()?;
        Ok(list.into_iter().find(|m| m.id == id))
    }

    /// 등록된 custom-model의 카운트.
    pub fn count(&self) -> Result<usize, ModelRegistryError> {
        Ok(self.list()?.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str) -> CustomModel {
        CustomModel {
            id: id.into(),
            base_model: "Qwen2.5-3B".into(),
            quant_type: "Q4_K_M".into(),
            lora_adapter: Some("workspace/workbench/r1/lora/adapter".into()),
            modelfile: "FROM ./model.gguf\nSYSTEM \"helper\"\n".into(),
            created_at: "2026-04-28T00:00:00Z".into(),
            eval_passed: 9,
            eval_total: 10,
            artifact_paths: vec!["a.gguf".into(), "b/adapter".into()],
        }
    }

    #[test]
    fn in_memory_register_then_list() {
        let r = ModelRegistry::in_memory();
        let id = r.register(sample("m1")).unwrap();
        assert_eq!(id, "m1");
        let list = r.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "m1");
    }

    #[test]
    fn in_memory_get_returns_some() {
        let r = ModelRegistry::in_memory();
        r.register(sample("m1")).unwrap();
        let got = r.get("m1").unwrap();
        assert!(got.is_some());
        assert_eq!(got.unwrap().base_model, "Qwen2.5-3B");
    }

    #[test]
    fn in_memory_get_missing_returns_none() {
        let r = ModelRegistry::in_memory();
        assert!(r.get("missing").unwrap().is_none());
    }

    #[test]
    fn empty_id_rejected() {
        let r = ModelRegistry::in_memory();
        let mut m = sample("ok");
        m.id = String::new();
        let err = r.register(m).unwrap_err();
        assert!(matches!(err, ModelRegistryError::InvalidModel { .. }));
    }

    #[test]
    fn empty_base_model_rejected() {
        let r = ModelRegistry::in_memory();
        let mut m = sample("m1");
        m.base_model = String::new();
        let err = r.register(m).unwrap_err();
        assert!(matches!(
            err,
            ModelRegistryError::InvalidModel { field } if field == "base_model"
        ));
    }

    #[test]
    fn empty_modelfile_rejected() {
        let r = ModelRegistry::in_memory();
        let mut m = sample("m1");
        m.modelfile = "   ".into();
        let err = r.register(m).unwrap_err();
        assert!(matches!(
            err,
            ModelRegistryError::InvalidModel { field } if field == "modelfile"
        ));
    }

    #[test]
    fn duplicate_id_overwrites() {
        let r = ModelRegistry::in_memory();
        r.register(sample("dup")).unwrap();
        let mut m2 = sample("dup");
        m2.eval_passed = 7;
        r.register(m2).unwrap();
        let list = r.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].eval_passed, 7);
    }

    #[test]
    fn disk_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let r = ModelRegistry::with_dir(tmp.path());
        r.register(sample("disk-1")).unwrap();
        r.register(sample("disk-2")).unwrap();
        // 파일 존재.
        let path = tmp.path().join("custom-models.json");
        assert!(path.exists());
        // 새 인스턴스로 reload — 동일.
        let r2 = ModelRegistry::with_dir(tmp.path());
        let list = r2.list().unwrap();
        assert_eq!(list.len(), 2);
        let ids: Vec<String> = list.iter().map(|m| m.id.clone()).collect();
        assert!(ids.contains(&"disk-1".to_string()));
        assert!(ids.contains(&"disk-2".to_string()));
    }

    #[test]
    fn disk_creates_parent_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("nested/registry");
        let r = ModelRegistry::with_dir(&nested);
        r.register(sample("m")).unwrap();
        assert!(nested.join("custom-models.json").exists());
    }

    #[test]
    fn disk_empty_file_returns_empty_list() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("custom-models.json"), "").unwrap();
        let r = ModelRegistry::with_dir(tmp.path());
        let list = r.list().unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn disk_invalid_json_returns_parse_error() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("custom-models.json"), "{not json").unwrap();
        let r = ModelRegistry::with_dir(tmp.path());
        let err = r.list().unwrap_err();
        assert!(matches!(err, ModelRegistryError::ParseFailed(_)));
    }

    #[test]
    fn count_returns_correct_size() {
        let r = ModelRegistry::in_memory();
        assert_eq!(r.count().unwrap(), 0);
        r.register(sample("a")).unwrap();
        r.register(sample("b")).unwrap();
        assert_eq!(r.count().unwrap(), 2);
    }

    #[test]
    fn round_trip_serde_preserves_lora_adapter_none() {
        let r = ModelRegistry::in_memory();
        let mut m = sample("none-adapter");
        m.lora_adapter = None;
        r.register(m).unwrap();
        let got = r.get("none-adapter").unwrap().unwrap();
        assert!(got.lora_adapter.is_none());
    }

    #[test]
    fn error_display_korean() {
        let e = ModelRegistryError::InvalidModel { field: "id".into() };
        let msg = format!("{e}");
        assert!(msg.contains("등록할 모델 정보"));
    }
}
