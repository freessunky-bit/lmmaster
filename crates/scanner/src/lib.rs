//! crate: scanner — 주기 self-scan + 한국어 자연어 요약.
//!
//! 정책 (ADR-0013, ADR-0020):
//! - **deterministic 판정 우선** — 모든 체크는 deterministic 로직.
//! - **opt-in 로컬 LLM 요약** — Ollama 설치돼 있을 때만 자연어 풀어쓰기. 외부 통신 0.
//! - 트리거 3종: 6h cron + 5분 on-launch grace + UI on-demand.
//! - 결과는 `tokio::sync::broadcast` 채널 — Tauri emit + log + UI 다중 구독 가능.
//! - LLM 실패는 사용자에게 노출 안 함 — deterministic 템플릿으로 자연 fallback.

pub mod checks;
pub mod error;
pub mod llm_summary;
pub mod scheduler;
pub mod templates;

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Mutex};
use tokio_cron_scheduler::JobScheduler;

pub use checks::{run_all as run_checks, CheckResult, Severity};
pub use error::ScannerError;
pub use llm_summary::{validate_korean_summary, OllamaClient, DEFAULT_CASCADE};
pub use templates::render_summary as render_deterministic_summary;

use runtime_detector::EnvironmentReport;

/// LLM 또는 deterministic 출처 표시.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SummarySource {
    /// 로컬 LLM이 한국어 요약을 생성.
    Llm,
    /// deterministic 템플릿으로 생성 (LLM 미사용 / 실패).
    Deterministic,
}

/// 단일 점검 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSummary {
    pub started_at: SystemTime,
    pub checks: Vec<CheckResult>,
    pub summary_korean: String,
    pub summary_source: SummarySource,
    /// LLM 사용 시 모델 이름.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_used: Option<String>,
    pub took_ms: u64,
}

/// 환경 점검 추상화 — 테스트에서 mock 주입 위해 trait.
#[async_trait]
pub trait EnvironmentProbe: Send + Sync {
    async fn probe(&self) -> Result<EnvironmentReport, ScannerError>;
}

/// runtime-detector를 사용하는 기본 probe.
pub struct DefaultProbe;

#[async_trait]
impl EnvironmentProbe for DefaultProbe {
    async fn probe(&self) -> Result<EnvironmentReport, ScannerError> {
        Ok(runtime_detector::probe_environment().await)
    }
}

/// Scanner 옵션 — `Scanner::new`에 전달.
pub struct ScannerOptions {
    pub probe: Arc<dyn EnvironmentProbe>,
    /// `http://127.0.0.1:11434` 등 — None이면 LLM 비활성.
    pub ollama_endpoint: Option<String>,
    pub model_cascade: Vec<String>,
    /// false면 LLM cascade 시도 자체를 안 함 (사용자 옵션).
    pub use_llm: bool,
    /// 6h cron — None이면 cron 비활성.
    pub cron: Option<String>,
    /// app launch 후 N분 grace. None이면 비활성.
    pub launch_grace: Option<Duration>,
}

impl ScannerOptions {
    /// runtime-detector + 기본 cascade + 6h cron + 5분 grace.
    pub fn defaults_with_ollama(endpoint: impl Into<String>) -> Self {
        Self {
            probe: Arc::new(DefaultProbe),
            ollama_endpoint: Some(endpoint.into()),
            model_cascade: DEFAULT_CASCADE.iter().map(|s| s.to_string()).collect(),
            use_llm: true,
            cron: Some(scheduler::DEFAULT_CRON_SIX_HOURS.to_string()),
            launch_grace: Some(Duration::from_secs(5 * 60)),
        }
    }
}

/// 진단 스캐너 진입점.
pub struct Scanner {
    inner: Arc<Inner>,
}

struct Inner {
    probe: Arc<dyn EnvironmentProbe>,
    llm: Option<OllamaClient>,
    use_llm: bool,
    in_flight: Mutex<()>,
    summary_tx: broadcast::Sender<ScanSummary>,
}

