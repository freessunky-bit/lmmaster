import { jsx as _jsx } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// Diagnostics 4-grid 렌더 + IPC mock + 새 측정 navigation event + a11y.
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { axe } from "vitest-axe";
vi.mock("react-i18next", () => ({
    useTranslation: () => ({
        t: (key, opts) => opts ? `${key}:${JSON.stringify(opts)}` : key,
        i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
    }),
}));
vi.mock("../ipc/scanner", () => ({
    getLastScan: vi.fn(),
    startScan: vi.fn(),
    onScanSummary: vi.fn().mockResolvedValue(() => { }),
}));
vi.mock("../ipc/gateway", () => ({
    getGatewayStatus: vi.fn(),
    onGatewayReady: vi.fn().mockResolvedValue(() => { }),
    onGatewayFailed: vi.fn().mockResolvedValue(() => { }),
}));
vi.mock("../ipc/workspace", () => ({
    getWorkspaceFingerprint: vi.fn(),
    checkWorkspaceRepair: vi.fn(),
}));
import { getLastScan, startScan, } from "../ipc/scanner";
import { getGatewayStatus } from "../ipc/gateway";
import { getWorkspaceFingerprint, } from "../ipc/workspace";
import { Diagnostics } from "./Diagnostics";
const FAKE_SCAN = {
    started_at: "2026-04-27T00:00:00Z",
    checks: [
        {
            id: "os-check",
            severity: "info",
            title_ko: "운영체제 확인",
            detail_ko: "Windows 11이 정상적으로 동작해요",
        },
        {
            id: "ram-check",
            severity: "warn",
            title_ko: "메모리 주의",
            detail_ko: "8GB 미만이라 큰 모델은 어려울 수 있어요",
        },
    ],
    summary_korean: "이 PC는 가벼운 모델 위주로 잘 돌아가요. 메모리만 주의해 주세요.",
    summary_source: "deterministic",
    took_ms: 1200,
};
const FAKE_GW = {
    port: 7373,
    status: "listening",
    error: null,
};
const FAKE_WS = {
    fingerprint: {
        os: "windows",
        arch: "x86_64",
        gpu_class: "nvidia",
        vram_bucket_mb: 16384,
        ram_bucket_mb: 16384,
        fingerprint_hash: "abc123",
    },
    previous: null,
    tier: "green",
    workspace_root: "C:/users/test/lmmaster",
};
const AXE_OPTIONS = {
    rules: {
        "color-contrast": { enabled: false },
        "html-has-lang": { enabled: false },
        "landmark-one-main": { enabled: false },
        region: { enabled: false },
    },
};
beforeEach(() => {
    vi.mocked(getLastScan).mockResolvedValue(FAKE_SCAN);
    vi.mocked(startScan).mockResolvedValue(FAKE_SCAN);
    vi.mocked(getGatewayStatus).mockResolvedValue(FAKE_GW);
    vi.mocked(getWorkspaceFingerprint).mockResolvedValue(FAKE_WS);
});
afterEach(() => {
    vi.clearAllMocks();
});
describe("Diagnostics 4-grid 렌더", () => {
    it("4 섹션 모두 렌더", async () => {
        render(_jsx(Diagnostics, {}));
        await waitFor(() => {
            expect(screen.getByTestId("diag-section-scan")).toBeTruthy();
            expect(screen.getByTestId("diag-section-gateway")).toBeTruthy();
            expect(screen.getByTestId("diag-section-bench")).toBeTruthy();
            expect(screen.getByTestId("diag-section-workspace")).toBeTruthy();
        });
    });
    it("scanner mock — getLastScan resolves with summary → 좌상에 텍스트 표시", async () => {
        render(_jsx(Diagnostics, {}));
        await waitFor(() => {
            const scanSection = screen.getByTestId("diag-section-scan");
            expect(within(scanSection).getByText(FAKE_SCAN.summary_korean)).toBeTruthy();
        });
    });
    it("scanner null → 좌상에 빈 상태 메시지 표시", async () => {
        vi.mocked(getLastScan).mockResolvedValueOnce(null);
        render(_jsx(Diagnostics, {}));
        await waitFor(() => {
            const scanSection = screen.getByTestId("diag-section-scan");
            expect(within(scanSection).getByText("screens.diagnostics.sections.scan.empty")).toBeTruthy();
        });
    });
    it("gateway StatusPill 표시 (status + port detail)", async () => {
        render(_jsx(Diagnostics, {}));
        await waitFor(() => {
            const gwSection = screen.getByTestId("diag-section-gateway");
            // StatusPill 안의 status 라벨 키.
            expect(within(gwSection).getByText("gateway.status.listening")).toBeTruthy();
            // port detail.
            expect(within(gwSection).getByText(":7373")).toBeTruthy();
        });
    });
    it("workspace fingerprint mock → 우하 섹션에 os/arch 표시", async () => {
        render(_jsx(Diagnostics, {}));
        await waitFor(() => {
            const wsSection = screen.getByTestId("diag-section-workspace");
            expect(within(wsSection).getByText("windows")).toBeTruthy();
            expect(within(wsSection).getByText("x86_64")).toBeTruthy();
        });
    });
    it('"새 측정 시작" 버튼 클릭 → navigation custom event 발생', async () => {
        const user = userEvent.setup();
        const onNav = vi.fn();
        window.addEventListener("lmmaster:navigate", onNav);
        render(_jsx(Diagnostics, {}));
        await waitFor(() => {
            expect(screen.getByTestId("diag-bench-start-new")).toBeTruthy();
        });
        const btn = screen.getByTestId("diag-bench-start-new");
        await user.click(btn);
        expect(onNav).toHaveBeenCalledTimes(1);
        const evt = onNav.mock.calls[0][0];
        expect(evt.detail).toBe("catalog");
        window.removeEventListener("lmmaster:navigate", onNav);
    });
    it("종합 health pill role=status + green tier", async () => {
        render(_jsx(Diagnostics, {}));
        await waitFor(() => {
            const overall = screen.getByTestId("diag-overall-health");
            expect(overall.getAttribute("role")).toBe("status");
            expect(overall.getAttribute("aria-live")).toBe("polite");
            // FAKE_SCAN에 warn check가 있어서 yellow tier여야 함.
            expect(overall.className).toContain("diag-health-yellow");
        });
    });
    it("a11y violations 없음", async () => {
        const { container } = render(_jsx(Diagnostics, {}));
        await waitFor(() => {
            expect(screen.getByTestId("diag-section-scan")).toBeTruthy();
        });
        const results = await axe(container, AXE_OPTIONS);
        expect(results.violations).toEqual([]);
    });
});
