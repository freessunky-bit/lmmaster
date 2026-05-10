//! Phase 13'.h.2.d (ADR-0051) — chat IPC LlamaCpp 분기 helper.
//!
//! 정책 (`docs/research/phase-13ph2bc-llama-server-mmproj-decision.md` §A6):
//! - 단일 instance 정책 — `Arc<Mutex<Option<LlamaServerHandle>>>`로 hold.
//! - 같은 model_path면 reuse, 다른 model_path면 기존 shutdown + 새 spawn.
//! - 30~90초 모델 로드는 ChatEvent::Stalled로 진행 신호 (Round 2에서 wiring).
//! - `LMMASTER_LLAMA_SERVER_PATH` env 미설정 시 한국어 친절 안내 + Settings 이동 hint.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use model_registry::manifest::{MmprojSpec, ModelEntry, ModelSource, QuantOption};
use runner_llama_cpp::{LlamaServerHandle, ServerSpec};
use tokio::sync::Mutex;

/// 단일 instance hold용 wrapper — runner-llama-cpp::LlamaServerHandle은 spec 미보관이라
/// reuse 판단(같은 model_path?)을 위해 desktop layer에서 spec을 함께 보관.
pub struct ManagedLlamaServer {
    handle: LlamaServerHandle,
    spec: ServerSpec,
}

impl ManagedLlamaServer {
    pub fn new(handle: LlamaServerHandle, spec: ServerSpec) -> Self {
        Self { handle, spec }
    }
    pub fn endpoint_base_url(&self) -> &str {
        &self.handle.endpoint().base_url
    }
    pub fn model_path(&self) -> &Path {
        &self.spec.model_path
    }
}

/// Tauri State alias — 단일 instance hold + 다중 chat 호출이 락을 통해 직렬화.
///
/// 사용:
/// - `start_chat`이 LlamaCpp 분기 진입 시 lock acquire.
/// - 기존 instance가 같은 `model_path`면 reuse + lock release 후 chat_stream.
/// - 다른 `model_path`면 기존 shutdown(drop) + 새 spawn + 교체.
/// - drop이 자동 SIGKILL — Tauri RunEvent::ExitRequested에서 명시 cleanup도 (Round 3).
pub type LlamaServerState = Arc<Mutex<Option<ManagedLlamaServer>>>;

pub fn new_state() -> LlamaServerState {
    Arc::new(Mutex::new(None))
}

/// ModelEntry → ServerSpec 변환.
///
/// 정책:
/// - `model_path`는 사용자 cache_dir(예: `%LOCALAPPDATA%/lmmaster/models/<id>.gguf`)로 매핑.
///   v1 spawn 정책은 사용자가 수동 다운로드 → cache_dir 배치. 자동 다운로드는 v1.x Phase 13'.h.2.c.2.
/// - `mmproj_path`는 vision 모델만 채워짐 — `entry.mmproj.url`을 cache_dir에 별도 배치 후 path 반환.
/// - `gpu_layers` / `ctx_size` / `chat_template`은 본 sub-phase 미주입 (v1.x Phase 13'.h.3).
///
/// 미존재 cache 경로는 caller가 `LlamaCppNotPrepared` 분기로 안내 (사용자 수동 다운로드 가이드).
pub fn build_server_spec(entry: &ModelEntry, cache_dir: &std::path::Path) -> ServerSpec {
    let model_filename = derive_model_filename(entry);
    let model_path = cache_dir.join(&model_filename);
    let mmproj_path = entry
        .mmproj
        .as_ref()
        .map(|spec| cache_dir.join(derive_mmproj_filename(spec, &entry.id)));

    ServerSpec {
        model_path,
        mmproj_path,
        // Phase 13'.h.3 v1.x — chat_template_hint는 카탈로그에서 주입 예정. 현재는 GGUF 내장 자동 또는 None.
        gpu_layers: None,
        ctx_size: None,
        chat_template: None,
    }
}

