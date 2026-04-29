/**
 * @vitest-environment jsdom
 */
// ShortcutsModal — Phase 12'.c 단위 테스트.

import { renderHook, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useState } from "react";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, fallback?: string) => fallback ?? key,
    i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
  }),
}));

import {
  isFormControlActive,
  ShortcutsModal,
  useShortcutsHotkey,
} from "./ShortcutsModal";

beforeEach(() => {
  globalThis.localStorage.clear();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("ShortcutsModal — 기본 렌더", () => {
  it("open=false면 null 렌더", () => {
    const { container } = render(
      <ShortcutsModal open={false} onClose={vi.fn()} />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("open=true → role=dialog + aria-modal=true + 13행 표시", () => {
    render(<ShortcutsModal open onClose={vi.fn()} />);
    const modal = screen.getByTestId("shortcuts-modal");
    expect(modal.getAttribute("role")).toBe("dialog");
    expect(modal.getAttribute("aria-modal")).toBe("true");
    expect(screen.getByTestId("shortcuts-row-palette")).toBeTruthy();
    expect(screen.getByTestId("shortcuts-row-shortcuts")).toBeTruthy();
    expect(screen.getByTestId("shortcuts-row-navHome")).toBeTruthy();
    expect(screen.getByTestId("shortcuts-row-escape")).toBeTruthy();
  });

  it("close 버튼 → onClose 호출", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<ShortcutsModal open onClose={onClose} />);
    await user.click(screen.getByTestId("shortcuts-close"));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("Esc 키 → onClose", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<ShortcutsModal open onClose={onClose} />);
    await user.keyboard("{Escape}");
    await waitFor(() => {
      expect(onClose).toHaveBeenCalledTimes(1);
    });
  });

  it("backdrop 클릭 → onClose (modal 내부 클릭은 X)", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<ShortcutsModal open onClose={onClose} />);
    await user.click(screen.getByTestId("shortcuts-backdrop"));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("a11y violations === []", async () => {
    const { container } = render(
      <ShortcutsModal open onClose={vi.fn()} />,
    );
    const results = await axe(container);
    expect(results.violations).toEqual([]);
  });
});

describe("isFormControlActive", () => {
  it("input focus → true", () => {
    const inp = document.createElement("input");
    document.body.appendChild(inp);
    inp.focus();
    expect(isFormControlActive()).toBe(true);
    document.body.removeChild(inp);
  });

  it("textarea focus → true", () => {
    const ta = document.createElement("textarea");
    document.body.appendChild(ta);
    ta.focus();
    expect(isFormControlActive()).toBe(true);
    document.body.removeChild(ta);
  });

  it("button focus → false", () => {
    const btn = document.createElement("button");
    document.body.appendChild(btn);
    btn.focus();
    expect(isFormControlActive()).toBe(false);
    document.body.removeChild(btn);
  });
});

// hook 통합 — F1 / Ctrl+1 등 글로벌 hotkey 동작.
describe("useShortcutsHotkey — F1/Ctrl+숫자", () => {
  function HookHarness({
    onNav,
    initialOpen = false,
  }: {
    onNav: (k: string) => void;
    initialOpen?: boolean;
  }) {
    const [open, setOpen] = useState(initialOpen);
    useShortcutsHotkey({ open, setOpen, onNav });
    return <div data-testid="hook-state">{open ? "open" : "closed"}</div>;
  }

  it("F1 → state 토글 (closed → open)", async () => {
    const user = userEvent.setup();
    const onNav = vi.fn();
    render(<HookHarness onNav={onNav} />);
    expect(screen.getByTestId("hook-state").textContent).toBe("closed");
    await user.keyboard("{F1}");
    await waitFor(() => {
      expect(screen.getByTestId("hook-state").textContent).toBe("open");
    });
  });

  it("Ctrl+1 → onNav('home')", async () => {
    const user = userEvent.setup();
    const onNav = vi.fn();
    render(<HookHarness onNav={onNav} />);
    await user.keyboard("{Control>}1{/Control}");
    await waitFor(() => {
      expect(onNav).toHaveBeenCalledWith("home");
    });
  });

  it("Ctrl+8 → onNav('workbench')", async () => {
    const user = userEvent.setup();
    const onNav = vi.fn();
    render(<HookHarness onNav={onNav} />);
    await user.keyboard("{Control>}8{/Control}");
    await waitFor(() => {
      expect(onNav).toHaveBeenCalledWith("workbench");
    });
  });
});
