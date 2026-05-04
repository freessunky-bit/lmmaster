//! adapter-ollama — 외부 설치형 attach.
//!
//! 정책 (ADR-0005, Phase 1' 결정):
//! - **Wrap-not-replace**: Ollama 바이너리 임베드 안 함. 별도 설치된 데몬에 HTTP attach.
//! - `start/stop/restart`은 no-op — 외부 데몬은 사용자가 통제.
//! - `install/update`는 bail — `crates/installer`가 책임.
//! - `pull_model`은 non-stream POST (UI streaming progress는 v1.x).
//! - `keep_alive: "5m"` — warmup 후 5분간 메모리 상주.

use std::collections::HashMap;
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

/// 모델 풀 한 번에 대해 emit되는 단일 이벤트.
///
/// 정책 (phase-install-bench-bugfix-decision §2.2):
/// - layer 단위가 아닌 *전체* 누적 진행률 + EMA 속도 + ETA — UI는 단일 progress bar 1개만.
/// - status는 Ollama가 보내는 그대로 (ko 라벨링은 frontend에서) — backward 호환 + i18n 분리.
/// - bytes_total이 None인 단계(manifest pulling)에선 progress 표시 보류.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ModelPullEvent {
    /// pull 단계 변화 ("pulling manifest", "pulling <digest>", "verifying", "writing manifest", "success" 등).
    Status {
        status: String,
    },
    /// 진행률 — bytes 누적 + EMA 속도. 모든 layer 합산.
    Progress {
        completed_bytes: u64,
        total_bytes: u64,
        speed_bps: u64,
        eta_secs: Option<u64>,
    },
    Completed,
    Cancelled,
    Failed {
        message: String,
    },
}

/// Ollama `/api/pull` 단일 NDJSON line. status 누락 객체는 error 필드 검사 후 처리.
#[derive(Debug, Deserialize)]
struct PullChunk {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    digest: Option<String>,
    #[serde(default)]
    total: Option<u64>,
    #[serde(default)]
    completed: Option<u64>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct PullStreamRequest<'a> {
    name: &'a str,
    stream: bool,
}

