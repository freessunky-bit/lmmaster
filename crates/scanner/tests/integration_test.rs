//! scanner 통합 테스트 — wiremock으로 Ollama API + mock EnvironmentProbe.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use scanner::{
    EnvironmentProbe, ScannerError, ScannerOptions, ScannerService, Severity, SummarySource,
    DEFAULT_CASCADE,
};

use hardware_probe::{
    CpuInfo, DiskInfo, DiskKind, GpuBackend, GpuDeviceType, GpuInfo, GpuVendor, HardwareReport,
    MemInfo, OsFamily, OsInfo, RuntimeInfo,
};
use runtime_detector::{DetectResult, EnvironmentReport, Status};
use shared_types::RuntimeKind;

const GIB: u64 = 1024 * 1024 * 1024;

struct MockProbe {
    env: EnvironmentReport,
}

#[async_trait]
impl EnvironmentProbe for MockProbe {
    async fn probe(&self) -> Result<EnvironmentReport, ScannerError> {
        Ok(self.env.clone())
    }
}

fn base_env() -> EnvironmentReport {
    EnvironmentReport {
        hardware: HardwareReport {
            os: OsInfo {
                family: OsFamily::Windows,
                version: "11".into(),
                arch: "x86_64".into(),
                kernel: "10.0".into(),
                rosetta: None,
                distro: None,
                distro_version: None,
            },
            cpu: CpuInfo {
                brand: "Intel".into(),
                vendor_id: "GenuineIntel".into(),
                physical_cores: 8,
                logical_cores: 16,
                frequency_mhz: 3000,
            },
            mem: MemInfo {
                total_bytes: 16 * GIB,
                available_bytes: 8 * GIB,
            },
            disks: vec![DiskInfo {
                mount_point: "C:".into(),
                kind: DiskKind::Ssd,
                total_bytes: 500 * GIB,
                available_bytes: 250 * GIB,
            }],
            gpus: vec![GpuInfo {
                vendor: GpuVendor::Nvidia,
                model: "NVIDIA RTX 4080".into(),
                vram_bytes: Some(16 * GIB),
                pci_id: None,
                driver_version: Some("551.86".into()),
                backend: GpuBackend::Dx12,
                device_type: GpuDeviceType::DiscreteGpu,
                apple_family: None,
            }],
            runtimes: RuntimeInfo {
                cuda_toolkits: vec!["v12.4".into()],
                cuda_runtime: true,
                vulkan: true,
                metal: false,
                directml: true,
                d3d12: true,
                rocm: false,
                webview2: Some("128.0".into()),
                vcredist_2022: Some("14.40".into()),
                glibc: None,
                libstdcpp: None,
                vulkan_devices: None,
            },
            probed_at: "2026-04-27T00:00:00Z".into(),
            probe_ms: 100,
        },
        runtimes: vec![DetectResult {
            runtime: RuntimeKind::Ollama,
            status: Status::Running,
            version: Some("0.4.0".into()),
            endpoint: Some("http://127.0.0.1:11434".into()),
            error: None,
        }],
    }
}

fn opts(env: EnvironmentReport, ollama: Option<String>, use_llm: bool) -> ScannerOptions {
    ScannerOptions {
        probe: Arc::new(MockProbe { env }),
        ollama_endpoint: ollama,
        model_cascade: DEFAULT_CASCADE.iter().map(|s| s.to_string()).collect(),
        use_llm,
        cron: None,         // 테스트는 cron 비활성
        launch_grace: None, // 테스트는 grace 비활성
    }
}

#[tokio::test]
async fn deterministic_only_when_use_llm_false() {
    let svc = ScannerService::new(opts(base_env(), None, false))
        .await
        .unwrap();
    let summary = svc.scanner.scan_now().await.unwrap();
    assert_eq!(summary.summary_source, SummarySource::Deterministic);
    assert!(summary.model_used.is_none());
    assert!(!summary.checks.is_empty());
    assert!(summary.summary_korean.contains("점검"));
}

