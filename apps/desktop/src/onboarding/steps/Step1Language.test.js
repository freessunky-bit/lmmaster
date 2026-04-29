import { jsx as _jsx } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// Step1Language 컴포넌트 테스트. Phase 1A.4.d.2.
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
// Context hooks mock — 실 OnboardingProvider 마운트 회피.
vi.mock("../context", () => ({
    useOnboardingLang: vi.fn(() => "ko"),
    useOnboardingSend: vi.fn(),
}));
// i18n mock — t() = key 반환.
const changeLanguageMock = vi.fn();
vi.mock("react-i18next", () => ({
    useTranslation: () => ({
        t: (key) => key,
        i18n: {
            changeLanguage: changeLanguageMock,
            resolvedLanguage: "ko",
        },
    }),
}));
import { useOnboardingLang, useOnboardingSend } from "../context";
import { Step1Language } from "./Step1Language";
const mockedLang = vi.mocked(useOnboardingLang);
const mockedSend = vi.mocked(useOnboardingSend);
beforeEach(() => {
    changeLanguageMock.mockReset();
    mockedSend.mockReset();
    mockedLang.mockReturnValue("ko");
});
afterEach(() => {
    vi.clearAllMocks();
});
describe("Step1Language", () => {
    it("렌더 — 제목과 두 라디오 옵션 노출", () => {
        mockedSend.mockReturnValue(vi.fn());
        render(_jsx(Step1Language, {}));
        expect(screen.getByRole("heading", { name: "onboarding.language.title" })).toBeInTheDocument();
        expect(screen.getByRole("radio", { name: "onboarding.language.option.ko" })).toBeInTheDocument();
        expect(screen.getByRole("radio", { name: "onboarding.language.option.en" })).toBeInTheDocument();
    });
    it("ko 활성 표시 — context.lang === 'ko'일 때 aria-checked", () => {
        mockedSend.mockReturnValue(vi.fn());
        render(_jsx(Step1Language, {}));
        expect(screen.getByRole("radio", { name: "onboarding.language.option.ko" })).toHaveAttribute("aria-checked", "true");
        expect(screen.getByRole("radio", { name: "onboarding.language.option.en" })).toHaveAttribute("aria-checked", "false");
    });
    it("en 클릭 시 i18n.changeLanguage + SET_LANG 송신", async () => {
        const send = vi.fn();
        mockedSend.mockReturnValue(send);
        const user = userEvent.setup();
        render(_jsx(Step1Language, {}));
        await user.click(screen.getByRole("radio", { name: "onboarding.language.option.en" }));
        expect(changeLanguageMock).toHaveBeenCalledWith("en");
        expect(send).toHaveBeenCalledWith({ type: "SET_LANG", lang: "en" });
    });
    it("이미 선택된 언어 클릭 시 noop", async () => {
        const send = vi.fn();
        mockedSend.mockReturnValue(send);
        mockedLang.mockReturnValue("ko");
        const user = userEvent.setup();
        render(_jsx(Step1Language, {}));
        await user.click(screen.getByRole("radio", { name: "onboarding.language.option.ko" }));
        expect(changeLanguageMock).not.toHaveBeenCalled();
        expect(send).not.toHaveBeenCalled();
    });
    it("계속할게요 클릭 시 NEXT 송신", async () => {
        const send = vi.fn();
        mockedSend.mockReturnValue(send);
        const user = userEvent.setup();
        render(_jsx(Step1Language, {}));
        await user.click(screen.getByRole("button", { name: "onboarding.actions.next" }));
        expect(send).toHaveBeenCalledWith({ type: "NEXT" });
    });
});
