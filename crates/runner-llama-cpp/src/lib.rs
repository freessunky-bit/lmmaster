//! runner-llama-cpp — `llama-server` 자식 프로세스 supervisor (Phase 13'.h.2.b, ADR-0051).
//!
//! 책임 분리:
//! - **adapter-llama-cpp** = HTTP client wrapping (OpenAI compat `/v1/chat/completions` SSE).
//! - **runner-llama-cpp** = process lifecycle (spawn/port/health/stderr_map).
//!
//! 정책 (ADR-0051):
//! - binary 발견은 `LMMASTER_LLAMA_SERVER_PATH` env override (ADR-0043 패턴) — Tauri sidecar 비-사용 (macOS 노타리 #11992 회피).
//! - 포트 자동 할당: `TcpListener::bind("127.0.0.1:0")` → `local_addr()` → drop → spawn `--port` 전달.
//! - 헬스체크: `/health` 200ms × 60초 backoff polling (vision 4B/CPU 약 30~90초 로드).
//! - graceful shutdown: `kill_on_drop(true)` + Windows `CREATE_NO_WINDOW` (콘솔 창 숨김).
//! - stderr 라인 단위 capture + 한국어 매핑 8 enum variant.
//!
//! v1.x 후속 (deferred):
//! - Windows Job Object + JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE (손자 프로세스 고아 방지).
//! - Tauri `RunEvent::ExitRequested` 훅 명시 cleanup.
//! - 자동 다운로드 + GPU detect (Phase 13'.h.4).

pub mod health;
pub mod port;
pub mod spawn;
pub mod stderr_map;

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::process::Child;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

pub use stderr_map::LlamaServerError;

/// `llama-server` spawn 사양 — 사용자 입력(model_path/mmproj/-ngl/ctx)으로 구성.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSpec {
    /// GGUF 모델 파일 절대 경로.
    pub model_path: PathBuf,
    /// mmproj 파일 절대 경로 — vision 모델일 때만 Some. None이면 텍스트 전용 spawn.
    #[serde(default)]
    pub mmproj_path: Option<PathBuf>,
    /// `-ngl, --gpu-layers N` — None이면 CPU 또는 자동.
    #[serde(default)]
    pub gpu_layers: Option<u32>,
    /// `-c, --ctx-size N` — None이면 빌드 default(보통 4096 또는 모델 메타).
    #[serde(default)]
    pub ctx_size: Option<u32>,
    /// `--chat-template` 프리셋 — gemma-3는 GGUF 내장 자동(None), llava류는 `"llava"` 등.
    /// 보강 리서치 §1.7 #4 — 본 sub-phase는 manifest 미주입(v1.x 후속 Phase 13'.h.3).
    #[serde(default)]
    pub chat_template: Option<String>,
}

/// `llama-server` 시작 후 반환되는 endpoint — adapter-llama-cpp가 받아 HTTP 호출.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEndpoint {
    /// `http://127.0.0.1:{port}` — adapter-llama-cpp가 base URL로 사용.
    pub base_url: String,
    pub port: u16,
}

/// runner 에러 — kebab-case tagged enum (CLAUDE.md §4.2 컨벤션).
#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RunnerError {
    #[error(
        "llama-server 경로를 찾을 수 없어요. LMMASTER_LLAMA_SERVER_PATH 환경변수를 설정해 주세요."
    )]
    BinaryNotFound,

    #[error("llama-server를 시작하지 못했어요: {message}")]
    SpawnFailed { message: String },

    #[error("포트를 할당하지 못했어요: {message}")]
    PortAllocFailed { message: String },

    #[error("헬스체크가 60초 안에 응답하지 않았어요. 모델 로드가 너무 오래 걸려요.")]
    HealthcheckTimeout,

    #[error(transparent)]
    Server(#[from] LlamaServerError),

    #[error("내부 오류: {message}")]
    Internal { message: String },
}

/// `llama-server` 자식 프로세스 핸들 — drop 시 kill_on_drop으로 종료.
///
/// **사용 패턴**:
/// 1. `LlamaServerHandle::start(spec, cancel)` — async, /health 200까지 대기.
/// 2. `endpoint()` — adapter-llama-cpp에 base_url 전달.
/// 3. drop — 자동 SIGKILL (Unix) / TerminateProcess (Windows).
///
/// **단일 instance 정책**: llama-server는 모델 1개만 로드. 다중 모델은 다중 instance.
/// 본 sub-phase는 단일 instance만 — 다중 instance pool은 v2 deferred.
pub struct LlamaServerHandle {
    endpoint: ServerEndpoint,
    /// Mutex로 감싸 외부에서 직접 kill 불가 (drop만이 권한).
    /// `Arc`로 clone 가능하게 — Tauri State<Arc<Mutex<Option<Self>>>> 패턴.
    child: Arc<Mutex<Option<Child>>>,
    /// stderr 매핑 결과를 모아둔 마지막 LlamaServerError. health 실패 시 표면화.
    last_stderr_error: Arc<Mutex<Option<LlamaServerError>>>,
}

