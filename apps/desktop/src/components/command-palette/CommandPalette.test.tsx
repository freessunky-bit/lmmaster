/**
 * @vitest-environment jsdom
 */
// CommandPalette 컴포넌트 테스트 — Provider + ⌘K hotkey + Combobox + 시드 명령. Phase 1A.4.d.2.

import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { useEffect } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
  }),
}));

import { CommandPalette } from "./CommandPalette";
import {
  CommandPaletteProvider,
  useCommandPalette,
  useCommandRegistration,
} from "./context";
import { useCommandPaletteHotkey } from "../../hooks/useCommandPaletteHotkey";
import type { Command } from "./types";

/** hotkey 등록을 위한 헬퍼. */
function HotkeyMounter() {
  useCommandPaletteHotkey();
  return null;
}

/** 명령 등록 + open 토글을 외부에서 제어할 헬퍼. */
function CommandsRegister({ commands }: { commands: Command[] }) {
  useCommandRegistration(commands);
  return null;
}

function OpenController({ open }: { open: boolean }) {
  const { setOpen } = useCommandPalette();
  useEffect(() => {
    setOpen(open);
  }, [open, setOpen]);
  return null;
}

function Wrap({
  open,
  commands,
  children,
}: {
  open?: boolean;
  commands?: Command[];
  children?: ReactNode;
}) {
  return (
    <CommandPaletteProvider>
      <HotkeyMounter />
      {commands && <CommandsRegister commands={commands} />}
      {open !== undefined && <OpenController open={open} />}
      <CommandPalette />
      {children}
    </CommandPaletteProvider>
  );
}

beforeEach(() => {
  document.body.innerHTML = "";
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("CommandPalette", () => {
  it("닫힌 상태 — dialog 미렌더", () => {
    render(<Wrap />);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("⌘K hotkey로 열림", async () => {
    const user = userEvent.setup();
    render(<Wrap />);
    await user.keyboard("{Meta>}k{/Meta}");
    await waitFor(() =>
      expect(
        screen.getByRole("dialog", { name: "palette.aria.dialog" }),
      ).toBeInTheDocument(),
    );
  });

  it("Ctrl+K hotkey로도 열림", async () => {
    const user = userEvent.setup();
    render(<Wrap />);
    await user.keyboard("{Control>}k{/Control}");
    await waitFor(() =>
      expect(
        screen.getByRole("dialog", { name: "palette.aria.dialog" }),
      ).toBeInTheDocument(),
    );
  });

  it("열림 — 입력 + 명령 리스트 + group 헤딩", async () => {
    const cmd1: Command = {
      id: "test.foo",
      group: "wizard",
      label: "Foo command",
      keywords: ["foo"],
      perform: vi.fn(),
    };
    const cmd2: Command = {
      id: "test.bar",
      group: "navigation",
      label: "Bar command",
      keywords: ["bar"],
      perform: vi.fn(),
    };
    render(<Wrap open commands={[cmd1, cmd2]} />);
    expect(
      await screen.findByPlaceholderText("palette.placeholder"),
    ).toBeInTheDocument();
    expect(screen.getByText("Foo command")).toBeInTheDocument();
    expect(screen.getByText("Bar command")).toBeInTheDocument();
    expect(screen.getByText("palette.group.wizard")).toBeInTheDocument();
    expect(screen.getByText("palette.group.navigation")).toBeInTheDocument();
  });

  it("검색어 입력 시 일치 명령만 노출", async () => {
    const cmd1: Command = {
      id: "test.foo",
      group: "wizard",
      label: "환경 다시 점검",
      keywords: ["scan"],
      perform: vi.fn(),
    };
    const cmd2: Command = {
      id: "test.bar",
      group: "wizard",
      label: "마법사 처음부터",
      keywords: ["restart"],
      perform: vi.fn(),
    };
    const user = userEvent.setup();
    render(<Wrap open commands={[cmd1, cmd2]} />);
    const input = await screen.findByPlaceholderText("palette.placeholder");
    await user.type(input, "환경");
    expect(screen.queryByText("환경 다시 점검")).toBeInTheDocument();
    expect(screen.queryByText("마법사 처음부터")).not.toBeInTheDocument();
  });

  it("일치 없음 — empty state 메시지", async () => {
    const cmd1: Command = {
      id: "test.foo",
      group: "wizard",
      label: "Foo",
      keywords: [],
      perform: vi.fn(),
    };
    const user = userEvent.setup();
    render(<Wrap open commands={[cmd1]} />);
    const input = await screen.findByPlaceholderText("palette.placeholder");
    await user.type(input, "zzzqqq");
    expect(screen.getByText("palette.empty")).toBeInTheDocument();
  });
});
