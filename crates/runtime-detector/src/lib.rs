//! crate: runtime-detector — 외부 런타임 (LM Studio · Ollama) 및 시스템 환경 감지.
//!
//! 정책 (ADR-0016, ADR-0017, ADR-0021):
//! - HTTP probe 우선, OS 레벨(레지스트리/plist/dpkg)은 fallback.
//! - LM Studio EULA 상 redistribution 금지 — `redistribution_allowed: false` 필드를 manifest에 명시.
//! - probe는 비차단 + 짧은 timeout(기본 1.5s) + connect_timeout(0.5s).
//! - 결과는 serde-able `DetectResult`로 통일 — frontend Channel/event 양쪽에 그대로 emit 가능.
//!
//! Phase 1A.1 책임 영역 (이 sub-phase):
//! - Ollama HTTP probe (`/api/version`)
//! - LM Studio HTTP probe (OpenAI-compatible `/v1/models`)
//! - 단일 HTTP client(connection pool 재사용)로 양쪽 detect.
//! - 통합 테스트(wiremock).
//!
//! Phase 1A.2 합류 예정 (다음 sub-phase):
//! - Windows registry / macOS plist / Linux dpkg fallback.
//! - 하드웨어 probe (sysinfo + nvml-wrapper + wgpu + ash + winreg + objc2-metal).
//! - 환경 prereq detect (WebView2, VC++ 2022 redist, NVIDIA driver, CUDA, Vulkan, Metal, DirectML).

use std::time::Duration;

use serde::{Deserialize, Serialize};
use shared_types::RuntimeKind;

pub mod lm_studio;
pub mod manifest;
pub mod ollama;

/// 외부 런타임의 런타임 가시성. UI에서 그대로 표시할 수 있도록 kebab-case로 직렬화.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// HTTP probe 응답 OK — daemon이 떠 있고 즉시 사용 가능.
    Running,
    /// HTTP probe 실패. OS 레벨 신호(레지스트리/PATH 등)는 발견됨 — 설치는 됐지만 미실행.
    /// (Phase 1A.1에서는 OS-레벨 신호 미수집 — 항상 NotInstalled로 분류한다.)
    Installed,
    /// 모든 신호가 없음.
    NotInstalled,
    /// probe 자체에 예외(네트워크 외 사유)가 발생.
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectResult {
    pub runtime: RuntimeKind,
    pub status: Status,
    /// daemon 또는 CLI에서 수집한 버전 문자열. LM Studio는 REST에서 노출하지 않으므로
    /// HTTP probe만으로는 None — Phase 1A.2에서 `lms version` CLI fallback 추가.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// 사용 가능한 base URL (예: `http://127.0.0.1:11434`). Running일 때만 Some.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// probe 도중 발생한 예외 메시지 (Status::Error일 때만).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl DetectResult {
    pub fn not_installed(runtime: RuntimeKind) -> Self {
        Self {
            runtime,
            status: Status::NotInstalled,
            version: None,
            endpoint: None,
            error: None,
        }
    }

    pub fn running(runtime: RuntimeKind, endpoint: String, version: Option<String>) -> Self {
        Self {
            runtime,
            status: Status::Running,
            version,
            endpoint: Some(endpoint),
            error: None,
        }
    }

    pub fn error(runtime: RuntimeKind, err: impl Into<String>) -> Self {
        Self {
            runtime,
            status: Status::Error,
            version: None,
            endpoint: None,
            error: Some(err.into()),
        }
    }
}

/// Detector 동작 설정. `Default::default()`로 v1 권장값 사용.
#[derive(Debug, Clone)]
pub struct DetectorConfig {
    pub probe_timeout: Duration,
    pub connect_timeout: Duration,
    pub user_agent: String,
}

impl Default for DetectorConfig {
    fn default() -> Self {
        Self {
            probe_timeout: Duration::from_millis(1500),
            connect_timeout: Duration::from_millis(500),
            user_agent: format!("LMmaster/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// Detector — 외부 런타임 detect의 진입점. 단일 reqwest::Client를 보유해 connection pool을
/// 재사용한다.
pub struct Detector {
    client: reqwest::Client,
}

impl Detector {
    pub fn new(cfg: DetectorConfig) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(&cfg.user_agent)
            .timeout(cfg.probe_timeout)
            .connect_timeout(cfg.connect_timeout)
            .pool_idle_timeout(Duration::from_secs(30))
            .no_proxy() // localhost-only probe — proxy 우회로 사내망 방화벽 영향 회피.
            .build()?;
        Ok(Self { client })
    }

    pub fn with_default_config() -> anyhow::Result<Self> {
        Self::new(DetectorConfig::default())
    }

    /// Ollama daemon 감지.
    pub async fn detect_ollama(&self) -> DetectResult {
        ollama::probe(&self.client).await.unwrap_or_else(|e| {
            tracing::warn!(error = %e, "ollama probe internal error");
            DetectResult::error(RuntimeKind::Ollama, e.to_string())
        })
    }

    /// LM Studio daemon 감지.
    pub async fn detect_lm_studio(&self) -> DetectResult {
        lm_studio::probe(&self.client).await.unwrap_or_else(|e| {
            tracing::warn!(error = %e, "lm studio probe internal error");
            DetectResult::error(RuntimeKind::LmStudio, e.to_string())
        })
    }

    /// 양 런타임을 병렬로 감지.
    pub async fn detect_all(&self) -> Vec<DetectResult> {
        let (a, b) = tokio::join!(self.detect_ollama(), self.detect_lm_studio());
        vec![a, b]
    }

    /// 내부 reqwest client 노출 — Phase 1A.2 OS-레벨 detect나 manifest fetch에서 재사용.
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }
}

// ── Phase 1A.4.b — 환경 점검 합성 ────────────────────────────────────────

/// 마법사/홈/진단 화면이 한 번의 IPC로 받는 통합 환경 보고.
/// hardware-probe + runtime-detector 결과를 같은 시점에 합쳐 직렬화 가능한 단일 struct로 노출.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentReport {
    pub hardware: hardware_probe::HardwareReport,
    pub runtimes: Vec<DetectResult>,
}

/// hardware probe + runtime detect를 병렬로 수행. 둘 다 graceful fail이라 본 함수는 panic하지 않는다.
///
/// 측정: 일반 Win11 기준 cold ~1.0~2.5s (hardware probe 500ms + runtime probe 1.5s timeout 병렬).
pub async fn probe_environment() -> EnvironmentReport {
    // Detector init은 reqwest::Client::builder() 호출 — 거의 항상 성공. 실패 시에도 빈 결과로 폴백.
    let detector_result = Detector::with_default_config();
    let runtimes_fut = async {
        match detector_result {
            Ok(d) => d.detect_all().await,
            Err(e) => {
                tracing::warn!(error = %e, "detector init 실패 — 빈 runtimes 반환");
                Vec::new()
            }
        }
    };
    let (hardware, runtimes) = tokio::join!(hardware_probe::probe(), runtimes_fut);
    EnvironmentReport { hardware, runtimes }
}
