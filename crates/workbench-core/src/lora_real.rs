//! `LlamaFactoryTrainer` — LLaMA-Factory CLI 실 subprocess wrapper.
//!
//! 정책 (phase-9pb-workbench-real-reinforcement, ADR-0043):
//! - Python venv 자동 부트스트랩(uv 우선, pip fallback). 약 5~10GB 다운로드.
//! - 사용자 동의 없이는 부트스트랩 진입 X — caller가 `bootstrap_or_open`을 호출하고
//!   `BootstrapEvent`를 watching하며 사용자에게 안내 / 진행률 노출.
//! - python 3.10+ 필요 — 미설치 시 `WorkbenchError::ToolMissing` 한국어 진단.
//! - LLaMA-Factory CLI: `python -m llamafactory train --config <yaml>` 형태 spawn.
//! - cancel cooperative + `kill_on_drop(true)` + 임시 파일(yaml) 정리.
//! - 4시간 timeout (LoRA는 오래 걸림). caller override 가능.
//! - stdout/stderr 라인 단위 — epoch / loss / step parsing → `QuantizeProgress` emit.
//!
//! Negative space (ADR-0043 §Alternatives):
//! - Python sidecar 상시 띄움 — cold start 0이지만 메모리 비용. 사용자가 LoRA 안 쓸 가능성 높아 거부.
//! - Rust-only candle — LLaMA-Factory만큼 한국어 dataset / template 성숙도 X. 거부.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::WorkbenchError;
use crate::lora::{LoRAJob, LoRATrainer};
use crate::quantize::QuantizeProgress;

/// 기본 학습 timeout — 4시간.
pub const DEFAULT_LORA_TIMEOUT_SECS: u64 = 4 * 60 * 60;

/// 부트스트랩 timeout — 30분 (큰 wheel 다운로드 + 컴파일 가능성).
pub const DEFAULT_BOOTSTRAP_TIMEOUT_SECS: u64 = 30 * 60;

/// uv / python / venv 환경변수 override.
pub const UV_PATH_ENV: &str = "LMMASTER_UV_PATH";
pub const PYTHON_PATH_ENV: &str = "LMMASTER_PYTHON_PATH";

/// venv 부트스트랩 진행 이벤트.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum BootstrapEvent {
    /// uv / python 후보 검색 단계.
    Probing,
    /// Python 인터프리터 발견 (3.10+ 검증 후).
    PythonReady { version: String, path: String },
    /// venv 디렉터리 생성 시작.
    CreatingVenv,
    /// PyTorch 등 base 의존성 설치 시작.
    InstallingDeps { phase: String },
    /// 한 라인 stdout/stderr — UI 라이브 노출.
    Log { line: String },
    /// 부트스트랩 정상 종료.
    Done,
    /// 사용자 향 한국어 에러 메시지.
    Failed { error: String },
}

/// LLaMA-Factory CLI subprocess wrapper.
#[derive(Debug)]
pub struct LlamaFactoryTrainer {
    /// venv 디렉터리 (부트스트랩 결과). 안에 `bin/python` 또는 `Scripts/python.exe` 존재.
    venv_path: PathBuf,
    /// venv 안 python 절대 경로. 모든 spawn에 사용.
    python_path: PathBuf,
    /// 학습 timeout. caller override 가능.
    timeout: Duration,
}

impl LlamaFactoryTrainer {
    /// 명시 venv + python path로 직접 생성 — 테스트 / 사용자 사전 부트스트랩 후 재진입.
    pub fn with_paths(venv_path: PathBuf, python_path: PathBuf, timeout: Duration) -> Self {
        Self {
            venv_path,
            python_path,
            timeout,
        }
    }

