//! runtime-detector 통합 테스트.
//!
//! wiremock으로 mock HTTP 서버를 띄워 Ollama/LM Studio 응답을 시뮬레이션한다.
//! 실제 사용자 PC에 Ollama/LM Studio가 떠 있을 수 있으므로,
//! 디폴트 포트를 가정하는 detect_all 테스트는 항상 통과 가능한 invariant만 단언한다.

use std::time::Duration;

use runtime_detector::{lm_studio, ollama, Detector, DetectorConfig, Status};
use shared_types::RuntimeKind;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn ollama_running_returns_version_and_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "0.5.7"})),
        )
        .mount(&server)
        .await;

    let client = client_with_short_timeout();
    let r = ollama::probe_at(&client, &server.uri())
        .await
        .expect("probe ok");

    assert_eq!(r.runtime, RuntimeKind::Ollama);
    assert_eq!(r.status, Status::Running);
    assert_eq!(r.version.as_deref(), Some("0.5.7"));
    assert_eq!(r.endpoint.as_deref(), Some(server.uri().as_str()));
    assert!(r.error.is_none());
}

#[tokio::test]
async fn ollama_unreachable_returns_not_installed() {
    let client = client_with_short_timeout();
    // 127.0.0.1:1 — 사용 가능성이 거의 없는 well-known 포트(TCPMUX), 보통 connect refused.
    let r = ollama::probe_at(&client, "http://127.0.0.1:1")
        .await
        .expect("probe should not error");
    assert_eq!(r.runtime, RuntimeKind::Ollama);
    assert_eq!(r.status, Status::NotInstalled);
    assert!(r.endpoint.is_none());
}

#[tokio::test]
async fn ollama_non_2xx_returns_error_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/version"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let client = client_with_short_timeout();
    let r = ollama::probe_at(&client, &server.uri())
        .await
        .expect("probe ok");
    assert_eq!(r.status, Status::Error);
    assert!(r.error.as_deref().unwrap().contains("503"));
}

#[tokio::test]
async fn lm_studio_running_with_models_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "object": "list",
            "data": [
                {"id": "qwen2.5-7b-instruct", "object": "model"},
                {"id": "exaone-3.5-2.4b-instruct", "object": "model"}
            ]
        })))
        .mount(&server)
        .await;

    let client = client_with_short_timeout();
    let r = lm_studio::probe_at(&client, &server.uri())
        .await
        .expect("probe ok");

    assert_eq!(r.runtime, RuntimeKind::LmStudio);
    assert_eq!(r.status, Status::Running);
    // LM Studio REST는 daemon 버전을 노출하지 않으므로 None.
    assert!(r.version.is_none());
    assert_eq!(r.endpoint.as_deref(), Some(server.uri().as_str()));
}

#[tokio::test]
async fn lm_studio_running_with_empty_model_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "object": "list",
            "data": []
        })))
        .mount(&server)
        .await;

    let client = client_with_short_timeout();
    let r = lm_studio::probe_at(&client, &server.uri())
        .await
        .expect("probe ok");
    // 모델이 0개여도 daemon이 응답하면 Running.
    assert_eq!(r.status, Status::Running);
}

#[tokio::test]
async fn lm_studio_unreachable_returns_not_installed() {
    let client = client_with_short_timeout();
    let r = lm_studio::probe_at(&client, "http://127.0.0.1:1")
        .await
        .expect("probe should not error");
    assert_eq!(r.status, Status::NotInstalled);
}

#[tokio::test]
async fn detector_detect_all_returns_two_results_in_parallel() {
    // 디폴트 포트(11434, 1234)에 실제 daemon이 떠 있을 수도/없을 수도 있다.
    // 결과 수와 status enum 유효성만 단언한다.
    let cfg = DetectorConfig {
        probe_timeout: Duration::from_millis(500),
        connect_timeout: Duration::from_millis(200),
        ..Default::default()
    };
    let det = Detector::new(cfg).expect("client build");
    let results = det.detect_all().await;

    assert_eq!(results.len(), 2);
    let kinds: Vec<_> = results.iter().map(|r| r.runtime).collect();
    assert!(kinds.contains(&RuntimeKind::Ollama));
    assert!(kinds.contains(&RuntimeKind::LmStudio));

    for r in &results {
        match r.status {
            Status::Running | Status::NotInstalled | Status::Error => {}
            Status::Installed => {
                // Phase 1A.1에서는 OS-레벨 신호 미수집 → 항상 Running 또는 NotInstalled로만 분류.
                panic!("Status::Installed should not appear in Phase 1A.1: {:?}", r);
            }
        }
    }
}

#[tokio::test]
async fn detect_result_serializes_to_clean_json() {
    let r = runtime_detector::DetectResult::running(
        RuntimeKind::Ollama,
        "http://127.0.0.1:11434".to_string(),
        Some("0.5.7".to_string()),
    );
    let json = serde_json::to_value(&r).expect("serialize");
    assert_eq!(json["runtime"], "ollama");
    assert_eq!(json["status"], "running");
    assert_eq!(json["version"], "0.5.7");
    assert_eq!(json["endpoint"], "http://127.0.0.1:11434");
    // skip_serializing_if Option::is_none 으로 error 키는 출력되지 않음.
    assert!(json.get("error").is_none());
}

#[tokio::test]
async fn detect_result_not_installed_omits_optional_fields() {
    let r = runtime_detector::DetectResult::not_installed(RuntimeKind::LmStudio);
    let json = serde_json::to_value(&r).expect("serialize");
    assert_eq!(json["status"], "not-installed");
    assert!(json.get("version").is_none());
    assert!(json.get("endpoint").is_none());
    assert!(json.get("error").is_none());
}

fn client_with_short_timeout() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .connect_timeout(Duration::from_millis(200))
        .no_proxy()
        .build()
        .expect("client build")
}
