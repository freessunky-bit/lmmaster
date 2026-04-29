// chat.ts unit tests — 단발 + streaming + envelope 에러.
//
// 정책 (CLAUDE.md §4.4):
// - SSE 파서 invariant: chunk 순서 보존, [DONE] 정확 인식, malformed skip.
// - LMmasterApiError envelope 보존.

import { describe, expect, it } from "vitest";

import { chatCompletions, streamChat, streamChatText } from "./chat";
import { LMmasterClient } from "./client";
import { LMmasterApiError } from "./types";

function fakeFetch(impl: (req: Request) => Response | Promise<Response>): typeof fetch {
  return ((input: RequestInfo | URL, init?: RequestInit) => {
    const req = new Request(typeof input === "string" || input instanceof URL ? input.toString() : (input as Request).url, init);
    return Promise.resolve(impl(req));
  }) as typeof fetch;
}

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function sseResponse(body: string): Response {
  return new Response(body, {
    status: 200,
    headers: { "content-type": "text/event-stream" },
  });
}

describe("chatCompletions (단발)", () => {
  it("OpenAI 호환 응답 파싱", async () => {
    const c = new LMmasterClient({
      baseUrl: "http://gw/v1",
      fetchImpl: fakeFetch(() =>
        jsonResponse({
          id: "x",
          object: "chat.completion",
          created: 0,
          model: "exaone",
          choices: [
            {
              index: 0,
              message: { role: "assistant", content: "안녕" },
              finish_reason: "stop",
            },
          ],
        }),
      ),
    });
    const r = await chatCompletions(c, {
      model: "exaone",
      messages: [{ role: "user", content: "hi" }],
    });
    expect(r.choices[0]?.message.content).toBe("안녕");
  });

  it("Authorization 헤더 자동 부착", async () => {
    let captured: string | null = null;
    const c = new LMmasterClient({
      baseUrl: "http://gw/v1",
      apiKey: "lm-key",
      fetchImpl: fakeFetch((req) => {
        captured = req.headers.get("authorization");
        return jsonResponse({
          id: "x",
          object: "chat.completion",
          created: 0,
          model: "x",
          choices: [],
        });
      }),
    });
    await chatCompletions(c, { model: "x", messages: [] });
    expect(captured).toBe("Bearer lm-key");
  });

  it("4xx envelope을 LMmasterApiError로 변환", async () => {
    const c = new LMmasterClient({
      baseUrl: "http://gw/v1",
      fetchImpl: fakeFetch(() =>
        jsonResponse(
          {
            error: {
              message: "모델을 찾을 수 없어요",
              type: "not_found_error",
              code: "model_not_found",
            },
          },
          404,
        ),
      ),
    });
    try {
      await chatCompletions(c, { model: "x", messages: [] });
      expect.fail("should throw");
    } catch (e) {
      expect(e).toBeInstanceOf(LMmasterApiError);
      const err = e as LMmasterApiError;
      expect(err.status).toBe(404);
      expect(err.code).toBe("model_not_found");
      expect(err.message).toContain("찾을 수 없어요");
    }
  });

  it("non-JSON 4xx도 generic envelope으로 throw", async () => {
    const c = new LMmasterClient({
      baseUrl: "http://gw/v1",
      fetchImpl: fakeFetch(() =>
        new Response("plain text fault", { status: 502 }),
      ),
    });
    try {
      await chatCompletions(c, { model: "x", messages: [] });
      expect.fail("should throw");
    } catch (e) {
      expect(e).toBeInstanceOf(LMmasterApiError);
      const err = e as LMmasterApiError;
      expect(err.status).toBe(502);
      expect(err.code).toBe("http_error");
    }
  });
});

describe("streamChat (SSE)", () => {
  function buildSseBody(): string {
    const ev = (obj: unknown) =>
      "data: " + JSON.stringify(obj) + "\n\n";
    return (
      ev({
        id: "x",
        object: "chat.completion.chunk",
        created: 0,
        model: "x",
        choices: [{ index: 0, delta: { content: "안" }, finish_reason: null }],
      }) +
      ev({
        id: "x",
        object: "chat.completion.chunk",
        created: 0,
        model: "x",
        choices: [{ index: 0, delta: { content: "녕" }, finish_reason: null }],
      }) +
      "data: [DONE]\n\n"
    );
  }

  it("chunk 순서 보존 + [DONE] 인식", async () => {
    const c = new LMmasterClient({
      baseUrl: "http://gw/v1",
      fetchImpl: fakeFetch(() => sseResponse(buildSseBody())),
    });
    const chunks: string[] = [];
    for await (const chunk of streamChat(c, { model: "x", messages: [] })) {
      const piece = chunk.choices[0]?.delta?.content ?? "";
      chunks.push(piece);
    }
    expect(chunks).toEqual(["안", "녕"]);
  });

  it("malformed payload는 skip", async () => {
    const body =
      "data: {invalid json}\n\n" +
      'data: {"id":"x","object":"chat.completion.chunk","created":0,"model":"x","choices":[{"index":0,"delta":{"content":"안"},"finish_reason":null}]}\n\n' +
      "data: [DONE]\n\n";
    const c = new LMmasterClient({
      baseUrl: "http://gw/v1",
      fetchImpl: fakeFetch(() => sseResponse(body)),
    });
    const chunks: string[] = [];
    for await (const chunk of streamChat(c, { model: "x", messages: [] })) {
      chunks.push(chunk.choices[0]?.delta?.content ?? "");
    }
    expect(chunks).toEqual(["안"]);
  });

  it("streamChatText — 누적 + onDelta 콜백", async () => {
    const c = new LMmasterClient({
      baseUrl: "http://gw/v1",
      fetchImpl: fakeFetch(() => sseResponse(buildSseBody())),
    });
    const deltas: string[] = [];
    const full = await streamChatText(
      c,
      { model: "x", messages: [] },
      (d) => deltas.push(d),
    );
    expect(full).toBe("안녕");
    expect(deltas).toEqual(["안", "녕"]);
  });

  it("4xx envelope → LMmasterApiError throw", async () => {
    const c = new LMmasterClient({
      baseUrl: "http://gw/v1",
      fetchImpl: fakeFetch(() =>
        jsonResponse(
          { error: { message: "x", type: "y", code: "z" } },
          401,
        ),
      ),
    });
    try {
      // eslint-disable-next-line no-empty
      for await (const _ of streamChat(c, { model: "x", messages: [] })) {
      }
      expect.fail("should throw");
    } catch (e) {
      expect(e).toBeInstanceOf(LMmasterApiError);
      expect((e as LMmasterApiError).status).toBe(401);
    }
  });
});