/// 모델 풀 진행 결과 — 호출 측은 last `Completed` event도 별도 받음.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PullOutcome {
    Completed,
    Cancelled,
    Failed(String),
}

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
        // Phase R-C (ADR-0055) — 폴백 제거. .no_proxy()는 이미 적용 — build 실패는 fail-fast.
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_millis(500))
            .timeout(Duration::from_secs(60))
            .pool_idle_timeout(Duration::from_secs(30))
            .no_proxy()
            .build()
            .expect("reqwest Client builder must succeed (TLS init)");
        Self {
            endpoint: endpoint.into(),
            http,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.endpoint.trim_end_matches('/'), path)
    }

    /// 모델이 이미 받아져 있는지 확인 — preflight + 풀 skip 판정용.
    ///
    /// `model_id`는 `name:tag` 또는 `org/name:tag` 형식. tag 누락 시 `:latest`로 정규화 후 비교.
    pub async fn has_model(&self, model_id: &str) -> anyhow::Result<bool> {
        let resp = self
            .http
            .get(self.url("/api/tags"))
            .timeout(PROBE_TIMEOUT)
            .send()
            .await?;
        if !resp.status().is_success() {
            anyhow::bail!("Ollama /api/tags HTTP {}", resp.status());
        }
        let body: TagsResponse = resp.json().await?;
        let needle = normalize_model_id(model_id);
        Ok(body
            .models
            .iter()
            .any(|m| normalize_model_id(&m.name) == needle))
    }

    /// 스트리밍 모델 풀 — `/api/pull stream:true` NDJSON 한 줄씩 파싱 + layer aggregate.
    ///
    /// 정책 (phase-install-bench-bugfix-decision §2.2 + 2026-04-30 사용자 경험 보강):
    /// - reqwest `bytes_stream()` + 줄 단위 buffer drain으로 NDJSON 파싱 (ollama-rs 우회).
    /// - layer 단위 total/completed를 `HashMap<digest, (total, completed)>`로 누적 후 sum.
    /// - EMA speed: 이전 0.7 + 현재 0.3, 5초 sliding window 효과.
    /// - cancel: stream drop으로 server abort (Ollama 0.1.40+).
    /// - 에러: 첫 객체에 `error` 필드 → ModelNotFound (404 미러).
    /// - **자동 재시도**: 일시적 끊김(stream chunk 디코딩 실패 / 연결 끊김)은 최대 2회 재시도.
    ///   Ollama 0.1.40+가 chunk-cache로 자동 resume 처리. 사용자에겐 "잠깐 끊겼어요. 다시 받을게요"
    ///   status 이벤트로 진행 흐름 끊지 않고 연결 복구를 안내.
    pub async fn pull_model_stream(
        &self,
        model_id: &str,
        on_event: impl Fn(ModelPullEvent),
        cancel: &CancellationToken,
    ) -> PullOutcome {
        const MAX_ATTEMPTS: u32 = 3;
        for attempt in 1..=MAX_ATTEMPTS {
            match self.pull_attempt(model_id, &on_event, cancel).await {
                PullAttemptOutcome::Completed => return PullOutcome::Completed,
                PullAttemptOutcome::Cancelled => return PullOutcome::Cancelled,
                PullAttemptOutcome::PermanentFailed(msg) => {
                    on_event(ModelPullEvent::Failed {
                        message: msg.clone(),
                    });
                    return PullOutcome::Failed(msg);
                }
                PullAttemptOutcome::TransientFailed(msg) => {
                    if attempt >= MAX_ATTEMPTS {
                        // 마지막 시도까지 실패 — 사용자에게 명확한 에러 노출.
                        let final_msg = format!(
                            "여러 번 시도했지만 받지 못했어요. 네트워크를 확인해 볼래요? ({msg})"
                        );
                        on_event(ModelPullEvent::Failed {
                            message: final_msg.clone(),
                        });
                        return PullOutcome::Failed(final_msg);
                    }
                    // 2s → 4s 지수 백오프 + 사용자 향 진행 카피 (Failed 노출 X — 풀 흐름 유지).
                    let backoff = Duration::from_secs(2_u64.pow(attempt));
                    tracing::warn!(
                        attempt = attempt,
                        max = MAX_ATTEMPTS,
                        backoff_ms = backoff.as_millis(),
                        error = %msg,
                        "ollama pull transient error — retrying"
                    );
                    on_event(ModelPullEvent::Status {
                        status: format!(
                            "잠깐 끊겼어요. 다시 받을게요 (시도 {}/{})",
                            attempt + 1,
                            MAX_ATTEMPTS
                        ),
                    });
                    tokio::select! {
                        () = cancel.cancelled() => {
                            on_event(ModelPullEvent::Cancelled);
                            return PullOutcome::Cancelled;
                        }
                        () = tokio::time::sleep(backoff) => {}
                    }
                }
            }
        }
        unreachable!("retry 루프는 MAX_ATTEMPTS 안에서 결과를 반환해야 해요");
    }

    /// 단일 풀 시도 — retry 래퍼가 transient 결과를 분류해 재시도 결정.
    async fn pull_attempt(
        &self,
        model_id: &str,
        on_event: &impl Fn(ModelPullEvent),
        cancel: &CancellationToken,
    ) -> PullAttemptOutcome {
        let body = PullStreamRequest {
            name: model_id,
            stream: true,
        };
        // 초기 connect도 cancel-aware — 그렇지 않으면 cancel 시 send.await가 끝날 때까지 응답 못 함.
        let send_fut = self.http.post(self.url("/api/pull")).json(&body).send();
        let resp = tokio::select! {
            biased;
            () = cancel.cancelled() => {
                return PullAttemptOutcome::Cancelled;
            }
            r = send_fut => match r {
                Ok(r) => r,
                Err(e) => {
                    // 연결 자체 실패 — 일시적 (네트워크/Ollama 데몬 잠깐 멈춤) 가능성.
                    return PullAttemptOutcome::TransientFailed(format!("Ollama 연결 실패: {e}"));
                }
            }
        };
        if !resp.status().is_success() {
            // 4xx/5xx — 5xx는 일시적, 4xx는 영구 (모델 명세 잘못 등).
            let status = resp.status();
            let msg = format!("Ollama HTTP {status}");
            return if status.is_server_error() {
                PullAttemptOutcome::TransientFailed(msg)
            } else {
                PullAttemptOutcome::PermanentFailed(msg)
            };
        }

        let mut stream = resp.bytes_stream();
        let mut buffer: Vec<u8> = Vec::new();

        // layer 단위 누적: digest → (total, completed).
        let mut layers: HashMap<String, (u64, u64)> = HashMap::new();
        let mut last_status: Option<String> = None;
        let mut last_emitted_pct: i32 = -1;
        let mut speed_ema: f64 = 0.0;
        let mut last_completed_total: u64 = 0;
        let mut last_progress_at = Instant::now();

        loop {
            tokio::select! {
                () = cancel.cancelled() => {
                    return PullAttemptOutcome::Cancelled;
                }
                next = stream.next() => {
                    match next {
                        Some(Ok(bytes)) => {
                            buffer.extend_from_slice(&bytes);
                            while let Some(pos) = buffer.iter().position(|b| *b == b'\n') {
                                let line: Vec<u8> = buffer.drain(..=pos).collect();
                                let trimmed = line.trim_ascii();
                                if trimmed.is_empty() {
                                    continue;
                                }
                                let chunk: PullChunk = match serde_json::from_slice(trimmed) {
                                    Ok(c) => c,
                                    Err(e) => {
                                        tracing::debug!(error = %e, "ollama pull chunk parse skip");
                                        continue;
                                    }
                                };

                                // status 누락 + error 있음 → 모델 없음 / 디스크 부족 / 권한 등.
                                // "not found"는 영구 (재시도해도 동일), 그 외는 일시적일 수 있음.
                                if let Some(err_msg) = chunk.error.as_deref() {
                                    if err_msg.contains("not found")
                                        || err_msg.contains("does not exist")
                                    {
                                        return PullAttemptOutcome::PermanentFailed(format!(
                                            "이 모델을 Ollama 저장소에서 찾지 못했어요 (id={model_id})"
                                        ));
                                    }
                                    return PullAttemptOutcome::TransientFailed(format!(
                                        "Ollama 풀 실패: {err_msg}"
                                    ));
                                }

                                // status 변화 — 사용자 카피용 emit.
                                if let Some(s) = chunk.status.as_deref() {
                                    if last_status.as_deref() != Some(s) {
                                        on_event(ModelPullEvent::Status { status: s.to_string() });
                                        last_status = Some(s.to_string());
                                    }
                                    if s == "success" {
                                        on_event(ModelPullEvent::Completed);
                                        return PullAttemptOutcome::Completed;
                                    }
                                }

                                // layer 누적 — digest 단위.
                                if let Some(digest) = chunk.digest.as_deref() {
                                    let total = chunk.total.unwrap_or(0);
                                    let completed = chunk.completed.unwrap_or(0);
                                    let entry = layers.entry(digest.to_string()).or_insert((0, 0));
                                    if total > entry.0 {
                                        entry.0 = total;
                                    }
                                    // completed는 monotonic — 이전 값 미만은 무시 (NDJSON 순서 보장 안 됨).
                                    if completed > entry.1 {
                                        entry.1 = completed;
                                    }
                                }

                                let total_sum: u64 = layers.values().map(|(t, _)| *t).sum();
                                let completed_sum: u64 = layers.values().map(|(_, c)| *c).sum();
                                if total_sum == 0 {
                                    continue;
                                }
                                let pct: i32 =
                                    ((completed_sum as f64 / total_sum as f64) * 100.0) as i32;

                                // EMA speed — 마지막 progress 이후 시간 기준.
                                let now = Instant::now();
                                let dt_ms = now.duration_since(last_progress_at).as_millis() as f64;
                                if dt_ms > 50.0 {
                                    let delta = completed_sum.saturating_sub(last_completed_total) as f64;
                                    let inst_bps = delta * 1000.0 / dt_ms;
                                    speed_ema = if speed_ema == 0.0 {
                                        inst_bps
                                    } else {
                                        0.7 * speed_ema + 0.3 * inst_bps
                                    };
                                    last_progress_at = now;
                                    last_completed_total = completed_sum;
                                }

                                // 1% 단위 throttle — frontend overhead 감소.
                                // 새 layer 발표로 분모(total_sum)가 늘면 pct가 일시적으로 작아질 수 있음.
                                // 사용자에게 "거꾸로 가는" 진행률은 큰 마찰이라, last_emitted_pct를
                                // floor로 사용해 단조성 보장. 실제 bytes 카운트는 함께 노출되니
                                // 정확도 손실 없음.
                                if pct > last_emitted_pct {
                                    let speed_bps = speed_ema as u64;
                                    let eta_secs = total_sum
                                        .saturating_sub(completed_sum)
                                        .checked_div(speed_bps);
                                    on_event(ModelPullEvent::Progress {
                                        completed_bytes: completed_sum,
                                        total_bytes: total_sum,
                                        speed_bps,
                                        eta_secs,
                                    });
                                    last_emitted_pct = pct;
                                }
                            }
                        }
                        Some(Err(e)) => {
                            // stream chunk 디코딩/네트워크 에러 — Ollama 0.1.40+ chunk-cache로
                            // resume 가능하므로 transient로 분류.
                            return PullAttemptOutcome::TransientFailed(format!(
                                "Ollama 응답 읽기 실패: {e}"
                            ));
                        }
                        None => {
                            // stream 정상 종료 후에도 success status 못 받았다면 이상 종료로 처리.
                            // 단, 마지막 status가 성공계열이면 Completed로 신뢰.
                            if matches!(last_status.as_deref(), Some("success")) {
                                on_event(ModelPullEvent::Completed);
                                return PullAttemptOutcome::Completed;
                            }
                            return PullAttemptOutcome::TransientFailed(
                                "Ollama 연결이 끊겼어요".into(),
                            );
                        }
                    }
                }
            }
        }
    }
}

