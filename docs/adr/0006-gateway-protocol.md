# ADR-0006: Gateway 프로토콜로 OpenAI-compatible REST + SSE 우선

- Status: Accepted
- Date: 2026-04-26

## Context
기존 웹앱이 provider abstraction에 1개만 추가하는 변경으로 끝나려면, 가장 보편적인 인터페이스를 노출해야 한다. 다수의 SDK(OpenAI, vercel/ai, langchain 등)가 OpenAI 스펙과 호환된다.

## Decision
Gateway의 1차 프로토콜은 **OpenAI-compatible REST**:
- `POST /v1/chat/completions` (stream=true 시 SSE)
- `POST /v1/embeddings` (스키마만 v1, 구현은 후속)
- `GET /v1/models`
- `GET /health`, `GET /capabilities`
- 인증: `Authorization: Bearer <local_api_key>`

향후 추가:
- **Anthropic-compatible shim** — `/v1/messages` 등을 OpenAI 형식으로 내부 변환.
- **streamChat** SSE는 `data: {delta...}\n\n`, 종료 `data: [DONE]`.

내부 management endpoint(별도 prefix `/_admin/...`)는 GUI 전용 IPC만 호출하거나 admin scope 키만 호출 가능.

## Consequences
- 기존 웹앱이 OpenAI SDK 그대로 baseURL만 바꿔 사용 가능.
- 일부 OpenAI 기능(파일/이미지 업로드 등)은 v1 미지원 — capability discovery로 명시.
- Anthropic shim은 v2.

## Alternatives considered
- **자체 protocol**: 거부 — adoption 비용 큼.
- **gRPC**: 거부 — 브라우저 호환성 문제 + SDK 부담.

## References
- ADR-0001 (companion)
