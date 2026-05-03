//! stderr 라인 → 한국어 `LlamaServerError` 매핑 (보강 리서치 §1.5).
//!
//! 정책:
//! - 빌드별 정확한 substring은 첫 검증에서 보강 — 본 모듈은 *알려진 패턴 8종*만 매핑.
//! - 미매칭 라인은 tracing::debug로 흘림 (사용자에 노출 X).
//! - 한국어 해요체 — `Display`만으로 사용자 향 노출 가능.

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Error, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum LlamaServerError {
    #[error("이 모델과 vision 파일이 안 맞아요. 카탈로그에서 모델을 다시 받아 볼래요?")]
    MmprojMismatch,

    #[error("GPU 메모리가 부족해요. 더 작은 양자화 또는 컨텍스트를 줄여 볼래요?")]
    OutOfMemory,

    #[error("포트가 다른 프로그램과 겹쳐요. 잠시 뒤에 다시 시도할게요.")]
    PortInUse,

    #[error("모델 파일을 못 읽었어요. 파일이 손상됐는지 확인할래요?")]
    ModelLoadFailed,

    #[error("그래픽 드라이버에 문제가 있어요. 드라이버를 업데이트하거나 CPU 모드로 바꿔 볼래요?")]
    GpuDeviceLost,

    #[error("런타임이 응답하지 않아요. 다시 시작해 볼래요?")]
    RuntimeUnreachable,

    #[error("런타임이 갑자기 종료됐어요. 로그를 확인해 볼래요?")]
    Crashed,

    #[error("이미지 처리는 vision 파일이 필요해요. 카탈로그에서 받아 올래요?")]
    UnsupportedConfig,
}

/// stderr 라인 1줄 → 매칭되는 enum variant. 매칭 안 되면 None.
///
/// 패턴 (case-insensitive substring):
/// - `mismatch between text model` / `wrong mmproj` → MmprojMismatch
/// - `out of memory` / `failed to allocate` / `cuda out of memory` → OutOfMemory
/// - `address already in use` / `bind() failed` → PortInUse
/// - `failed to load model` / `error loading model` → ModelLoadFailed
/// - `vk::queue::submit: errordevicelost` → GpuDeviceLost
/// - `multimodal not supported` / `--mmproj is required` → UnsupportedConfig
pub fn map_stderr_line(line: &str) -> Option<LlamaServerError> {
    let l = line.to_ascii_lowercase();
    if l.contains("mismatch between text model") || l.contains("wrong mmproj") {
        return Some(LlamaServerError::MmprojMismatch);
    }
    if l.contains("out of memory")
        || l.contains("failed to allocate")
        || l.contains("ggml_cuda_pool_alloc")
    {
        return Some(LlamaServerError::OutOfMemory);
    }
    if l.contains("address already in use") || l.contains("bind() failed") {
        return Some(LlamaServerError::PortInUse);
    }
    if l.contains("failed to load model") || l.contains("error loading model") {
        return Some(LlamaServerError::ModelLoadFailed);
    }
    if l.contains("errordevicelost") || l.contains("device lost") {
        return Some(LlamaServerError::GpuDeviceLost);
    }
    if l.contains("multimodal not supported") || l.contains("--mmproj is required") {
        return Some(LlamaServerError::UnsupportedConfig);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mmproj_mismatch_pattern() {
        let line = "error: mismatch between text model (n_embd = 2816) and mmproj (n_embd = 1536)";
        assert_eq!(
            map_stderr_line(line),
            Some(LlamaServerError::MmprojMismatch)
        );
    }

    #[test]
    fn cuda_oom_pattern() {
        let line = "CUDA out of memory: tried to allocate 2.5 GB";
        assert_eq!(map_stderr_line(line), Some(LlamaServerError::OutOfMemory));
    }

    #[test]
    fn port_in_use_pattern() {
        let line = "bind: address already in use";
        assert_eq!(map_stderr_line(line), Some(LlamaServerError::PortInUse));
    }

    #[test]
    fn model_load_failed_pattern() {
        let line = "error loading model: file is corrupted";
        assert_eq!(
            map_stderr_line(line),
            Some(LlamaServerError::ModelLoadFailed)
        );
    }

    #[test]
    fn gpu_device_lost_pattern() {
        let line = "vk::Queue::submit: ErrorDeviceLost";
        assert_eq!(map_stderr_line(line), Some(LlamaServerError::GpuDeviceLost));
    }

    #[test]
    fn unsupported_config_pattern() {
        let line = "multimodal not supported in this build";
        assert_eq!(
            map_stderr_line(line),
            Some(LlamaServerError::UnsupportedConfig)
        );
    }

    #[test]
    fn unknown_line_returns_none() {
        assert_eq!(map_stderr_line("info: starting server on port 8080"), None);
    }

    #[test]
    fn case_insensitive_match() {
        let line = "OUT OF MEMORY: ...";
        assert_eq!(map_stderr_line(line), Some(LlamaServerError::OutOfMemory));
    }

    #[test]
    fn error_serializes_with_kind_tag() {
        let e = LlamaServerError::MmprojMismatch;
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "mmproj-mismatch");
    }

    #[test]
    fn error_display_is_korean_haeyo() {
        let e = LlamaServerError::OutOfMemory;
        let s = format!("{e}");
        assert!(s.contains("부족해요"), "해요체 한국어 표시: {s}");
    }
}
