// Workbench hash 파서 단위 테스트 — Phase 12'.a invariants.

import { describe, expect, it } from "vitest";

import { buildWorkbenchHash, parseWorkbenchHash } from "./hash";

describe("parseWorkbenchHash", () => {
  it("빈 hash → 둘 다 null", () => {
    expect(parseWorkbenchHash("")).toEqual({ model: null, intent: null });
    expect(parseWorkbenchHash("#")).toEqual({ model: null, intent: null });
    expect(parseWorkbenchHash("#/workbench")).toEqual({
      model: null,
      intent: null,
    });
  });

  it("model만 있는 hash 파싱", () => {
    const ctx = parseWorkbenchHash("#/workbench?model=qwen-7b");
    expect(ctx.model).toBe("qwen-7b");
    expect(ctx.intent).toBeNull();
  });

  it("model + intent 둘 다 파싱", () => {
    const ctx = parseWorkbenchHash(
      "#/workbench?model=qwen-7b&intent=coding-general",
    );
    expect(ctx.model).toBe("qwen-7b");
    expect(ctx.intent).toBe("coding-general");
  });

  it("intent만 있는 hash 파싱", () => {
    const ctx = parseWorkbenchHash("#/workbench?intent=ko-rag");
    expect(ctx.model).toBeNull();
    expect(ctx.intent).toBe("ko-rag");
  });

  it("선두 # 없어도 파싱", () => {
    const ctx = parseWorkbenchHash("/workbench?model=x");
    expect(ctx.model).toBe("x");
  });

  it("빈 query value → null", () => {
    const ctx = parseWorkbenchHash("#/workbench?model=&intent=");
    expect(ctx.model).toBeNull();
    expect(ctx.intent).toBeNull();
  });

  it("URL 인코딩된 값 디코드", () => {
    const ctx = parseWorkbenchHash(
      "#/workbench?model=hf-elyza%2FLlama&intent=ko-conversation",
    );
    expect(ctx.model).toBe("hf-elyza/Llama");
    expect(ctx.intent).toBe("ko-conversation");
  });
});

describe("buildWorkbenchHash", () => {
  it("model만 있을 때", () => {
    expect(buildWorkbenchHash("qwen-7b")).toBe(
      "#/workbench?model=qwen-7b",
    );
  });

  it("model + intent 둘 다 있을 때", () => {
    expect(buildWorkbenchHash("qwen-7b", "coding-fim")).toBe(
      "#/workbench?model=qwen-7b&intent=coding-fim",
    );
  });

  it("intent=null이면 model만", () => {
    expect(buildWorkbenchHash("x", null)).toBe("#/workbench?model=x");
  });

  it("round-trip — build → parse 동일", () => {
    const built = buildWorkbenchHash("hf-elyza/Llama", "ko-conversation");
    const parsed = parseWorkbenchHash(built);
    expect(parsed.model).toBe("hf-elyza/Llama");
    expect(parsed.intent).toBe("ko-conversation");
  });
});
