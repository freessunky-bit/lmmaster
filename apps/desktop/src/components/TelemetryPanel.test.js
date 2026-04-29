import { jsx as _jsx } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// TelemetryPanel — Phase 7'.a opt-in 토글 단위 테스트.
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("react-i18next", () => ({
    useTranslation: () => ({
        t: (key) => key,
        i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
    }),
}));
vi.mock("../ipc/telemetry", () => ({
    getTelemetryConfig: vi.fn(),
    setTelemetryEnabled: vi.fn(),
}));
import { getTelemetryConfig, setTelemetryEnabled, } from "../ipc/telemetry";
import { TelemetryPanel } from "./TelemetryPanel";
const DEFAULT_OFF = {
    enabled: false,
    anon_id: null,
    opted_in_at: null,
};
const ENABLED_WITH_ID = {
    enabled: true,
    anon_id: "12345678-aaaa-bbbb-cccc-1234567890ab",
    opted_in_at: "2026-04-28T01:23:45Z",
};
beforeEach(() => {
    vi.mocked(getTelemetryConfig).mockResolvedValue(DEFAULT_OFF);
    vi.mocked(setTelemetryEnabled).mockResolvedValue(ENABLED_WITH_ID);
});
afterEach(() => {
    vi.clearAllMocks();
});
describe("TelemetryPanel — 기본 렌더", () => {
    it("기본 비활성 상태로 렌더돼요 (aria-checked=false + statusOff)", async () => {
        render(_jsx(TelemetryPanel, {}));
        await waitFor(() => {
            expect(getTelemetryConfig).toHaveBeenCalled();
        });
        const toggle = screen.getByTestId("telemetry-toggle");
        await waitFor(() => {
            expect(toggle.getAttribute("aria-checked")).toBe("false");
        });
        expect(screen.getByText("screens.settings.telemetry.statusOff")).toBeTruthy();
    });
    it("description / privacyNote i18n 키가 노출돼요", async () => {
        render(_jsx(TelemetryPanel, {}));
        await waitFor(() => {
            expect(getTelemetryConfig).toHaveBeenCalled();
        });
        expect(screen.getByText("screens.settings.telemetry.description")).toBeTruthy();
        expect(screen.getByText("screens.settings.telemetry.privacyNote")).toBeTruthy();
    });
    it("토글 켜면 setTelemetryEnabled(true) 호출 + UUID hint 노출", async () => {
        const user = userEvent.setup();
        render(_jsx(TelemetryPanel, {}));
        await waitFor(() => {
            expect(getTelemetryConfig).toHaveBeenCalled();
        });
        const toggle = screen.getByTestId("telemetry-toggle");
        await user.click(toggle);
        await waitFor(() => {
            expect(setTelemetryEnabled).toHaveBeenCalledWith(true);
        });
        await waitFor(() => {
            expect(toggle.getAttribute("aria-checked")).toBe("true");
        });
        expect(screen.getByTestId("telemetry-anon-id-hint")).toBeTruthy();
        expect(screen.getByText("screens.settings.telemetry.statusOn")).toBeTruthy();
    });
    it("UUID는 8자만 표시해요 (전체 노출 X)", async () => {
        vi.mocked(getTelemetryConfig).mockResolvedValueOnce(ENABLED_WITH_ID);
        render(_jsx(TelemetryPanel, {}));
        await waitFor(() => {
            expect(screen.getByTestId("telemetry-anon-id-hint")).toBeTruthy();
        });
        const hint = screen.getByTestId("telemetry-anon-id-hint");
        expect(hint.textContent ?? "").toContain("12345678…");
        expect(hint.textContent ?? "").not.toContain("12345678-aaaa-bbbb-cccc-1234567890ab");
    });
    it("토글 실패 → revert + 한국어 에러 키 노출", async () => {
        const user = userEvent.setup();
        vi.mocked(setTelemetryEnabled).mockRejectedValueOnce(new Error("ipc fail"));
        const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => { });
        render(_jsx(TelemetryPanel, {}));
        await waitFor(() => {
            expect(getTelemetryConfig).toHaveBeenCalled();
        });
        const toggle = screen.getByTestId("telemetry-toggle");
        await user.click(toggle);
        await waitFor(() => {
            expect(setTelemetryEnabled).toHaveBeenCalled();
        });
        // revert: aria-checked는 다시 false.
        await waitFor(() => {
            expect(toggle.getAttribute("aria-checked")).toBe("false");
        });
        await waitFor(() => {
            expect(screen.getByText("screens.settings.telemetry.errors.toggleFailed")).toBeTruthy();
        });
        warnSpy.mockRestore();
    });
    it("초기 로드 실패 → loadFailed 에러 키 노출", async () => {
        vi.mocked(getTelemetryConfig).mockRejectedValueOnce(new Error("fail"));
        const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => { });
        render(_jsx(TelemetryPanel, {}));
        await waitFor(() => {
            expect(screen.getByText("screens.settings.telemetry.errors.loadFailed")).toBeTruthy();
        });
        warnSpy.mockRestore();
    });
});
describe("TelemetryPanel — a11y", () => {
    it("axe violations === [] (기본 OFF)", async () => {
        const { container } = render(_jsx(TelemetryPanel, {}));
        await waitFor(() => {
            expect(screen.getByTestId("telemetry-panel")).toBeTruthy();
        });
        await waitFor(() => {
            expect(getTelemetryConfig).toHaveBeenCalled();
        });
        const results = await axe.run(container, {
            rules: { region: { enabled: false } },
        });
        expect(results.violations).toEqual([]);
    });
    it("axe violations === [] (ON 상태)", async () => {
        vi.mocked(getTelemetryConfig).mockResolvedValueOnce(ENABLED_WITH_ID);
        const { container } = render(_jsx(TelemetryPanel, {}));
        await waitFor(() => {
            expect(screen.getByTestId("telemetry-anon-id-hint")).toBeTruthy();
        });
        const results = await axe.run(container, {
            rules: { region: { enabled: false } },
        });
        expect(results.violations).toEqual([]);
    });
});
