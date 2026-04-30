/**
 * @vitest-environment jsdom
 */
// WorkbenchContextBar a11y + selection invariants — Phase 12'.a.

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { axe } from "vitest-axe";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, defaultValue?: string | Record<string, unknown>) =>
      typeof defaultValue === "string" ? defaultValue : key,
    i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
  }),
}));

import { WorkbenchContextBar } from "./WorkbenchContextBar";

describe("WorkbenchContextBar", () => {
  it("a11y: violations 없음", async () => {
    const { container } = render(
      <WorkbenchContextBar
        modelDisplayName="Qwen2.5-Coder-7B"
        intent="coding-general"
      />,
    );
    const results = await axe(container);
    expect(results.violations).toEqual([]);
  });

  it("의도 + 모델 둘 다 표시", () => {
    render(
      <WorkbenchContextBar
        modelDisplayName="Qwen2.5-Coder-7B"
        intent="coding-general"
      />,
    );
    expect(screen.getByTestId("workbench-context-intent").textContent).toContain(
      "코딩",
    );
    expect(screen.getByTestId("workbench-context-model").textContent).toContain(
      "Qwen2.5-Coder-7B",
    );
  });

  it("intent=null이면 의도 칩 미렌더, 모델 칩만", () => {
    render(<WorkbenchContextBar modelDisplayName="X" intent={null} />);
    expect(screen.queryByTestId("workbench-context-intent")).toBeNull();
    expect(screen.getByTestId("workbench-context-model")).toBeDefined();
  });

  it("model=null이면 모델 칩 미렌더", () => {
    render(
      <WorkbenchContextBar
        modelDisplayName={null}
        intent="ko-conversation"
      />,
    );
    expect(screen.queryByTestId("workbench-context-model")).toBeNull();
    expect(screen.getByTestId("workbench-context-intent")).toBeDefined();
  });

  it("'의도 변경' 클릭 → onChangeIntent 호출", async () => {
    const onChangeIntent = vi.fn();
    render(
      <WorkbenchContextBar
        modelDisplayName="X"
        intent="ko-rag"
        onChangeIntent={onChangeIntent}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("workbench-context-change-intent"));
    expect(onChangeIntent).toHaveBeenCalled();
  });

  it("'모델 변경' 클릭 → onChangeModel 호출", async () => {
    const onChangeModel = vi.fn();
    render(
      <WorkbenchContextBar
        modelDisplayName="X"
        intent="coding-fim"
        onChangeModel={onChangeModel}
      />,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId("workbench-context-change-model"));
    expect(onChangeModel).toHaveBeenCalled();
  });

  it("onChange* 콜백이 없으면 변경 버튼 미렌더", () => {
    render(
      <WorkbenchContextBar
        modelDisplayName="X"
        intent="coding-fim"
      />,
    );
    expect(
      screen.queryByTestId("workbench-context-change-model"),
    ).toBeNull();
    expect(
      screen.queryByTestId("workbench-context-change-intent"),
    ).toBeNull();
  });
});
