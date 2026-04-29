// chat.ts — OpenAI 호환 chat completions 단발 + streaming.
//
// 정책 (ADR-0022 §1, §2):
// - 단발: POST /v1/chat/completions { stream: false } → ChatCompletion JSON.
// - streaming: { stream: true } → SSE iterator(ChatCompletionChunk).
// - 거부 envelope은 LMmasterApiError로 throw.

import type { LMmasterClient } from "./client";
import type {
  ChatCompletion,
  ChatCompletionChunk,
  ChatRequest,
} from "./types";

/** 단발 chat completion. */
export async function chatCompletions(
  client: LMmasterClient,
  req: ChatRequest,
): Promise<ChatCompletion> {
  const res = await client.fetchImpl(`${client.baseUrl}/chat/completions`, {
    method: "POST",
    headers: { "Content-Type": "application/json", ...client.authHeaders() },
    body: JSON.stringify({ ...req, stream: false }),
  });
  await client.ensureOk(res);
  return (await res.json()) as ChatCompletion;
}

/**
 * Streaming chat completion — OpenAI SSE 포맷.
 * 각 chunk는 이미 JSON.parse 된 `ChatCompletionChunk`.
 *
 * 사용 예:
 * ```ts
 * for await (const chunk of streamChat(client, { model, messages })) {
 *   process.stdout.write(chunk.choices[0]?.delta?.content ?? "");
 * }
 * ```
 */
export async function* streamChat(
  client: LMmasterClient,
  req: ChatRequest,
): AsyncGenerator<ChatCompletionChunk, void, void> {
  const res = await client.fetchImpl(`${client.baseUrl}/chat/completions`, {
    method: "POST",
    headers: { "Content-Type": "application/json", ...client.authHeaders() },
    body: JSON.stringify({ ...req, stream: true }),
  });
  await client.ensureOk(res);
  if (!res.body) {
    throw new Error("gateway response has no body");
  }

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  for (;;) {
    const { value, done } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });

    // SSE 메시지는 "\n\n"으로 구분.
    let sep: number;
    while ((sep = buffer.indexOf("\n\n")) >= 0) {
      const event = buffer.slice(0, sep);
      buffer = buffer.slice(sep + 2);
      // 한 event 안에 여러 line 있을 수 있음 (data:가 여러 줄에 걸치는 경우는 OpenAI에서 거의 없음 — 단순 처리).
      for (const line of event.split("\n")) {
        const trimmed = line.trim();
        if (!trimmed.startsWith("data:")) continue;
        const payload = trimmed.slice("data:".length).trim();
        if (payload === "[DONE]") return;
        if (payload.length === 0) continue;
        try {
          const chunk = JSON.parse(payload) as ChatCompletionChunk;
          yield chunk;
        } catch {
          // malformed chunk — skip (gateway가 byte-perfect relay하므로 거의 안 나옴).
        }
      }
    }
  }
}

/** 사용자 향 헬퍼 — streaming chunk → 누적 텍스트 string. */
export async function streamChatText(
  client: LMmasterClient,
  req: ChatRequest,
  onDelta?: (delta: string) => void,
): Promise<string> {
  let full = "";
  for await (const chunk of streamChat(client, req)) {
    const piece = chunk.choices[0]?.delta?.content ?? "";
    if (piece) {
      full += piece;
      onDelta?.(piece);
    }
  }
  return full;
}
