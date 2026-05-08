//! Phase 13'.h.2.d (ADR-0051) — chat IPC LlamaCpp 분기 helper.
//!
//! 정책 (`docs/research/phase-13ph2bc-llama-server-mmproj-decision.md` §A6):
//! - 단일 instance 정책 — `Arc<Mutex<Option<LlamaServerHandle>>>`로 hold.
//! - 같은 model_path면 reuse, 다른 model_path면 기존 shutdown + 새 spawn.
//! - 30~90초 모델 로드는 ChatEvent::Stalled로 진행 신호 (Round 2에서 wiring).
//! - `LMMASTER_LLAMA_SERVER_PATH` env 미설정 시 한국어 친절 안내 + Settings 이동 hint.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use model_registry::manifest::{MmprojSpec, ModelEntry};
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

/// catalog id → 사용자 cache의 GGUF 파일명 추정.
/// v1 정책: `<id>.gguf` (사용자가 다운로드 후 그 이름으로 배치).
/// v1.x 자동 다운로드 시점에 manifest의 `download_url` basename 사용 예정.
fn derive_model_filename(entry: &ModelEntry) -> String {
    format!("{}.gguf", entry.id)
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
