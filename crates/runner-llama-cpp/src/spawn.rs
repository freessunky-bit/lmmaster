//! `llama-server` Command 구성 + spawn + stderr task.
//!
//! 정책 (ADR-0051, 보강 리서치 §1.4):
//! - binary 발견: `LMMASTER_LLAMA_SERVER_PATH` env override (ADR-0043 패턴).
//! - `kill_on_drop(true)` — drop 시 SIGKILL/TerminateProcess.
//! - Windows: `CREATE_NO_WINDOW` 플래그 — 콘솔 창 숨김.
//! - stderr `Stdio::piped()` + 라인 단위 capture task → stderr_map 매핑.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::stderr_map::{map_stderr_line, LlamaServerError};
use crate::{RunnerError, ServerSpec};

/// `LMMASTER_LLAMA_SERVER_PATH` env 우선. 없으면 `BinaryNotFound`.
///
/// 향후 v1.x (Phase 13'.h.4): GPU detect → ggml-org Releases 자동 다운로드 → 캐시 경로 폴백.
pub fn resolve_binary_path() -> Result<PathBuf, RunnerError> {
    if let Ok(p) = std::env::var("LMMASTER_LLAMA_SERVER_PATH") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Ok(path);
        }
        return Err(RunnerError::BinaryNotFound);
    }
    Err(RunnerError::BinaryNotFound)
}

/// `llama-server` spawn + stderr capture task. (child, last_stderr_error) 반환.
pub async fn spawn_server(
    binary: &Path,
    spec: &ServerSpec,
    port: u16,
) -> Result<(Child, Arc<Mutex<Option<LlamaServerError>>>), RunnerError> {
    let mut cmd = Command::new(binary);
    cmd.arg("--model").arg(&spec.model_path);
    cmd.arg("--host").arg("127.0.0.1");
    cmd.arg("--port").arg(port.to_string());

    if let Some(mmproj) = &spec.mmproj_path {
        cmd.arg("--mmproj").arg(mmproj);
    }
    if let Some(ngl) = spec.gpu_layers {
        cmd.arg("--gpu-layers").arg(ngl.to_string());
    }
    if let Some(ctx) = spec.ctx_size {
        cmd.arg("--ctx-size").arg(ctx.to_string());
    }
    if let Some(template) = &spec.chat_template {
        cmd.arg("--chat-template").arg(template);
    }

    cmd.kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    // Windows: CREATE_NO_WINDOW (0x0800_0000) — 콘솔 창 숨김.
    // tokio::process::Command::creation_flags는 inherent (Windows-only cfg).
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);

    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            RunnerError::BinaryNotFound
        } else {
            RunnerError::SpawnFailed {
                message: e.to_string(),
            }
        }
    })?;

    let last_error: Arc<Mutex<Option<LlamaServerError>>> = Arc::new(Mutex::new(None));

    // stderr capture task — 라인 단위 매핑.
    if let Some(stderr) = child.stderr.take() {
        let last_error_clone = last_error.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if let Some(mapped) = map_stderr_line(&line) {
                    tracing::warn!(error = ?mapped, raw = %line, "llama-server stderr mapped");
                    let mut guard = last_error_clone.lock().await;
                    *guard = Some(mapped);
                } else {
                    tracing::debug!(raw = %line, "llama-server stderr unmapped");
                }
            }
        });
    }

    Ok((child, last_error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_returns_not_found_when_env_unset() {
        // env 단언은 race 위험 — 기존 값을 잠시 백업/복원.
        // 다중 thread 테스트 시 #[serial]로 격리 권장(본 invariant 1건만이라 OK).
        let prev = std::env::var("LMMASTER_LLAMA_SERVER_PATH").ok();
        std::env::remove_var("LMMASTER_LLAMA_SERVER_PATH");
        let r = resolve_binary_path();
        if let Some(p) = prev {
            std::env::set_var("LMMASTER_LLAMA_SERVER_PATH", p);
        }
        assert!(matches!(r, Err(RunnerError::BinaryNotFound)));
    }

    #[test]
    fn resolve_returns_not_found_when_path_does_not_exist() {
        let prev = std::env::var("LMMASTER_LLAMA_SERVER_PATH").ok();
        std::env::set_var(
            "LMMASTER_LLAMA_SERVER_PATH",
            "/this/path/definitely/does/not/exist/llama-server",
        );
        let r = resolve_binary_path();
        if let Some(p) = prev {
            std::env::set_var("LMMASTER_LLAMA_SERVER_PATH", p);
        } else {
            std::env::remove_var("LMMASTER_LLAMA_SERVER_PATH");
        }
        assert!(matches!(r, Err(RunnerError::BinaryNotFound)));
    }

    #[test]
    fn resolve_returns_path_when_env_points_to_existing_file() {
        let f = tempfile::NamedTempFile::new().expect("temp ok");
        let prev = std::env::var("LMMASTER_LLAMA_SERVER_PATH").ok();
        std::env::set_var("LMMASTER_LLAMA_SERVER_PATH", f.path());
        let r = resolve_binary_path();
        if let Some(p) = prev {
            std::env::set_var("LMMASTER_LLAMA_SERVER_PATH", p);
        } else {
            std::env::remove_var("LMMASTER_LLAMA_SERVER_PATH");
        }
        assert_eq!(r.unwrap(), f.path().to_path_buf());
    }
}
