//! 설치 액션 실행기 — manifest의 `install` 객체를 실제 OS 동작으로 변환.
//!
//! 정책 (ADR-0017, Phase 1A.3.b 보강 리서치):
//! - `tokio::process::Command` 사용 (async + cancel-on-kill + biased select).
//! - `tauri-plugin-shell` capability scope는 동적 EXE 경로에 부적합 — 우리 crate 내부에서
//!   `tokio::process::Command` 직접 사용. Tauri command boundary가 보안 perimeter.
//! - Inno Setup / NSIS / MSI 정확한 exit code 인식 (성공: 0 + 추가 success_exit_codes).
//! - 사용자 cancel: `CancellationToken` → `child.start_kill()` → 5s wait → return.
//! - 설치 후 `.partial` 등 임시 파일은 다운로더가 이미 정리.
//! - `open_url`: `webbrowser` crate (Win/mac/Linux 통합).
//!
//! Phase 1A.3.b.1 책임 영역 (이 sub-phase):
//! - `download_and_run` + `open_url` 2 method 실행
//! - `ActionExecutor::execute()` dispatch
//! - cancel/timeout/exit code 안전 처리
//!
//! Phase 1A.3.b.2 합류 예정:
//! - `download_and_extract` (zip / tar.gz / dmg)
//! - `shell.curl_pipe_sh` (Linux)
//! - post_install_check 실제 평가 (현재는 stub)
//! - Tauri Channel 직접 통합 + capability JSON

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use runtime_detector::manifest::{
    DownloadAndExtractSpec, DownloadAndRunSpec, OpenUrlSpec, PlatformInstall, PostInstallCheck,
    ShellCurlPipeShSpec,
};

use crate::downloader::{DownloadRequest, Downloader};
use crate::error::DownloadError;
use crate::extract::{detect_format, extract as extract_archive, ExtractError, ExtractFormat};
use crate::progress::ProgressSink;

/// 기본 timeout 15분.
const DEFAULT_INSTALLER_TIMEOUT_SECONDS: u64 = 900;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ActionOutcome {
    /// installer가 0으로 종료 또는 success_exit_codes 매치.
    Success {
        method: &'static str,
        exit_code: Option<i32>,
        post_install_check_passed: Option<bool>,
    },
    /// MSI 3010 / 1641 / Inno Setup 8 등 — 사용자에게 reboot 필요 안내.
    SuccessRebootRequired {
        method: &'static str,
        exit_code: i32,
    },
    /// open_url 실행 — 외부 브라우저 호출 성공. 실제 설치는 사용자 책임.
    OpenedUrl { url: String },
}

