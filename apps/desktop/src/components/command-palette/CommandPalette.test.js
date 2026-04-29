import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// CommandPalette 컴포넌트 테스트 — Provider + ⌘K hotkey + Combobox + 시드 명령. Phase 1A.4.d.2.
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useEffect } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("react-i18next", () => ({
    useTranslation: () => ({
        t: (key) => key,
        i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
    }),
}));
import { CommandPalette } from "./CommandPalette";
import { CommandPaletteProvider, useCommandPalette, useCommandRegistration, } from "./context";
import { useCommandPaletteHotkey } from "../../hooks/useCommandPaletteHotkey";
/** hotkey 등록을 위한 헬퍼. */
function HotkeyMounter() {
    useCommandPaletteHotkey();
    return null;
}
/** 명령 등록 + open 토글을 외부에서 제어할 헬퍼. */
function CommandsRegister({ commands }) {
    useCommandRegistration(commands);
    return null;
}
function OpenController({ open }) {
    const { setOpen } = useCommandPalette();
    useEffect(() => {
        setOpen(open);
    }, [open, setOpen]);
    return null;
}
function Wrap({ open, commands, children, }) {
    return (_jsxs(CommandPaletteProvider, { children: [_jsx(HotkeyMounter, {}), commands && _jsx(CommandsRegister, { commands: commands }), open !== undefined && _jsx(OpenController, { open: open }), _jsx(CommandPalette, {}), children] }));
}
beforeEach(() => {
    document.body.innerHTML = "";
});
afterEach(() => {
    vi.clearAllMocks();
});
describe("CommandPalette", () => {
    it("닫힌 상태 — dialog 미렌더", () => {
        render(_jsx(Wrap, {}));
        expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });
    it("⌘K hotkey로 열림", async () => {
        const user = userEvent.setup();
        render(_jsx(Wrap, {}));
        await user.keyboard("{Meta>}k{/Meta}");
        await waitFor(() => expect(screen.getByRole("dialog", { name: "palette.aria.dialog" })).toBeInTheDocument());
    });
    it("Ctrl+K hotkey로도 열림", async () => {
        const user = userEvent.setup();
        render(_jsx(Wrap, {}));
        await user.keyboard("{Control>}k{/Control}");
        await waitFor(() => expect(screen.getByRole("dialog", { name: "palette.aria.dialog" })).toBeInTheDocument());
    });
    it("열림 — 입력 + 명령 리스트 + group 헤딩", async () => {
        const cmd1 = {
            id: "test.foo",
            group: "wizard",
            label: "Foo command",
            keywords: ["foo"],
            perform: vi.fn(),
        };
        const cmd2 = {
            id: "test.bar",
            group: "navigation",
            label: "Bar command",
            keywords: ["bar"],
            perform: vi.fn(),
        };
        render(_jsx(Wrap, { open: true, commands: [cmd1, cmd2] }));
        expect(await screen.findByPlaceholderText("palette.placeholder")).toBeInTheDocument();
        expect(screen.getByText("Foo command")).toBeInTheDocument();
        expect(screen.getByText("Bar command")).toBeInTheDocument();
        expect(screen.getByText("palette.group.wizard")).toBeInTheDocument();
        expect(screen.getByText("palette.group.navigation")).toBeInTheDocument();
    });
    it("검색어 입력 시 일치 명령만 노출", async () => {
        const cmd1 = {
            id: "test.foo",
            group: "wizard",
            label: "환경 다시 점검",
            keywords: ["scan"],
            perform: vi.fn(),
        };
        const cmd2 = {
            id: "test.bar",
            group: "wizard",
            label: "마법사 처음부터",
            keywords: ["restart"],
            perform: vi.fn(),
        };
        const user = userEvent.setup();
        render(_jsx(Wrap, { open: true, commands: [cmd1, cmd2] }));
        const input = await screen.findByPlaceholderText("palette.placeholder");
        await user.type(input, "환경");
        expect(screen.queryByText("환경 다시 점검")).toBeInTheDocument();
        expect(screen.queryByText("마법사 처음부터")).not.toBeInTheDocument();
    });
    it("일치 없음 — empty state 메시지", async () => {
        const cmd1 = {
            id: "test.foo",
            group: "wizard",
            label: "Foo",
            keywords: [],
            perform: vi.fn(),
        };
        const user = userEvent.setup();
        render(_jsx(Wrap, { open: true, commands: [cmd1] }));
        const input = await screen.findByPlaceholderText("palette.placeholder");
        await user.type(input, "zzzqqq");
        expect(screen.getByText("palette.empty")).toBeInTheDocument();
    });
});