/// 단일 풀 시도 결과 분류 — retry 결정에 사용.
#[derive(Debug)]
enum PullAttemptOutcome {
    Completed,
    Cancelled,
    /// 재시도해도 동일 결과 (모델 없음 / 4xx) — 즉시 사용자에게 노출.
    PermanentFailed(String),
    /// 일시적 (5xx / 연결 끊김 / chunk 디코딩 실패) — 재시도 후 복구 가능.
    TransientFailed(String),
}

// ── Chat streaming (사용자 in-app 채팅 체험) ─────────────────────────────

/// 한 chat turn 메시지 — Ollama `/api/chat`의 messages 필드 미러.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// "system" / "user" / "assistant".
    pub role: String,
    pub content: String,
    /// Phase 13'.h (ADR-0050) — 멀티모달 이미지. base64 인코딩된 string 배열.
    /// `None` 또는 빈 vec이면 텍스트 전용 (기존 호환). Ollama API: messages[i].images.
    /// `vision_support: true` 모델만 의미 있음 — 그 외 모델은 Ollama가 무시 또는 에러.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub images: Option<Vec<String>>,
}

/// Chat 스트림 이벤트 — UI에 실시간 token chunk 전달.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ChatEvent {
    /// 토큰 단위 추가 텍스트 (delta). UI는 누적 표시.
    Delta {
        text: String,
    },
    /// 정상 종료. 마지막 chunk 후 emit.
    Completed {
        /// 총 응답 ms — 호출 측 elapsed 측정용 hint.
        took_ms: u64,
    },
    Cancelled,
    Failed {
        message: String,
    },
}

