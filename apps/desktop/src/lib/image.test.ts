// scaleToMax + stripDataUrlPrefix pure function 테스트 — Phase 13'.h.
// processImageForVision은 jsdom canvas 미구현으로 단위 테스트 X (e2e 영역).

import { describe, expect, it } from "vitest";

import { scaleToMax, stripDataUrlPrefix } from "./image";

describe("scaleToMax", () => {
  it("작은 이미지는 변경 없음", () => {
    expect(scaleToMax(1024, 768, 4096)).toEqual({ width: 1024, height: 768 });
  });

  it("정확히 max-dimension은 변경 없음", () => {
    expect(scaleToMax(4096, 2048, 4096)).toEqual({
      width: 4096,
      height: 2048,
    });
  });

  it("가로가 max보다 크면 비율 유지 축소", () => {
    const r = scaleToMax(8192, 4096, 4096);
    expect(r.width).toBe(4096);
    expect(r.height).toBe(2048);
  });

  it("세로가 max보다 크면 비율 유지 축소", () => {
    const r = scaleToMax(2048, 8192, 4096);
    expect(r.height).toBe(4096);
    expect(r.width).toBe(1024);
  });

  it("정사각형 큰 이미지 → 비율 유지", () => {
    expect(scaleToMax(8000, 8000, 4096)).toEqual({
      width: 4096,
      height: 4096,
    });
  });
});

describe("stripDataUrlPrefix", () => {
  it("data:image/jpeg;base64, prefix 제거", () => {
    expect(stripDataUrlPrefix("data:image/jpeg;base64,abcdef")).toBe("abcdef");
  });

  it("다른 mime도 제거", () => {
    expect(stripDataUrlPrefix("data:image/png;base64,xyz")).toBe("xyz");
  });

  it("prefix 없으면 원본 반환", () => {
    expect(stripDataUrlPrefix("plain-base64-string")).toBe("plain-base64-string");
  });

  it("빈 string", () => {
    expect(stripDataUrlPrefix("")).toBe("");
  });
});
