/**
 * @vitest-environment jsdom
 */
// Step4Done 컴포넌트 테스트. Phase 1A.4.d.2.

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
  }),
}));

import { Step4Done } from "./Step4Done";

describe("Step4Done", () => {
  it("렌더 — 제목 + CTA 노출", () => {
    render(<Step4Done onFinish={vi.fn()} />);
    expect(
      screen.getByRole("heading", { name: "onboarding.done.title" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "onboarding.done.cta" }),
    ).toBeInTheDocument();
  });

  it("CTA 클릭 시 onFinish 호출", async () => {
    const finish = vi.fn();
    const user = userEvent.setup();
    render(<Step4Done onFinish={finish} />);
    await user.click(
      screen.getByRole("button", { name: "onboarding.done.cta" }),
    );
    expect(finish).toHaveBeenCalledTimes(1);
  });
});