/// Chat 호출 결과 — IPC outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatOutcome {
    Completed,
    Cancelled,
    Failed(String),
}

/// Ollama `/api/chat` request DTO.
#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    stream: bool,
    keep_alive: &'a str,
}

/// Ollama `/api/chat` 응답 chunk — `{message: {role, content}, done}`.
#[derive(Debug, Deserialize)]
struct ChatChunk {
    #[serde(default)]
    message: Option<ChatChunkMessage>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatChunkMessage {
    #[serde(default)]
    content: String,
}

impl OllamaAdapter {
    /// 사용자 in-app 채팅용 streaming 호출.
    ///
    /// 정책 (사용자 모델 검증/체험 — 2026-04-30):
    /// - `/api/chat stream:true` NDJSON 한 줄씩 파싱 → Delta 이벤트로 token 단위 emit.
    /// - cancel은 stream drop으로 server abort.
    /// - HTTP 4xx (모델 없음 / 잘못된 메시지)는 즉시 실패. 5xx는 사용자 향 에러.
    /// - keep_alive 5분 — 연이은 메시지에 cold load 안 일어나게.
    pub async fn chat_stream(
        &self,
        model_id: &str,
        messages: &[ChatMessage],
        on_event: impl Fn(ChatEvent),
        cancel: &CancellationToken,
    ) -> ChatOutcome {
        let started = Instant::now();
        let body = ChatRequest {
            model: model_id,
            messages,
            stream: true,
            keep_alive: "5m",
        };
        let send_fut = self.http.post(self.url("/api/chat")).json(&body).send();
        let resp = tokio::select! {
            biased;
            () = cancel.cancelled() => {
                on_event(ChatEvent::Cancelled);
                return ChatOutcome::Cancelled;
            }
            r = send_fut => match r {
                Ok(r) => r,
                Err(e) => {
                    let msg = format!("Ollama 연결 실패: {e}");
                    on_event(ChatEvent::Failed { message: msg.clone() });
                    return ChatOutcome::Failed(msg);
                }
            }
        };

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            let msg = if text.contains("not found") {
                format!("이 모델이 Ollama에 없어요. 먼저 받아주세요. (id={model_id})")
            } else {
                format!("Ollama HTTP {status}: {text}")
            };
            on_event(ChatEvent::Failed {
                message: msg.clone(),
            });
            return ChatOutcome::Failed(msg);
        }

