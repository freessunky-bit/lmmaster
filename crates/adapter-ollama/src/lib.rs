//! adapter-ollama — 외부 설치형 attach.
//!
//! 정책 (ADR-0005, Phase 1' 결정):
//! - **Wrap-not-replace**: Ollama 바이너리 임베드 안 함. 별도 설치된 데몬에 HTTP attach.
//! - `start/stop/restart`은 no-op — 외부 데몬은 사용자가 통제.
//! - `install/update`는 bail — `crates/installer`가 책임.
//! - `pull_model`은 non-stream POST (UI streaming progress는 v1.x).
//! - `keep_alive: "5m"` — warmup 후 5분간 메모리 상주.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use bench_harness::{BenchAdapter, BenchError, BenchMetricsSource, BenchSample};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use runtime_manager::{
    DetectResult, HealthReport, InstallOpts, LocalModel, ProgressSink, ProgressUpdate,
    RuntimeAdapter, RuntimeCfg, RuntimeHandle,
};
use shared_types::{CapabilityMatrix, ModelRef, RuntimeKind, RuntimeState};

const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:11434";
const PROBE_TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Clone)]
pub struct OllamaAdapter {
    endpoint: String,
    http: reqwest::Client,
}

impl OllamaAdapter {
    pub fn new() -> Self {
        Self::with_endpoint(DEFAULT_ENDPOINT)
    }

    pub fn with_endpoint(endpoint: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_millis(500))
            .timeout(Duration::from_secs(60))
            .pool_idle_timeout(Duration::from_secs(30))
            .no_proxy()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            endpoint: endpoint.into(),
            http,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.endpoint.trim_end_matches('/'), path)
    }
}

impl Default for OllamaAdapter {
    fn default() -> Self {
        Self::new()
    }
}

// ── Ollama API DTO ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct VersionResponse {
    version: String,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<TagsModel>,
}

#[derive(Debug, Deserialize)]
struct TagsModel {
    name: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    digest: String,
}

#[derive(Debug, Serialize)]
struct PullRequest<'a> {
    name: &'a str,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct DeleteRequest<'a> {
    name: &'a str,
}

#[derive(Debug, Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
    keep_alive: &'a str,
}

// ── RuntimeAdapter impl ───────────────────────────────────────────────