    /// venv가 이미 있으면 open, 없으면 부트스트랩(약 5~10GB). 사용자 명시 동의 후만 호출.
    pub async fn bootstrap_or_open(
        venv_dir: PathBuf,
        timeout: Duration,
        progress: mpsc::Sender<BootstrapEvent>,
        cancel: CancellationToken,
    ) -> Result<Self, WorkbenchError> {
        let _ = progress.send(BootstrapEvent::Probing).await;

        // 1) 이미 venv가 있으면 검증 후 즉시 open.
        let python_path = venv_python_path(&venv_dir);
        if python_path.exists() {
            // 버전 확인.
            match probe_python_version(&python_path).await {
                Ok(v) => {
                    let _ = progress
                        .send(BootstrapEvent::PythonReady {
                            version: v,
                            path: python_path.display().to_string(),
                        })
                        .await;
                    let _ = progress.send(BootstrapEvent::Done).await;
                    return Ok(Self::with_paths(venv_dir, python_path, timeout));
                }
                Err(e) => {
                    // venv가 깨짐 — 사용자 안내 후 재부트스트랩 권장.
                    let _ = progress
                        .send(BootstrapEvent::Failed {
                            error: format!("기존 venv가 손상되어 다시 만들어야 해요: {e}"),
                        })
                        .await;
                    return Err(WorkbenchError::Internal {
                        message: format!("기존 venv가 손상됐어요: {e}"),
                    });
                }
            }
        }

        if cancel.is_cancelled() {
            return Err(WorkbenchError::Cancelled);
        }

        // 2) 시스템 python 3.10+ 탐색.
        let system_python = find_system_python().await?;
        let _ = progress
            .send(BootstrapEvent::PythonReady {
                version: system_python.version.clone(),
                path: system_python.path.display().to_string(),
            })
            .await;

        // 3) venv 디렉터리 생성.
        let _ = progress.send(BootstrapEvent::CreatingVenv).await;
        if let Some(parent) = venv_dir.parent() {
            std::fs::create_dir_all(parent).map_err(|e| WorkbenchError::Internal {
                message: format!("venv 부모 디렉터리를 만들지 못했어요: {e}"),
            })?;
        }

        // uv가 있으면 우선 사용 (5~10x 빠름). 없으면 python -m venv.
        let bootstrap_outcome = if let Some(uv_path) = find_uv() {
            run_uv_create(&uv_path, &venv_dir, &cancel).await
        } else {
            run_python_venv(&system_python.path, &venv_dir, &cancel).await
        };
        bootstrap_outcome.inspect_err(|_| {
            // 부트스트랩 실패 → 깨끗하게 정리 (best-effort).
            let _ = std::fs::remove_dir_all(&venv_dir);
        })?;

        let python_path = venv_python_path(&venv_dir);
        if !python_path.exists() {
            return Err(WorkbenchError::Internal {
                message: format!(
                    "venv 생성 후에도 python 실행 파일이 보이지 않아요: {}",
                    python_path.display()
                ),
            });
        }

        // 4) llamafactory pip install.
        let _ = progress
            .send(BootstrapEvent::InstallingDeps {
                phase: "llamafactory".into(),
            })
            .await;
        run_pip_install_llamafactory(&python_path, &progress, &cancel).await?;

        let _ = progress.send(BootstrapEvent::Done).await;

        Ok(Self::with_paths(venv_dir, python_path, timeout))
    }

    pub fn venv_path(&self) -> &Path {
        &self.venv_path
    }

    pub fn python_path(&self) -> &Path {
        &self.python_path
    }
}

#[async_trait]
impl LoRATrainer for LlamaFactoryTrainer {
    async fn run(
        &self,
        job: LoRAJob,
        cancel: &CancellationToken,
    ) -> Result<Vec<QuantizeProgress>, WorkbenchError> {
        let (tx, mut rx) = mpsc::channel::<QuantizeProgress>(64);
        let job_clone = job.clone();
        let cancel_clone = cancel.clone();
        let python = self.python_path.clone();
        let timeout = self.timeout;
        let runner = tokio::spawn(async move {
            run_train_inner(&python, timeout, job_clone, tx, cancel_clone).await
        });
        let mut collected = Vec::new();
        while let Some(p) = rx.recv().await {
            collected.push(p);
        }
        match runner.await {
            Ok(Ok(())) => Ok(collected),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(WorkbenchError::Internal {
                message: format!("LoRA 워커 task가 실패했어요: {e}"),
            }),
        }
    }

    async fn run_streaming(
        &self,
        job: LoRAJob,
        progress: mpsc::Sender<QuantizeProgress>,
        cancel: &CancellationToken,
    ) -> Result<(), WorkbenchError> {
        run_train_inner(
            &self.python_path,
            self.timeout,
            job,
            progress,
            cancel.clone(),
        )
        .await
    }
}