        let mut stream = resp.bytes_stream();
        let mut buffer: Vec<u8> = Vec::new();
        // Phase R-C (ADR-0055) — delta 발행 여부 추적. transport 에러 발생 시:
        //   - delta 1건 이상 emit됨 → 부분 응답 정상 표시 (graceful early disconnect 가능성).
        //   - delta 0건 → Failed 유지 (실 에러).
        let mut delta_emitted = false;

        loop {
            tokio::select! {
                () = cancel.cancelled() => {
                    on_event(ChatEvent::Cancelled);
                    return ChatOutcome::Cancelled;
                }
                next = stream.next() => {
                    match next {
                        Some(Ok(bytes)) => {
                            buffer.extend_from_slice(&bytes);
                            while let Some(pos) = buffer.iter().position(|b| *b == b'\n') {
                                let line: Vec<u8> = buffer.drain(..=pos).collect();
                                let trimmed = line.trim_ascii();
                                if trimmed.is_empty() {
                                    continue;
                                }
                                let chunk: ChatChunk = match serde_json::from_slice(trimmed) {
                                    Ok(c) => c,
                                    Err(e) => {
                                        tracing::debug!(error = %e, "ollama chat chunk parse skip");
                                        continue;
                                    }
                                };
                                if let Some(err_msg) = chunk.error.as_deref() {
                                    let msg = format!("Ollama 채팅 실패: {err_msg}");
                                    on_event(ChatEvent::Failed { message: msg.clone() });
                                    return ChatOutcome::Failed(msg);
                                }
                                if let Some(m) = chunk.message {
                                    if !m.content.is_empty() {
                                        delta_emitted = true;
                                        on_event(ChatEvent::Delta { text: m.content });
                                    }
                                }
                                if chunk.done {
                                    on_event(ChatEvent::Completed {
                                        took_ms: started.elapsed().as_millis() as u64,
                                    });
                                    return ChatOutcome::Completed;
                                }
                            }
                        }
                        Some(Err(e)) => {
                            // Phase R-C — delta 1건 이상 emit됐으면 graceful early disconnect로 간주.
                            if delta_emitted {
                                tracing::warn!(error = %e, "ollama 스트림 중단 — 부분 응답으로 마감");
                                on_event(ChatEvent::Completed {
                                    took_ms: started.elapsed().as_millis() as u64,
                                });
                                return ChatOutcome::Completed;
                            }
                            let msg = format!("Ollama 응답 읽기 실패: {e}");
                            on_event(ChatEvent::Failed { message: msg.clone() });
                            return ChatOutcome::Failed(msg);
                        }
                        None => {
                            // stream EOF — done 마커 못 받아도 부분 응답으로 마감 (graceful).
                            on_event(ChatEvent::Completed {
                                took_ms: started.elapsed().as_millis() as u64,
                            });
                            return ChatOutcome::Completed;
                        }
                    }
                }
            }
        }
    }
}