impl Scanner {
    pub async fn new(opts: ScannerOptions) -> Result<Arc<Self>, ScannerError> {
        let llm = match (&opts.ollama_endpoint, opts.use_llm) {
            (Some(ep), true) => Some(OllamaClient::new(ep.clone(), opts.model_cascade)?),
            _ => None,
        };
        let (summary_tx, _) = broadcast::channel(8);
        let scanner = Arc::new(Self {
            inner: Arc::new(Inner {
                probe: opts.probe,
                llm,
                use_llm: opts.use_llm,
                in_flight: Mutex::new(()),
                summary_tx,
            }),
        });
        Ok(scanner)
    }

    /// 명시적 점검 실행. 동시 호출은 `AlreadyRunning` 에러.
    pub async fn scan_now(self: &Arc<Self>) -> Result<ScanSummary, ScannerError> {
        let _guard = self
            .inner
            .in_flight
            .try_lock()
            .map_err(|_| ScannerError::AlreadyRunning)?;
        let started_at = SystemTime::now();
        let started_inst = std::time::Instant::now();

        // 1. 환경 점검.
        let env = self.inner.probe.probe().await?;

        // 2. deterministic 체크.
        let checks = run_checks(&env);

        // 3. LLM 또는 deterministic 요약.
        let env_summary = format_env_summary(&env);
        let (summary_korean, source, model_used) = if self.inner.use_llm {
            if let Some(llm) = &self.inner.llm {
                match llm.summarize(&env_summary, &checks).await {
                    Ok(text) => {
                        let model = llm.pick_model().await.ok();
                        (text, SummarySource::Llm, model)
                    }
                    Err(e) => {
                        tracing::info!(error = %e, "LLM 요약 실패 — deterministic 템플릿으로 fallback");
                        (
                            render_deterministic_summary(&checks),
                            SummarySource::Deterministic,
                            None,
                        )
                    }
                }
            } else {
                (
                    render_deterministic_summary(&checks),
                    SummarySource::Deterministic,
                    None,
                )
            }
        } else {
            (
                render_deterministic_summary(&checks),
                SummarySource::Deterministic,
                None,
            )
        };

        let took_ms = started_inst.elapsed().as_millis() as u64;
        let summary = ScanSummary {
            started_at,
            checks,
            summary_korean,
            summary_source: source,
            model_used,
            took_ms,
        };

        // 4. broadcast — silent if no subscribers.
        let _ = self.inner.summary_tx.send(summary.clone());

        Ok(summary)
    }

    /// broadcast 구독자 — Tauri emit / log / UI 등.
    pub fn subscribe(&self) -> broadcast::Receiver<ScanSummary> {
        self.inner.summary_tx.subscribe()
    }
}

/// EnvironmentReport에서 LLM 입력용 1줄 요약 생성.
fn format_env_summary(env: &EnvironmentReport) -> String {
    use hardware_probe::OsFamily;
    let os = match env.hardware.os.family {
        OsFamily::Windows => "Windows",
        OsFamily::Macos => "macOS",
        OsFamily::Linux => "Linux",
        OsFamily::Other => "기타 OS",
    };
    let ram_gb = env.hardware.mem.total_bytes / (1024 * 1024 * 1024);
    let gpu = env
        .hardware
        .gpus
        .first()
        .map(|g| g.model.clone())
        .unwrap_or_else(|| "GPU 없음".to_string());
    format!(
        "{os} {} | RAM {ram_gb}GB | GPU {gpu}",
        env.hardware.os.version
    )
}

/// `Scanner` + cron `JobScheduler`를 함께 묶은 wrapper — caller가 `start/shutdown` 일괄 관리.
pub struct ScannerService {
    pub scanner: Arc<Scanner>,
    sched: tokio::sync::Mutex<Option<JobScheduler>>,
}

impl ScannerService {
    pub async fn new(opts: ScannerOptions) -> Result<Self, ScannerError> {
        let cron = opts.cron.clone();
        let grace = opts.launch_grace;
        let scanner = Scanner::new(opts).await?;
        let sched = scheduler::build(Arc::clone(&scanner), cron.as_deref(), grace).await?;
        Ok(Self {
            scanner,
            sched: tokio::sync::Mutex::new(Some(sched)),
        })
    }

    pub async fn start(&self) -> Result<(), ScannerError> {
        let mut g = self.sched.lock().await;
        if let Some(sched) = g.as_mut() {
            sched.start().await?;
        }
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<(), ScannerError> {
        let mut g = self.sched.lock().await;
        if let Some(mut sched) = g.take() {
            sched.shutdown().await?;
        }
        Ok(())
    }
}