#[tokio::test]
async fn ollama_unreachable_falls_back_to_deterministic() {
    let svc = ScannerService::new(opts(
        base_env(),
        Some("http://127.0.0.1:65000".into()), // 빈 포트
        true,
    ))
    .await
    .unwrap();
    let summary = svc.scanner.scan_now().await.unwrap();
    assert_eq!(summary.summary_source, SummarySource::Deterministic);
    assert!(summary.model_used.is_none());
}

#[tokio::test]
async fn llm_happy_path_returns_korean_summary() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{ "name": "exaone:1.2b" }]
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": "환경 점검을 마쳤어요. 메모리는 16GB, NVIDIA GPU도 잘 인식됐어요. Ollama가 동작 중이라 모델을 바로 불러올 수 있어요.",
            "done": true
        })))
        .mount(&server)
        .await;

    let svc = ScannerService::new(opts(base_env(), Some(server.uri()), true))
        .await
        .unwrap();
    let summary = svc.scanner.scan_now().await.unwrap();
    assert_eq!(summary.summary_source, SummarySource::Llm);
    assert!(summary.model_used.as_deref() == Some("exaone:1.2b"));
    assert!(summary.summary_korean.contains("환경"));
}

#[tokio::test]
async fn llm_invalid_response_falls_back_to_deterministic() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{ "name": "exaone:1.2b" }]
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": "All systems nominal. No issues detected.", // 영어만
            "done": true
        })))
        .mount(&server)
        .await;

    let svc = ScannerService::new(opts(base_env(), Some(server.uri()), true))
        .await
        .unwrap();
    let summary = svc.scanner.scan_now().await.unwrap();
    // 검증 실패 → deterministic.
    assert_eq!(summary.summary_source, SummarySource::Deterministic);
}

#[tokio::test]
async fn no_cascade_match_falls_back_to_deterministic() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{ "name": "irrelevant:99b" }]
        })))
        .mount(&server)
        .await;

    let svc = ScannerService::new(opts(base_env(), Some(server.uri()), true))
        .await
        .unwrap();
    let summary = svc.scanner.scan_now().await.unwrap();
    assert_eq!(summary.summary_source, SummarySource::Deterministic);
}

#[tokio::test]
async fn deterministic_check_emits_ram_warn_below_8gb() {
    let mut env = base_env();
    env.hardware.mem.total_bytes = 4 * GIB;
    let svc = ScannerService::new(opts(env, None, false)).await.unwrap();
    let summary = svc.scanner.scan_now().await.unwrap();
    let ram = summary.checks.iter().find(|c| c.id == "ram-low").unwrap();
    assert_eq!(ram.severity, Severity::Warn);
}

#[tokio::test]
async fn broadcast_subscriber_receives_summary() {
    let svc = ScannerService::new(opts(base_env(), None, false))
        .await
        .unwrap();
    let mut rx = svc.scanner.subscribe();
    let scanner = Arc::clone(&svc.scanner);
    let task = tokio::spawn(async move { scanner.scan_now().await });
    let received = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("timeout")
        .expect("recv ok");
    let _ = task.await.unwrap().unwrap();
    assert!(!received.summary_korean.is_empty());
}

#[tokio::test]
async fn cascade_cache_hits_tags_only_once() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": [{ "name": "exaone:1.2b" }]
        })))
        .expect(1) // ★ 두 번 scan하지만 tags는 한 번만 hit (1h 캐시).
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/generate"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "response": "환경이 정상이에요. 모든 항목을 잘 마쳤어요.",
            "done": true
        })))
        .mount(&server)
        .await;

    let svc = ScannerService::new(opts(base_env(), Some(server.uri()), true))
        .await
        .unwrap();
    let _ = svc.scanner.scan_now().await.unwrap();
    let _ = svc.scanner.scan_now().await.unwrap();
    // server.expect(1) 위반 시 wiremock이 자동 panic — 명시 assert 불필요.
}
