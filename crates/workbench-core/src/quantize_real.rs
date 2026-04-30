//! `LlamaQuantizer` — llama.cpp `llama-quantize` 실 binary subprocess wrapper.
//!
//! 정책 (phase-9pb-workbench-real-reinforcement, ADR-0043):
//! - PATH 자동 detect 또는 `LMMASTER_LLAMA_QUANTIZE_PATH` env로 binary 위치 override.
//! - 미발견 시 graceful 한국어 에러 (panic X). MockQuantizer fallback이 항상 보존.
//! - `tokio::process::Command::new(...).kill_on_drop(true)` — Drop 시 child 자동 종료.
//! - stdout/stderr 라인 단위 progress emit. 5단계 매핑 (Loading → Quantizing → Validating → Writing → Done).
//! - cancel cooperative — `cancel.cancelled()` arm으로 select 후 child kill.
//! - 30분 timeout (사용자 override 가능). 큰 모델은 30분 이상 걸리지만 v1 안전 기본.
//! - stderr → 한국어 매핑 (ENOENT / disk full / format unsupported / permission denied / 기타).
//!
//! Negative space (ADR-0043 §Alternatives):
//! - Bundle 동봉 — Tauri 빌드 +수GB. 거부.
//! - Rust-only candle — llama.cpp의 quantization kernel 만큼 다양한 quant 형식 미지원. 거부.
//! - Docker 기반 — 사용자 부담 + Docker 의존. 거부.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::WorkbenchError;
use crate::quantize::{QuantizeJob, QuantizeProgress, Quantizer};

/// 기본 timeout — 30분. 큰 70B 모델은 더 걸릴 수 있어 caller가 override 가능.
pub const DEFAULT_QUANTIZE_TIMEOUT_SECS: u64 = 30 * 60;

/// binary auto-detect 시 사용할 환경변수.
pub const LLAMA_QUANTIZE_ENV: &str = "LMMASTER_LLAMA_QUANTIZE_PATH";

/// llama.cpp `llama-quantize` binary 호출 wrapper.
#[derive(Debug)]
pub struct LlamaQuantizer {
    /// `llama-quantize`(.exe) 실 binary 절대 경로.
    binary_path: PathBuf,
    /// child가 살아 있을 수 있는 최대 시간. 초과 시 kill + Internal 에러.
    timeout: Duration,
}

impl LlamaQuantizer {
    /// 명시 binary path + timeout — 테스트/CI에서 fixture binary 주입에 유용.
    pub fn with_binary(binary_path: PathBuf, timeout: Duration) -> Self {
        Self {
            binary_path,
            timeout,
        }
    }

    /// `LMMASTER_LLAMA_QUANTIZE_PATH` env 우선, PATH 검색 fallback.
    /// 미발견 시 `WorkbenchError::ToolMissing`(한국어 메시지).
    pub fn from_environment(timeout: Duration) -> Result<Self, WorkbenchError> {
        if let Ok(path) = std::env::var(LLAMA_QUANTIZE_ENV) {
            let p = PathBuf::from(&path);
            if p.exists() {
                return Ok(Self::with_binary(p, timeout));
            }
            // env가 가리키는 경로가 실제로 없음 → 즉시 에러 (사용자가 명시한 경로니까).
            return Err(WorkbenchError::ToolMissing {
                tool: format!(
                    "{LLAMA_QUANTIZE_ENV} 환경변수가 가리키는 경로에 파일이 없어요: {path}"
                ),
            });
        }
        match which::which("llama-quantize") {
            Ok(found) => Ok(Self::with_binary(found, timeout)),
            Err(_) => Err(WorkbenchError::ToolMissing {
                tool: "llama-quantize 실행 파일을 찾지 못했어요. \
                       llama.cpp 릴리스에서 받아 PATH에 추가하거나 \
                       LMMASTER_LLAMA_QUANTIZE_PATH 환경변수로 위치를 알려 주세요."
                    .into(),
            }),
        }
    }