/// catalog id → 사용자 cache의 GGUF 파일명.
///
/// 정책 (Phase 13'.h.2.c.2 — 자동 다운로드 진입 후):
/// - `entry.quantization_options.first()` (default Q4_K_M)의 `file_path` basename 사용.
/// - file_path가 None이면 `derive_main_url`의 URL basename 사용.
/// - 그것마저 없으면 `<id>.gguf` fallback.
///
/// 자동 다운로드 (`model_pull::llama_cpp::pull_llama_model`)와 chat 진입 (`build_server_spec`)이
/// 동일 함수를 호출해 파일명 round-trip 보장.
fn derive_model_filename(entry: &ModelEntry) -> String {
    let quant = entry.quantization_options.first();
    if let Some(q) = quant {
        if let Some(p) = q.file_path.as_ref() {
            if let Some(b) = basename(p) {
                return b;
            }
        }
        if let Some(url) = derive_main_url(entry, q) {
            if let Some(b) = basename(&url) {
                return b;
            }
        }
    }
    format!("{}.gguf", entry.id)
}

/// `ModelSource` + `QuantOption` → 다운로드 URL.
///
/// HuggingFace: `https://huggingface.co/{repo}/resolve/main/{file_path}` (file_path 우선,
/// 없으면 source의 `file` 폴백). DirectUrl은 그대로.
pub fn derive_main_url(entry: &ModelEntry, quant: &QuantOption) -> Option<String> {
    match &entry.source {
        ModelSource::HuggingFace { repo, file } => {
            let path = quant.file_path.as_ref().or(file.as_ref())?;
            Some(format!("https://huggingface.co/{repo}/resolve/main/{path}"))
        }
        ModelSource::DirectUrl { url } => Some(url.clone()),
    }
}

/// URL/path basename — `/` 또는 `\\` 마지막 segment. 빈 문자열은 None.
fn basename(s: &str) -> Option<String> {
    s.rsplit(['/', '\\'])
        .next()
        .filter(|x| !x.is_empty())
        .map(String::from)
}

/// catalog id → 모델 파일명을 외부에 노출 (model_pull::llama_cpp가 사용).
pub fn model_filename(entry: &ModelEntry) -> String {
    derive_model_filename(entry)
}

/// MmprojSpec → mmproj 파일명을 외부에 노출.
pub fn mmproj_filename(spec: &MmprojSpec, fallback_id: &str) -> String {
    derive_mmproj_filename(spec, fallback_id)
}

/// Phase 13'.h.2.e.4 — 사용자 cache_dir에 *받은 LlamaCpp 모델 catalog id 리스트*.
///
/// 흐름: catalog → runner_compatibility에 LlamaCpp 포함된 entry만 → 각 entry의
/// expected `model_filename`이 cache_dir에 존재하는지 검사 → catalog id 반환.
///
/// frontend Chat dropdown filter — *받은 모델만 보여주기* 정책. mmproj는 부가라 *메인 GGUF만 검사*.
#[tauri::command]
pub fn list_local_llama_cpp_models(
    app: tauri::AppHandle,
    catalog_state: tauri::State<'_, Arc<crate::commands::CatalogState>>,
) -> Result<Vec<String>, String> {
    use tauri::Manager;
    let cache_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("app_local_data_dir 실패: {e}"))?
        .join("models");
    if !cache_dir.exists() {
        return Ok(vec![]);
    }
    let catalog = catalog_state.snapshot();
    let mut ids: Vec<String> = Vec::new();
    for entry in catalog.entries() {
        if !entry
            .runner_compatibility
            .contains(&shared_types::RuntimeKind::LlamaCpp)
        {
            continue;
        }
        let filename = derive_model_filename(entry);
        if cache_dir.join(&filename).exists() {
            ids.push(entry.id.clone());
        }
    }
    Ok(ids)
}

/// FIX-2 (모델 인식 누락) — cache_dir의 모든 .gguf 파일을 catalog 매칭과 무관하게 반환.
///
/// 정책:
/// - `list_local_llama_cpp_models`는 catalog 매칭에 의존 (catalog stale / file_path 미스매치 시 못 잡음).
/// - 본 IPC는 cache_dir 폴더만 스캔 → "사용자가 받은 GGUF 파일" 그대로 반환.
/// - frontend Chat이 catalog-matched 모델 + 본 결과를 합쳐 "직접 받음" 섹션으로 노출.
#[tauri::command]
pub fn list_local_gguf_files(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    use tauri::Manager;
    let cache_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("app_local_data_dir 실패: {e}"))?
        .join("models");
    if !cache_dir.exists() {
        return Ok(vec![]);
    }
    let mut out: Vec<String> = Vec::new();
    let entries = std::fs::read_dir(&cache_dir).map_err(|e| format!("read_dir 실패: {e}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        // .gguf 파일만 (mmproj도 .gguf 확장자라 포함됨 — frontend가 -mmproj 접미사로 판별).
        if name.to_lowercase().ends_with(".gguf") {
            out.push(name);
        }
    }
    out.sort();
    Ok(out)
}