/// `name:tag` / `org/name:tag` / `name` (no tag) 정규화. tag 누락이면 `:latest` 부착.
fn normalize_model_id(id: &str) -> String {
    let id = id.trim();
    if id.contains(':') {
        id.to_string()
    } else {
        format!("{id}:latest")
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
    async fn has_model_returns_true_when_listed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "models": [
                    { "name": "exaone:1.2b", "size": 800u64, "digest": "abc" },
                    { "name": "qwen2.5:3b", "size": 2000u64, "digest": "def" }
                ]
            })))
            .mount(&server)
            .await;
        let a = OllamaAdapter::with_endpoint(server.uri());
        assert!(a.has_model("exaone:1.2b").await.unwrap());
        // tag 누락 — :latest로 정규화 후 매치.
        let server2 = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "models": [
                    { "name": "polyglot-ko:latest", "size": 800u64, "digest": "abc" }
                ]
            })))
            .mount(&server2)
            .await;
        let b = OllamaAdapter::with_endpoint(server2.uri());
        assert!(b.has_model("polyglot-ko").await.unwrap());
        assert!(!b.has_model("nonexistent").await.unwrap());
    }

    /// invariant: 모든 layer 합산 진행률은 monotonic이어야 한다 (0→100→0 점프 없음).
    /// open-webui v0.1대 패턴 회귀 방지.
    #[tokio::test]
    async fn pull_model_stream_aggregates_layers_monotonic() {
        let server = MockServer::start().await;
        // 2 layer × 2 chunk + success.
        let body = vec![
            serde_json::json!({"status":"pulling manifest"}),
            serde_json::json!({"status":"pulling abc","digest":"abc","total":1000u64,"completed":300u64}),
            serde_json::json!({"status":"pulling def","digest":"def","total":2000u64,"completed":500u64}),
            serde_json::json!({"status":"pulling abc","digest":"abc","total":1000u64,"completed":1000u64}),
            serde_json::json!({"status":"pulling def","digest":"def","total":2000u64,"completed":2000u64}),
            serde_json::json!({"status":"verifying sha256 digest"}),
            serde_json::json!({"status":"writing manifest"}),
            serde_json::json!({"status":"success"}),
        ]
        .into_iter()
        .map(|v| serde_json::to_string(&v).unwrap())
        .collect::<Vec<_>>()
        .join("\n")
            + "\n";

        Mock::given(method("POST"))
            .and(path("/api/pull"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let a = OllamaAdapter::with_endpoint(server.uri());
        let cancel = CancellationToken::new();
        let events: std::sync::Arc<std::sync::Mutex<Vec<ModelPullEvent>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let events_inner = events.clone();
        let outcome = a
            .pull_model_stream(
                "exaone:1.2b",
                move |e| events_inner.lock().unwrap().push(e),
                &cancel,
            )
            .await;
        assert_eq!(outcome, PullOutcome::Completed);

        // 모든 progress 이벤트의 percentage가 단조 증가하는지 — invariant.
        let evs = events.lock().unwrap();
        let mut last_pct: f64 = -1.0;
        let mut saw_progress = false;
        for e in evs.iter() {
            if let ModelPullEvent::Progress {
                completed_bytes,
                total_bytes,
                ..
            } = e
            {
                saw_progress = true;
                let pct = (*completed_bytes as f64) * 100.0 / (*total_bytes as f64);
                assert!(
                    pct >= last_pct,
                    "진행률이 거꾸로 갔어요: {last_pct} → {pct}"
                );
                last_pct = pct;
            }
        }
        assert!(saw_progress, "progress 이벤트가 없었어요");
        assert!(
            evs.iter().any(|e| matches!(e, ModelPullEvent::Completed)),
            "Completed 이벤트가 없었어요"
        );
    }

    /// invariant: 모델이 Ollama 저장소에 없을 때 ModelNotFound로 매핑되고 메시지가 한국어여야 한다.
    #[tokio::test]
    async fn pull_model_stream_maps_model_not_found() {
        let server = MockServer::start().await;
        // status 누락 + error 필드만 있는 단일 객체 (Ollama 실측 패턴).
        let body = serde_json::json!({"error": "model 'nope' not found"}).to_string() + "\n";
        Mock::given(method("POST"))
            .and(path("/api/pull"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;

        let a = OllamaAdapter::with_endpoint(server.uri());
        let cancel = CancellationToken::new();
        let events: std::sync::Arc<std::sync::Mutex<Vec<ModelPullEvent>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let events_inner = events.clone();
        let outcome = a
            .pull_model_stream(
                "nope",
                move |e| events_inner.lock().unwrap().push(e),
                &cancel,
            )
            .await;
        match outcome {
            PullOutcome::Failed(msg) => {
                assert!(
                    msg.contains("찾지 못했어요") || msg.contains("not found"),
                    "한국어 메시지가 누락됨: {msg}"
                );
            }
            other => panic!("기대: Failed, 실제: {other:?}"),
        }
    }

    /// invariant: cancel은 즉시 PullOutcome::Cancelled 반환 + Cancelled 이벤트 emit.
    #[tokio::test]
    async fn pull_model_stream_cancel_is_immediate() {
        let server = MockServer::start().await;
        // 1초 delay 후에야 첫 chunk 옴 — cancel이 그 사이에 발동.
        Mock::given(method("POST"))
            .and(path("/api/pull"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("{\"status\":\"pulling manifest\"}\n")
                    .set_delay(Duration::from_secs(2)),
            )
            .mount(&server)
            .await;

        let a = OllamaAdapter::with_endpoint(server.uri());
        let cancel = CancellationToken::new();
        let cancel_for_task = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            cancel_for_task.cancel();
        });
        let outcome = a.pull_model_stream("exaone:1.2b", |_| {}, &cancel).await;
        assert_eq!(outcome, PullOutcome::Cancelled);
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

    // ── Phase 13'.h — ChatMessage vision (images) invariants ──────────

    #[test]
    fn chat_message_without_images_does_not_serialize_field() {
        // 백워드 호환 — images=None은 wire format에서 사라져야 함.
        let m = ChatMessage {
            role: "user".into(),
            content: "안녕".into(),
            images: None,
        };
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["role"], "user");
        assert_eq!(v["content"], "안녕");
        assert!(v.get("images").is_none(), "images=None은 직렬화 X");
    }

    #[test]
    fn chat_message_with_images_serializes_array() {
        let m = ChatMessage {
            role: "user".into(),
            content: "이 사진은 뭐예요?".into(),
            images: Some(vec!["base64-payload".into()]),
        };
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["images"][0], "base64-payload");
    }

    #[test]
    fn chat_message_legacy_without_images_field_parses() {
        // 기존 frontend가 보내는 {role, content}만 있는 메시지도 파싱.
        let json = r#"{"role":"user","content":"x"}"#;
        let m: ChatMessage = serde_json::from_str(json).unwrap();
        assert!(m.images.is_none());
    }

    #[test]
    fn chat_message_with_images_field_parses() {
        let json = r#"{"role":"user","content":"x","images":["abc"]}"#;
        let m: ChatMessage = serde_json::from_str(json).unwrap();
        assert_eq!(m.images.as_deref().map(|v| v.len()), Some(1));
    }
}
