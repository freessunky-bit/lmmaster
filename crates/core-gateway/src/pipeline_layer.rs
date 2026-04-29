//! `PipelineLayer` — gateway middleware for Phase 6'.b filter pipelines.
//!
//! 정책 (ADR-0025, phase-6p-updater-pipelines-decision.md §5):
//! - request 도착 시 body를 JSON으로 파싱 → `PipelineChain::apply_request` 적용 → 변경된 body로 inner forward.
//! - response 도착 시 *content-type*이 `text/event-stream`이면:
//!     - Phase 8'.c.4 (ADR-0030): SSE chunk parser로 chunk 단위 변환. PII redact 등이 streaming 응답에 적용.
//!     - 단, NoOp pipeline 통과 시에는 byte-identical 보장 (golden test로 회귀 가드).
//!     - non-SSE면 body를 파싱 → `PipelineChain::apply_response` → 변경된 body로 클라이언트.
//! - Pipeline이 `Err` 반환 시 OpenAI envelope 형태의 4xx/5xx 응답으로 short-circuit.
//! - body 크기는 `MAX_BODY_BYTES` (2 MiB) 제한 — gateway 기본 limit과 일치.
//! - 본 레이어는 *opt-in* — 기본 라우터에는 미적용. 호출자가 명시적으로 `with_pipelines`로 mount.
//!
//! Phase 8'.c.2 (ADR-0028): chain은 `Arc<ArcSwap<PipelineChain>>`으로 보관 — 사용자 토글 시 lock-free 교체.
//! Phase 8'.c.3 (ADR-0029): `principal_key_scope`가 `enabled_pipelines = Some([...])`이면 그 ID 화이트리스트만 적용.

use std::sync::Arc;
use std::task::{Context, Poll};

use arc_swap::ArcSwap;
use axum::body::{to_bytes, Body};
use axum::http::{header, HeaderValue, Request, Response, StatusCode};
use bytes::Bytes;
use futures::future::BoxFuture;
use http_body_util::BodyExt as _;
use pipelines::{AuditEntry, PipelineChain, PipelineContext, PipelineError};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tower::{Layer, Service};

use crate::sse_chunk::{SseChunk, SseChunkParser};

/// 본문 크기 제한 — 가벼운 전체 버퍼링이 가능하도록 2 MiB cap.
pub const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

/// Phase 8'.c.3 (ADR-0029) — auth middleware가 인증된 키의 `enabled_pipelines`를
/// request extension으로 set하는 마커 타입.
///
/// 정책:
/// - `None` (또는 extension 미존재) = 전역 토글 따름.
/// - `Some(Vec)` = 해당 ID 화이트리스트만 적용. PipelineLayer가 chain.filter_by_ids로 좁힘.
/// - `Some(빈 Vec)` = 모든 Pipeline 비활성 (raw passthrough).
#[derive(Debug, Clone)]
pub struct KeyPipelinesOverride(pub Option<Vec<String>>);

/// Pipeline 미들웨어 레이어 — `Arc<ArcSwap<PipelineChain>>`을 보관 + `Service`를 wrap.
///
/// 사용 예:
/// ```ignore
/// let chain = PipelineChain::new().add(Arc::new(PiiRedactPipeline::new()));
/// let layer = PipelineLayer::new(chain);
/// let router = Router::new().layer(layer);
/// ```
///
/// Phase 6'.d — `with_audit_channel`로 `AuditEntry`를 외부 receiver(예: Tauri PipelinesState
/// ring buffer)에 best-effort try_send. channel full / drop 시 처리 흐름은 절대 block 안 해요.
///
/// Phase 8'.c.2 (ADR-0028) — chain은 `ArcSwap`으로 보관해 사용자가 Pipelines 토글을 바꿀 때
/// `swap_chain()`이 lock-free하게 교체. 진행 중 요청은 `load()` 시점의 chain 인스턴스를 보존해서
/// 안전하게 종료해요. 다음 요청부터 새 chain 적용.
#[derive(Clone)]
pub struct PipelineLayer {
    chain_swap: Arc<ArcSwap<PipelineChain>>,
    audit_sender: Option<mpsc::Sender<AuditEntry>>,
}

impl PipelineLayer {
    pub fn new(chain: PipelineChain) -> Self {
        Self {
            chain_swap: Arc::new(ArcSwap::new(Arc::new(chain))),
            audit_sender: None,
        }
    }

    /// 외부에서 만든 ArcSwap handle을 공유. Phase 8'.c.2에서 Tauri PipelinesState가 동일 swap을 보유.
    pub fn from_swap(chain_swap: Arc<ArcSwap<PipelineChain>>) -> Self {
        Self {
            chain_swap,
            audit_sender: None,
        }
    }

    /// 현재 chain snapshot — 디버깅 / 테스트 용.
    pub fn chain(&self) -> Arc<PipelineChain> {
        self.chain_swap.load_full()
    }

    /// chain_swap handle — Tauri 측에서 토글 시 swap 호출용.
    pub fn chain_swap(&self) -> Arc<ArcSwap<PipelineChain>> {
        Arc::clone(&self.chain_swap)
    }

    /// 새 chain으로 atomic swap. 다음 요청부터 새 chain 적용. 진행 중 요청은 옛 chain 안전 보존.
    ///
    /// Phase 8'.c.2 (ADR-0028) — `RwLock<Arc<T>>` 대비 lock-free라서 read 경합 0.
    pub fn swap_chain(&self, new_chain: PipelineChain) {
        self.chain_swap.store(Arc::new(new_chain));
    }

    /// 외부 receiver(예: Tauri PipelinesState)로 AuditEntry를 전달할 채널을 주입.
    ///
    /// 정책:
    /// - Sender capacity는 호출자가 결정 (gateway burst 흡수용 256 권장).
    /// - middleware는 `try_send`만 사용 — channel full 또는 drop 시 tracing::warn + drop.
    ///   audit drain은 절대 request 처리 흐름을 block 하지 않아요.
    pub fn with_audit_channel(mut self, sender: mpsc::Sender<AuditEntry>) -> Self {
        self.audit_sender = Some(sender);
        self
    }

