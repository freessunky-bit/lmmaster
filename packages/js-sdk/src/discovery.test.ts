// discovery.ts unit tests — pingHealth + autoFindGateway.

import { describe, expect, it, vi } from "vitest";

import { LMmasterClient } from "./client";
import { autoFindGateway, buildLaunchUrl, pingHealth } from "./discovery";

describe("pingHealth", () => {
  it("200 응답이면 HealthResponse", async () => {
    const c = new LMmasterClient({
      baseUrl: "http://gw/v1",
      fetchImpl: vi.fn().mockResolvedValue(
        new Response(JSON.stringify({ status: "ok", version: "1.0" }), {
          status: 200,
          headers: { "content-type": "application/json" },
        }),
      ) as unknown as typeof fetch,
    });
    const r = await pingHealth(c);
    expect(r?.status).toBe("ok");
  });

  it("실패는 null 반환", async () => {
    const c = new LMmasterClient({
      baseUrl: "http://gw/v1",
      fetchImpl: vi.fn().mockRejectedValue(new Error("offline")) as unknown as typeof fetch,
    });
    expect(await pingHealth(c)).toBeNull();
  });

  it("non-2xx는 null 반환", async () => {
    const c = new LMmasterClient({
      baseUrl: "http://gw/v1",
      fetchImpl: vi.fn().mockResolvedValue(
        new Response("nope", { status: 500 }),
      ) as unknown as typeof fetch,
    });
    expect(await pingHealth(c)).toBeNull();
  });
});

describe("buildLaunchUrl", () => {
  it("custom scheme + return_to query", () => {
    const u = buildLaunchUrl("https://my-blog.com");
    expect(u.startsWith("lmmaster://launch")).toBe(true);
    expect(u).toContain("return_to=https");
  });

  it("returnTo 없으면 query 없음", () => {
    const u = buildLaunchUrl();
    expect(u).toBe("lmmaster://launch");
  });
});

describe("autoFindGateway", () => {
  it("첫 살아있는 포트 반환", async () => {
    const fetchImpl = vi.fn().mockImplementation(async (url: string) => {
      if (url.includes("43117")) return new Response("nope", { status: 500 });
      if (url.includes("43118"))
        return new Response("ok", { status: 200 });
      return new Response("nope", { status: 500 });
    }) as unknown as typeof fetch;
    const c = new LMmasterClient({ baseUrl: "http://x/v1", fetchImpl });
    const r = await autoFindGateway(c, [43117, 43118, 43119]);
    expect(r).toBe("http://127.0.0.1:43118/v1");
  });

  it("모두 실패면 null", async () => {
    const c = new LMmasterClient({
      baseUrl: "http://x/v1",
      fetchImpl: vi.fn().mockRejectedValue(new Error("offline")) as unknown as typeof fetch,
    });
    const r = await autoFindGateway(c, [1, 2, 3]);
    expect(r).toBeNull();
  });
});
