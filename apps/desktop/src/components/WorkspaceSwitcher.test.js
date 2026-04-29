import { jsx as _jsx } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// WorkspaceSwitcher Phase 8'.1 — dropdown / 모달 / a11y / 키보드.
//
// 정책 (CLAUDE.md §4.4):
// - Provider 자체를 mock하지 않고 실 Provider + ipc/workspaces mock으로 backend 격리.
// - scoped 쿼리 (data-testid) — 동일 텍스트 중복 회피.
// - a11y: vitest-axe violations === [].
// - 한국어 i18n key는 stub t로 그대로 노출.
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { axe } from "vitest-axe";
vi.mock("../ipc/workspaces", () => ({
    listWorkspaces: vi.fn(),
    getActiveWorkspace: vi.fn(),
    createWorkspace: vi.fn(),
    renameWorkspace: vi.fn(),
    deleteWorkspace: vi.fn(),
    setActiveWorkspace: vi.fn(),
    onWorkspacesChanged: vi.fn(),
}));
vi.mock("react-i18next", () => ({
    useTranslation: () => ({
        t: (key, opts) => opts ? `${key}:${JSON.stringify(opts)}` : key,
        i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
    }),
}));
import * as ipc from "../ipc/workspaces";
import { ActiveWorkspaceProvider } from "../contexts/ActiveWorkspaceContext";
import { WorkspaceSwitcher } from "./WorkspaceSwitcher";
const listMock = vi.mocked(ipc.listWorkspaces);
const getActiveMock = vi.mocked(ipc.getActiveWorkspace);
const createMock = vi.mocked(ipc.createWorkspace);
const renameMock = vi.mocked(ipc.renameWorkspace);
const deleteMock = vi.mocked(ipc.deleteWorkspace);
const setActiveMock = vi.mocked(ipc.setActiveWorkspace);
const onChangedMock = vi.mocked(ipc.onWorkspacesChanged);
const AXE_OPTIONS = {
    rules: {
        "color-contrast": { enabled: false },
        "html-has-lang": { enabled: false },
        "landmark-one-main": { enabled: false },
        region: { enabled: false },
    },
};
const W1 = {
    id: "ws-default",
    name: "기본 워크스페이스",
    description: null,
    created_at_iso: "2026-04-01T00:00:00Z",
    last_used_iso: "2026-04-28T00:00:00Z",
};
const W2 = {
    id: "ws-second",
    name: "두 번째",
    description: "사이드 프로젝트",
    created_at_iso: "2026-04-02T00:00:00Z",
    last_used_iso: null,
};
beforeEach(() => {
    listMock.mockReset();
    getActiveMock.mockReset();
    createMock.mockReset();
    renameMock.mockReset();
    deleteMock.mockReset();
    setActiveMock.mockReset();
    onChangedMock.mockReset();
    // 기본 — 두 workspace.
    listMock.mockResolvedValue([W1, W2]);
    getActiveMock.mockResolvedValue(W1);
    onChangedMock.mockResolvedValue(() => {
        /* unlisten noop */
    });
});
afterEach(() => {
    vi.clearAllMocks();
});
function renderSwitcher() {
    return render(_jsx(ActiveWorkspaceProvider, { children: _jsx(WorkspaceSwitcher, {}) }));
}
describe("WorkspaceSwitcher Phase 8'.1", () => {
    it("초기 마운트 — list + getActive 호출 + 트리거에 active 이름 표시", async () => {
        renderSwitcher();
        await waitFor(() => {
            expect(listMock).toHaveBeenCalled();
            expect(getActiveMock).toHaveBeenCalled();
        });
        const trigger = screen.getByTestId("workspace-switcher-trigger");
        await waitFor(() => {
            expect(within(trigger).getByText("기본 워크스페이스")).toBeInTheDocument();
        });
    });
    it("트리거 클릭 → dropdown 열림 + 두 항목 + 생성 버튼", async () => {
        const user = userEvent.setup();
        renderSwitcher();
        await waitFor(() => expect(getActiveMock).toHaveBeenCalled());
        await user.click(screen.getByTestId("workspace-switcher-trigger"));
        const menu = await screen.findByTestId("workspace-switcher-menu");
        expect(menu).toBeInTheDocument();
        expect(menu).toHaveAttribute("role", "menu");
        expect(within(menu).getByTestId(`workspace-switcher-item-${W1.id}`))
            .toBeInTheDocument();
        expect(within(menu).getByTestId(`workspace-switcher-item-${W2.id}`))
            .toBeInTheDocument();
        expect(within(menu).getByTestId("workspace-switcher-create"))
            .toBeInTheDocument();
    });
    it("active workspace는 aria-checked=true + ✓ 표시", async () => {
        const user = userEvent.setup();
        renderSwitcher();
        await waitFor(() => expect(getActiveMock).toHaveBeenCalled());
        await user.click(screen.getByTestId("workspace-switcher-trigger"));
        const item = await screen.findByTestId(`workspace-switcher-item-${W1.id}`);
        expect(item).toHaveAttribute("aria-checked", "true");
        const otherItem = screen.getByTestId(`workspace-switcher-item-${W2.id}`);
        expect(otherItem).toHaveAttribute("aria-checked", "false");
    });
    it("다른 workspace 클릭 → setActive 호출 + dropdown 닫힘", async () => {
        setActiveMock.mockResolvedValue(undefined);
        const user = userEvent.setup();
        renderSwitcher();
        await waitFor(() => expect(getActiveMock).toHaveBeenCalled());
        await user.click(screen.getByTestId("workspace-switcher-trigger"));
        await user.click(await screen.findByTestId(`workspace-switcher-item-${W2.id}`));
        await waitFor(() => {
            expect(setActiveMock).toHaveBeenCalledWith(W2.id);
        });
        await waitFor(() => {
            expect(screen.queryByTestId("workspace-switcher-menu")).toBeNull();
        });
    });
    it("Esc 누름 → dropdown 닫힘", async () => {
        const user = userEvent.setup();
        renderSwitcher();
        await waitFor(() => expect(getActiveMock).toHaveBeenCalled());
        await user.click(screen.getByTestId("workspace-switcher-trigger"));
        await screen.findByTestId("workspace-switcher-menu");
        await user.keyboard("{Escape}");
        await waitFor(() => {
            expect(screen.queryByTestId("workspace-switcher-menu")).toBeNull();
        });
    });
    it("‘새 워크스페이스 만들기’ 클릭 → 생성 모달 + 입력 → submit", async () => {
        createMock.mockResolvedValue({
            id: "ws-new",
            name: "새것",
            description: null,
            created_at_iso: "2026-04-28T00:00:00Z",
            last_used_iso: null,
        });
        const user = userEvent.setup();
        renderSwitcher();
        await waitFor(() => expect(getActiveMock).toHaveBeenCalled());
        await user.click(screen.getByTestId("workspace-switcher-trigger"));
        await user.click(await screen.findByTestId("workspace-switcher-create"));
        const modal = await screen.findByTestId("workspace-switcher-create-modal");
        expect(modal).toBeInTheDocument();
        const input = within(modal).getByTestId("workspace-switcher-create-name");
        await user.type(input, "새것");
        await user.click(within(modal).getByTestId("workspace-switcher-create-submit"));
        await waitFor(() => {
            expect(createMock).toHaveBeenCalledWith("새것", undefined);
        });
    });
    it("생성 모달 — 빈 이름 submit → empty 에러 표기", async () => {
        const user = userEvent.setup();
        renderSwitcher();
        await waitFor(() => expect(getActiveMock).toHaveBeenCalled());
        await user.click(screen.getByTestId("workspace-switcher-trigger"));
        await user.click(await screen.findByTestId("workspace-switcher-create"));
        const modal = await screen.findByTestId("workspace-switcher-create-modal");
        await user.click(within(modal).getByTestId("workspace-switcher-create-submit"));
        await waitFor(() => {
            expect(within(modal).getByTestId("workspace-switcher-create-error"))
                .toBeInTheDocument();
        });
        expect(createMock).not.toHaveBeenCalled();
    });
    it("생성 모달 — duplicate-name 에러 → 한국어 메시지", async () => {
        createMock.mockRejectedValue({ kind: "duplicate-name", name: "기본" });
        const user = userEvent.setup();
        renderSwitcher();
        await waitFor(() => expect(getActiveMock).toHaveBeenCalled());
        await user.click(screen.getByTestId("workspace-switcher-trigger"));
        await user.click(await screen.findByTestId("workspace-switcher-create"));
        const modal = await screen.findByTestId("workspace-switcher-create-modal");
        await user.type(within(modal).getByTestId("workspace-switcher-create-name"), "기본");
        await user.click(within(modal).getByTestId("workspace-switcher-create-submit"));
        await waitFor(() => {
            const err = within(modal).getByTestId("workspace-switcher-create-error");
            expect(err.textContent).toContain("duplicate");
        });
    });
    it("이름 바꾸기 클릭 → rename 모달 → submit → renameWorkspace 호출", async () => {
        renameMock.mockResolvedValue({ ...W2, name: "수정됨" });
        const user = userEvent.setup();
        renderSwitcher();
        await waitFor(() => expect(getActiveMock).toHaveBeenCalled());
        await user.click(screen.getByTestId("workspace-switcher-trigger"));
        await user.click(await screen.findByTestId(`workspace-switcher-rename-${W2.id}`));
        const modal = await screen.findByTestId("workspace-switcher-rename-modal");
        const input = within(modal).getByTestId("workspace-switcher-rename-name");
        expect(input.value).toBe(W2.name);
        await user.clear(input);
        await user.type(input, "수정됨");
        await user.click(within(modal).getByTestId("workspace-switcher-rename-submit"));
        await waitFor(() => {
            expect(renameMock).toHaveBeenCalledWith(W2.id, "수정됨");
        });
    });
    it("삭제 클릭 → confirmation dialog → 확인 → deleteWorkspace 호출", async () => {
        deleteMock.mockResolvedValue(undefined);
        const user = userEvent.setup();
        renderSwitcher();
        await waitFor(() => expect(getActiveMock).toHaveBeenCalled());
        await user.click(screen.getByTestId("workspace-switcher-trigger"));
        await user.click(await screen.findByTestId(`workspace-switcher-delete-${W2.id}`));
        const modal = await screen.findByTestId("workspace-switcher-delete-modal");
        expect(modal).toBeInTheDocument();
        // 취소 버튼이 default focus.
        await user.click(within(modal).getByTestId("workspace-switcher-delete-confirm"));
        await waitFor(() => {
            expect(deleteMock).toHaveBeenCalledWith(W2.id);
        });
    });
    it("삭제 confirmation — 취소 시 deleteWorkspace 호출 X", async () => {
        const user = userEvent.setup();
        renderSwitcher();
        await waitFor(() => expect(getActiveMock).toHaveBeenCalled());
        await user.click(screen.getByTestId("workspace-switcher-trigger"));
        await user.click(await screen.findByTestId(`workspace-switcher-delete-${W2.id}`));
        const modal = await screen.findByTestId("workspace-switcher-delete-modal");
        await user.click(within(modal).getByTestId("workspace-switcher-delete-cancel"));
        await waitFor(() => {
            expect(screen.queryByTestId("workspace-switcher-delete-modal")).toBeNull();
        });
        expect(deleteMock).not.toHaveBeenCalled();
    });
    it("workspace 1개일 때 — 삭제 버튼 disabled", async () => {
        listMock.mockResolvedValue([W1]);
        const user = userEvent.setup();
        renderSwitcher();
        await waitFor(() => expect(getActiveMock).toHaveBeenCalled());
        await user.click(screen.getByTestId("workspace-switcher-trigger"));
        const deleteBtn = await screen.findByTestId(`workspace-switcher-delete-${W1.id}`);
        expect(deleteBtn).toBeDisabled();
    });
    it("a11y violations 없음 (idle dropdown 닫힌 상태)", async () => {
        const { container } = renderSwitcher();
        await waitFor(() => expect(getActiveMock).toHaveBeenCalled());
        const results = await axe(container, AXE_OPTIONS);
        expect(results.violations).toEqual([]);
    });
    it("a11y violations 없음 (dropdown 열린 상태)", async () => {
        const user = userEvent.setup();
        const { container } = renderSwitcher();
        await waitFor(() => expect(getActiveMock).toHaveBeenCalled());
        await user.click(screen.getByTestId("workspace-switcher-trigger"));
        await screen.findByTestId("workspace-switcher-menu");
        const results = await axe(container, AXE_OPTIONS);
        expect(results.violations).toEqual([]);
    });
});