#[async_trait]
impl RuntimeAdapter for OllamaAdapter {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Ollama
    }

    async fn detect(&self) -> anyhow::Result<DetectResult> {
        let resp = self
            .http
            .get(self.url("/api/version"))
            .timeout(PROBE_TIMEOUT)
            .send()
            .await;
        match resp {
            Ok(r) if r.status().is_success() => {
                let body: VersionResponse = r.json().await?;
                Ok(DetectResult {
                    installed: true,
                    version: Some(body.version),
                    build_target: None,
                })
            }
            Ok(r) => Ok(DetectResult {
                installed: false,
                version: None,
                build_target: Some(format!("HTTP {}", r.status())),
            }),
            Err(_) => Ok(DetectResult {
                installed: false,
                version: None,
                build_target: None,
            }),
        }
    }

    async fn install(&self, _: InstallOpts) -> anyhow::Result<()> {
        anyhow::bail!(
            "Ollama는 외부 설치형이에요. crates/installer + manifests/apps/ollama.json을 사용해 주세요."
        )
    }

    async fn update(&self) -> anyhow::Result<()> {
        anyhow::bail!("Ollama는 자체 업데이트를 사용해 주세요.")
    }

    async fn start(&self, _cfg: RuntimeCfg) -> anyhow::Result<RuntimeHandle> {
        let detect = self.detect().await?;
        if !detect.installed {
            anyhow::bail!("Ollama가 실행 중이 아니에요. 데몬을 먼저 시작해 주세요.");
        }
        Ok(RuntimeHandle {
            kind: RuntimeKind::Ollama,
            instance_id: "external-ollama".into(),
            internal_port: 11434,
        })
    }

    async fn stop(&self, _h: &RuntimeHandle) -> anyhow::Result<()> {
        Ok(())
    }

    async fn restart(&self, _h: &RuntimeHandle) -> anyhow::Result<()> {
        Ok(())
    }

    async fn health(&self, _h: &RuntimeHandle) -> HealthReport {
        let started = Instant::now();
        let resp = self
            .http
            .get(self.url("/api/version"))
            .timeout(PROBE_TIMEOUT)
            .send()
            .await;
        let latency_ms = started.elapsed().as_millis() as u32;
        match resp {
            Ok(r) if r.status().is_success() => HealthReport {
                state: Some(RuntimeState::Active),
                latency_ms: Some(latency_ms),
                error: None,
            },
            Ok(r) => HealthReport {
                state: Some(RuntimeState::Failed),
                latency_ms: Some(latency_ms),
                error: Some(format!("HTTP {}", r.status())),
            },
            Err(e) => HealthReport {
                state: Some(RuntimeState::Failed),
                latency_ms: None,
                error: Some(e.to_string()),
            },
        }
    }

    async fn list_models(&self) -> anyhow::Result<Vec<LocalModel>> {
        let resp = self.http.get(self.url("/api/tags")).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("Ollama /api/tags HTTP {}", resp.status());
        }
        let body: TagsResponse = resp.json().await?;
        Ok(body
            .models
            .into_iter()
            .map(|m| LocalModel {
                r#ref: None,
                file_rel_path: m.name,
                size_bytes: m.size,
                sha256: m.digest,
            })
            .collect())
    }

    async fn pull_model(&self, m: &ModelRef, sink: ProgressSink) -> anyhow::Result<()> {
        let _ = sink
            .send(ProgressUpdate {
                stage: "pull".into(),
                bytes_done: 0,
                bytes_total: None,
                message: Some(format!("{}을(를) 받고 있어요", m.id)),
            })
            .await;
        let body = PullRequest {
            name: &m.id,
            stream: false,
        };
        let resp = self
            .http
            .post(self.url("/api/pull"))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            anyhow::bail!("Ollama pull 실패: HTTP {}", resp.status());
        }
        let _ = sink
            .send(ProgressUpdate {
                stage: "done".into(),
                bytes_done: 1,
                bytes_total: Some(1),
                message: Some(format!("{} 받기 완료", m.id)),
            })
            .await;
        Ok(())
    }

    async fn remove_model(&self, m: &ModelRef) -> anyhow::Result<()> {
        let body = DeleteRequest { name: &m.id };
        let resp = self
            .http
            .delete(self.url("/api/delete"))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            anyhow::bail!("Ollama delete 실패: HTTP {}", resp.status());
        }
        Ok(())
    }

    async fn warmup(&self, _h: &RuntimeHandle, m: &ModelRef) -> anyhow::Result<()> {
        let body = GenerateRequest {
            model: &m.id,
            prompt: "",
            stream: false,
            keep_alive: "5m",
        };
        let resp = self
            .http
            .post(self.url("/api/generate"))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            anyhow::bail!("Ollama warmup 실패: HTTP {}", resp.status());
        }
        Ok(())
    }

    fn capability_matrix(&self) -> CapabilityMatrix {
        CapabilityMatrix {
            vision: false,
            tools: true,
            structured_output: true,
            embeddings: true,
        }
    }
}

// ── BenchAdapter impl (Phase 2'.c.2) ──────────────────────────────────
//
// 정책 (phase-2pc-bench-decision.md):
// - `/api/generate { stream: true, keep_alive }` — bytes_stream으로 NDJSON 라인 누적.
// - 첫 non-empty `response` chunk = TTFT.
// - `done: true` 마지막 chunk의 `eval_count` / `eval_duration` / `prompt_eval_*` / `load_duration` 추출.
// - cancel 시 stream drop → server abort.
// - metrics_source = Native (Ollama는 ns 단위 native counter 제공).