impl LlamaServerHandle {
    /// 본 핸들의 endpoint — adapter-llama-cpp가 HTTP 호출에 사용.
    pub fn endpoint(&self) -> &ServerEndpoint {
        &self.endpoint
    }

    /// `llama-server` spawn + /health 200까지 대기. cancel 가능.
    ///
    /// 시작 흐름:
    /// 1. `LMMASTER_LLAMA_SERVER_PATH` env로 binary 경로 확인 — 없으면 `BinaryNotFound`.
    /// 2. ephemeral port 할당 (TcpListener bind 0).
    /// 3. `kill_on_drop(true)` + `CREATE_NO_WINDOW`(Windows) + stderr `Stdio::piped()` 자식 spawn.
    /// 4. stderr 라인 capture task spawn — 매핑 후 last_stderr_error에 기록.
    /// 5. `/health` 200ms × 60초 backoff polling — 200 응답까지 대기.
    /// 6. 200 받으면 endpoint 반환 + 핸들 drop 시 자동 kill.
    pub async fn start(spec: ServerSpec, cancel: CancellationToken) -> Result<Self, RunnerError> {
        let binary = spawn::resolve_binary_path()?;
        let port = port::allocate_localhost_port()?;

        let (child, last_stderr_error) = spawn::spawn_server(&binary, &spec, port).await?;

        let endpoint = ServerEndpoint {
            base_url: format!("http://127.0.0.1:{port}"),
            port,
        };

        let child = Arc::new(Mutex::new(Some(child)));

        // /health polling — 60초 timeout. cancel 시 child drop으로 자동 종료.
        let health_url = format!("{}/health", endpoint.base_url);
        let polling_result = health::wait_for_ready(&health_url, &cancel).await;
        if let Err(e) = polling_result {
            // 헬스체크 실패 — child drop trigger.
            let mut guard = child.lock().await;
            if let Some(mut c) = guard.take() {
                let _ = c.kill().await;
            }
            // stderr 매핑 결과가 있으면 그걸 우선 노출.
            let stderr_err = last_stderr_error.lock().await.take();
            if let Some(se) = stderr_err {
                return Err(RunnerError::Server(se));
            }
            return Err(e);
        }

        Ok(Self {
            endpoint,
            child,
            last_stderr_error,
        })
    }

    /// 명시적 종료 — drop 대신 호출 가능 (graceful 시도). 사용자 명시 stop 시 호출.
    /// 실패해도 drop이 SIGKILL로 보장.
    pub async fn shutdown(&self) {
        let mut guard = self.child.lock().await;
        if let Some(mut child) = guard.take() {
            let _ = child.kill().await;
            let _ = child.wait().await; // 좀비 회피.
        }
    }

    /// stderr 매핑 결과 (경고/에러 노출용). 헬스체크 통과 후라도 런타임 중 발생한 에러 추적용.
    pub async fn last_error(&self) -> Option<LlamaServerError> {
        self.last_stderr_error.lock().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_spec_round_trip() {
        let spec = ServerSpec {
            model_path: PathBuf::from("/path/to/model.gguf"),
            mmproj_path: Some(PathBuf::from("/path/to/mmproj.gguf")),
            gpu_layers: Some(35),
            ctx_size: Some(8192),
            chat_template: None,
        };
        let json = serde_json::to_string(&spec).unwrap();
        let back: ServerSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back.model_path, spec.model_path);
        assert_eq!(back.mmproj_path, spec.mmproj_path);
        assert_eq!(back.gpu_layers, spec.gpu_layers);
        assert_eq!(back.ctx_size, spec.ctx_size);
    }

    #[test]
    fn runner_error_serializes_with_kind_tag() {
        let e = RunnerError::BinaryNotFound;
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "binary-not-found");
    }

    #[test]
    fn runner_error_korean_message() {
        let e = RunnerError::HealthcheckTimeout;
        let s = format!("{e}");
        assert!(s.contains("60초"), "한국어 메시지에 60초 포함: {s}");
    }

    #[test]
    fn server_endpoint_format() {
        let ep = ServerEndpoint {
            base_url: "http://127.0.0.1:8765".into(),
            port: 8765,
        };
        let v = serde_json::to_value(&ep).unwrap();
        assert_eq!(v["base_url"], "http://127.0.0.1:8765");
        assert_eq!(v["port"], 8765);
    }
}