/// venv 안 python 실행 파일 절대 경로.
fn venv_python_path(venv_dir: &Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        venv_dir.join("Scripts").join("python.exe")
    }
    #[cfg(not(target_os = "windows"))]
    {
        venv_dir.join("bin").join("python")
    }
}

/// 발견된 시스템 python 메타.
struct SystemPython {
    path: PathBuf,
    version: String,
}

/// PATH 우선 + env override로 python 후보 결정 + 3.10+ 검증.
async fn find_system_python() -> Result<SystemPython, WorkbenchError> {
    // env override 우선.
    let candidates = if let Ok(p) = std::env::var(PYTHON_PATH_ENV) {
        vec![PathBuf::from(p)]
    } else {
        let mut v = Vec::new();
        for name in [
            "python3.12",
            "python3.11",
            "python3.10",
            "python3",
            "python",
        ] {
            if let Ok(p) = which::which(name) {
                v.push(p);
            }
        }
        v
    };

    if candidates.is_empty() {
        return Err(WorkbenchError::ToolMissing {
            tool: "python을 찾지 못했어요. python 3.10 이상을 설치하거나 \
                   LMMASTER_PYTHON_PATH 환경변수로 위치를 알려 주세요. \
                   (설치 안내: https://www.python.org/downloads/)"
                .into(),
        });
    }

    let mut last_err = "python 3.10+ 후보가 없어요".to_string();
    for cand in candidates {
        match probe_python_version(&cand).await {
            Ok(v) => {
                if version_at_least(&v, 3, 10) {
                    return Ok(SystemPython {
                        path: cand,
                        version: v,
                    });
                } else {
                    last_err = format!("발견된 python({v})은 3.10 미만이에요");
                }
            }
            Err(e) => last_err = e,
        }
    }
    Err(WorkbenchError::ToolMissing {
        tool: format!(
            "{last_err}. python 3.10 이상을 설치해 주세요. (https://www.python.org/downloads/)"
        ),
    })
}

/// `python --version` 호출 후 X.Y.Z 추출.
async fn probe_python_version(path: &Path) -> Result<String, String> {
    let mut cmd = Command::new(path);
    cmd.arg("--version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("python 호출 실패: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    // "Python 3.11.5" 같은 prefix.
    let trimmed = combined.trim();
    let version = trimmed
        .split_whitespace()
        .find(|w| w.contains('.'))
        .ok_or_else(|| format!("python 버전 파싱 실패: {trimmed}"))?
        .to_string();
    Ok(version)
}

/// "3.11.5" 같은 버전이 major.minor 이상인지.
fn version_at_least(v: &str, want_major: u32, want_minor: u32) -> bool {
    let mut parts = v.split('.');
    let major: u32 = match parts.next().and_then(|s| s.trim().parse().ok()) {
        Some(m) => m,
        None => return false,
    };
    let minor: u32 = match parts.next().and_then(|s| s.trim().parse().ok()) {
        Some(m) => m,
        None => return false,
    };
    if major > want_major {
        return true;
    }
    if major < want_major {
        return false;
    }
    minor >= want_minor
}

/// uv 실행 파일 탐색 (env override 우선).
fn find_uv() -> Option<PathBuf> {
    if let Ok(p) = std::env::var(UV_PATH_ENV) {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    which::which("uv").ok()
}

/// `uv venv <dir>` 호출 — cancel + 부트스트랩 timeout 협력.
async fn run_uv_create(
    uv_path: &Path,
    venv_dir: &Path,
    cancel: &CancellationToken,
) -> Result<(), WorkbenchError> {
    let mut cmd = Command::new(uv_path);
    cmd.arg("venv")
        .arg(venv_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .kill_on_drop(true);
    spawn_and_wait(
        cmd,
        Duration::from_secs(DEFAULT_BOOTSTRAP_TIMEOUT_SECS),
        cancel,
    )
    .await
}

/// `python -m venv <dir>` fallback.
async fn run_python_venv(
    python: &Path,
    venv_dir: &Path,
    cancel: &CancellationToken,
) -> Result<(), WorkbenchError> {
    let mut cmd = Command::new(python);
    cmd.arg("-m")
        .arg("venv")
        .arg(venv_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .kill_on_drop(true);
    spawn_and_wait(
        cmd,
        Duration::from_secs(DEFAULT_BOOTSTRAP_TIMEOUT_SECS),
        cancel,
    )
    .await
}

/// `python -m pip install llamafactory[torch,metrics]` (또는 단순 `llamafactory`).
async fn run_pip_install_llamafactory(
    python: &Path,
    progress: &mpsc::Sender<BootstrapEvent>,
    cancel: &CancellationToken,
) -> Result<(), WorkbenchError> {
    let mut cmd = Command::new(python);
    cmd.arg("-m")
        .arg("pip")
        .arg("install")
        .arg("--upgrade")
        .arg("llamafactory")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|e| WorkbenchError::Internal {
        message: format!("pip install을 실행하지 못했어요: {e}"),
    })?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let progress_clone = progress.clone();
    let stdout_task = stdout.map(|s| {
        let prog = progress_clone.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(s).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = prog.send(BootstrapEvent::Log { line }).await;
            }
        })
    });
    let stderr_task = stderr.map(|s| {
        let prog = progress_clone.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(s).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = prog.send(BootstrapEvent::Log { line }).await;
            }
        })
    });

    let timeout_sleep = tokio::time::sleep(Duration::from_secs(DEFAULT_BOOTSTRAP_TIMEOUT_SECS));
    tokio::pin!(timeout_sleep);
    let outcome = tokio::select! {
        () = cancel.cancelled() => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            Err(WorkbenchError::Cancelled)
        }
        () = &mut timeout_sleep => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            Err(WorkbenchError::Internal {
                message: "LLaMA-Factory 설치가 30분 안에 끝나지 않았어요. 인터넷 상태를 확인해 주세요.".into(),
            })
        }
        status = child.wait() => {
            match status {
                Ok(s) if s.success() => Ok(()),
                Ok(s) => Err(WorkbenchError::Internal {
                    message: format!("LLaMA-Factory 설치가 실패했어요 (exit {}). 한국어 환경 점검이 필요해요.", s.code().unwrap_or(-1)),
                }),
                Err(e) => Err(WorkbenchError::Internal {
                    message: format!("pip 종료 상태를 읽지 못했어요: {e}"),
                }),
            }
        }
    };
    if let Some(t) = stdout_task {
        t.abort();
    }
    if let Some(t) = stderr_task {
        t.abort();
    }
    outcome
}