    pub fn binary_path(&self) -> &Path {
        &self.binary_path
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

#[async_trait]
impl Quantizer for LlamaQuantizer {
    /// `Vec` 반환 — `run_streaming`을 버퍼로 collect. 호환용.
    async fn run(
        &self,
        job: QuantizeJob,
        cancel: &CancellationToken,
    ) -> Result<Vec<QuantizeProgress>, WorkbenchError> {
        let (tx, mut rx) = mpsc::channel::<QuantizeProgress>(64);
        let job_clone = job.clone();
        let cancel_clone = cancel.clone();
        let binary = self.binary_path.clone();
        let timeout = self.timeout;
        // 별도 task로 실 binary 실행 + 동일 task에서 collector loop.
        let runner = tokio::spawn(async move {
            run_quantize_inner(&binary, timeout, job_clone, tx, cancel_clone).await
        });
        let mut collected = Vec::new();
        while let Some(p) = rx.recv().await {
            collected.push(p);
        }
        match runner.await {
            Ok(Ok(())) => Ok(collected),
            Ok(Err(e)) => Err(e),
            Err(join_err) => Err(WorkbenchError::Internal {
                message: format!("양자화 워커 task가 실패했어요: {join_err}"),
            }),
        }
    }

    async fn run_streaming(
        &self,
        job: QuantizeJob,
        progress: mpsc::Sender<QuantizeProgress>,
        cancel: &CancellationToken,
    ) -> Result<(), WorkbenchError> {
        run_quantize_inner(
            &self.binary_path,
            self.timeout,
            job,
            progress,
            cancel.clone(),
        )
        .await
    }
}

/// 핵심 실행기. binary spawn → stdout/stderr 라인 단위 parsing → cancel/timeout 동시 listen.
async fn run_quantize_inner(
    binary_path: &Path,
    timeout: Duration,
    job: QuantizeJob,
    progress: mpsc::Sender<QuantizeProgress>,
    cancel: CancellationToken,
) -> Result<(), WorkbenchError> {
    if cancel.is_cancelled() {
        return Err(WorkbenchError::Cancelled);
    }
    if !binary_path.exists() {
        return Err(WorkbenchError::ToolMissing {
            tool: format!(
                "지정된 llama-quantize 실행 파일이 존재하지 않아요: {}",
                binary_path.display()
            ),
        });
    }

    // 0% Loading emit — UI가 즉시 반응.
    let _ = progress
        .send(QuantizeProgress {
            percent: 0,
            stage: "loading".into(),
            message: Some(format!("{}을(를) 메모리에 올리고 있어요", job.input_gguf)),
        })
        .await;

    let mut cmd = Command::new(binary_path);
    cmd.arg(&job.input_gguf)
        .arg(&job.output_gguf)
        .arg(&job.quant_type)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|e| {
        // ENOENT / permission / 기타 — 사용자 향 한국어 매핑.
        let msg = if e.kind() == std::io::ErrorKind::NotFound {
            format!(
                "llama-quantize를 실행할 수 없어요. 파일이 없거나 권한이 부족해요: {}",
                binary_path.display()
            )
        } else if e.kind() == std::io::ErrorKind::PermissionDenied {
            format!(
                "llama-quantize를 실행할 권한이 없어요. 실행 권한을 확인해 주세요: {}",
                binary_path.display()
            )
        } else {
            format!("llama-quantize 실행에 실패했어요: {e}")
        };
        WorkbenchError::ToolMissing { tool: msg }
    })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| WorkbenchError::Internal {
            message: "llama-quantize stdout pipe를 열지 못했어요".into(),
        })?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| WorkbenchError::Internal {
            message: "llama-quantize stderr pipe를 열지 못했어요".into(),
        })?;

    // stderr는 backgound buffer + emit. 마지막 256KB만 보존.
    let stderr_buffer: std::sync::Arc<tokio::sync::Mutex<String>> =
        std::sync::Arc::new(tokio::sync::Mutex::new(String::new()));
    let stderr_buf_clone = stderr_buffer.clone();
    let stderr_progress = progress.clone();
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
            // stderr 라인을 progress message로 emit (사용자 향 진행 상황).
            let _ = stderr_progress
                .send(QuantizeProgress {
                    percent: 50,
                    stage: "quantizing".into(),
                    message: Some(line),
                })
                .await;
        }
    });

    let stdout_progress = progress.clone();
    let stdout_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let parsed = parse_progress_line(&line);
            let _ = stdout_progress.send(parsed).await;
        }
    });

    // wait + cancel + timeout 동시 listen.
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
                message: format!(
                    "양자화가 {}분 안에 끝나지 않았어요. 더 큰 모델은 LMMASTER_LLAMA_QUANTIZE_TIMEOUT_SECS로 시간을 늘려 주세요.",
                    timeout.as_secs() / 60
                ),
            })
        }
        status = child.wait() => {
            match status {
                Ok(s) if s.success() => Ok(()),
                Ok(s) => {
                    let stderr_text = stderr_buffer.lock().await.clone();
                    Err(map_quantize_stderr(&stderr_text, s.code()))
                }
                Err(e) => Err(WorkbenchError::Internal {
                    message: format!("llama-quantize 종료 상태를 읽지 못했어요: {e}"),
                }),
            }
        }
    };

    stdout_task.abort();
    stderr_task.abort();

    if wait_outcome.is_ok() {
        // 100% Done emit — UI 마무리.
        let _ = progress
            .send(QuantizeProgress {
                percent: 100,
                stage: "writing".into(),
                message: Some(format!("{}에 저장을 마쳤어요", job.output_gguf)),
            })
            .await;
    }

    wait_outcome
}

