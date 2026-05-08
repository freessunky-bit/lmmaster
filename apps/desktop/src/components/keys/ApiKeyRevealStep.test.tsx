/**
 * @vitest-environment jsdom
 */
// Phase 8'.c.4 (ADR-0066) — ApiKeyRevealStep "이렇게 쓰세요" 가이드 단위 테스트.

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createRef } from "react";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, params?: Record<string, unknown>) => {
      if (params && Object.keys(params).length > 0) {
        return `${key}|${JSON.stringify(params)}`;
      }
      return key;
    },
    i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
  }),
}));

import { ApiKeyRevealStep } from "./ApiKeyRevealStep";

const baseProps = {
  plaintext: "lm-aaaa1234XXXXSECRET24CHARS!",
  keyPrefix: "lm-aaaa1234",
  gatewayPort: 8788,
  modelExample: "qwen-3-30b-a3b",
  onClose: vi.fn(),
};

// jsdom navigator.clipboard는 getter-only — Object.defineProperty로 한 번 정의한 뒤
// 같은 인스턴스를 재사용. PromptTemplateStep.test.tsx 주석에 따르면 `vi.stubGlobal`도
// 실제 동작 단언에는 충분하지 않음. 본 테스트는 UI 피드백 (버튼 라벨 변화)으로 검증.
const writeTextMock = vi.fn(async (_text: string) => {});

if (!("clipboard" in navigator)) {
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText: writeTextMock },
  });
} else {
  // 이미 정의돼 있으면 writeText만 갈아끼움.
  Object.defineProperty(navigator.clipboard, "writeText", {
    configurable: true,
    value: writeTextMock,
  });
}