    /// audit_sender가 주입되어 있는지 (테스트 / 디버깅용).
    pub fn has_audit_channel(&self) -> bool {
        self.audit_sender.is_some()
    }
}

impl<S> Layer<S> for PipelineLayer {
    type Service = PipelineMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        PipelineMiddleware {
            inner,
            chain_swap: Arc::clone(&self.chain_swap),
            audit_sender: self.audit_sender.clone(),
        }
    }
}

#[derive(Clone)]
pub struct PipelineMiddleware<S> {
    inner: S,
    chain_swap: Arc<ArcSwap<PipelineChain>>,
    audit_sender: Option<mpsc::Sender<AuditEntry>>,
}

/// `ctx.audit_log`을 drain → audit_sender에 best-effort try_send.
///
/// 정책:
/// - `try_send`만 사용 — channel full / closed 시 tracing::warn + drop.
/// - 절대 await 하지 않음 (audit이 request 처리 흐름을 block하면 안 돼요).
/// - audit_sender가 None이면 drain만 하고 silently 종료 (chain 내부 누적은 정리).
fn drain_audit(sender: Option<&mpsc::Sender<AuditEntry>>, ctx: &mut PipelineContext) {
    if ctx.audit_log.is_empty() {
        return;
    }
    let entries: Vec<AuditEntry> = ctx.audit_log.drain(..).collect();
    let Some(tx) = sender else {
        return;
    };
    let request_id = ctx.request_id.clone();
    for entry in entries {
        if let Err(e) = tx.try_send(entry) {
            match e {
                mpsc::error::TrySendError::Full(dropped) => {
                    tracing::warn!(
                        target: "lmmaster.pipelines",
                        request_id = %request_id,
                        pipeline_id = %dropped.pipeline_id,
                        action = %dropped.action,
                        "audit channel full — entry dropped"
                    );
                }
                mpsc::error::TrySendError::Closed(dropped) => {
                    tracing::warn!(
                        target: "lmmaster.pipelines",
                        request_id = %request_id,
                        pipeline_id = %dropped.pipeline_id,
                        action = %dropped.action,
                        "audit channel closed — entry dropped"
                    );
                }
            }
        }
    }
}

impl<S> Service<Request<Body>> for PipelineMiddleware<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        // Phase 8'.c.2 — chain은 ArcSwap에서 1회 load. 이 future가 진행 중인 동안 토글이
        // 발생해도 옛 chain 인스턴스를 안전하게 사용 (Arc strong_count로 보존).
        let chain = self.chain_swap.load_full();
        let audit_sender = self.audit_sender.clone();
        // Service::call 안에서 self.inner를 unwrap-clone해 future로 옮김 (tower standard).
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);

        Box::pin(async move {
            // 빈 chain이면 기존 흐름 그대로 — 빠른 path.
            if chain.is_empty() {
                return inner.call(req).await;
            }

            // 1) request body 추출 → JSON 파싱.
            let (parts, body) = req.into_parts();
            let collected = match to_bytes(body, MAX_BODY_BYTES).await {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(error = %e, "request body 읽기 실패 — pipeline skip");
                    return Ok(envelope_response(
                        StatusCode::PAYLOAD_TOO_LARGE,
                        "invalid_request_error",
                        "body_too_large",
                        "요청 본문이 너무 커요. 2 MiB 이하로 보내주세요.",
                    ));
                }
            };

            // 빈 본문이면 chain 적용 없이 그대로 통과.
            if collected.is_empty() {
                let req = Request::from_parts(parts, Body::from(collected));
                return inner.call(req).await;
            }

            // JSON 파싱 — non-JSON body면 chain skip하고 그대로 forward.
            let mut request_body: Value = match serde_json::from_slice(&collected) {
                Ok(v) => v,
                Err(_) => {
                    tracing::debug!("non-JSON body — pipeline skip");
                    let req = Request::from_parts(parts, Body::from(collected));
                    return inner.call(req).await;
                }
            };

            // PipelineContext — request_id는 SetRequestIdLayer가 헤더에 설정한 것을 채택. 없으면 uuid 생성.
            let request_id = parts
                .headers
                .get("x-request-id")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
                .unwrap_or_else(|| uuid_like(&collected));
            let mut ctx = PipelineContext::new(request_id);
            ctx.user_agent = parts
                .headers
                .get(header::USER_AGENT)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            ctx.model = request_body
                .get("model")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            // Phase 8'.c.3 (ADR-0029) — auth middleware가 request extension에 set한
            // `KeyPipelinesOverride`가 있으면 ctx에 복사. 없으면 None (전역 설정 따름).
            if let Some(over) = parts.extensions.get::<KeyPipelinesOverride>() {
                ctx.principal_key_pipelines = over.0.clone();
            }

            // chain은 ArcSwap에서 load한 *전역* chain. 키별 override가 있다면 sub-chain으로 좁혀요.
            // 본 effective_chain은 request + response + SSE 모두 동일하게 사용 — 키 정책 일관성.
            let effective_chain =
                Arc::new(chain.filter_by_ids(ctx.principal_key_pipelines.as_deref()));

            // 2) request 단계 적용.
            if effective_chain.is_empty() {
                // 본 키는 모든 Pipeline 비활성 — chain을 적용하지 않고 inner forward.
                // (request body는 그대로). SSE / non-SSE 모두 raw passthrough.
                tracing::debug!(
                    target: "lmmaster.pipelines",
                    request_id = %ctx.request_id,
                    "principal_key_pipelines empty — pipeline 적용 skip"
                );
                let req = Request::from_parts(parts, Body::from(collected));
                return inner.call(req).await;
            }

            if let Err(e) = effective_chain
                .apply_request(&mut ctx, &mut request_body)
                .await
            {
                // 차단된 경우라도 누적된 audit (이전 Pipeline의 passed/modified + 본 Pipeline의 blocked)을
                // 보존하기 위해 drain 후 envelope 반환.
                drain_audit(audit_sender.as_ref(), &mut ctx);
                return Ok(error_envelope_for(&e));
            }
            // request 단계 성공 — 누적 audit drain.
            drain_audit(audit_sender.as_ref(), &mut ctx);

            // 3) inner 호출.
            let new_body_bytes = match serde_json::to_vec(&request_body) {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(error = %e, "pipeline 후 body 직렬화 실패");
                    return Ok(envelope_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "internal_error",
                        "pipeline_serialize_error",
                        "필터 처리 후 본문 직렬화에 실패했어요.",
                    ));
                }
            };
            let new_body_len = new_body_bytes.len();
            let mut new_parts = parts;
            // content-length 갱신 — 이전 값은 무효.
            new_parts.headers.remove(header::CONTENT_LENGTH);
            if let Ok(v) = HeaderValue::from_str(&new_body_len.to_string()) {
                new_parts.headers.insert(header::CONTENT_LENGTH, v);
            }
            let new_req = Request::from_parts(new_parts, Body::from(new_bytes(new_body_bytes)));

            let response = inner.call(new_req).await?;

            // 4) response 단계 — Phase 8'.c.4 (ADR-0030).
            //    SSE면 chunk parser로 chunk 단위 변환을 적용. NoOp 통과 시 byte-identical 보장.
            let is_sse = response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.starts_with("text/event-stream"))
                .unwrap_or(false);
            if is_sse {
                return process_sse_response(
                    response,
                    Arc::clone(&effective_chain),
                    audit_sender.clone(),
                    &mut ctx,
                )
                .await;
            }

            // non-SSE — body를 파싱해 apply_response. 파싱 실패하면 그대로 pass-through.
            let (resp_parts, resp_body) = response.into_parts();
            let resp_bytes = match resp_body.collect().await {
                Ok(c) => c.to_bytes(),
                Err(e) => {
                    tracing::warn!(error = %e, "response body 읽기 실패 — pipeline skip");
                    return Ok(envelope_response(
                        StatusCode::BAD_GATEWAY,
                        "upstream_error",
                        "response_body_read_failed",
                        "응답 본문 읽기에 실패했어요.",
                    ));
                }
            };

            // empty body — 그대로 forward.
            if resp_bytes.is_empty() {
                let response = Response::from_parts(resp_parts, Body::from(resp_bytes));
                return Ok(response);
            }

            let mut response_body: Value = match serde_json::from_slice(&resp_bytes) {
                Ok(v) => v,
                Err(_) => {
                    tracing::debug!(
                        target: "lmmaster.pipelines",
                        request_id = %ctx.request_id,
                        "non-JSON response — pipeline response stage skipped"
                    );
                    let response = Response::from_parts(resp_parts, Body::from(resp_bytes));
                    return Ok(response);
                }
            };

            if let Err(e) = effective_chain
                .apply_response(&mut ctx, &mut response_body)
                .await
            {
                drain_audit(audit_sender.as_ref(), &mut ctx);
                return Ok(error_envelope_for(&e));
            }
            // response 단계 성공 — 누적 audit drain.
            drain_audit(audit_sender.as_ref(), &mut ctx);

            // 응답 본문 직렬화 + content-length 갱신.
            let new_resp_bytes = match serde_json::to_vec(&response_body) {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(error = %e, "pipeline 후 response body 직렬화 실패");
                    return Ok(envelope_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "internal_error",
                        "pipeline_serialize_error",
                        "응답 본문 직렬화에 실패했어요.",
                    ));
                }
            };
            let mut new_resp_parts = resp_parts;
            let len = new_resp_bytes.len();
            new_resp_parts.headers.remove(header::CONTENT_LENGTH);
            if let Ok(v) = HeaderValue::from_str(&len.to_string()) {
                new_resp_parts.headers.insert(header::CONTENT_LENGTH, v);
            }
            Ok(Response::from_parts(
                new_resp_parts,
                Body::from(new_bytes(new_resp_bytes)),
            ))
        })
    }
}

