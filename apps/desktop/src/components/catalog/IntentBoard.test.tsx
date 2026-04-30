/**
 * @vitest-environment jsdom
 */
// IntentBoard — Phase 11'.b (ADR-0048) a11y + selection invariants.

import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { axe } from "vitest-axe";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, defaultValue?: string) => defaultValue ?? key,
    i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
  }),
}));

// catalog.ts module-load 시점의 @tauri-apps/api/core 호출 회피.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { IntentBoard } from "./IntentBoard";

describe("IntentBoard", () => {
  it("a11y: violations 없음", async () => {
    const { container } = render(
      <IntentBoard selected={null} onSelect={() => {}} />,
    );
    const results = await axe(container);
    expect(results.violations).toEqual([]);
  });

  it("11 의도 칩 + '전체' = 12 radio 노출", () => {
    render(<IntentBoard selected={null} onSelect={() => {}} />);
    const group = screen.getByRole("radiogroup");
    const radios = within(group).getAllByRole("radio");
    expect(radios).toHaveLength(12);
  });

  it("selected가 'vision-image'면 해당 칩만 aria-checked=true", () => {
    render(<IntentBoard selected="vision-image" onSelect={() => {}} />);
    const chip = screen.getByTestId("intent-chip-vision-image");
    expect(chip).toHaveAttribute("aria-checked", "true");
    const all = screen.getByTestId("intent-chip-all");
    expect(all).toHaveAttribute("aria-checked", "false");
  });

  it("'전체' 클릭 시 onSelect(null)", async () => {
    const onSelect = vi.fn();
    render(<IntentBoard selected="ko-rag" onSelect={onSelect} />);
    const user = userEvent.setup();
    await user.click(screen.getByTestId("intent-chip-all"));
    expect(onSelect).toHaveBeenCalledWith(null);
  });

  it("의도 칩 클릭 시 onSelect(IntentId)", async () => {
    const onSelect = vi.fn();
    render(<IntentBoard selected={null} onSelect={onSelect} />);
    const user = userEvent.setup();
    await user.click(screen.getByTestId("intent-chip-coding-fim"));
    expect(onSelect).toHaveBeenCalledWith("coding-fim");
  });

  it("기본 한국어 라벨 fallback (i18n key 없을 때 default 사용)", () => {
    render(<IntentBoard selected={null} onSelect={() => {}} />);
    expect(
      screen.getByTestId("intent-chip-vision-image").textContent,
    ).toContain("이미지 분석");
    expect(screen.getByTestId("intent-chip-ko-rag").textContent).toContain(
      "한국어 RAG",
    );
  });
});