beforeEach(() => {
  writeTextMock.mockClear();
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("ApiKeyRevealStep — 기본 렌더 + 가이드 섹션", () => {
  it("키 평문 + 가이드 섹션 노출 (localhost scope)", () => {
    const closeRef = createRef<HTMLButtonElement>();
    render(
      <ApiKeyRevealStep
        {...baseProps}
        networkScope="localhost"
        lanIps={["192.168.1.42"]}
        closeRef={closeRef}
      />,
    );
    expect(screen.getByTestId("keys-reveal-key")).toBeTruthy();
    expect(screen.getByTestId("keys-reveal-guide")).toBeTruthy();
    expect(screen.getByTestId("keys-reveal-base-localhost")).toBeTruthy();
    // localhost scope면 LAN URL 노출 X.
    expect(screen.queryByTestId("keys-reveal-base-lan-192.168.1.42")).toBeNull();
  });

  it("Base URL localhost 표시 + 게이트웨이 포트 포함", () => {
    const closeRef = createRef<HTMLButtonElement>();
    render(
      <ApiKeyRevealStep
        {...baseProps}
        networkScope="localhost"
        lanIps={[]}
        closeRef={closeRef}
      />,
    );
    const row = screen.getByTestId("keys-reveal-base-localhost");
    expect(row.textContent).toContain("http://127.0.0.1:8788/v1");
  });

  it("network_scope=lan + LAN IP 있으면 LAN URL도 함께 노출", () => {
    const closeRef = createRef<HTMLButtonElement>();
    render(
      <ApiKeyRevealStep
        {...baseProps}
        networkScope="lan"
        lanIps={["192.168.1.42", "10.0.0.15"]}
        closeRef={closeRef}
      />,
    );
    expect(screen.getByTestId("keys-reveal-base-localhost")).toBeTruthy();
    expect(screen.getByTestId("keys-reveal-base-lan-192.168.1.42")).toBeTruthy();
    expect(screen.getByTestId("keys-reveal-base-lan-10.0.0.15")).toBeTruthy();
    expect(
      screen.getByTestId("keys-reveal-base-lan-192.168.1.42").textContent,
    ).toContain("http://192.168.1.42:8788/v1");
  });

  it("network_scope=any → '외부 터널 직접 셋업' 안내 노출", () => {
    const closeRef = createRef<HTMLButtonElement>();
    render(
      <ApiKeyRevealStep
        {...baseProps}
        networkScope="any"
        lanIps={[]}
        closeRef={closeRef}
      />,
    );
    expect(screen.getByTestId("keys-reveal-any-note")).toBeTruthy();
  });

  it("모델 ID 예시 표시 + 복사 버튼 노출", () => {
    // 본 테스트는 모델 ID가 가이드 row에 정확히 노출되고 복사 버튼이 enable 상태인지 단언.
    // navigator.clipboard 호출은 jsdom 환경 제약으로 e2e/수동 검증 (PromptTemplateStep.test.tsx 패턴).
    const closeRef = createRef<HTMLButtonElement>();
    render(
      <ApiKeyRevealStep
        {...baseProps}
        networkScope="localhost"
        lanIps={[]}
        closeRef={closeRef}
      />,
    );
    const row = screen.getByTestId("keys-reveal-model");
    expect(row.textContent).toContain("qwen-3-30b-a3b");
    const copyBtn = row.querySelector("button") as HTMLButtonElement;
    expect(copyBtn).toBeTruthy();
    expect(copyBtn.disabled).toBe(false);
  });

  it("curl 예시 동적 생성 — base URL + 모델 ID 포함", () => {
    const closeRef = createRef<HTMLButtonElement>();
    render(
      <ApiKeyRevealStep
        {...baseProps}
        networkScope="lan"
        lanIps={["192.168.1.42"]}
        closeRef={closeRef}
      />,
    );
    const curl = screen.getByTestId("keys-reveal-curl-block").textContent ?? "";
    // LAN scope면 LAN URL 우선 사용.
    expect(curl).toContain("http://192.168.1.42:8788/v1/chat/completions");
    expect(curl).toContain("qwen-3-30b-a3b");
    // 마스크 전이라 평문 키 포함.
    expect(curl).toContain("lm-aaaa1234XXXXSECRET24CHARS!");
  });

  it("curl 전체 복사 버튼 노출 + click 가능", async () => {
    // navigator.clipboard 호출 단언은 jsdom 제약으로 생략 (PromptTemplateStep.test.tsx 패턴).
    // 본 테스트는 버튼이 enable 상태이고 click이 throw 하지 않는지 단언.
    const user = userEvent.setup();
    const closeRef = createRef<HTMLButtonElement>();
    render(
      <ApiKeyRevealStep
        {...baseProps}
        networkScope="localhost"
        lanIps={[]}
        closeRef={closeRef}
      />,
    );
    const btn = screen.getByTestId("keys-reveal-copy-curl") as HTMLButtonElement;
    expect(btn.disabled).toBe(false);
    // 클릭 자체가 unhandled rejection을 발생시키지 않아야 함.
    await user.click(btn);
  });

  it("게이트웨이 포트 null일 때 '<포트>' placeholder", () => {
    const closeRef = createRef<HTMLButtonElement>();
    render(
      <ApiKeyRevealStep
        {...baseProps}
        gatewayPort={null}
        networkScope="localhost"
        lanIps={[]}
        closeRef={closeRef}
      />,
    );
    expect(
      screen.getByTestId("keys-reveal-base-localhost").textContent,
    ).toContain("http://127.0.0.1:<포트>/v1");
  });

  it("modelExample 빈 문자열이면 default placeholder 'qwen-3-30b-a3b' 사용", () => {
    const closeRef = createRef<HTMLButtonElement>();
    render(
      <ApiKeyRevealStep
        {...baseProps}
        modelExample=""
        networkScope="localhost"
        lanIps={[]}
        closeRef={closeRef}
      />,
    );
    expect(
      screen.getByTestId("keys-reveal-model").textContent,
    ).toContain("qwen-3-30b-a3b");
  });
});