/// SSE 응답에 Pipeline의 `apply_response`를 chunk 단위로 적용.
///
/// 정책 (ADR-0030, Phase 8'.c.4):
/// - chunk parser로 frame 단위 추출 → JSON parse → `chain.apply_response` → reserialize → emit.
/// - `[DONE]` sentinel은 변형 없이 그대로 forward.
/// - JSON parse 실패한 chunk는 `tracing::warn!` 후 원본 그대로 forward (best-effort).
/// - chain이 chunk를 변경하지 않은 경우(equal) 원본 frame bytes를 그대로 emit → byte-identical 보장.
/// - 본 v1 구현은 stream을 *전체 buffer*해서 한 번에 emit. 즉 streaming의 latency 이점은
///   사라지지만 PII redact가 응답 전체에 적용된다는 가치를 우선. v1.x에서 진정한 streaming
///   transformation으로 진화 (chunk-by-chunk emit + Body::from_stream).
async fn process_sse_response<E>(
    response: Response<Body>,
    chain: Arc<PipelineChain>,
    audit_sender: Option<mpsc::Sender<AuditEntry>>,
    ctx: &mut PipelineContext,
) -> Result<Response<Body>, E> {
    let (resp_parts, resp_body) = response.into_parts();
    let resp_bytes = match resp_body.collect().await {
        Ok(c) => c.to_bytes(),
        Err(e) => {
            tracing::warn!(error = %e, "SSE response body 읽기 실패 — pipeline skip");
            return Ok(envelope_response(
                StatusCode::BAD_GATEWAY,
                "upstream_error",
                "response_body_read_failed",
                "응답 본문 읽기에 실패했어요.",
            ));
        }
    };
    if resp_bytes.is_empty() {
        return Ok(Response::from_parts(resp_parts, Body::from(resp_bytes)));
    }

    let mut parser = SseChunkParser::new();
    let mut chunks = parser.parse(Bytes::copy_from_slice(&resp_bytes));
    chunks.extend(parser.flush());

    // chain이 비어있거나 결과가 비어있으면 원본 그대로 forward.
    if chunks.is_empty() {
        return Ok(Response::from_parts(resp_parts, Body::from(resp_bytes)));
    }

    let mut out_buf = String::with_capacity(resp_bytes.len() + 64);
    let mut any_change = false;

    for chunk_result in chunks {
        let chunk = match chunk_result {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    target: "lmmaster.pipelines",
                    request_id = %ctx.request_id,
                    error = %e,
                    "SSE frame parse 실패 — chunk skip (data 누락)"
                );
                continue;
            }
        };

        // [DONE] sentinel은 변형 없이 그대로.
        if chunk.is_done() {
            out_buf.push_str(&chunk.serialize());
            continue;
        }

        // JSON parse 시도. 실패 시 원본 그대로 emit + warn.
        let mut value = match chunk.parse_json() {
            Some(Ok(v)) => v,
            Some(Err(e)) => {
                tracing::warn!(
                    target: "lmmaster.pipelines",
                    request_id = %ctx.request_id,
                    error = %e,
                    "SSE chunk JSON parse 실패 — 원본 forward"
                );
                out_buf.push_str(&chunk.serialize());
                continue;
            }
            None => {
                // is_done()이 true가 아닌데도 None — 정의상 발생 안 함.
                out_buf.push_str(&chunk.serialize());
                continue;
            }
        };

        // chain.apply_response 호출.
        let before = value.clone();
        if let Err(e) = chain.apply_response(ctx, &mut value).await {
            // chain 에러 시 audit drain + envelope 반환.
            drain_audit(audit_sender.as_ref(), ctx);
            return Ok(error_envelope_for(&e));
        }

        if value == before {
            // 변경 없음 — 원본 frame을 그대로 emit. byte-identical guarantee를 위해 reserialize 회피.
            out_buf.push_str(&chunk.serialize());
        } else {
            any_change = true;
            // 변경 있음 — 새 frame 직렬화.
            let new_data = match serde_json::to_string(&value) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(
                        target: "lmmaster.pipelines",
                        request_id = %ctx.request_id,
                        error = %e,
                        "SSE chunk reserialize 실패 — 원본 forward"
                    );
                    out_buf.push_str(&chunk.serialize());
                    continue;
                }
            };
            let new_chunk = SseChunk {
                event: chunk.event,
                id: chunk.id,
                data: new_data,
            };
            out_buf.push_str(&new_chunk.serialize());
        }
    }

    drain_audit(audit_sender.as_ref(), ctx);

    let final_bytes: Bytes = if any_change {
        Bytes::from(out_buf.into_bytes())
    } else {
        // 정책 (ADR-0030 byte-identical invariant):
        // 어떤 chunk도 변경되지 않았으면 원본 buffer를 그대로 emit. parser/serializer round-trip이
        // 미세한 byte 차이를 발생시킬 수 있어 NoOp 시에는 입력 그대로 보장.
        resp_bytes
    };

    let mut new_parts = resp_parts;
    new_parts.headers.remove(header::CONTENT_LENGTH);
    if let Ok(v) = HeaderValue::from_str(&final_bytes.len().to_string()) {
        new_parts.headers.insert(header::CONTENT_LENGTH, v);
    }
    Ok(Response::from_parts(new_parts, Body::from(final_bytes)))
}