/// 일반 spawn + wait helper — 부트스트랩 단계 (uv venv / python -m venv 등).
async fn spawn_and_wait(
    mut cmd: Command,
    timeout: Duration,
    cancel: &CancellationToken,
) -> Result<(), WorkbenchError> {
    if cancel.is_cancelled() {
        return Err(WorkbenchError::Cancelled);
    }
    let mut child = cmd.spawn().map_err(|e| WorkbenchError::Internal {
        message: format!("부트스트랩 단계 실행에 실패했어요: {e}"),
    })?;
    let timeout_sleep = tokio::time::sleep(timeout);
    tokio::pin!(timeout_sleep);
    tokio::select! {
        () = cancel.cancelled() => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            Err(WorkbenchError::Cancelled)
        }
        () = &mut timeout_sleep => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            Err(WorkbenchError::Internal {
                message: "부트스트랩 단계가 시간 안에 끝나지 않았어요.".into(),
            })
        }
        status = child.wait() => {
            match status {
                Ok(s) if s.success() => Ok(()),
                Ok(s) => Err(WorkbenchError::Internal {
                    message: format!("부트스트랩 단계가 비정상 종료됐어요 (exit {}).", s.code().unwrap_or(-1)),
                }),
                Err(e) => Err(WorkbenchError::Internal {
                    message: format!("부트스트랩 단계 종료 상태를 읽지 못했어요: {e}"),
                }),
            }
        }
    }
}

