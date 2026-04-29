import { jsx as _jsx } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// HelpButton — Phase 12'.b 단위 테스트.
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("react-i18next", () => ({
    useTranslation: () => ({
        t: (key, fallback) => fallback ?? key,
        i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
    }),
}));
import { HelpButton } from "./HelpButton";
beforeEach(() => {
    globalThis.localStorage.clear();
});
afterEach(() => {
    vi.restoreAllMocks();
});
describe("HelpButton — 기본 렌더 + popover 토글", () => {
    it("닫혀있을 때 trigger button만 보여요", () => {
        render(_jsx(HelpButton, { sectionId: "workbench" }));
        expect(screen.getByTestId("help-workbench")).toBeTruthy();
        expect(screen.queryByTestId("help-workbench-popover")).toBeNull();
    });
    it("trigger 클릭 시 popover 노출 + role=dialog + aria-modal=true", async () => {
        const user = userEvent.setup();
        render(_jsx(HelpButton, { sectionId: "workbench", hint: "\uC9E7\uC740 \uC124\uBA85" }));
        await user.click(screen.getByTestId("help-workbench"));
        const popover = screen.getByTestId("help-workbench-popover");
        expect(popover.getAttribute("role")).toBe("dialog");
        expect(popover.getAttribute("aria-modal")).toBe("true");
        expect(screen.getByTestId("help-workbench-hint").textContent).toContain("짧은 설명");
    });
    it("닫기 버튼 클릭 → popover 닫힘", async () => {
        const user = userEvent.setup();
        render(_jsx(HelpButton, { sectionId: "workbench" }));
        await user.click(screen.getByTestId("help-workbench"));
        expect(screen.getByTestId("help-workbench-popover")).toBeTruthy();
        await user.click(screen.getByTestId("help-workbench-close"));
        await waitFor(() => {
            expect(screen.queryByTestId("help-workbench-popover")).toBeNull();
        });
    });
    it("Esc 키 → popover 닫힘", async () => {
        const user = userEvent.setup();
        render(_jsx(HelpButton, { sectionId: "workbench" }));
        await user.click(screen.getByTestId("help-workbench"));
        expect(screen.getByTestId("help-workbench-popover")).toBeTruthy();
        await user.keyboard("{Escape}");
        await waitFor(() => {
            expect(screen.queryByTestId("help-workbench-popover")).toBeNull();
        });
    });
});
describe("HelpButton — 가이드 진입", () => {
    it("'전체 가이드 보기' 클릭 시 lmmaster:navigate=guide + lmmaster:guide:open detail.section=sectionId", async () => {
        const user = userEvent.setup();
        render(_jsx(HelpButton, { sectionId: "catalog" }));
        const navEvents = [];
        const guideEvents = [];
        const navHandler = (e) => {
            const detail = e.detail;
            if (typeof detail === "string")
                navEvents.push(detail);
        };
        const guideHandler = (e) => {
            const detail = e.detail;
            if (detail?.section)
                guideEvents.push(detail.section);
        };
        window.addEventListener("lmmaster:navigate", navHandler);
        window.addEventListener("lmmaster:guide:open", guideHandler);
        try {
            await user.click(screen.getByTestId("help-catalog"));
            await user.click(screen.getByTestId("help-catalog-open-guide"));
            await waitFor(() => {
                expect(navEvents).toContain("guide");
                expect(guideEvents).toContain("catalog");
            });
        }
        finally {
            window.removeEventListener("lmmaster:navigate", navHandler);
            window.removeEventListener("lmmaster:guide:open", guideHandler);
        }
    });
});
describe("HelpButton — a11y", () => {
    it("aria-haspopup + aria-expanded toggling", async () => {
        const user = userEvent.setup();
        render(_jsx(HelpButton, { sectionId: "workbench" }));
        const trigger = screen.getByTestId("help-workbench");
        expect(trigger.getAttribute("aria-haspopup")).toBe("dialog");
        expect(trigger.getAttribute("aria-expanded")).toBe("false");
        await user.click(trigger);
        expect(trigger.getAttribute("aria-expanded")).toBe("true");
    });
    it("axe a11y violations === [] (closed)", async () => {
        const { container } = render(_jsx(HelpButton, { sectionId: "workbench" }));
        const results = await axe(container);
        expect(results.violations).toEqual([]);
    });
    it("axe a11y violations === [] (open)", async () => {
        const user = userEvent.setup();
        const { container } = render(_jsx(HelpButton, { sectionId: "workbench" }));
        await user.click(screen.getByTestId("help-workbench"));
        const results = await axe(container);
        expect(results.violations).toEqual([]);
    });
});