/// 빠른 uuid-like — request_id 헤더가 없을 때 fallback. tracing용 식별자만 필요해서
/// uuid crate를 호출하지 않고 body의 SHA-style 짧은 hex로 대체.
fn uuid_like(bytes: &bytes::Bytes) -> String {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut h = DefaultHasher::new();
    bytes.hash(&mut h);
    format!("req-{:x}", h.finish())
}

fn new_bytes(v: Vec<u8>) -> bytes::Bytes {
    bytes::Bytes::from(v)
}

/// `PipelineError`를 OpenAI envelope 응답으로 변환.
fn error_envelope_for(err: &PipelineError) -> Response<Body> {
    let status = match err {
        PipelineError::Blocked { .. } => StatusCode::FORBIDDEN,
        PipelineError::BudgetExceeded { .. } => StatusCode::TOO_MANY_REQUESTS,
        PipelineError::Configuration(_) => StatusCode::INTERNAL_SERVER_ERROR,
        PipelineError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        PipelineError::Cancelled => StatusCode::SERVICE_UNAVAILABLE,
    };
    envelope_response(
        status,
        err.error_type(),
        err.error_code(),
        &format!("{err}"),
    )
}

/// OpenAI envelope `{"error":{"message","type","code"}}` 응답.
pub fn envelope_response(
    status: StatusCode,
    error_type: &str,
    code: &str,
    message: &str,
) -> Response<Body> {
    let body = json!({
        "error": {
            "message": message,
            "type": error_type,
            "code": code,
        }
    });
    let bytes = serde_json::to_vec(&body).unwrap_or_else(|_| b"{}".to_vec());
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CONTENT_LENGTH, bytes.len().to_string())
        .body(Body::from(bytes))
        .expect("envelope build")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::post;
    use axum::Router;
    use http_body_util::BodyExt;
    use pipelines::{PiiRedactPipeline, TokenQuotaPipeline};
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    /// echo handler — request body를 그대로 반환. pipeline의 request 변형이 inner까지 전달됐는지 검증용.
    async fn echo(body: axum::body::Bytes) -> Response<Body> {
        let bytes_for_response = body.to_vec();
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(bytes_for_response))
            .expect("echo response")
    }

    /// SSE handler — 응답을 byte-perfect로 검증할 수 있도록 고정 SSE chunk 반환.
    async fn sse_handler() -> Response<Body> {
        let body =
            "data: {\"choices\":[{\"delta\":{\"content\":\"010-1234-5678\"}}]}\n\ndata: [DONE]\n\n";
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/event-stream")
            .body(Body::from(body))
            .expect("sse response")
    }

    fn make_router(chain: PipelineChain) -> Router {
        Router::new()
            .route("/echo", post(echo))
            .route("/sse", post(sse_handler))
            .layer(PipelineLayer::new(chain))
    }

    #[tokio::test]
    async fn empty_chain_passes_body_unchanged() {
        let chain = PipelineChain::new();
        let router = make_router(chain);
        let req = Request::builder()
            .method("POST")
            .uri("/echo")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"messages":[{"role":"user","content":"안녕"}]}"#,
            ))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["messages"][0]["content"], "안녕");
    }

    #[tokio::test]
    async fn pii_redact_modifies_request_body_received_by_inner() {
        let chain = PipelineChain::new().add(Arc::new(PiiRedactPipeline::new()));
        let router = make_router(chain);
        let req = Request::builder()
            .method("POST")
            .uri("/echo")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"messages":[{"role":"user","content":"alice@example.com"}]}"#,
            ))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        // echo handler가 받아 그대로 돌려준 본문 — pipeline 적용 후 body여야 함.
        let v: Value = serde_json::from_slice(&body).unwrap();
        let content = v["messages"][0]["content"].as_str().unwrap();
        assert!(
            content.contains("[REDACTED-이메일]"),
            "echo body should reflect redacted body, got: {content}"
        );
    }

    /// response body를 변형하는 시나리오 — handler 응답이 PII를 담고 있으면 클라이언트는 redacted 본문을 받음.
    async fn assistant_pii_handler() -> Response<Body> {
        let body = json!({
            "id": "chatcmpl-1",
            "choices": [
                {"index": 0, "message": {"role": "assistant", "content": "연락처: 010-9999-8888"}}
            ]
        });
        let bytes = serde_json::to_vec(&body).unwrap();
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(bytes))
            .expect("assistant response")
    }

    #[tokio::test]
    async fn pii_redact_modifies_response_body_for_client() {
        let chain = PipelineChain::new().add(Arc::new(PiiRedactPipeline::new()));
        let router = Router::new()
            .route("/assistant", post(assistant_pii_handler))
            .layer(PipelineLayer::new(chain));

        let req = Request::builder()
            .method("POST")
            .uri("/assistant")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"model":"x","messages":[]}"#))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&body).unwrap();
        let content = v["choices"][0]["message"]["content"].as_str().unwrap();
        assert!(
            content.contains("[REDACTED-휴대폰]"),
            "response body PII must be redacted before client, got: {content}"
        );
    }

    /// Phase 8'.c.4 (ADR-0030) — SSE chunk transformation.
    /// PII redact pipeline은 streaming chunk 안의 휴대폰 번호도 redact 해야 해요.
    /// ADR-0025 §"감내한 트레이드오프" supersede — byte-perfect 보존 정책에서 chunk 단위 변환으로 전환.
    #[tokio::test]
    async fn sse_response_pii_redacted_per_chunk() {
        let chain = PipelineChain::new().add(Arc::new(PiiRedactPipeline::new()));
        let router = make_router(chain);
        let req = Request::builder()
            .method("POST")
            .uri("/sse")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"model":"x","messages":[],"stream":true}"#))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/event-stream"
        );
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let received = std::str::from_utf8(&body).unwrap();
        // 010-1234-5678이 redact 되어 사라져야 해요.
        assert!(
            !received.contains("010-1234-5678"),
            "SSE chunk PII redact가 적용되지 않았어요: {received}"
        );
        // [REDACTED-휴대폰] 마커 + [DONE] 둘 다 있어야 해요.
        assert!(
            received.contains("[REDACTED-휴대폰]"),
            "redact 마커가 보여야 해요: {received}"
        );
        assert!(
            received.contains("[DONE]"),
            "[DONE] sentinel 보존: {received}"
        );
    }

    /// NoOp pipeline (실제로 body를 변경하지 않는 chain)이 SSE 응답을 통과시킬 때 byte-identical 보장.
    /// ADR-0030 invariant — "변경하지 않으면 원본 그대로".
    #[tokio::test]
    async fn sse_response_with_observability_only_is_byte_identical() {
        use pipelines::ObservabilityPipeline;
        // ObservabilityPipeline은 body를 변경하지 않음 — tracing only.
        let chain = PipelineChain::new().add(Arc::new(ObservabilityPipeline::new()));
        let router = make_router(chain);
        let req = Request::builder()
            .method("POST")
            .uri("/sse")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"model":"x","messages":[],"stream":true}"#))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let received = std::str::from_utf8(&body).unwrap();
        // sse_handler가 emit한 원본 그대로.
        let original =
            "data: {\"choices\":[{\"delta\":{\"content\":\"010-1234-5678\"}}]}\n\ndata: [DONE]\n\n";
        assert_eq!(
            received, original,
            "NoOp pipeline은 byte-identical 보장 (ADR-0030)"
        );
    }

    #[tokio::test]
    async fn budget_exceeded_returns_korean_envelope_short_circuit() {
        // budget=1로 매우 작게 — token-quota Pipeline이 차단.
        // 단, layer는 ctx.token_budget을 외부에서 설정하지 않으므로 Pipeline에 budget을 주려면
        // chain 안에서 직접 ctx mutate가 필요. v1에서는 token-budget이 None이면 no-op이므로,
        // 본 테스트는 wrapper Pipeline이 budget을 강제로 세팅하는 형태로 설계.

        use async_trait::async_trait;
        use pipelines::{Pipeline, PipelineStage};

        struct ForceBudget;
        #[async_trait]
        impl Pipeline for ForceBudget {
            fn id(&self) -> &str {
                "force-budget"
            }
            fn stage(&self) -> PipelineStage {
                PipelineStage::Both
            }
            async fn apply_request(
                &self,
                ctx: &mut PipelineContext,
                _body: &mut Value,
            ) -> Result<(), PipelineError> {
                ctx.token_budget = Some(1);
                Ok(())
            }
            async fn apply_response(
                &self,
                _ctx: &mut PipelineContext,
                _body: &mut Value,
            ) -> Result<(), PipelineError> {
                Ok(())
            }
        }

        let chain = PipelineChain::new()
            .add(Arc::new(ForceBudget))
            .add(Arc::new(TokenQuotaPipeline::new()));
        let router = make_router(chain);

        let req = Request::builder()
            .method("POST")
            .uri("/echo")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"messages":[{"role":"user","content":"이 내용은 budget 1을 초과해요"}]}"#,
            ))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["error"]["type"], "budget_exceeded");
        let msg = v["error"]["message"].as_str().unwrap();
        assert!(msg.contains("토큰 한도"), "Korean error 메시지: {msg}");
        assert!(msg.contains("초과"));
    }

    /// pipeline이 호출되지 않아야 inner가 호출되었는지 검증 (call counter pipeline).
    #[tokio::test]
    async fn empty_chain_does_not_buffer_or_modify_body() {
        let chain = PipelineChain::new();
        let counter = Arc::new(Mutex::new(0usize));
        let counter_clone = counter.clone();

        let inner = move |body: axum::body::Bytes| {
            *counter_clone.lock().unwrap() += 1;
            async move {
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap()
            }
        };

        let router = Router::new()
            .route("/x", post(inner))
            .layer(PipelineLayer::new(chain));

        let req = Request::builder()
            .method("POST")
            .uri("/x")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"a":1}"#))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(*counter.lock().unwrap(), 1);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        // 빈 chain — body 변경 없음.
        assert_eq!(&body[..], br#"{"a":1}"#);
    }

    // ───────────────────────────────────────────────────────────────────
    // Phase 6'.d — audit channel wiring tests
    // ───────────────────────────────────────────────────────────────────

    fn make_router_with_audit(chain: PipelineChain, sender: mpsc::Sender<AuditEntry>) -> Router {
        Router::new()
            .route("/echo", post(echo))
            .route("/sse", post(sse_handler))
            .layer(PipelineLayer::new(chain).with_audit_channel(sender))
    }

    /// audit_sender 없이도 기존 동작이 유지되어야 해요 (regression guard).
    #[tokio::test]
    async fn pipeline_layer_without_audit_channel_works_unchanged() {
        let chain = PipelineChain::new().add(Arc::new(PiiRedactPipeline::new()));
        let layer = PipelineLayer::new(chain.clone());
        assert!(!layer.has_audit_channel());

        let router = make_router(chain);
        let req = Request::builder()
            .method("POST")
            .uri("/echo")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"messages":[{"role":"user","content":"alice@example.com"}]}"#,
            ))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&body).unwrap();
        let content = v["messages"][0]["content"].as_str().unwrap();
        assert!(content.contains("[REDACTED-이메일]"));
    }

    /// PII redact request → modified audit entry가 채널에 도착해야 해요.
    #[tokio::test]
    async fn audit_channel_receives_modified_entry_after_pii_redact() {
        let (tx, mut rx) = mpsc::channel::<AuditEntry>(256);
        let chain = PipelineChain::new().add(Arc::new(PiiRedactPipeline::new()));
        let router = make_router_with_audit(chain, tx);

        let req = Request::builder()
            .method("POST")
            .uri("/echo")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"messages":[{"role":"user","content":"alice@example.com"}]}"#,
            ))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // 채널에 최소 1개 entry — modified action.
        let entry = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .expect("audit channel timeout")
            .expect("audit channel closed");
        assert_eq!(entry.pipeline_id, "pii-redact");
        assert_eq!(entry.action, "modified");
        assert!(entry.details.is_some());
    }

    /// 채널이 가득 차면 처리 흐름은 block 안 되고 entry만 drop돼요.
    #[tokio::test]
    async fn audit_channel_full_drops_entry_without_blocking() {
        // capacity=1 — 첫 entry 후 즉시 가득 참.
        let (tx, mut rx) = mpsc::channel::<AuditEntry>(1);
        // 미리 채워두기.
        tx.send(AuditEntry::passed("preload")).await.unwrap();

        let chain = PipelineChain::new().add(Arc::new(PiiRedactPipeline::new()));
        let router = make_router_with_audit(chain, tx);

        let req = Request::builder()
            .method("POST")
            .uri("/echo")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"messages":[{"role":"user","content":"alice@example.com"}]}"#,
            ))
            .unwrap();
        // 채널 full이지만 응답은 timeout 없이 빠르게 와야 해요.
        let resp = tokio::time::timeout(std::time::Duration::from_secs(2), router.oneshot(req))
            .await
            .expect("response did not block")
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // 채널은 capacity=1 + preload로 차 있는 상태에서 매 drain 시 try_send 실패 → drop.
        // 핵심 invariant: middleware는 block 안 되고(timeout 통과) preload는 살아있어야 해요.
        let first = rx.try_recv().expect("preload should be in channel");
        assert_eq!(
            first.pipeline_id, "preload",
            "preload는 채널에 남아 있어야 해요. 채널이 full이라 새 entry는 drop되어야 함."
        );
        // 추가 entry가 있을 수도(buffered) 있고 없을 수도 있는데, 여기서 결정적인 것은
        // (1) middleware가 block되지 않은 점 + (2) preload가 head로 살아남은 점이에요.
        // 실제 capacity 1 채널에서는 try_send 실패 후 drop이지만,
        // 응답 단계에서 channel이 잠시 비는 시점이 생기면 1개가 들어갈 수 있어요 (race 허용).
    }

    /// receiver가 drop되면 채널 closed — try_send 실패하지만 middleware는 정상 동작.
    #[tokio::test]
    async fn audit_channel_closed_does_not_break_middleware() {
        let (tx, rx) = mpsc::channel::<AuditEntry>(8);
        // 즉시 drop — channel은 closed 상태.
        drop(rx);

        let chain = PipelineChain::new().add(Arc::new(PiiRedactPipeline::new()));
        let router = make_router_with_audit(chain, tx);

        let req = Request::builder()
            .method("POST")
            .uri("/echo")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"messages":[{"role":"user","content":"alice@example.com"}]}"#,
            ))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        // 응답 body는 정상적으로 redact 적용.
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert!(v["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("[REDACTED-이메일]"));
    }

    /// 다중 pipeline 체인 — request 단계 audit 순서 보존.
    #[tokio::test]
    async fn audit_channel_preserves_pipeline_order_in_chain() {
        use async_trait::async_trait;
        use pipelines::{Pipeline, PipelineStage};

        // chain의 forward 순으로 audit 발행되는 testing pipeline 3개.
        struct Marker(&'static str);
        #[async_trait]
        impl Pipeline for Marker {
            fn id(&self) -> &str {
                self.0
            }
            fn stage(&self) -> PipelineStage {
                PipelineStage::Request
            }
            async fn apply_request(
                &self,
                ctx: &mut PipelineContext,
                _body: &mut Value,
            ) -> Result<(), PipelineError> {
                ctx.record(AuditEntry::passed(self.0));
                Ok(())
            }
            async fn apply_response(
                &self,
                _ctx: &mut PipelineContext,
                _body: &mut Value,
            ) -> Result<(), PipelineError> {
                Ok(())
            }
        }

        let (tx, mut rx) = mpsc::channel::<AuditEntry>(256);
        let chain = PipelineChain::new()
            .add(Arc::new(Marker("alpha")))
            .add(Arc::new(Marker("beta")))
            .add(Arc::new(Marker("gamma")));
        let router = make_router_with_audit(chain, tx);

        let req = Request::builder()
            .method("POST")
            .uri("/echo")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"messages":[]}"#))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // forward 순서 = alpha, beta, gamma.
        let mut received = Vec::new();
        for _ in 0..3 {
            let e = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
                .await
                .unwrap()
                .unwrap();
            received.push(e.pipeline_id);
        }
        assert_eq!(received, vec!["alpha", "beta", "gamma"]);
    }

    /// 에러 short-circuit 시점에도 누적된 audit (이전 Pipeline의 passed + 본 Pipeline의 blocked) 보존.
    #[tokio::test]
    async fn audit_channel_drains_on_pipeline_error_short_circuit() {
        use async_trait::async_trait;
        use pipelines::{Pipeline, PipelineStage};

        struct PassPipeline;
        #[async_trait]
        impl Pipeline for PassPipeline {
            fn id(&self) -> &str {
                "pass-one"
            }
            fn stage(&self) -> PipelineStage {
                PipelineStage::Request
            }
            async fn apply_request(
                &self,
                ctx: &mut PipelineContext,
                _body: &mut Value,
            ) -> Result<(), PipelineError> {
                ctx.record(AuditEntry::passed("pass-one"));
                Ok(())
            }
            async fn apply_response(
                &self,
                _ctx: &mut PipelineContext,
                _body: &mut Value,
            ) -> Result<(), PipelineError> {
                Ok(())
            }
        }

        struct BlockPipeline;
        #[async_trait]
        impl Pipeline for BlockPipeline {
            fn id(&self) -> &str {
                "block-one"
            }
            fn stage(&self) -> PipelineStage {
                PipelineStage::Request
            }
            async fn apply_request(
                &self,
                ctx: &mut PipelineContext,
                _body: &mut Value,
            ) -> Result<(), PipelineError> {
                ctx.record(AuditEntry::blocked("block-one", "policy"));
                Err(PipelineError::Blocked {
                    pipeline: "block-one".into(),
                    reason: "policy".into(),
                })
            }
            async fn apply_response(
                &self,
                _ctx: &mut PipelineContext,
                _body: &mut Value,
            ) -> Result<(), PipelineError> {
                Ok(())
            }
        }

        let (tx, mut rx) = mpsc::channel::<AuditEntry>(256);
        let chain = PipelineChain::new()
            .add(Arc::new(PassPipeline))
            .add(Arc::new(BlockPipeline));
        let router = make_router_with_audit(chain, tx);

        let req = Request::builder()
            .method("POST")
            .uri("/echo")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"messages":[]}"#))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);

        // pass-one (passed) + block-one (blocked) 두 entry 모두 채널에 도착.
        let first = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        let second = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first.pipeline_id, "pass-one");
        assert_eq!(first.action, "passed");
        assert_eq!(second.pipeline_id, "block-one");
        assert_eq!(second.action, "blocked");
    }

    /// drain_audit 헬퍼 — sender None이면 audit_log 비워주고 panic 없이 끝나야 해요.
    #[tokio::test]
    async fn drain_audit_with_no_sender_clears_log() {
        let mut ctx = PipelineContext::new("r1");
        ctx.record(AuditEntry::passed("a"));
        ctx.record(AuditEntry::modified("b", "x"));
        assert_eq!(ctx.audit_log.len(), 2);
        drain_audit(None, &mut ctx);
        assert!(
            ctx.audit_log.is_empty(),
            "sender None이어도 audit_log은 drain되어야 해요"
        );
    }

    // ───────────────────────────────────────────────────────────────────
    // Phase 8'.c.2 — Pipelines hot-reload (ArcSwap)
    // ───────────────────────────────────────────────────────────────────

    /// `swap_chain()` 후 다음 요청에서 새 chain 인스턴스가 적용돼요.
    #[tokio::test]
    async fn swap_chain_applies_new_chain_to_next_request() {
        // 1) PII redact만 있는 chain으로 시작.
        let chain1 = PipelineChain::new().add(Arc::new(PiiRedactPipeline::new()));
        let layer = PipelineLayer::new(chain1);

        // 첫 요청 — alice@example.com이 redact 되어야.
        let router = Router::new()
            .route("/echo", post(echo))
            .layer(layer.clone());
        let req = Request::builder()
            .method("POST")
            .uri("/echo")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"messages":[{"role":"user","content":"alice@example.com"}]}"#,
            ))
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert!(v["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("[REDACTED-이메일]"));

        // 2) chain을 빈 것으로 swap. 다음 요청은 redact 안 됨.
        let new_chain = PipelineChain::new();
        layer.swap_chain(new_chain);
        let router2 = Router::new()
            .route("/echo", post(echo))
            .layer(layer.clone());
        let req2 = Request::builder()
            .method("POST")
            .uri("/echo")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"messages":[{"role":"user","content":"alice@example.com"}]}"#,
            ))
            .unwrap();
        let resp2 = router2.oneshot(req2).await.unwrap();
        let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
        let v2: Value = serde_json::from_slice(&body2).unwrap();
        // swap 후엔 redact 안 됨.
        assert_eq!(v2["messages"][0]["content"], "alice@example.com");
    }

    /// `chain_swap` 핸들이 외부에서 공유 가능 — 외부 store로도 swap 작동.
    #[tokio::test]
    async fn chain_swap_handle_shares_state_externally() {
        let chain1 = PipelineChain::new().add(Arc::new(PiiRedactPipeline::new()));
        let layer = PipelineLayer::new(chain1);
        let swap_handle = layer.chain_swap();

        // 외부에서 직접 store.
        swap_handle.store(Arc::new(PipelineChain::new()));

        // layer가 본 swap에서 chain을 load → 빈 chain.
        let loaded = layer.chain();
        assert!(loaded.is_empty(), "external store가 layer에도 보여야");
    }

    /// 여러 swap이 race 없이 모두 적용돼요. 마지막 swap이 결정적으로 보임.
    #[tokio::test]
    async fn concurrent_swaps_do_not_panic_or_corrupt() {
        use async_trait::async_trait;
        use pipelines::{Pipeline, PipelineStage};

        struct MarkPipeline(&'static str);
        #[async_trait]
        impl Pipeline for MarkPipeline {
            fn id(&self) -> &str {
                self.0
            }
            fn stage(&self) -> PipelineStage {
                PipelineStage::Request
            }
            async fn apply_request(
                &self,
                _c: &mut PipelineContext,
                _b: &mut Value,
            ) -> Result<(), PipelineError> {
                Ok(())
            }
            async fn apply_response(
                &self,
                _c: &mut PipelineContext,
                _b: &mut Value,
            ) -> Result<(), PipelineError> {
                Ok(())
            }
        }

        let layer = PipelineLayer::new(PipelineChain::new());
        let mut handles = Vec::new();
        for i in 0..50 {
            let l = layer.clone();
            handles.push(tokio::spawn(async move {
                let mark: &'static str = match i % 3 {
                    0 => "alpha",
                    1 => "beta",
                    _ => "gamma",
                };
                let chain = PipelineChain::new().add(Arc::new(MarkPipeline(mark)));
                l.swap_chain(chain);
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        // 마지막 chain의 첫 Pipeline ID가 alpha/beta/gamma 중 하나여야.
        let final_chain = layer.chain();
        assert_eq!(final_chain.len(), 1);
        let id = final_chain.pipelines()[0].id();
        assert!(matches!(id, "alpha" | "beta" | "gamma"));
    }

    /// 진행 중 요청 중간에 swap이 발생해도 그 요청은 옛 chain을 안전하게 끝내요.
    /// (Arc strong_count로 옛 chain 인스턴스 보존)
    #[tokio::test]
    async fn in_flight_request_keeps_old_chain_after_swap() {
        let chain1 = PipelineChain::new().add(Arc::new(PiiRedactPipeline::new()));
        let layer = PipelineLayer::new(chain1);

        // call 안에서 chain_swap을 비우고도 그 future가 잡고 있는 chain은 그대로.
        let swap_handle = layer.chain_swap();
        let snapshot = swap_handle.load_full();
        assert_eq!(snapshot.len(), 1);

        // 외부 swap.
        layer.swap_chain(PipelineChain::new());
        // snapshot은 여전히 1개 — Arc로 잡혀 있음.
        assert_eq!(snapshot.len(), 1);
        // 새 load는 빈 chain.
        let new_load = swap_handle.load_full();
        assert!(new_load.is_empty());
    }

    // ───────────────────────────────────────────────────────────────────
    // Phase 8'.c.3 — Per-key Pipelines override (KeyPipelinesOverride)
    // ───────────────────────────────────────────────────────────────────

    /// `KeyPipelinesOverride(None)` extension이 있으면 전역 설정을 그대로 따라요.
    #[tokio::test]
    async fn key_override_none_uses_global_chain() {
        use pipelines::PromptSanitizePipeline;
        let chain = PipelineChain::new()
            .add(Arc::new(PiiRedactPipeline::new()))
            .add(Arc::new(PromptSanitizePipeline::new()));
        let router = make_router(chain);

        let mut req = Request::builder()
            .method("POST")
            .uri("/echo")
            .header("content-type", "application/json")
            .body(Body::from(
                "{\"messages\":[{\"role\":\"user\",\"content\":\"alice@example.com x\u{200B}y\"}]}",
            ))
            .unwrap();
        // None override = 전역 그대로.
        req.extensions_mut().insert(KeyPipelinesOverride(None));
        let resp = router.oneshot(req).await.unwrap();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&body).unwrap();
        let content = v["messages"][0]["content"].as_str().unwrap();
        assert!(content.contains("[REDACTED-이메일]"));
        // prompt-sanitize도 적용 — zero-width 제거.
        assert!(!content.contains('\u{200B}'));
    }

    /// `KeyPipelinesOverride(Some(["pii-redact"]))` — PII redact만 적용, prompt-sanitize는 skip.
    #[tokio::test]
    async fn key_override_subset_only_applies_listed_pipelines() {
        use pipelines::PromptSanitizePipeline;
        let chain = PipelineChain::new()
            .add(Arc::new(PiiRedactPipeline::new()))
            .add(Arc::new(PromptSanitizePipeline::new()));
        let router = make_router(chain);

        let mut req = Request::builder()
            .method("POST")
            .uri("/echo")
            .header("content-type", "application/json")
            .body(Body::from(
                "{\"messages\":[{\"role\":\"user\",\"content\":\"alice@example.com x\u{200B}y\"}]}",
            ))
            .unwrap();
        req.extensions_mut()
            .insert(KeyPipelinesOverride(Some(vec!["pii-redact".to_string()])));
        let resp = router.oneshot(req).await.unwrap();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&body).unwrap();
        let content = v["messages"][0]["content"].as_str().unwrap();
        // PII는 redact 됨.
        assert!(content.contains("[REDACTED-이메일]"));
        // prompt-sanitize는 skip — zero-width 살아있음.
        assert!(
            content.contains('\u{200B}'),
            "prompt-sanitize는 화이트리스트에 없으므로 skip — ZWSP 유지: {content:?}"
        );
    }

    /// `KeyPipelinesOverride(Some(빈 Vec))` — 모든 Pipeline 비활성. raw passthrough.
    #[tokio::test]
    async fn key_override_empty_disables_all_pipelines() {
        let chain = PipelineChain::new().add(Arc::new(PiiRedactPipeline::new()));
        let router = make_router(chain);

        let mut req = Request::builder()
            .method("POST")
            .uri("/echo")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"messages":[{"role":"user","content":"alice@example.com"}]}"#,
            ))
            .unwrap();
        req.extensions_mut()
            .insert(KeyPipelinesOverride(Some(vec![])));
        let resp = router.oneshot(req).await.unwrap();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let v: Value = serde_json::from_slice(&body).unwrap();
        // 모든 Pipeline 비활성 → 원본 그대로.
        assert_eq!(v["messages"][0]["content"], "alice@example.com");
    }
}