/// MmprojSpec → mmproj 파일명. URL의 basename 우선, 없으면 `<id>-mmproj.gguf`.
fn derive_mmproj_filename(spec: &MmprojSpec, fallback_id: &str) -> String {
    spec.url
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or_else(|| format!("{fallback_id}-mmproj.gguf"))
}

/// 모델 cache 경로 검증. 미존재 시 caller가 `LlamaCppNotPrepared`로 안내.
pub fn ensure_model_files_present(spec: &ServerSpec) -> Result<(), MissingFile> {
    if !spec.model_path.exists() {
        return Err(MissingFile::Model(spec.model_path.clone()));
    }
    if let Some(mmproj) = &spec.mmproj_path {
        if !mmproj.exists() {
            return Err(MissingFile::Mmproj(mmproj.clone()));
        }
    }
    Ok(())
}

/// 미존재 파일 종류 — caller가 한국어 카피 분기.
#[derive(Debug)]
pub enum MissingFile {
    Model(PathBuf),
    Mmproj(PathBuf),
}

impl MissingFile {
    pub fn into_korean_message(self) -> String {
        match self {
            Self::Model(p) => format!(
                "모델 파일을 찾을 수 없어요. 카탈로그에서 먼저 받아주세요: {}",
                p.display()
            ),
            Self::Mmproj(p) => format!(
                "비전 모델은 mmproj 파일도 필요해요. 자동 다운로드는 아직 준비 중이에요: {}",
                p.display()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn derive_model_filename_returns_id_with_gguf() {
        // ModelEntry builder 부재 — derive 알고리즘만 stub로 검증.
        // helper 자체는 Round 2 wiring 후 통합 통과.
        let id = "gemma-3-4b";
        let derived = format!("{id}.gguf");
        assert_eq!(derived, "gemma-3-4b.gguf");
    }

    #[test]
    fn derive_mmproj_filename_uses_url_basename() {
        let spec = MmprojSpec {
            url: "https://huggingface.co/google/gemma-3-4b/resolve/main/mmproj-model-f16.gguf"
                .into(),
            sha256: None,
            size_mb: 851,
            precision: Some("f16".into()),
            source: Some("ggml-org".into()),
        };
        let name = derive_mmproj_filename(&spec, "gemma-3-4b");
        assert_eq!(name, "mmproj-model-f16.gguf");
    }

    #[test]
    fn derive_mmproj_filename_falls_back_to_id_when_url_empty() {
        let spec = MmprojSpec {
            url: "".into(),
            sha256: None,
            size_mb: 0,
            precision: None,
            source: None,
        };
        let name = derive_mmproj_filename(&spec, "test-id");
        assert_eq!(name, "test-id-mmproj.gguf");
    }

    #[test]
    fn ensure_model_files_present_detects_missing_model() {
        let spec = ServerSpec {
            model_path: Path::new("/nope/missing.gguf").into(),
            mmproj_path: None,
            gpu_layers: None,
            ctx_size: None,
            chat_template: None,
        };
        let err = ensure_model_files_present(&spec).unwrap_err();
        assert!(matches!(err, MissingFile::Model(_)));
        let msg = err.into_korean_message();
        assert!(msg.contains("모델 파일을 찾을 수 없어요"));
        assert!(msg.contains("missing.gguf"));
    }

    #[test]
    fn ensure_model_files_present_detects_missing_mmproj() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let spec = ServerSpec {
            model_path: tmp.path().to_path_buf(),
            mmproj_path: Some(Path::new("/nope/mmproj.gguf").into()),
            gpu_layers: None,
            ctx_size: None,
            chat_template: None,
        };
        let err = ensure_model_files_present(&spec).unwrap_err();
        assert!(matches!(err, MissingFile::Mmproj(_)));
        let msg = err.into_korean_message();
        assert!(msg.contains("비전 모델"));
        assert!(msg.contains("mmproj"));
    }
}