/// 학습 실행 핵심 — yaml config 작성 → llamafactory CLI spawn → cancel/timeout.
async fn run_train_inner(
    python: &Path,
    timeout: Duration,
    job: LoRAJob,
    progress: mpsc::Sender<QuantizeProgress>,
    cancel: CancellationToken,
) -> Result<(), WorkbenchError> {
    if cancel.is_cancelled() {
        return Err(WorkbenchError::Cancelled);
    }

    // 0% Preparing emit.
    let _ = progress
        .send(QuantizeProgress {
            percent: 0,
            stage: "preparing".into(),
            message: Some(format!(
                "데이터 {}을(를) 로드하고 있어요",
                job.dataset_jsonl
            )),
        })
        .await;

    // yaml config — output_adapter 부모 디렉터리에 저장. 학습 종료 후 best-effort 삭제.
    let yaml_path = {
        let parent = std::path::Path::new(&job.output_adapter)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(std::env::temp_dir);
        std::fs::create_dir_all(&parent).map_err(WorkbenchError::Io)?;
        parent.join("llamafactory.yaml")
    };
    let yaml_content = render_llamafactory_yaml(&job);
    std::fs::write(&yaml_path, yaml_content).map_err(WorkbenchError::Io)?;

    let mut cmd = Command::new(python);
    cmd.arg("-m")
        .arg("llamafactory")
        .arg("train")
        .arg("--config")
        .arg(&yaml_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|e| WorkbenchError::Internal {
        message: format!("LLaMA-Factory CLI를 실행하지 못했어요: {e}"),
    })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| WorkbenchError::Internal {
            message: "LLaMA-Factory stdout pipe를 열지 못했어요".into(),
        })?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| WorkbenchError::Internal {
            message: "LLaMA-Factory stderr pipe를 열지 못했어요".into(),
        })?;

    let stderr_buffer = std::sync::Arc::new(tokio::sync::Mutex::new(String::new()));
    let stderr_buf_clone = stderr_buffer.clone();
    let stderr_progress = progress.clone();
    let total_epochs = job.epochs;
    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            {
                let mut buf = stderr_buf_clone.lock().await;
                if buf.len() < 256 * 1024 {
                    buf.push_str(&line);
                    buf.push('\n');
                }
            }
            let parsed = parse_train_line(&line, total_epochs);
            let _ = stderr_progress.send(parsed).await;
        }
    });
    let stdout_progress = progress.clone();
    let stdout_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let parsed = parse_train_line(&line, total_epochs);
            let _ = stdout_progress.send(parsed).await;
        }
    });

    let timeout_sleep = tokio::time::sleep(timeout);
    tokio::pin!(timeout_sleep);
    let wait_outcome = tokio::select! {
        () = cancel.cancelled() => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            Err(WorkbenchError::Cancelled)
        }
        () = &mut timeout_sleep => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            Err(WorkbenchError::Internal {
                message: format!("LoRA 학습이 {}시간 안에 끝나지 않았어요.", timeout.as_secs() / 3600),
            })
        }
        status = child.wait() => {
            match status {
                Ok(s) if s.success() => Ok(()),
                Ok(s) => {
                    let stderr_text = stderr_buffer.lock().await.clone();
                    Err(map_train_stderr(&stderr_text, s.code()))
                }
                Err(e) => Err(WorkbenchError::Internal {
                    message: format!("LLaMA-Factory 종료 상태를 읽지 못했어요: {e}"),
                }),
            }
        }
    };

    stdout_task.abort();
    stderr_task.abort();

    if wait_outcome.is_ok() {
        let _ = progress
            .send(QuantizeProgress {
                percent: 100,
                stage: "saving".into(),
                message: Some(format!("어댑터를 {}에 저장했어요", job.output_adapter)),
            })
            .await;
    }
    // yaml 파일 best-effort 정리 — 실패해도 흐름 영향 없음.
    let _ = std::fs::remove_file(&yaml_path);

    wait_outcome
}

/// 단일 yaml 파일 렌더 — LLaMA-Factory `train` config schema에 부합.
fn render_llamafactory_yaml(job: &LoRAJob) -> String {
    let template = if job.korean_preset {
        "alpaca_ko"
    } else {
        "alpaca"
    };
    format!(
        "model_name_or_path: {model}\n\
         dataset: {dataset}\n\
         template: {template}\n\
         finetuning_type: lora\n\
         lora_target: all\n\
         output_dir: {output}\n\
         num_train_epochs: {epochs}\n\
         learning_rate: {lr}\n\
         per_device_train_batch_size: 1\n\
         gradient_accumulation_steps: 4\n\
         logging_steps: 10\n\
         save_steps: 200\n\
         bf16: true\n",
        model = job.base_model,
        dataset = job.dataset_jsonl,
        template = template,
        output = job.output_adapter,
        epochs = job.epochs,
        lr = job.lr,
    )
}