#[derive(Debug, Error)]
pub enum ActionError {
    #[error("download failed: {0}")]
    Download(#[from] DownloadError),

    #[error("extraction failed: {0}")]
    Extract(#[from] ExtractError),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("installer exited with code {0}")]
    ExitCode(i32),

    #[error("installer terminated by signal (no exit code)")]
    NoExitCode,

    #[error("installer timed out after {0:?}")]
    Timeout(Duration),

    #[error("installation cancelled by caller")]
    Cancelled,

    #[error("failed to open url '{url}': {source}")]
    OpenUrl {
        url: String,
        #[source]
        source: std::io::Error,
    },

    #[error("install method not supported on this OS or sub-phase: {0}")]
    Unsupported(&'static str),

    #[error("invalid spec: {0}")]
    InvalidSpec(String),
}

/// 설치 액션 실행기. cache_dir에 다운로드 후 자식 프로세스 spawn.
pub struct ActionExecutor {
    downloader: Downloader,
    cache_dir: PathBuf,
    /// post_install_check 전용 짧은 timeout HTTP client.
    post_check_http: reqwest::Client,
}

impl ActionExecutor {
    /// 자체 Downloader 생성. cache_dir는 `.partial` + 다운로드된 installer가 머무는 곳.
    pub fn new(cache_dir: PathBuf) -> Result<Self, DownloadError> {
        let post_check_http = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .connect_timeout(Duration::from_millis(500))
            .no_proxy()
            .build()
            .map_err(DownloadError::Http)?;
        Ok(Self {
            downloader: Downloader::new()?,
            cache_dir,
            post_check_http,
        })
    }

    pub fn with_downloader(downloader: Downloader, cache_dir: PathBuf) -> Self {
        let post_check_http = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .connect_timeout(Duration::from_millis(500))
            .no_proxy()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            downloader,
            cache_dir,
            post_check_http,
        }
    }

    pub async fn execute<S: ProgressSink>(
        &self,
        method: &PlatformInstall,
        cancel: &CancellationToken,
        sink: &S,
    ) -> Result<ActionOutcome, ActionError> {
        match method {
            PlatformInstall::DownloadAndRun(spec) => {
                self.run_download_and_run(spec, cancel, sink).await
            }
            PlatformInstall::DownloadAndExtract(spec) => {
                self.run_download_and_extract(spec, cancel, sink).await
            }
            PlatformInstall::ShellCurlPipeSh(spec) => {
                self.run_shell_curl_pipe_sh(spec, cancel).await
            }
            PlatformInstall::OpenUrl(spec) => self.run_open_url(spec),
        }
    }

    async fn run_download_and_run<S: ProgressSink>(
        &self,
        spec: &DownloadAndRunSpec,
        cancel: &CancellationToken,
        sink: &S,
    ) -> Result<ActionOutcome, ActionError> {
        // 1. 다운로드.
        let installer_path = self.download_installer(spec, cancel, sink).await?;

        // 2. 실행.
        if cancel.is_cancelled() {
            return Err(ActionError::Cancelled);
        }
        let timeout = Duration::from_secs(
            spec.timeout_seconds
                .unwrap_or(DEFAULT_INSTALLER_TIMEOUT_SECONDS),
        );
        let exit_code = spawn_and_wait(&installer_path, &spec.args, timeout, cancel).await?;

        // 3. exit code 분기.
        if exit_code == 0 || spec.success_exit_codes.contains(&exit_code) {
            const REBOOT_REQUIRED_CODES: [i32; 3] = [3010, 1641, 8];
            if REBOOT_REQUIRED_CODES.contains(&exit_code) {
                return Ok(ActionOutcome::SuccessRebootRequired {
                    method: "download_and_run",
                    exit_code,
                });
            }
            // 4. post_install_check 실평가.
            let post_check_passed = self
                .evaluate_post_install_check_opt(&spec.post_install_check, cancel)
                .await;
            return Ok(ActionOutcome::Success {
                method: "download_and_run",
                exit_code: Some(exit_code),
                post_install_check_passed: post_check_passed,
            });
        }

        Err(ActionError::ExitCode(exit_code))
    }

    /// `download_and_extract` — archive 다운로드 → format 자동 감지 → extract.
    async fn run_download_and_extract<S: ProgressSink>(
        &self,
        spec: &DownloadAndExtractSpec,
        cancel: &CancellationToken,
        sink: &S,
    ) -> Result<ActionOutcome, ActionError> {
        if !self.cache_dir.exists() {
            tokio::fs::create_dir_all(&self.cache_dir).await?;
        }
        let url = &spec.url_template;
        let filename = derive_filename(url)?;
        let archive_path = self.cache_dir.join(&filename);

        let expected_sha256 = spec
            .sha256
            .as_deref()
            .map(parse_sha256)
            .transpose()
            .map_err(ActionError::InvalidSpec)?;

        let req = DownloadRequest {
            url: url.clone(),
            final_path: archive_path.clone(),
            expected_sha256,
            size_hint: None,
            max_retries: Some(5),
        };
        self.downloader.download(&req, cancel, sink).await?;

        if cancel.is_cancelled() {
            return Err(ActionError::Cancelled);
        }

        // Format은 파일명에서 자동 감지. dmg는 macOS 외에서 실패.
        let fmt = detect_format(&archive_path).ok_or_else(|| {
            ActionError::InvalidSpec(format!(
                "cannot detect archive format from filename {}",
                archive_path.display()
            ))
        })?;

        let target = std::path::PathBuf::from(&spec.extract_to);
        if !target_path_is_safe(&target) {
            return Err(ActionError::InvalidSpec(format!(
                "extract_to must be absolute and not contain '..': {}",
                target.display()
            )));
        }

        let extract_outcome = extract_archive(&archive_path, &target, fmt, cancel).await?;
        tracing::info!(
            entries = extract_outcome.entries,
            bytes = extract_outcome.total_bytes,
            target = %target.display(),
            "archive extracted"
        );

        let post_check_passed = self
            .evaluate_post_install_check_opt(&spec.post_install_check, cancel)
            .await;
        Ok(ActionOutcome::Success {
            method: match fmt {
                ExtractFormat::Zip => "download_and_extract.zip",
                ExtractFormat::TarGz => "download_and_extract.tar_gz",
                ExtractFormat::Dmg => "download_and_extract.dmg",
            },
            exit_code: None,
            post_install_check_passed: post_check_passed,
        })
    }

    /// `shell.curl_pipe_sh` — `bash -c "curl -fsSL <url> | sh"`. Linux 전용.
    /// macOS에선 동일하게 동작 가능하나 v1에서는 Linux로 한정. Win은 Unsupported.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    async fn run_shell_curl_pipe_sh(
        &self,
        spec: &ShellCurlPipeShSpec,
        cancel: &CancellationToken,
    ) -> Result<ActionOutcome, ActionError> {
        validate_shell_safe_url(&spec.url_template)?;
        tracing::info!(
            url = %spec.url_template,
            "shell.curl_pipe_sh starting"
        );
        let cmd_str = format!("curl -fsSL {} | sh", spec.url_template);
        let exe = std::path::PathBuf::from("bash");
        let args = vec!["-c".to_string(), cmd_str];
        let timeout = Duration::from_secs(DEFAULT_INSTALLER_TIMEOUT_SECONDS);
        let exit_code = spawn_and_wait(&exe, &args, timeout, cancel).await?;
        if exit_code == 0 {
            let post = self
                .evaluate_post_install_check_opt(&spec.post_install_check, cancel)
                .await;
            Ok(ActionOutcome::Success {
                method: "shell.curl_pipe_sh",
                exit_code: Some(exit_code),
                post_install_check_passed: post,
            })
        } else {
            Err(ActionError::ExitCode(exit_code))
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    async fn run_shell_curl_pipe_sh(
        &self,
        _spec: &ShellCurlPipeShSpec,
        _cancel: &CancellationToken,
    ) -> Result<ActionOutcome, ActionError> {
        Err(ActionError::Unsupported(
            "shell.curl_pipe_sh — Unix bash only",
        ))
    }

    /// post_install_check 평가. 결과: Some(true)=통과, Some(false)=실패/timeout, None=check 없음.
    /// 본 구현은 method="http.get"만. 다른 method는 Some(false).
    async fn evaluate_post_install_check_opt(
        &self,
        check: &Option<PostInstallCheck>,
        cancel: &CancellationToken,
    ) -> Option<bool> {
        let check = check.as_ref()?;
        Some(self.evaluate_post_install_check(check, cancel).await)
    }

    async fn evaluate_post_install_check(
        &self,
        check: &PostInstallCheck,
        cancel: &CancellationToken,
    ) -> bool {
        if check.method != "http.get" {
            tracing::warn!(
                method = %check.method,
                "post_install_check method not supported; treating as failure"
            );
            return false;
        }
        let total = Duration::from_secs(check.wait_seconds.max(1) as u64);
        let poll = Duration::from_secs(2);
        let deadline = std::time::Instant::now() + total;
        loop {
            if cancel.is_cancelled() {
                return false;
            }
            if std::time::Instant::now() > deadline {
                tracing::info!(url = %check.url, "post_install_check timed out");
                return false;
            }
            match self.post_check_http.get(&check.url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    tracing::info!(url = %check.url, "post_install_check passed");
                    return true;
                }
                Ok(resp) => {
                    tracing::debug!(
                        url = %check.url,
                        status = %resp.status(),
                        "post_install_check non-2xx — retry"
                    );
                }
                Err(e) => {
                    tracing::debug!(url = %check.url, error = %e, "post_install_check connect — retry");
                }
            }
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            let wait = std::cmp::min(poll, remaining);
            if wait.is_zero() {
                return false;
            }
            tokio::select! {
                _ = cancel.cancelled() => return false,
                _ = tokio::time::sleep(wait) => {}
            }
        }
    }

    async fn download_installer<S: ProgressSink>(
        &self,
        spec: &DownloadAndRunSpec,
        cancel: &CancellationToken,
        sink: &S,
    ) -> Result<PathBuf, ActionError> {
        if !self.cache_dir.exists() {
            tokio::fs::create_dir_all(&self.cache_dir).await?;
        }
        let url = &spec.url_template;
        let filename = derive_filename(url)?;
        let final_path = self.cache_dir.join(&filename);

        let expected_sha256 = spec
            .sha256
            .as_deref()
            .map(parse_sha256)
            .transpose()
            .map_err(ActionError::InvalidSpec)?;

        let req = DownloadRequest {
            url: url.clone(),
            final_path: final_path.clone(),
            expected_sha256,
            size_hint: None,
            max_retries: Some(5),
        };
        let outcome = self.downloader.download(&req, cancel, sink).await?;
        Ok(outcome.final_path)
    }

    fn run_open_url(&self, spec: &OpenUrlSpec) -> Result<ActionOutcome, ActionError> {
        // SAFETY 측면: webbrowser는 cmd /c start "" / open / xdg-open를 spawn. URL은 manifest 신뢰원.
        webbrowser::open(&spec.url).map_err(|e| ActionError::OpenUrl {
            url: spec.url.clone(),
            source: e,
        })?;
        Ok(ActionOutcome::OpenedUrl {
            url: spec.url.clone(),
        })
    }
}

/// installer를 spawn하고 exit code 수집. cancel 시 start_kill + 5s wait.
/// stdout/stderr는 `Stdio::null()`로 폐기 — 진단 캡처는 Phase 1A.3.b.2에서 추가 (background drain task).
pub(crate) async fn spawn_and_wait(
    exe: &std::path::Path,
    args: &[String],
    timeout: Duration,
    cancel: &CancellationToken,
) -> Result<i32, ActionError> {
    let mut cmd = tokio::process::Command::new(exe);
    cmd.args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);

    let mut child = cmd.spawn()?;

    tokio::select! {
        biased;
        _ = cancel.cancelled() => {
            tracing::info!(exe = %exe.display(), "cancellation received — terminating installer");
            let _ = child.start_kill();
            let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
            Err(ActionError::Cancelled)
        }
        res = tokio::time::timeout(timeout, child.wait()) => {
            match res {
                Ok(Ok(status)) => {
                    tracing::debug!(exit = ?status.code(), exe = %exe.display(), "installer finished");
                    status.code().ok_or(ActionError::NoExitCode)
                }
                Ok(Err(e)) => Err(ActionError::Io(e)),
                Err(_) => {
                    tracing::warn!(timeout = ?timeout, "installer timed out — killing");
                    let _ = child.start_kill();
                    let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
                    Err(ActionError::Timeout(timeout))
                }
            }
        }
    }
}

/// URL의 마지막 path segment에서 파일 이름 추출. `?query` 제거.
pub(crate) fn derive_filename(url: &str) -> Result<String, ActionError> {
    // 매우 단순한 파서 — "://" 뒤의 마지막 '/' 이후 부분.
    let after_scheme = url.find("://").map(|i| &url[i + 3..]).unwrap_or(url);
    let no_query = after_scheme.split('?').next().unwrap_or(after_scheme);
    let last = no_query.rsplit('/').next().unwrap_or("");
    if last.is_empty() {
        return Err(ActionError::InvalidSpec(format!(
            "cannot derive filename from url {url}"
        )));
    }
    Ok(last.to_string())
}

/// 32 bytes hex string → [u8; 32].
pub(crate) fn parse_sha256(hex_str: &str) -> Result<[u8; 32], String> {
    let trimmed = hex_str.trim();
    if trimmed.len() != 64 {
        return Err(format!(
            "sha256 must be 64 hex chars, got {} ({:?})",
            trimmed.len(),
            trimmed
        ));
    }
    let bytes = hex::decode(trimmed).map_err(|e| format!("hex decode failed: {e}"))?;
    bytes
        .try_into()
        .map_err(|_| "sha256 hex did not produce 32 bytes".to_string())
}

/// `extract_to` 검증 — 절대 경로여야 하고 `..` 컴포넌트를 포함하면 안 됨.
fn target_path_is_safe(p: &std::path::Path) -> bool {
    p.is_absolute()
        && !p
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
}

/// shell URL 안전성 검증 — 쉘 메타문자 거부. 인라인 정책: HTTP URL은 보통 메타문자 없음.
/// 본 함수는 `shell.curl_pipe_sh` (Linux/macOS) 경로에서만 호출되므로 Win 빌드에서는 dead.
#[allow(dead_code)]
fn validate_shell_safe_url(url: &str) -> Result<(), ActionError> {
    const BAD: &[char] = &[
        '"', '\'', '`', '$', '\\', ';', '|', '&', '<', '>', '\n', '\r', ' ',
    ];
    if url.is_empty() {
        return Err(ActionError::InvalidSpec("empty url".into()));
    }
    if url.chars().any(|c| BAD.contains(&c)) {
        return Err(ActionError::InvalidSpec(format!(
            "url contains shell-unsafe char: {url}"
        )));
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(ActionError::InvalidSpec(format!(
            "shell.curl_pipe_sh url must be http(s): {url}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_filename_from_typical_urls() {
        assert_eq!(
            derive_filename(
                "https://github.com/ollama/ollama/releases/latest/download/OllamaSetup.exe"
            )
            .unwrap(),
            "OllamaSetup.exe"
        );
        assert_eq!(
            derive_filename("https://example.com/foo.dmg?token=abc").unwrap(),
            "foo.dmg"
        );
        assert!(derive_filename("https://example.com/").is_err());
    }

    #[test]
    fn parse_sha256_valid_and_invalid() {
        let valid = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let parsed = parse_sha256(valid).unwrap();
        assert_eq!(parsed.len(), 32);
        assert_eq!(parsed[0], 0x01);
        assert_eq!(parsed[31], 0xef);

        assert!(parse_sha256("short").is_err());
        // 정확히 64자이지만 hex 아님.
        assert!(
            parse_sha256("zz23456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
                .is_err()
        );
    }

    #[test]
    fn action_outcome_serializes_kebab() {
        let o = ActionOutcome::Success {
            method: "download_and_run",
            exit_code: Some(0),
            post_install_check_passed: None,
        };
        let v = serde_json::to_value(&o).unwrap();
        assert_eq!(v["kind"], "success");
        assert_eq!(v["method"], "download_and_run");
        assert_eq!(v["exit_code"], 0);
    }

    #[test]
    fn open_url_outcome_serializes() {
        let o = ActionOutcome::OpenedUrl {
            url: "https://lmstudio.ai/download".into(),
        };
        let v = serde_json::to_value(&o).unwrap();
        assert_eq!(v["kind"], "opened-url");
        assert_eq!(v["url"], "https://lmstudio.ai/download");
    }

    #[test]
    fn manifest_install_section_parses_ollama_full() {
        let json = r#"{
            "schema_version": 1,
            "id": "ollama",
            "display_name": "Ollama",
            "license": "MIT",
            "redistribution_allowed": true,
            "detect": [],
            "install": {
                "windows": {
                    "method": "download_and_run",
                    "url_template": "https://github.com/ollama/ollama/releases/latest/download/OllamaSetup.exe",
                    "version_url": "https://api.github.com/repos/ollama/ollama/releases/latest",
                    "args": ["/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART"],
                    "min_disk_mb": 800,
                    "min_ram_mb": 2048,
                    "post_install_check": {
                        "method": "http.get",
                        "url": "http://127.0.0.1:11434/api/version",
                        "wait_seconds": 30
                    }
                },
                "macos": {
                    "method": "download_and_extract",
                    "url_template": "https://github.com/ollama/ollama/releases/latest/download/Ollama-darwin.zip",
                    "extract_to": "/Applications"
                },
                "linux": {
                    "method": "shell.curl_pipe_sh",
                    "url_template": "https://ollama.com/install.sh"
                }
            }
        }"#;
        let parsed: runtime_detector::manifest::AppManifest =
            serde_json::from_str(json).expect("parse");
        let install = parsed.install.expect("install section");
        match install.windows.expect("windows") {
            PlatformInstall::DownloadAndRun(spec) => {
                assert_eq!(spec.args.len(), 3);
                assert_eq!(spec.min_disk_mb, Some(800));
                assert!(spec.post_install_check.is_some());
                let pic = spec.post_install_check.unwrap();
                assert_eq!(pic.method, "http.get");
                assert_eq!(pic.wait_seconds, 30);
            }
            other => panic!("expected DownloadAndRun, got: {other:?}"),
        }
        assert!(matches!(
            install.macos.unwrap(),
            PlatformInstall::DownloadAndExtract(_)
        ));
        assert!(matches!(
            install.linux.unwrap(),
            PlatformInstall::ShellCurlPipeSh(_)
        ));
    }

    #[test]
    fn manifest_lm_studio_open_url_only() {
        let json = r#"{
            "schema_version": 1,
            "id": "lm-studio",
            "display_name": "LM Studio",
            "license": "Element Labs EULA",
            "redistribution_allowed": false,
            "detect": [],
            "install": {
                "windows": { "method": "open_url", "url": "https://lmstudio.ai/download" },
                "macos":   { "method": "open_url", "url": "https://lmstudio.ai/download" },
                "linux":   { "method": "open_url", "url": "https://lmstudio.ai/download" }
            }
        }"#;
        let parsed: runtime_detector::manifest::AppManifest =
            serde_json::from_str(json).expect("parse");
        let install = parsed.install.expect("install");
        match install.windows.expect("windows") {
            PlatformInstall::OpenUrl(spec) => assert_eq!(spec.url, "https://lmstudio.ai/download"),
            other => panic!("expected OpenUrl, got: {other:?}"),
        }
    }

    #[test]
    fn manifest_for_current_platform_picks_correct() {
        let json = r#"{
            "schema_version": 1, "id": "x", "display_name": "X", "license": "MIT",
            "detect": [],
            "install": {
                "windows": { "method": "open_url", "url": "https://win.example/" },
                "macos":   { "method": "open_url", "url": "https://mac.example/" },
                "linux":   { "method": "open_url", "url": "https://linux.example/" }
            }
        }"#;
        let parsed: runtime_detector::manifest::AppManifest = serde_json::from_str(json).unwrap();
        let install = parsed.install.unwrap();
        let current = install
            .for_current_platform()
            .expect("current platform should match");
        match current {
            PlatformInstall::OpenUrl(spec) => {
                let expected_host = if cfg!(windows) {
                    "win.example"
                } else if cfg!(target_os = "macos") {
                    "mac.example"
                } else {
                    "linux.example"
                };
                assert!(
                    spec.url.contains(expected_host),
                    "expected url to contain {expected_host}, got {}",
                    spec.url
                );
            }
            other => panic!("expected OpenUrl, got: {other:?}"),
        }
    }

    /// 시스템 셸을 가짜 installer로 사용 — 즉시 exit 0.
    #[tokio::test]
    async fn spawn_zero_exit() {
        let (exe, args) = system_shell_exit(0);
        let cancel = CancellationToken::new();
        let code = spawn_and_wait(&exe, &args, Duration::from_secs(10), &cancel)
            .await
            .expect("spawn ok");
        assert_eq!(code, 0);
    }

    #[tokio::test]
    async fn spawn_nonzero_exit_returns_code() {
        let (exe, args) = system_shell_exit(7);
        let cancel = CancellationToken::new();
        let code = spawn_and_wait(&exe, &args, Duration::from_secs(10), &cancel)
            .await
            .expect("spawn ok");
        assert_eq!(code, 7);
    }

    #[tokio::test]
    async fn spawn_cancel_before_completion_returns_cancelled() {
        // 30s sleep — 100ms 후 cancel.
        let (exe, args) = system_shell_sleep(30);
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            cancel_clone.cancel();
        });
        let r = spawn_and_wait(&exe, &args, Duration::from_secs(60), &cancel).await;
        match r {
            Err(ActionError::Cancelled) => {}
            other => panic!("expected Cancelled, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn spawn_timeout_returns_timeout() {
        // 30s sleep — 200ms timeout.
        let (exe, args) = system_shell_sleep(30);
        let cancel = CancellationToken::new();
        let r = spawn_and_wait(&exe, &args, Duration::from_millis(200), &cancel).await;
        match r {
            Err(ActionError::Timeout(_)) => {}
            other => panic!("expected Timeout, got: {other:?}"),
        }
    }

    fn system_shell_exit(code: i32) -> (PathBuf, Vec<String>) {
        if cfg!(windows) {
            (
                PathBuf::from("cmd"),
                vec!["/c".into(), format!("exit {code}")],
            )
        } else {
            (
                PathBuf::from("sh"),
                vec!["-c".into(), format!("exit {code}")],
            )
        }
    }

    fn system_shell_sleep(seconds: u64) -> (PathBuf, Vec<String>) {
        if cfg!(windows) {
            // ping을 sleep으로 사용. ping -n N+1 → N 초.
            (
                PathBuf::from("cmd"),
                vec![
                    "/c".into(),
                    format!("ping -n {} 127.0.0.1 >nul", seconds + 1),
                ],
            )
        } else {
            (
                PathBuf::from("sh"),
                vec!["-c".into(), format!("sleep {seconds}")],
            )
        }
    }

    #[test]
    fn validate_shell_safe_url_accepts_normal_https() {
        assert!(validate_shell_safe_url("https://ollama.com/install.sh").is_ok());
        assert!(validate_shell_safe_url("http://example.com/path/to/file").is_ok());
    }

    #[test]
    fn validate_shell_safe_url_rejects_metachars() {
        assert!(validate_shell_safe_url("https://x.com; rm -rf /").is_err());
        assert!(validate_shell_safe_url("https://x.com|cat").is_err());
        assert!(validate_shell_safe_url("https://x.com`id`").is_err());
        assert!(validate_shell_safe_url("https://x.com$VAR").is_err());
        assert!(validate_shell_safe_url("https://x.com space").is_err());
        assert!(validate_shell_safe_url("").is_err());
    }

    #[test]
    fn validate_shell_safe_url_requires_http_scheme() {
        assert!(validate_shell_safe_url("ftp://example.com/x").is_err());
        assert!(validate_shell_safe_url("file:///etc/passwd").is_err());
        assert!(validate_shell_safe_url("javascript:alert(1)").is_err());
    }

    #[test]
    fn target_path_is_safe_accepts_absolute_no_parent() {
        if cfg!(windows) {
            assert!(target_path_is_safe(std::path::Path::new(
                "C:\\Applications"
            )));
            assert!(!target_path_is_safe(std::path::Path::new(
                "C:\\Applications\\..\\escape"
            )));
        } else {
            assert!(target_path_is_safe(std::path::Path::new("/Applications")));
            assert!(!target_path_is_safe(std::path::Path::new(
                "/Applications/../escape"
            )));
        }
        assert!(!target_path_is_safe(std::path::Path::new("relative/path")));
    }

    #[tokio::test]
    async fn evaluate_post_install_check_http_get_passes() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/version"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "0.5.0"})),
            )
            .mount(&server)
            .await;

        let dir = tempfile::TempDir::new().unwrap();
        let exec = ActionExecutor::new(dir.path().to_path_buf()).unwrap();
        let check = PostInstallCheck {
            method: "http.get".into(),
            url: format!("{}/api/version", server.uri()),
            wait_seconds: 5,
        };
        let cancel = CancellationToken::new();
        let passed = exec.evaluate_post_install_check(&check, &cancel).await;
        assert!(passed, "post_install_check should pass on 200");
    }