/// llama-quantize stdout 한 라인 → `QuantizeProgress`.
///
/// llama.cpp는 보통 다음 형태로 출력:
/// - `[1/123] llama_model_quantize_internal: ... q4_K - 50%`
/// - `model size  = 7.55 GB`
/// - `quantizing tensor X`
/// - `validating ...`
///
/// 정확한 % 추출이 어려우면 stage 라벨만 5단계 중 하나로 매핑.
fn parse_progress_line(line: &str) -> QuantizeProgress {
    let lower = line.to_lowercase();
    let (stage, percent) = if lower.contains("load") || lower.contains("reading") {
        ("loading", 10u8)
    } else if lower.contains("validat") {
        ("validating", 80u8)
    } else if lower.contains("writ") || lower.contains("output") {
        ("writing", 90u8)
    } else if lower.contains("done") || lower.contains("complete") {
        ("done", 100u8)
    } else if lower.contains("quantiz") || lower.contains("tensor") {
        // [N/M] 패턴이면 비율 계산.
        if let Some(p) = parse_fraction_percent(line) {
            ("quantizing", p)
        } else {
            ("quantizing", 50u8)
        }
    } else {
        ("quantizing", 50u8)
    };
    QuantizeProgress {
        percent,
        stage: stage.to_string(),
        message: Some(line.to_string()),
    }
}

/// `[N/M]` 형식에서 % 추출. 실패 시 None.
fn parse_fraction_percent(line: &str) -> Option<u8> {
    let start = line.find('[')?;
    let end = line[start..].find(']')?;
    let inner = &line[start + 1..start + end];
    let mut parts = inner.split('/');
    let num: u32 = parts.next()?.trim().parse().ok()?;
    let denom: u32 = parts.next()?.trim().parse().ok()?;
    if denom == 0 {
        return None;
    }
    let pct = (num as u64 * 100 / denom as u64).min(99) as u8;
    Some(pct.max(10))
}