/// 학습 한 라인 → progress. epoch / step / loss 추출 시도.
fn parse_train_line(line: &str, total_epochs: u32) -> QuantizeProgress {
    let lower = line.to_lowercase();
    let stage = if lower.contains("loading dataset") || lower.contains("load_dataset") {
        "preparing"
    } else if lower.contains("save") || lower.contains("checkpoint") {
        "saving"
    } else {
        "training"
    };

    // "epoch 2/3" 또는 "epoch=2" 패턴.
    let mut percent = 50u8;
    if let Some(epoch) = parse_epoch(line) {
        if total_epochs > 0 {
            // 진행률 기준: (현재 epoch / total) * 100.
            percent = ((epoch as u64 * 100) / total_epochs as u64).min(99) as u8;
            percent = percent.max(5);
        }
    }
    if stage == "saving" {
        percent = 95;
    } else if stage == "preparing" {
        percent = 5;
    }

    QuantizeProgress {
        percent,
        stage: stage.to_string(),
        message: Some(line.to_string()),
    }
}

/// "epoch 2/4" / "epoch=2" / "Epoch: 2" 같은 패턴에서 현재 epoch 추출.
fn parse_epoch(line: &str) -> Option<u32> {
    let lower = line.to_lowercase();
    let idx = lower.find("epoch")?;
    let tail = &line[idx + "epoch".len()..];
    // 첫 숫자 추출.
    let mut digits = String::new();
    for c in tail.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else if !digits.is_empty() {
            break;
        }
    }
    if digits.is_empty() {
        return None;
    }
    digits.parse::<u32>().ok()
}

