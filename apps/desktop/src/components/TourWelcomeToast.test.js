import { jsx as _jsx } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// TourWelcomeToast — Phase 12'.c 단위 테스트.
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("react-i18next", () => ({
    useTranslation: () => ({
        t: (key, fallback) => fallback ?? key,
        i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
    }),
}));
import { TourWelcomeToast } from "./TourWelcomeToast";
beforeEach(() => {
    globalThis.localStorage.clear();
});
afterEach(() => {
    vi.restoreAllMocks();
});
describe("TourWelcomeToast — 첫 표시", () => {
    it("trigger=true 시 toast 노출", () => {
        render(_jsx(TourWelcomeToast, { trigger: true }));
        expect(screen.getByTestId("tour-welcome-toast")).toBeTruthy();
        expect(screen.getByTestId("tour-welcome-accept")).toBeTruthy();
        expect(screen.getByTestId("tour-welcome-decline")).toBeTruthy();
    });
    it("trigger=false 시 toast 숨김", () => {
        render(_jsx(TourWelcomeToast, { trigger: false }));
        expect(screen.queryByTestId("tour-welcome-toast")).toBeNull();
    });
    it("role=status + aria-live=polite", () => {
        render(_jsx(TourWelcomeToast, { trigger: true }));
        const toast = screen.getByTestId("tour-welcome-toast");
        expect(toast.getAttribute("role")).toBe("status");
        expect(toast.getAttribute("aria-live")).toBe("polite");
    });
});
describe("TourWelcomeToast — accept", () => {
    it("'지금 볼게요' 클릭 → lmmaster:navigate=guide + lmmaster:guide:open=getting-started + onDismiss('accepted')", async () => {
        const user = userEvent.setup();
        const onDismiss = vi.fn();
        render(_jsx(TourWelcomeToast, { trigger: true, onDismiss: onDismiss }));
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
            await user.click(screen.getByTestId("tour-welcome-accept"));
            await waitFor(() => {
                expect(navEvents).toContain("guide");
                expect(guideEvents).toContain("getting-started");
                expect(onDismiss).toHaveBeenCalledWith("accepted");
            });
        }
        finally {
            window.removeEventListener("lmmaster:navigate", navHandler);
            window.removeEventListener("lmmaster:guide:open", guideHandler);
        }
    });
});
describe("TourWelcomeToast — decline + persistence", () => {
    it("'다음에 할게요' 클릭 → localStorage skipped=true + onDismiss('declined')", async () => {
        const user = userEvent.setup();
        const onDismiss = vi.fn();
        render(_jsx(TourWelcomeToast, { trigger: true, onDismiss: onDismiss }));
        await user.click(screen.getByTestId("tour-welcome-decline"));
        await waitFor(() => {
            expect(globalThis.localStorage.getItem("lmmaster.tour.skipped")).toBe("true");
            expect(onDismiss).toHaveBeenCalledWith("declined");
        });
    });
    it("이미 skipped면 재 trigger에서도 표시 X", () => {
        globalThis.localStorage.setItem("lmmaster.tour.skipped", "true");
        render(_jsx(TourWelcomeToast, { trigger: true }));
        expect(screen.queryByTestId("tour-welcome-toast")).toBeNull();
    });
    it("이미 한 번 본 적 있으면 (shown=true) 재 trigger에서도 표시 X", () => {
        globalThis.localStorage.setItem("lmmaster.tour.shown", "true");
        render(_jsx(TourWelcomeToast, { trigger: true }));
        expect(screen.queryByTestId("tour-welcome-toast")).toBeNull();
    });
    it("표시 후 localStorage shown=true 마킹 (1회 영속)", async () => {
        render(_jsx(TourWelcomeToast, { trigger: true }));
        await waitFor(() => {
            expect(globalThis.localStorage.getItem("lmmaster.tour.shown")).toBe("true");
        });
    });
});
