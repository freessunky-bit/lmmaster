# ADR-0030: SSE chunk transformation policy

- Status: Accepted (supersedes ADR-0025 §"감내한 트레이드오프" — SSE byte-perfect 정책 일부)
- Date: 2026-04-28
- Related: ADR-0006 (Gateway 프로토콜 — SSE byte-perfect), ADR-0022 (Gateway routing), ADR-0025 (Pipelines)
- Phase: 8'.c.4

## Context

ADR-0025 §"감내한 트레이드오프"에서 SSE streaming chunk transformation은 **v1.x로 미루고 byte-perfect pass-through**를 약속했어요. 이유:

- ADR-0006이 SSE byte-perfect를 OpenAI 호환성의 핵심으로 명시.
- chunk-by-chunk JSON parse + reserialize 비용 우려.
- byte-perfect 깨지면 "OpenAI SDK가 정확히 똑같이 동작" 약속 약화.

하지만 사용자 시나리오에서 **streaming 응답이 PII redact를 통과하지 못하는 문제**가 명확해졌어요:

- chat/completions stream=true가 압도적인 사용 패턴 (사용자가 ChatGPT-like UX 기대).
- non-streaming만 PII redact가 적용되면 "보안 정책 우회로"가 됨 (악성 사용자가 stream=true로 재호출).
- "PII redact는 보안 약속"이고 streaming만 빠지면 약속 자체가 무력화.

따라서 SSE chunk transformation을 v1에 도입하되 *byte-identical 보장*은 **NoOp pipeline 통과 시에만** 유지.

## Decision

### Pipeline 적용 시점

`PipelineMiddleware::call`이 응답 content-type을 검사 → `text/event-stream`이면 `process_sse_response` 진입.

```text
1. response body 전체 buffer (v1 — chunk-by-chunk emit은 v1.x).
2. SseChunkParser로 frame 단위 추출 (\n\n separator).
3. 각 frame:
   - [DONE] sentinel → 변형 없이 emit.
   - chunk JSON parse → chain.apply_response → 변경된 frame 직렬화 → emit.
   - parse 실패 → 원본 frame 그대로 emit + tracing::warn.
4. 어떤 frame도 변경되지 않았으면 *원본 buffer 통째*로 emit (byte-identical guarantee).
```

### byte-identical invariant (NoOp pipeline 통과 시)

`any_change` 플래그로 chunk별 변경 여부 추적. 모두 false이면 `final_bytes = resp_bytes` (원본). 즉 *parser/serializer round-trip이 미세한 byte 차이를 내더라도* NoOp 시에는 입력 그대로 emit.

이 invariant는 골든 테스트로 회귀 가드 (`sse_response_with_observability_only_is_byte_identical`).

### Pipeline shape 확장

`PiiRedactPipeline::redact_choices`에 `choices[].delta.content` 처리 추가 (streaming chunk shape). 기존 `choices[].message.content` (non-streaming)는 그대로.

### chunk JSON parse 실패 정책

best-effort: 단일 frame parse 실패는 **원본 그대로 emit** + `tracing::warn`. 전체 stream을 끊지 않음. 사용자 시나리오: upstream이 가끔 keep-alive comment(`: keepalive`)나 metadata frame을 보낼 수 있어 graceful degradation 우선.

## Consequences

### 긍정

- PII redact가 streaming 응답에도 적용 → 보안 약속 일관성 회복.
- per-key Pipelines override(ADR-0029)가 streaming에도 적용 (effective_chain 공유).
- NoOp pipeline 통과 시 byte-identical 보장 → 사용자가 PII redact를 끄면 ADR-0006 약속 그대로.

### 부정

- v1 구현은 stream을 *전체 buffer*해서 한 번에 emit → streaming의 latency 이점이 사라짐 (TTFT가 종료 시점). 사용자에게는 "모델이 응답하는 동안 한 번에 노출" UX. v1.x에서 진정한 chunk-by-chunk emit으로 진화.
- chunk JSON parse 비용 추가 (frame당 micros). non-streaming 응답 1회 비용 ≈ streaming N frames 비용 합 — 상한 ~수십 ms.
- `text/event-stream` 응답 메모리 사용량 증가 (전체 buffer). 2 MiB body limit 그대로 적용.

### supersede 명시

ADR-0025 §"감내한 트레이드오프":
- "SSE 응답에는 Pipelines 미적용" → **본 ADR에서 supersede**. v1.x로 미뤘던 streaming transformation을 v1.0에 도입.
- "byte-perfect SSE relay 보존" → **NoOp pipeline 통과 시에만** 유지로 약화. 변형이 발생하면 reserialize.

## Alternatives considered + rejected

### 1. byte-perfect 영구 보존 (v1.x로 영구 미루기)

- 사용자 시나리오 "stream=true로 PII 우회"가 실제 위협. 보안 정책의 *명시적 우회로* 만드는 셈.
- 사용자가 streaming만 쓰면 PII redact가 의미 0 — ADR-0025 약속이 사실상 무력.

### 2. streaming chunk 차단 (SSE는 모든 PII Pipeline 자동 disable)

- 사용자 적대적 — 가장 흔한 패턴(stream=true)에서 보호 받지 못함을 명시.
- "이 PC의 Pipelines는 streaming에 안 작동해요" 라벨이 UX 죽음.

### 3. 외부 SSE parser crate (`eventsource-stream` 등)

- 의존성 +1. 본 코드 ~250 LOC로 충분.
- crate가 transformation 인터페이스를 제공 안 함 → 어차피 reserialize 코드 필요.

### 4. Pipeline trait에 `apply_chunk(&mut Value)` 별도 메서드

- 매 Pipeline 구현체가 streaming 형식을 알아야 함 → coupling 증가.
- `apply_response` 재사용이 효율적 — chunk 단위 JSON value도 결국 OpenAI shape의 sub-tree.
- streaming-aware Pipeline은 `delta.content` 처리만 추가하면 됨 (PiiRedact에서 적용).

### 5. chunk-by-chunk emit (Body::from_stream)

- 진정한 streaming UX. v1.x 후속 작업으로 분리.
- v1은 *PII가 streaming 응답에 적용된다*는 정책 합의가 우선. 성능은 v1.x.

## Open follow-ups (v1.x)

- chunk-by-chunk emit (현재 v1은 전체 buffer).
- streaming SSE chunk별 audit log 발행 (현재는 ctx accumulate → 응답 끝에서 drain).
- chunk parse 실패 횟수 metric (telemetry / observability).
- 매우 긴 streaming 응답에 대한 점진적 buffer 압축 (>2MiB 시).

## References

- `crates/core-gateway/src/sse_chunk.rs::SseChunkParser` — line-aware parser.
- `crates/core-gateway/src/pipeline_layer.rs::process_sse_response` — chunk transformation 흐름.
- `crates/pipelines/src/pii_redact.rs::redact_choices` — delta + message dual-shape.
- 골든 테스트: `sse_response_with_observability_only_is_byte_identical`.
- OpenAI streaming spec: `https://platform.openai.com/docs/api-reference/chat/streaming`.