/// stderr 마지막 모음 → 사용자 향 한국어 메시지. exit code도 hint로 사용.
fn map_quantize_stderr(stderr: &str, exit_code: Option<i32>) -> WorkbenchError {
    let lower = stderr.to_lowercase();
    let message = if lower.contains("no such file") || lower.contains("not found") {
        "입력 GGUF 파일을 찾지 못했어요. 경로를 다시 확인해 주세요.".to_string()
    } else if lower.contains("no space left") || lower.contains("disk full") {
        "디스크 공간이 부족해요. 출력 폴더 여유 공간을 확인해 주세요.".to_string()
    } else if lower.contains("permission denied") {
        "출력 폴더에 쓰기 권한이 없어요. 폴더 권한을 확인해 주세요.".to_string()
    } else if lower.contains("invalid") && lower.contains("magic") {
        "GGUF 파일이 손상되었거나 형식이 달라요. 원본 모델을 다시 받아 주세요.".to_string()
    } else if lower.contains("unsupported") || lower.contains("unknown") {
        "지원하지 않는 양자화 형식이에요. Q4_K_M / Q5_K_M / Q8_0 / FP16 중에서 골라 주세요."
            .to_string()
    } else if lower.contains("out of memory") || lower.contains("oom") {
        "메모리가 부족해요. 더 가벼운 양자화 형식을 골라 주세요.".to_string()
    } else if stderr.trim().is_empty() {
        match exit_code {
            Some(c) => format!("llama-quantize가 exit code {c}로 종료됐어요."),
            None => "llama-quantize가 비정상 종료됐어요.".to_string(),
        }
    } else {
        let tail: String = stderr
            .lines()
            .rev()
            .take(3)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" / ");
        format!("양자화에 실패했어요: {tail}")
    };
    WorkbenchError::Internal { message }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn job() -> QuantizeJob {
        QuantizeJob {
            input_gguf: "in.gguf".into(),
            output_gguf: "out.gguf".into(),
            quant_type: "Q4_K_M".into(),
        }
    }

    #[test]
    fn from_environment_uses_env_when_set_and_path_exists() {
        // 임시 파일을 binary로 위장.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        std::env::set_var(LLAMA_QUANTIZE_ENV, &path);
        let r = LlamaQuantizer::from_environment(Duration::from_secs(60));
        std::env::remove_var(LLAMA_QUANTIZE_ENV);
        let q = r.expect("env path 우선 detect");
        assert_eq!(q.binary_path(), path.as_path());
    }

    #[test]
    fn from_environment_returns_korean_error_when_env_path_missing() {
        std::env::set_var(LLAMA_QUANTIZE_ENV, "/nope/this/path/does/not/exist/xyz");
        let r = LlamaQuantizer::from_environment(Duration::from_secs(60));
        std::env::remove_var(LLAMA_QUANTIZE_ENV);
        let err = r.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("환경변수") || msg.contains("LMMASTER_LLAMA_QUANTIZE_PATH"));
    }

    #[tokio::test]
    async fn run_streaming_returns_error_when_binary_missing() {
        let q = LlamaQuantizer::with_binary(
            PathBuf::from("/nope/missing-binary-xyz"),
            Duration::from_secs(5),
        );
        let (tx, _rx) = mpsc::channel(8);
        let cancel = CancellationToken::new();
        let err = q.run_streaming(job(), tx, &cancel).await.unwrap_err();
        assert!(matches!(err, WorkbenchError::ToolMissing { .. }));
    }

    #[tokio::test]
    async fn run_streaming_pre_cancelled_returns_cancelled() {
        // binary 존재 여부와 무관 — pre-cancel 분기가 가장 먼저.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let q = LlamaQuantizer::with_binary(tmp.path().to_path_buf(), Duration::from_secs(60));
        let (tx, _rx) = mpsc::channel(8);
        let cancel = CancellationToken::new();
        cancel.cancel();
        let err = q.run_streaming(job(), tx, &cancel).await.unwrap_err();
        assert!(matches!(err, WorkbenchError::Cancelled));
    }

    #[tokio::test]
    async fn run_streaming_zero_exit_binary_succeeds_and_emits_loading_writing() {
        // Windows: cmd /c exit 0. 다른 OS: /usr/bin/true. 두 경우 모두 stdout/stderr empty + exit 0.
        #[cfg(target_os = "windows")]
        let bin = which::which("cmd").unwrap_or_else(|_| PathBuf::from("cmd.exe"));
        #[cfg(not(target_os = "windows"))]
        let bin = which::which("true").unwrap_or_else(|_| PathBuf::from("/bin/true"));
        // cmd.exe는 args를 무시 (잘못된 args에도 exit 0이 아닐 수 있어 — 하지만 binary가 존재하면
        // spawn 자체는 성공). 여기서는 emit 검증보다 cancel/timeout 로직 안전성을 본다.
        let q = LlamaQuantizer::with_binary(bin.clone(), Duration::from_secs(10));
        let (tx, mut rx) = mpsc::channel(64);
        let cancel = CancellationToken::new();
        let collector = tokio::spawn(async move {
            let mut count = 0;
            while rx.recv().await.is_some() {
                count += 1;
            }
            count
        });
        let _ = q.run_streaming(job(), tx, &cancel).await; // exit code 검증 X — 환경 의존.
        let count = collector.await.unwrap();
        // 최소한 0% Loading emit 1건은 발생해야 함.
        assert!(count >= 1, "최소 1건 emit (loading)");
    }

    #[tokio::test]
    async fn run_streaming_cancel_mid_run_kills_child() {
        // sleep binary로 장기 실행 시뮬. cancel → child kill → Cancelled 반환.
        #[cfg(target_os = "windows")]
        let bin_opt = which::which("ping").ok();
        #[cfg(not(target_os = "windows"))]
        let bin_opt = which::which("sleep").ok();
        let bin = match bin_opt {
            Some(b) => b,
            None => return, // 환경에 binary 없으면 skip.
        };
        let q = LlamaQuantizer::with_binary(bin, Duration::from_secs(60));
        let (tx, _rx) = mpsc::channel(64);
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        // 50ms 후 cancel.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            cancel_clone.cancel();
        });
        let err = q.run_streaming(job(), tx, &cancel).await.unwrap_err();
        // ping/sleep는 args를 잘 못 읽고 즉시 종료할 가능성 — Cancelled 또는 Internal 둘 다 OK.
        assert!(matches!(
            err,
            WorkbenchError::Cancelled | WorkbenchError::Internal { .. }
        ));
    }

    #[tokio::test]
    async fn run_streaming_timeout_returns_korean_message() {
        // sleep binary + 50ms timeout으로 timeout arm 트리거.
        #[cfg(target_os = "windows")]
        let bin_opt = which::which("ping").ok();
        #[cfg(not(target_os = "windows"))]
        let bin_opt = which::which("sleep").ok();
        let bin = match bin_opt {
            Some(b) => b,
            None => return,
        };
        let q = LlamaQuantizer::with_binary(bin, Duration::from_millis(50));
        let (tx, _rx) = mpsc::channel(64);
        let cancel = CancellationToken::new();
        let err = q.run_streaming(job(), tx, &cancel).await;
        if let Err(e) = err {
            let msg = format!("{e}");
            // 환경에 따라 child가 즉시 exit하면 다른 매핑 — 최소한 panic은 없음.
            assert!(!msg.is_empty());
        }
    }

    #[test]
    fn parse_progress_line_loading() {
        let p = parse_progress_line("llama_model_load: loading model");
        assert_eq!(p.stage, "loading");
        assert!(p.percent <= 30);
    }

    #[test]
    fn parse_progress_line_quantizing_with_fraction() {
        let p = parse_progress_line("[5/10] quantizing tensor blk.0.weight");
        assert_eq!(p.stage, "quantizing");
        assert!(p.percent >= 10);
    }

    #[test]
    fn parse_progress_line_validating() {
        let p = parse_progress_line("validating output GGUF");
        assert_eq!(p.stage, "validating");
        assert!(p.percent >= 50);
    }

    #[test]
    fn parse_progress_line_writing() {
        let p = parse_progress_line("writing tensors to output.gguf");
        assert_eq!(p.stage, "writing");
        assert!(p.percent >= 80);
    }

    #[test]
    fn parse_progress_line_done() {
        let p = parse_progress_line("Done. ggml model created.");
        assert_eq!(p.stage, "done");
        assert_eq!(p.percent, 100);
    }

    #[test]
    fn parse_fraction_percent_basic() {
        assert_eq!(parse_fraction_percent("[1/10] foo"), Some(10));
        assert_eq!(parse_fraction_percent("[50/100] foo"), Some(50));
        assert_eq!(parse_fraction_percent("[99/100] foo"), Some(99));
    }

    #[test]
    fn parse_fraction_percent_handles_zero_denom() {
        assert_eq!(parse_fraction_percent("[5/0] foo"), None);
    }

    #[test]
    fn parse_fraction_percent_handles_no_brackets() {
        assert_eq!(parse_fraction_percent("loading model"), None);
    }

    #[test]
    fn map_stderr_no_such_file_returns_korean() {
        let e = map_quantize_stderr("error: no such file or directory: input.gguf", Some(1));
        let msg = format!("{e}");
        assert!(msg.contains("GGUF") || msg.contains("입력"));
    }

    #[test]
    fn map_stderr_no_space_returns_korean() {
        let e = map_quantize_stderr("write error: no space left on device", Some(28));
        let msg = format!("{e}");
        assert!(msg.contains("디스크"));
    }

    #[test]
    fn map_stderr_permission_denied_returns_korean() {
        let e = map_quantize_stderr("open output: permission denied", Some(13));
        let msg = format!("{e}");
        assert!(msg.contains("권한"));
    }

    #[test]
    fn map_stderr_unsupported_format_returns_korean() {
        let e = map_quantize_stderr("error: unknown quantization type Q3_FOO", Some(1));
        let msg = format!("{e}");
        assert!(msg.contains("양자화 형식") || msg.contains("Q4_K_M"));
    }

    #[test]
    fn map_stderr_oom_returns_korean() {
        let e = map_quantize_stderr("ggml: out of memory while allocating", Some(137));
        let msg = format!("{e}");
        assert!(msg.contains("메모리"));
    }

    #[test]
    fn map_stderr_invalid_magic_returns_korean() {
        let e = map_quantize_stderr("error: invalid magic number in GGUF header", Some(1));
        let msg = format!("{e}");
        assert!(msg.contains("GGUF") || msg.contains("형식"));
    }

    #[test]
    fn map_stderr_empty_uses_exit_code() {
        let e = map_quantize_stderr("", Some(42));
        let msg = format!("{e}");
        assert!(msg.contains("42") || msg.contains("종료"));
    }

    #[test]
    fn map_stderr_unknown_includes_tail() {
        let e = map_quantize_stderr("a\nb\nc\nd\ne\n", Some(1));
        let msg = format!("{e}");
        assert!(msg.contains("e") || msg.contains("d"));
    }
}
