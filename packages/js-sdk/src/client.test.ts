// client.ts unit tests — baseUrl 정규화 + authHeaders + ensureOk.

import { describe, expect, it } from "vitest";

import { LMmasterClient } from "./client";
import { LMmasterApiError } from "./types";

describe("LMmasterClient", () => {
  it("trailing slash 제거", () => {
    const c = new LMmasterClient({ baseUrl: "http://x.com/v1/" });
    expect(c.baseUrl).toBe("http://x.com/v1");
  });

  it("기본 baseUrl은 127.0.0.1:43117/v1", () => {
    const c = new LMmasterClient();
    expect(c.baseUrl).toBe("http://127.0.0.1:43117/v1");
  });

  it("apiKey 있으면 Authorization Bearer 헤더", () => {
    const c = new LMmasterClient({ apiKey: "lm-abc" });
    expect(c.authHeaders()).toEqual({ Authorization: "Bearer lm-abc" });
  });

  it("apiKey 없으면 빈 객체", () => {
    const c = new LMmasterClient();
    expect(c.authHeaders()).toEqual({});
  });

  it("ensureOk — 200은 통과", async () => {
    const c = new LMmasterClient();
    await c.ensureOk(new Response("ok", { status: 200 }));
  });

  it("ensureOk — 4xx envelope은 LMmasterApiError", async () => {
    const c = new LMmasterClient();
    const res = new Response(
      JSON.stringify({
        error: { message: "키 만료", type: "invalid_request_error", code: "key_expired" },
      }),
      { status: 401, headers: { "content-type": "application/json" } },
    );
    try {
      await c.ensureOk(res);
      expect.fail("should throw");
    } catch (e) {
      expect(e).toBeInstanceOf(LMmasterApiError);
      const err = e as LMmasterApiError;
      expect(err.status).toBe(401);
      expect(err.code).toBe("key_expired");
      expect(err.message).toBe("키 만료");
    }
  });
});
