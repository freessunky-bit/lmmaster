import { jsx as _jsx } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// Step3Install 컴포넌트 테스트 — substate별 분기. Phase 1A.4.d.2.
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("../context", () => ({
    useOnboardingEnv: vi.fn(),
    useOnboardingInstallError: vi.fn(),
    useOnboardingInstallLatest: vi.fn(),
    useOnboardingInstallLog: vi.fn(),
    useOnboardingInstallOutcome: vi.fn(),
    useOnboardingInstallProgress: vi.fn(),
    useOnboardingInstallSub: vi.fn(),
    useOnboardingModelId: vi.fn(),
    useOnboardingRetryAttempt: vi.fn(),
    useOnboardingSend: vi.fn(),
}));
vi.mock("react-i18next", () => ({
    useTranslation: () => ({
        t: (key, opts) => opts ? `${key}:${JSON.stringify(opts)}` : key,
        i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
    }),
}));
import * as ctx from "../context";
import { Step3Install } from "./Step3Install";
const mocks = {
    sub: vi.mocked(ctx.useOnboardingInstallSub),
    env: vi.mocked(ctx.useOnboardingEnv),
    error: vi.mocked(ctx.useOnboardingInstallError),
    latest: vi.mocked(ctx.useOnboardingInstallLatest),
    log: vi.mocked(ctx.useOnboardingInstallLog),
    outcome: vi.mocked(ctx.useOnboardingInstallOutcome),
    progress: vi.mocked(ctx.useOnboardingInstallProgress),
    modelId: vi.mocked(ctx.useOnboardingModelId),
    retryAttempt: vi.mocked(ctx.useOnboardingRetryAttempt),
    send: vi.mocked(ctx.useOnboardingSend),
};
const ENV_NO_RUNTIMES = {
    hardware: {
        os: { family: "windows", version: "11", arch: "x86_64", kernel: "10" },
        cpu: { brand: "Intel", vendor_id: "GI", physical_cores: 8, logical_cores: 16, frequency_mhz: 3000 },
        mem: { total_bytes: 16 * 1024 ** 3, available_bytes: 8 * 1024 ** 3 },
        disks: [],
        gpus: [],
        runtimes: {},
        probed_at: "",
        probe_ms: 0,
    },
    runtimes: [],
};
beforeEach(() => {
    for (const m of Object.values(mocks))
        m.mockReset();
    mocks.send.mockReturnValue(vi.fn());
    mocks.log.mockReturnValue([]);
});
afterEach(() => {
    vi.clearAllMocks();
});
describe("Step3Install", () => {
    it("idle 상태 — Ollama/LM Studio 두 카드 노출", () => {
        mocks.sub.mockReturnValue("idle");
        mocks.env.mockReturnValue(ENV_NO_RUNTIMES);
        render(_jsx(Step3Install, {}));
        expect(screen.getByText("Ollama")).toBeInTheDocument();
        expect(screen.getByText("LM Studio")).toBeInTheDocument();
    });
    it("idle — 받을게요 클릭 시 SELECT_MODEL 송신", async () => {
        mocks.sub.mockReturnValue("idle");
        mocks.env.mockReturnValue(ENV_NO_RUNTIMES);
        const send = vi.fn();
        mocks.send.mockReturnValue(send);
        const user = userEvent.setup();
        render(_jsx(Step3Install, {}));
        const installButtons = screen.getAllByRole("button", {
            name: "onboarding.install.cta.install",
        });
        await user.click(installButtons[0]);
        expect(send).toHaveBeenCalledWith({
            type: "SELECT_MODEL",
            id: "ollama",
        });
    });
    it("skip 상태 — 안내 banner 노출 (1.2초 후 자동 done은 머신 책임)", () => {
        mocks.sub.mockReturnValue("skip");
        render(_jsx(Step3Install, {}));
        expect(screen.getByRole("status")).toBeInTheDocument();
        expect(screen.getByRole("heading", { name: "onboarding.install.skip.title" })).toBeInTheDocument();
    });
    it("running 상태 — 진행률 + phase 라벨 + 그만두기 버튼", async () => {
        mocks.sub.mockReturnValue("running");
        mocks.modelId.mockReturnValue("ollama");
        mocks.progress.mockReturnValue({
            downloaded: 500,
            total: 1000,
            speed_bps: 1000 * 1024,
        });
        mocks.latest.mockReturnValue({
            kind: "download",
            download: {
                kind: "progress",
                downloaded: 500,
                total: 1000,
                speed_bps: 1000 * 1024,
            },
        });
        const send = vi.fn();
        mocks.send.mockReturnValue(send);
        const user = userEvent.setup();
        render(_jsx(Step3Install, {}));
        expect(screen.getByRole("progressbar", { name: "onboarding.install.progressAria" })).toBeInTheDocument();
        await user.click(screen.getByRole("button", { name: "onboarding.install.cancel" }));
        expect(send).toHaveBeenCalledWith({ type: "BACK" });
    });
    it("failed 상태 — 에러 메시지 + RETRY 버튼 (500ms debounce)", async () => {
        mocks.sub.mockReturnValue("failed");
        mocks.error.mockReturnValue({
            code: "download-failed",
            message: "network down",
        });
        const send = vi.fn();
        mocks.send.mockReturnValue(send);
        const user = userEvent.setup();
        render(_jsx(Step3Install, {}));
        expect(screen.getByRole("alert")).toBeInTheDocument();
        expect(screen.getByText("network down")).toBeInTheDocument();
        await user.click(screen.getByRole("button", { name: "onboarding.error.retry" }));
        expect(send).toHaveBeenCalledWith({ type: "RETRY" });
    });
    it("openedUrl 상태 — 안내 + NEXT 클릭 시 NEXT 송신 (Issue A 후속)", async () => {
        mocks.sub.mockReturnValue("openedUrl");
        mocks.outcome.mockReturnValue({
            kind: "opened-url",
            url: "https://lmstudio.ai/",
        });
        const send = vi.fn();
        mocks.send.mockReturnValue(send);
        const user = userEvent.setup();
        render(_jsx(Step3Install, {}));
        expect(screen.getByRole("heading", {
            name: "onboarding.install.openedUrl.title",
        })).toBeInTheDocument();
        await user.click(screen.getByRole("button", { name: "onboarding.actions.next" }));
        expect(send).toHaveBeenCalledWith({ type: "NEXT" });
    });
});