    #[tokio::test]
    async fn evaluate_post_install_check_unreachable_times_out() {
        let dir = tempfile::TempDir::new().unwrap();
        let exec = ActionExecutor::new(dir.path().to_path_buf()).unwrap();
        let check = PostInstallCheck {
            method: "http.get".into(),
            url: "http://127.0.0.1:1/never".into(),
            wait_seconds: 1, // 짧게 — 1초 후 timeout.
        };
        let cancel = CancellationToken::new();
        let start = std::time::Instant::now();
        let passed = exec.evaluate_post_install_check(&check, &cancel).await;
        assert!(!passed);
        // 1초 wait + a bit of poll overhead. 5초는 충분히 여유.
        assert!(start.elapsed() < Duration::from_secs(5));
    }

    #[tokio::test]
    async fn evaluate_post_install_check_unsupported_method() {
        let dir = tempfile::TempDir::new().unwrap();
        let exec = ActionExecutor::new(dir.path().to_path_buf()).unwrap();
        let check = PostInstallCheck {
            method: "shell.echo".into(), // unsupported
            url: "irrelevant".into(),
            wait_seconds: 30,
        };
        let cancel = CancellationToken::new();
        let passed = exec.evaluate_post_install_check(&check, &cancel).await;
        assert!(!passed);
    }

    #[tokio::test]
    async fn evaluate_post_install_check_cancellation() {
        let dir = tempfile::TempDir::new().unwrap();
        let exec = ActionExecutor::new(dir.path().to_path_buf()).unwrap();
        let check = PostInstallCheck {
            method: "http.get".into(),
            url: "http://127.0.0.1:1/never".into(),
            wait_seconds: 60, // 길게 — 사용자 cancel이 끊어야 함.
        };
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            cancel_clone.cancel();
        });
        let start = std::time::Instant::now();
        let passed = exec.evaluate_post_install_check(&check, &cancel).await;
        assert!(!passed);
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "cancel should short-circuit"
        );
    }
}