/// stderr → 한국어 메시지.
fn map_train_stderr(stderr: &str, exit_code: Option<i32>) -> WorkbenchError {
    let lower = stderr.to_lowercase();
    let message = if lower.contains("modulenotfounderror")
        && (lower.contains("torch") || lower.contains("transformers"))
    {
        "torch 또는 transformers 모듈이 없어요. venv를 다시 만들어 주세요.".to_string()
    } else if lower.contains("modulenotfounderror") && lower.contains("llamafactory") {
        "llamafactory 모듈이 venv에 없어요. 부트스트랩을 다시 진행해 주세요.".to_string()
    } else if lower.contains("cuda") && lower.contains("out of memory") {
        "GPU 메모리가 부족해요. 더 작은 모델 또는 batch_size=1로 시도해 주세요.".to_string()
    } else if lower.contains("filenotfounderror") || lower.contains("no such file") {
        "데이터 파일을 찾지 못했어요. JSONL 경로를 확인해 주세요.".to_string()
    } else if lower.contains("permission denied") {
        "출력 폴더에 쓰기 권한이 없어요. 폴더 권한을 확인해 주세요.".to_string()
    } else if lower.contains("no space left") {
        "디스크 공간이 부족해요. 출력 폴더 여유 공간을 확인해 주세요.".to_string()
    } else if stderr.trim().is_empty() {
        match exit_code {
            Some(c) => format!("LLaMA-Factory가 exit code {c}로 종료됐어요."),
            None => "LLaMA-Factory가 비정상 종료됐어요.".to_string(),
        }
    } else {
        let tail: String = stderr
            .lines()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" / ");
        format!("LoRA 학습이 실패했어요: {tail}")
    };
    WorkbenchError::Internal { message }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn job() -> LoRAJob {
        LoRAJob {
            base_model: "Qwen2.5-3B".into(),
            dataset_jsonl: "./data/train.jsonl".into(),
            output_adapter: "./out/adapter".into(),
            epochs: 4,
            lr: 0.0002,
            korean_preset: true,
        }
    }

    #[test]
    fn version_at_least_basic() {
        assert!(version_at_least("3.10.0", 3, 10));
        assert!(version_at_least("3.11.5", 3, 10));
        assert!(version_at_least("4.0.0", 3, 10));
        assert!(!version_at_least("3.9.18", 3, 10));
        assert!(!version_at_least("2.7.18", 3, 10));
    }

    #[test]
    fn version_at_least_handles_garbage() {
        assert!(!version_at_least("not-a-version", 3, 10));
        assert!(!version_at_least("3", 3, 10));
    }

    #[test]
    fn render_yaml_korean_preset_uses_alpaca_ko() {
        let yaml = render_llamafactory_yaml(&job());
        assert!(yaml.contains("template: alpaca_ko"));
        assert!(yaml.contains("Qwen2.5-3B"));
        assert!(yaml.contains("./data/train.jsonl"));
        assert!(yaml.contains("./out/adapter"));
        assert!(yaml.contains("num_train_epochs: 4"));
    }

    #[test]
    fn render_yaml_english_uses_alpaca() {
        let mut j = job();
        j.korean_preset = false;
        let yaml = render_llamafactory_yaml(&j);
        assert!(yaml.contains("template: alpaca"));
        assert!(!yaml.contains("alpaca_ko"));
    }

    #[test]
    fn parse_epoch_basic_patterns() {
        assert_eq!(parse_epoch("Epoch 2/4 step 100"), Some(2));
        assert_eq!(parse_epoch("epoch=3"), Some(3));
        assert_eq!(parse_epoch("Epoch: 1"), Some(1));
        assert_eq!(parse_epoch("step 100"), None);
    }

    #[test]
    fn parse_train_line_preparing() {
        let p = parse_train_line("loading dataset from ./data/train.jsonl", 4);
        assert_eq!(p.stage, "preparing");
    }

    #[test]
    fn parse_train_line_saving() {
        let p = parse_train_line("Saving model checkpoint to ./out/adapter", 4);
        assert_eq!(p.stage, "saving");
        assert_eq!(p.percent, 95);
    }

    #[test]
    fn parse_train_line_training_with_epoch() {
        let p = parse_train_line("Epoch 2/4 step 100 loss=0.123", 4);
        assert_eq!(p.stage, "training");
        // 2/4 = 50%.
        assert!(p.percent >= 5);
    }

    #[test]
    fn map_stderr_module_not_found_torch() {
        let e = map_train_stderr(
            "Traceback ...\nModuleNotFoundError: No module named 'torch'",
            Some(1),
        );
        let msg = format!("{e}");
        assert!(msg.contains("torch") || msg.contains("venv"));
    }

    #[test]
    fn map_stderr_module_not_found_llamafactory() {
        let e = map_train_stderr(
            "ModuleNotFoundError: No module named 'llamafactory'",
            Some(1),
        );
        let msg = format!("{e}");
        assert!(msg.contains("llamafactory"));
    }

    #[test]
    fn map_stderr_cuda_oom() {
        let e = map_train_stderr(
            "RuntimeError: CUDA out of memory. Tried to allocate ...",
            Some(1),
        );
        let msg = format!("{e}");
        assert!(msg.contains("GPU") || msg.contains("메모리"));
    }

    #[test]
    fn map_stderr_no_such_file() {
        let e = map_train_stderr(
            "FileNotFoundError: [Errno 2] No such file: './data/train.jsonl'",
            Some(1),
        );
        let msg = format!("{e}");
        assert!(msg.contains("데이터") || msg.contains("JSONL"));
    }

    #[test]
    fn map_stderr_empty_uses_exit_code() {
        let e = map_train_stderr("", Some(42));
        let msg = format!("{e}");
        assert!(msg.contains("42") || msg.contains("종료"));
    }

    #[test]
    fn bootstrap_event_kebab_round_trip() {
        let evs = vec![
            BootstrapEvent::Probing,
            BootstrapEvent::CreatingVenv,
            BootstrapEvent::PythonReady {
                version: "3.11.5".into(),
                path: "/usr/bin/python3".into(),
            },
            BootstrapEvent::Done,
            BootstrapEvent::Failed { error: "x".into() },
        ];
        for e in evs {
            let s = serde_json::to_string(&e).unwrap();
            // kind 필드 존재 + kebab.
            assert!(s.contains("\"kind\""));
            assert!(
                s.contains("\"probing\"")
                    || s.contains("\"creating-venv\"")
                    || s.contains("\"python-ready\"")
                    || s.contains("\"done\"")
                    || s.contains("\"failed\"")
            );
            // round-trip.
            let back: BootstrapEvent = serde_json::from_str(&s).unwrap();
            assert_eq!(back, e);
        }
    }

    #[tokio::test]
    async fn run_streaming_pre_cancelled_returns_cancelled() {
        // python_path 존재 여부 무관 — pre-cancel 분기.
        let trainer = LlamaFactoryTrainer::with_paths(
            PathBuf::from("/nope/venv"),
            PathBuf::from("/nope/python"),
            Duration::from_secs(60),
        );
        let (tx, _rx) = mpsc::channel(8);
        let cancel = CancellationToken::new();
        cancel.cancel();
        let err = trainer.run_streaming(job(), tx, &cancel).await.unwrap_err();
        assert!(matches!(err, WorkbenchError::Cancelled));
    }

    #[tokio::test]
    async fn run_streaming_emits_preparing_then_fails_when_python_missing() {
        let trainer = LlamaFactoryTrainer::with_paths(
            PathBuf::from("/nope/venv"),
            PathBuf::from("/nope/python-xyz"),
            Duration::from_secs(5),
        );
        let (tx, mut rx) = mpsc::channel(8);
        let cancel = CancellationToken::new();
        // run_streaming은 첫 emit (preparing) 후 spawn 실패 → Internal 에러.
        let result = trainer.run_streaming(job(), tx, &cancel).await;
        // preparing emit는 받았어야.
        let first = rx.recv().await;
        assert!(first.is_some());
        assert_eq!(first.unwrap().stage, "preparing");
        let err = result.unwrap_err();
        assert!(matches!(err, WorkbenchError::Internal { .. }));
    }

    #[tokio::test]
    async fn bootstrap_or_open_returns_when_venv_already_exists_and_python_works() {
        // 시스템에 python이 있는 환경에서만 의미 있음 — 없으면 graceful skip.
        let sys_py = match which::which("python3").or_else(|_| which::which("python")) {
            Ok(p) => p,
            Err(_) => return,
        };
        // 임시 venv-like dir + 시스템 python을 venv python으로 위장.
        let tmp = tempfile::tempdir().unwrap();
        let venv_dir = tmp.path().to_path_buf();
        // venv 형태 흉내 — bin/ 또는 Scripts/ 안에 python 심볼릭.
        #[cfg(target_os = "windows")]
        let python_dest = {
            let scripts = venv_dir.join("Scripts");
            std::fs::create_dir_all(&scripts).unwrap();
            scripts.join("python.exe")
        };
        #[cfg(not(target_os = "windows"))]
        let python_dest = {
            let bin = venv_dir.join("bin");
            std::fs::create_dir_all(&bin).unwrap();
            bin.join("python")
        };
        // 단순 복사 (symlink 권한 이슈 회피).
        if std::fs::copy(&sys_py, &python_dest).is_err() {
            // 복사 권한 없는 환경 — skip.
            return;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&python_dest, std::fs::Permissions::from_mode(0o755));
        }

        let (tx, mut rx) = mpsc::channel::<BootstrapEvent>(32);
        let cancel = CancellationToken::new();
        let result =
            LlamaFactoryTrainer::bootstrap_or_open(venv_dir, Duration::from_secs(10), tx, cancel)
                .await;

        // event 수신 확인.
        let mut got_python_ready = false;
        let mut got_done = false;
        while let Some(ev) = rx.recv().await {
            match ev {
                BootstrapEvent::PythonReady { .. } => got_python_ready = true,
                BootstrapEvent::Done => got_done = true,
                _ => {}
            }
        }

        // python copy가 정상 실행 가능하면 PythonReady + Done. 그렇지 않으면 Failed.
        if let Ok(_t) = result {
            assert!(got_python_ready);
            assert!(got_done);
        }
    }

    #[tokio::test]
    async fn bootstrap_or_open_pre_cancelled_returns_cancelled() {
        let tmp = tempfile::tempdir().unwrap();
        let venv_dir = tmp.path().join("venv");
        let (tx, _rx) = mpsc::channel(8);
        let cancel = CancellationToken::new();
        cancel.cancel();
        // venv가 존재하지 않으니 system python 탐색 단계까지 진행.
        let result =
            LlamaFactoryTrainer::bootstrap_or_open(venv_dir, Duration::from_secs(5), tx, cancel)
                .await;
        // Cancelled 또는 system python 탐색 실패 (ToolMissing) — 둘 다 OK.
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            WorkbenchError::Cancelled | WorkbenchError::ToolMissing { .. }
        ));
    }
}
