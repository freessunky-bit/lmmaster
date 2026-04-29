import { jsx as _jsx } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// Step2Scan 컴포넌트 테스트 — substate별 렌더 분기. Phase 1A.4.d.2.
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("../context", () => ({
    useOnboardingEnv: vi.fn(),
    useOnboardingScanError: vi.fn(),
    useOnboardingScanSub: vi.fn(),
    useOnboardingSend: vi.fn(),
}));
vi.mock("react-i18next", () => ({
    useTranslation: () => ({
        t: (key, opts) => opts ? `${key}:${JSON.stringify(opts)}` : key,
        i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
    }),
}));
import { useOnboardingEnv, useOnboardingScanError, useOnboardingScanSub, useOnboardingSend, } from "../context";
import { Step2Scan } from "./Step2Scan";
const mockedSub = vi.mocked(useOnboardingScanSub);
const mockedEnv = vi.mocked(useOnboardingEnv);
const mockedScanError = vi.mocked(useOnboardingScanError);
const mockedSend = vi.mocked(useOnboardingSend);
const FAKE_ENV = {
    hardware: {
        os: { family: "windows", version: "11", arch: "x86_64", kernel: "10" },
        cpu: { brand: "Intel", vendor_id: "GI", physical_cores: 8, logical_cores: 16, frequency_mhz: 3000 },
        mem: { total_bytes: 16 * 1024 ** 3, available_bytes: 8 * 1024 ** 3 },
        disks: [{ mount_point: "C:", kind: "ssd", total_bytes: 500e9, available_bytes: 250e9 }],
        gpus: [{ vendor: "nvidia", name: "RTX 4080", vram_bytes: 16 * 1024 ** 3 }],
        runtimes: {},
        probed_at: "2026-04-27T00:00:00Z",
        probe_ms: 100,
    },
    runtimes: [
        { runtime: "ollama", status: "running", version: "0.3.0" },
        { runtime: "lm-studio", status: "not-installed" },
    ],
};
beforeEach(() => {
    mockedSub.mockReset();
    mockedEnv.mockReset();
    mockedScanError.mockReset();
    mockedSend.mockReset();
    mockedSend.mockReturnValue(vi.fn());
});
afterEach(() => {
    vi.clearAllMocks();
});
describe("Step2Scan", () => {
    it("running 상태 — skeleton 렌더 + NEXT 비활성", () => {
        mockedSub.mockReturnValue("running");
        mockedEnv.mockReturnValue(undefined);
        render(_jsx(Step2Scan, {}));
        expect(screen.getByRole("button", { name: "onboarding.actions.next" })).toBeDisabled();
        // aria-busy로 skeleton 영역 표시.
        expect(document.querySelector('[aria-busy="true"]')).not.toBeNull();
    });
    it("done 상태 + env — 4 카드 + NEXT 활성", () => {
        mockedSub.mockReturnValue("done");
        mockedEnv.mockReturnValue(FAKE_ENV);
        render(_jsx(Step2Scan, {}));
        expect(screen.getByRole("button", { name: "onboarding.actions.next" })).not.toBeDisabled();
        expect(screen.getByText("onboarding.scan.card.os")).toBeInTheDocument();
        expect(screen.getByText("onboarding.scan.card.memory")).toBeInTheDocument();
        expect(screen.getByText("onboarding.scan.card.gpu")).toBeInTheDocument();
        expect(screen.getByText("onboarding.scan.card.runtimes")).toBeInTheDocument();
    });
    it("failed 상태 — 에러 카드 + RETRY 버튼", async () => {
        mockedSub.mockReturnValue("failed");
        mockedScanError.mockReturnValue("network down");
        const send = vi.fn();
        mockedSend.mockReturnValue(send);
        const user = userEvent.setup();
        render(_jsx(Step2Scan, {}));
        expect(screen.getByRole("alert")).toBeInTheDocument();
        expect(screen.getByText("network down")).toBeInTheDocument();
        await user.click(screen.getByRole("button", { name: "onboarding.error.retry" }));
        expect(send).toHaveBeenCalledWith({ type: "RETRY" });
    });
    it("BACK 버튼 클릭 시 BACK 송신", async () => {
        mockedSub.mockReturnValue("done");
        mockedEnv.mockReturnValue(FAKE_ENV);
        const send = vi.fn();
        mockedSend.mockReturnValue(send);
        const user = userEvent.setup();
        render(_jsx(Step2Scan, {}));
        await user.click(screen.getByRole("button", { name: "onboarding.actions.back" }));
        expect(send).toHaveBeenCalledWith({ type: "BACK" });
    });
});
