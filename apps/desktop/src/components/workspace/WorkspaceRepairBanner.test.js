import { jsx as _jsx } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// WorkspaceRepairBanner — green/yellow/red 분기 + repair confirm 플로우.
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
vi.mock("react-i18next", () => ({
    useTranslation: () => ({
        t: (key, opts) => opts ? `${key}:${JSON.stringify(opts)}` : key,
        i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
    }),
}));
vi.mock("../../ipc/workspace", () => ({
    getWorkspaceFingerprint: vi.fn(),
    checkWorkspaceRepair: vi.fn(),
}));
import { checkWorkspaceRepair, getWorkspaceFingerprint, } from "../../ipc/workspace";
import { WorkspaceRepairBanner } from "./WorkspaceRepairBanner";
function makeStatus(tier, prevOs = "windows") {
    return {
        fingerprint: {
            os: tier === "red" ? "macos" : "windows",
            arch: "x86_64",
            gpu_class: "nvidia",
            vram_bucket_mb: 16384,
            ram_bucket_mb: 65536,
            fingerprint_hash: "abc1234567890def",
        },
        previous: tier === "green" ? null : {
            os: prevOs,
            arch: "x86_64",
            gpu_class: "nvidia",
            vram_bucket_mb: 16384,
            ram_bucket_mb: 65536,
            fingerprint_hash: "0000000000000000",
        },
        tier,
        workspace_root: "/tmp/workspace",
    };
}
afterEach(() => {
    vi.clearAllMocks();
});
describe("WorkspaceRepairBanner", () => {
    it("green tier — banner unmount (아무것도 표시 안 함)", async () => {
        vi.mocked(getWorkspaceFingerprint).mockResolvedValueOnce(makeStatus("green"));
        const { container } = render(_jsx(WorkspaceRepairBanner, {}));
        await waitFor(() => {
            expect(getWorkspaceFingerprint).toHaveBeenCalled();
        });
        // green이라 banner 자체가 없음.
        expect(container.querySelector(".ws-banner")).toBeNull();
        expect(container.querySelector(".ws-modal")).toBeNull();
    });
    it("yellow tier — toast 표시 + applyYellow 클릭 시 repair 호출", async () => {
        const user = userEvent.setup();
        vi.mocked(getWorkspaceFingerprint).mockResolvedValueOnce(makeStatus("yellow"));
        vi.mocked(checkWorkspaceRepair).mockResolvedValueOnce({
            tier: "yellow",
            invalidated_caches: ["bench", "scan"],
            invalidated_runtimes: 0,
            models_preserved: 3,
        });
        render(_jsx(WorkspaceRepairBanner, {}));
        await waitFor(() => {
            expect(screen.getByTestId("ws-banner-yellow")).toBeTruthy();
        });
        await user.click(screen.getByRole("button", { name: /workspace\.repair\.applyYellow/ }));
        await waitFor(() => {
            expect(checkWorkspaceRepair).toHaveBeenCalled();
        });
        // 적용 후 success banner 표시.
        await waitFor(() => {
            expect(screen.getByText(/workspace\.repair\.applied/)).toBeTruthy();
        });
    });
    it("yellow tier — '나중에 할게요' 클릭 시 banner dismiss", async () => {
        const user = userEvent.setup();
        vi.mocked(getWorkspaceFingerprint).mockResolvedValueOnce(makeStatus("yellow"));
        const { container } = render(_jsx(WorkspaceRepairBanner, {}));
        await waitFor(() => {
            expect(screen.getByTestId("ws-banner-yellow")).toBeTruthy();
        });
        await user.click(screen.getByRole("button", { name: /workspace\.repair\.later/ }));
        await waitFor(() => {
            expect(container.querySelector("[data-testid='ws-banner-yellow']")).toBeNull();
        });
        expect(checkWorkspaceRepair).not.toHaveBeenCalled();
    });
    it("red tier — modal 표시 + 이전/현재 OS 정보 노출", async () => {
        vi.mocked(getWorkspaceFingerprint).mockResolvedValueOnce(makeStatus("red", "windows"));
        render(_jsx(WorkspaceRepairBanner, {}));
        await waitFor(() => {
            expect(screen.getByTestId("ws-modal-red")).toBeTruthy();
        });
        expect(screen.getByText(/workspace\.repair\.redTitle/)).toBeTruthy();
        // redDetail은 prev/current OS 인자 포함.
        expect(screen.getByText(/redDetail/)).toBeTruthy();
    });
    it("red tier — applyRed 클릭 시 repair 호출", async () => {
        const user = userEvent.setup();
        vi.mocked(getWorkspaceFingerprint).mockResolvedValueOnce(makeStatus("red"));
        vi.mocked(checkWorkspaceRepair).mockResolvedValueOnce({
            tier: "red",
            invalidated_caches: ["bench", "scan"],
            invalidated_runtimes: 1,
            models_preserved: 3,
        });
        render(_jsx(WorkspaceRepairBanner, {}));
        await waitFor(() => expect(screen.getByTestId("ws-modal-red")).toBeTruthy());
        await user.click(screen.getByRole("button", { name: /workspace\.repair\.applyRed/ }));
        await waitFor(() => {
            expect(checkWorkspaceRepair).toHaveBeenCalled();
        });
    });
    it("getWorkspaceFingerprint 실패 시 silent (banner 표시 안 함)", async () => {
        const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => { });
        vi.mocked(getWorkspaceFingerprint).mockRejectedValueOnce(new Error("offline"));
        const { container } = render(_jsx(WorkspaceRepairBanner, {}));
        await waitFor(() => {
            expect(getWorkspaceFingerprint).toHaveBeenCalled();
        });
        expect(container.querySelector(".ws-banner")).toBeNull();
        expect(container.querySelector(".ws-modal")).toBeNull();
        warnSpy.mockRestore();
    });
    it("checkWorkspaceRepair 실패는 silent (UI 망가짐 없음)", async () => {
        const user = userEvent.setup();
        const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => { });
        vi.mocked(getWorkspaceFingerprint).mockResolvedValueOnce(makeStatus("yellow"));
        vi.mocked(checkWorkspaceRepair).mockRejectedValueOnce(new Error("disk full"));
        render(_jsx(WorkspaceRepairBanner, {}));
        await waitFor(() => {
            expect(screen.getByTestId("ws-banner-yellow")).toBeTruthy();
        });
        await user.click(screen.getByRole("button", { name: /workspace\.repair\.applyYellow/ }));
        await waitFor(() => {
            expect(checkWorkspaceRepair).toHaveBeenCalled();
        });
        // banner는 여전히 보임 (사용자가 다시 시도 가능).
        expect(screen.getByTestId("ws-banner-yellow")).toBeTruthy();
        warnSpy.mockRestore();
    });
});