/// Ollama streaming 응답의 chunk — done false 또는 done true.
#[derive(Debug, Deserialize)]
struct GenerateChunk {
    #[serde(default)]
    response: String,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    eval_count: Option<u64>,
    #[serde(default)]
    eval_duration: Option<u64>, // ns
    #[serde(default)]
    prompt_eval_count: Option<u64>,
    #[serde(default)]
    prompt_eval_duration: Option<u64>, // ns
    #[serde(default)]
    load_duration: Option<u64>, // ns
    #[serde(default)]
    total_duration: Option<u64>, // ns
}

#[derive(Debug, Serialize)]
struct StreamGenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
    keep_alive: &'a str,
}

#[async_trait]
impl BenchAdapter for OllamaAdapter {
    fn runtime_label(&self) -> &'static str {
        "ollama"
    }

    async fn run_prompt(
        &self,
        model_id: &str,
        prompt_id: &str,
        prompt_text: &str,
        keep_alive: &str,
        cancel: &CancellationToken,
    ) -> Result<BenchSample, BenchError> {
        let body = StreamGenerateRequest {
            model: model_id,
            prompt: prompt_text,
            stream: true,
            keep_alive,
        };

        let req_started = Instant::now();
        let resp = self
            .http
            .post(self.url("/api/generate"))
            .json(&body)
            .send()
            .await
            .map_err(|e| BenchError::RuntimeUnreachable(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            // 모델이 없는 케이스(`model not found`)와 그 외 분리.
            let text = resp.text().await.unwrap_or_default();
            if text.contains("not found") {
                return Err(BenchError::ModelNotLoaded(model_id.to_string()));
            }
            return Err(BenchError::Internal(format!(
                "ollama HTTP {status}: {text}"
            )));
        }

        let mut stream = resp.bytes_stream();
        let mut buffer: Vec<u8> = Vec::new();

        let mut first_chunk_at: Option<Instant> = None;
        let mut accumulated_text = String::new();
        let mut last_done: Option<GenerateChunk> = None;

        loop {
            tokio::select! {
                () = cancel.cancelled() => {
                    return Err(BenchError::Cancelled);
                }
                next = stream.next() => {
                    match next {
                        Some(Ok(bytes)) => {
                            buffer.extend_from_slice(&bytes);
                            // NDJSON — 줄 단위 파싱.
                            while let Some(pos) = buffer.iter().position(|b| *b == b'\n') {
                                let line: Vec<u8> = buffer.drain(..=pos).collect();
                                let trimmed = line.trim_ascii();
                                if trimmed.is_empty() {
                                    continue;
                                }
                                let chunk: GenerateChunk = match serde_json::from_slice(trimmed) {
                                    Ok(c) => c,
                                    Err(e) => {
                                        tracing::debug!(error = %e, "ollama chunk parse skip");
                                        continue;
                                    }
                                };
                                if !chunk.response.is_empty() && first_chunk_at.is_none() {
                                    first_chunk_at = Some(Instant::now());
                                }
                                if !chunk.response.is_empty() {
                                    accumulated_text.push_str(&chunk.response);
                                }
                                if chunk.done {
                                    last_done = Some(chunk);
                                }
                            }
                        }
                        Some(Err(e)) => {
                            return Err(BenchError::RuntimeUnreachable(e.to_string()));
                        }
                        None => break,
                    }
                }
            }
        }

        let e2e = req_started.elapsed();
        let ttft = first_chunk_at.unwrap_or_else(Instant::now) - req_started;

        let done = last_done
            .ok_or_else(|| BenchError::Internal("ollama stream ended without done=true".into()))?;

        let tg_tps = match (done.eval_count, done.eval_duration) {
            (Some(count), Some(dur)) if dur > 0 => count as f64 / (dur as f64 / 1e9),
            _ => 0.0,
        };
        let pp_tps = match (done.prompt_eval_count, done.prompt_eval_duration) {
            (Some(count), Some(dur)) if dur > 0 => Some(count as f64 / (dur as f64 / 1e9)),
            _ => None,
        };
        let load_ms = done.load_duration.map(|ns| (ns / 1_000_000) as u32);

        // total_duration이 wall-clock보다 정확 — 있으면 사용.
        let e2e_ms = done
            .total_duration
            .map(|ns| (ns / 1_000_000) as u32)
            .unwrap_or_else(|| e2e.as_millis() as u32);

        let excerpt = if accumulated_text.is_empty() {
            None
        } else {
            // 첫 80 unicode chars (한국어 기준 ~80글자).
            Some(accumulated_text.chars().take(80).collect())
        };

        Ok(BenchSample {
            tg_tps,
            pp_tps,
            ttft_ms: ttft.as_millis() as u32,
            e2e_ms,
            load_ms,
            sample_text_excerpt: excerpt,
            prompt_id: prompt_id.to_string(),
            metrics_source: BenchMetricsSource::Native,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared_types::ModelCategory;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn model_ref(id: &str) -> ModelRef {
        ModelRef {
            id: id.into(),
            display_name: id.into(),
            category: ModelCategory::AgentGeneral,
        }
    }

    #[tokio::test]
    async fn detect_returns_version_when_running() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/version"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "0.4.0"})),
            )
            .mount(&server)
            .await;
        let a = OllamaAdapter::with_endpoint(server.uri());
        let d = a.detect().await.unwrap();
        assert!(d.installed);
        assert_eq!(d.version.as_deref(), Some("0.4.0"));
    }

    #[tokio::test]
    async fn detect_returns_not_installed_on_404() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/version"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let a = OllamaAdapter::with_endpoint(server.uri());
        let d = a.detect().await.unwrap();
        assert!(!d.installed);
    }

    #[tokio::test]
    async fn detect_returns_not_installed_on_unreachable() {
        let a = OllamaAdapter::with_endpoint("http://127.0.0.1:65000");
        let d = a.detect().await.unwrap();
        assert!(!d.installed);
    }

    #[tokio::test]
    async fn list_models_parses_tags() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "models": [
                    { "name": "exaone:1.2b", "size": 800_000_000u64, "digest": "abc" },
                    { "name": "qwen2.5:3b", "size": 2_000_000_000u64, "digest": "def" }
                ]
            })))
            .mount(&server)
            .await;
        let a = OllamaAdapter::with_endpoint(server.uri());
        let models = a.list_models().await.unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].file_rel_path, "exaone:1.2b");
        assert_eq!(models[0].size_bytes, 800_000_000);
    }

    #[tokio::test]
    async fn health_active_on_200() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/version"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "0.4.0"})),
            )
            .mount(&server)
            .await;
        let a = OllamaAdapter::with_endpoint(server.uri());
        let h = a
            .health(&RuntimeHandle {
                kind: RuntimeKind::Ollama,
                instance_id: "x".into(),
                internal_port: 11434,
            })
            .await;
        assert_eq!(h.state, Some(RuntimeState::Active));
        assert!(h.latency_ms.is_some());
    }

    #[tokio::test]
    async fn pull_model_sends_progress_events() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/pull"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let (tx, mut rx) = tokio::sync::mpsc::channel::<ProgressUpdate>(8);
        let a = OllamaAdapter::with_endpoint(server.uri());
        a.pull_model(&model_ref("exaone:1.2b"), tx).await.unwrap();
        let first = rx.recv().await.unwrap();
        assert_eq!(first.stage, "pull");
        let last = rx.recv().await.unwrap();
        assert_eq!(last.stage, "done");
    }

    #[tokio::test]
    async fn warmup_calls_generate_with_keep_alive() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"response": "", "done": true})),
            )
            .expect(1)
            .mount(&server)
            .await;
        let a = OllamaAdapter::with_endpoint(server.uri());
        a.warmup(
            &RuntimeHandle {
                kind: RuntimeKind::Ollama,
                instance_id: "x".into(),
                internal_port: 11434,
            },
            &model_ref("exaone:1.2b"),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn install_bails_with_guidance() {
        let a = OllamaAdapter::new();
        let err = a.install(InstallOpts::default()).await.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("외부 설치형"));
    }

    // ── BenchAdapter 통합 테스트 (Phase 2'.c.2) ──────────────────────

    use bench_harness::BenchAdapter;
    use tokio_util::sync::CancellationToken;

    /// streaming 응답 — 3 chunk + 마지막 done=true (native counter 포함).
    fn ollama_stream_body() -> String {
        let chunks = vec![
            serde_json::json!({"model":"x","response":"안","done":false}),
            serde_json::json!({"model":"x","response":"녕","done":false}),
            serde_json::json!({"model":"x","response":"하세요","done":false}),
            serde_json::json!({
                "model":"x",
                "response":"",
                "done":true,
                "eval_count": 30,
                "eval_duration": 3_000_000_000u64, // 3s → 10 tps
                "prompt_eval_count": 12,
                "prompt_eval_duration": 100_000_000u64, // 100ms → 120 tps
                "load_duration": 50_000_000u64, // 50ms
                "total_duration": 3_500_000_000u64
            }),
        ];
        chunks
            .into_iter()
            .map(|c| serde_json::to_string(&c).unwrap())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    }

    #[tokio::test]
    async fn run_prompt_extracts_native_counters() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(ollama_stream_body())
                    .insert_header("content-type", "application/x-ndjson"),
            )
            .mount(&server)
            .await;
        let a = OllamaAdapter::with_endpoint(server.uri());
        let cancel = CancellationToken::new();
        let sample = a
            .run_prompt("test-model", "bench-ko-chat", "안녕하세요?", "5m", &cancel)
            .await
            .unwrap();
        assert!(matches!(sample.metrics_source, BenchMetricsSource::Native));
        // eval_count=30, eval_duration=3s → 10 tps.
        assert!((sample.tg_tps - 10.0).abs() < 0.01);
        // prompt_eval_count=12, prompt_eval_duration=100ms → 120 tps.
        assert!((sample.pp_tps.unwrap() - 120.0).abs() < 0.01);
        assert_eq!(sample.load_ms, Some(50));
        assert!(sample.e2e_ms >= 3500); // total_duration 우선.
        assert_eq!(sample.prompt_id, "bench-ko-chat");
        assert!(sample
            .sample_text_excerpt
            .as_deref()
            .unwrap()
            .contains("안녕"));
    }

    #[tokio::test]
    async fn run_prompt_returns_model_not_loaded_on_404_text() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(404).set_body_string("model 'unknown' not found"))
            .mount(&server)
            .await;
        let a = OllamaAdapter::with_endpoint(server.uri());
        let cancel = CancellationToken::new();
        let err = a
            .run_prompt("unknown", "bench-ko-chat", "test", "5m", &cancel)
            .await
            .unwrap_err();
        assert!(matches!(err, BenchError::ModelNotLoaded(_)));
    }

    #[tokio::test]
    async fn run_prompt_returns_unreachable_when_endpoint_dead() {
        let a = OllamaAdapter::with_endpoint("http://127.0.0.1:65000");
        let cancel = CancellationToken::new();
        let err = a
            .run_prompt("x", "p", "test", "5m", &cancel)
            .await
            .unwrap_err();
        assert!(matches!(err, BenchError::RuntimeUnreachable(_)));
    }

    #[tokio::test]
    async fn run_prompt_returns_internal_when_no_done_chunk() {
        let server = MockServer::start().await;
        // done=true 없는 응답 → Internal 에러.
        let body = serde_json::to_string(&serde_json::json!({
            "model":"x","response":"only","done":false
        }))
        .unwrap()
            + "\n";
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
        let a = OllamaAdapter::with_endpoint(server.uri());
        let cancel = CancellationToken::new();
        let err = a
            .run_prompt("x", "p", "test", "5m", &cancel)
            .await
            .unwrap_err();
        assert!(matches!(err, BenchError::Internal(_)));
    }

    #[tokio::test]
    async fn run_prompt_label_is_ollama() {
        let a = OllamaAdapter::new();
        assert_eq!(a.runtime_label(), "ollama");
    }
}
