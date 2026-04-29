import { jsx as _jsx } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// CatalogRefreshPanel — Phase 1' integration UI 테스트.
// 정책 (CLAUDE.md §4.4):
// - IPC mock — backend 격리.
// - data-testid scoped 쿼리.
// - a11y — vitest-axe.
// - 한국어 키 stub (useTranslation은 키 그대로 반환).
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { axe } from "vitest-axe";
vi.mock("../ipc/catalog-refresh", () => ({
    refreshCatalogNow: vi.fn(),
    getLastCatalogRefresh: vi.fn(),
    onCatalogRefreshed: vi.fn(),
}));
vi.mock("react-i18next", () => ({
    useTranslation: () => ({
        t: (key, opts) => opts ? `${key}:${JSON.stringify(opts)}` : key,
        i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
    }),
}));
import * as ipc from "../ipc/catalog-refresh";
import { CatalogRefreshPanel } from "./CatalogRefreshPanel";
const refreshMock = vi.mocked(ipc.refreshCatalogNow);
const getLastMock = vi.mocked(ipc.getLastCatalogRefresh);
const onCatalogRefreshedMock = vi.mocked(ipc.onCatalogRefreshed);
const AXE_OPTIONS = {
    rules: {
        "color-contrast": { enabled: false },
        "html-has-lang": { enabled: false },
        "landmark-one-main": { enabled: false },
        region: { enabled: false },
    },
};
beforeEach(() => {
    refreshMock.mockReset();
    getLastMock.mockReset();
    onCatalogRefreshedMock.mockReset();
    // 기본: 한 번도 안 됐고, listener 등록은 noop unlisten 반환.
    getLastMock.mockResolvedValue(null);
    onCatalogRefreshedMock.mockResolvedValue(() => { });
});
afterEach(() => {
    vi.clearAllMocks();
});
describe("CatalogRefreshPanel", () => {
    it("첫 렌더 시 'never' 메시지 표시", async () => {
        render(_jsx(CatalogRefreshPanel, {}));
        await waitFor(() => {
            expect(screen.getByTestId("catalog-refresh-last")).toHaveTextContent("screens.settings.catalogRefresh.never");
        });
    });
    it("'지금 갱신할게요' 버튼 클릭 시 IPC 호출 + 성공 메시지", async () => {
        const user = userEvent.setup();
        refreshMock.mockResolvedValue({
            at_ms: Date.now(),
            fetched_count: 2,
            failed_count: 0,
            outcome: "ok",
        });
        render(_jsx(CatalogRefreshPanel, {}));
        const btn = await screen.findByTestId("catalog-refresh-now-btn");
        await user.click(btn);
        await waitFor(() => {
            expect(refreshMock).toHaveBeenCalledTimes(1);
        });
        // last 텍스트가 'never'가 아니라 lastRefresh 키로 변경됐는지.
        await waitFor(() => {
            expect(screen.getByTestId("catalog-refresh-last")).toHaveTextContent("screens.settings.catalogRefresh.lastRefresh");
        });
    });
    it("partial outcome 시 경고 메시지 노출", async () => {
        refreshMock.mockResolvedValue({
            at_ms: Date.now(),
            fetched_count: 1,
            failed_count: 1,
            outcome: "partial",
        });
        const user = userEvent.setup();
        render(_jsx(CatalogRefreshPanel, {}));
        await user.click(await screen.findByTestId("catalog-refresh-now-btn"));
        await waitFor(() => {
            expect(screen.getByText(/screens\.settings\.catalogRefresh\.partial/)).toBeInTheDocument();
        });
    });
    it("failed outcome 시 실패 메시지 노출", async () => {
        refreshMock.mockResolvedValue({
            at_ms: Date.now(),
            fetched_count: 0,
            failed_count: 3,
            outcome: "failed",
        });
        const user = userEvent.setup();
        render(_jsx(CatalogRefreshPanel, {}));
        await user.click(await screen.findByTestId("catalog-refresh-now-btn"));
        await waitFor(() => {
            expect(screen.getByText(/screens\.settings\.catalogRefresh\.failed/)).toBeInTheDocument();
        });
    });
    it("IPC 실패 시 error 메시지 + alert role", async () => {
        refreshMock.mockRejectedValue({ kind: "internal", message: "boom" });
        const user = userEvent.setup();
        render(_jsx(CatalogRefreshPanel, {}));
        await user.click(await screen.findByTestId("catalog-refresh-now-btn"));
        await waitFor(() => {
            const alert = screen.getByRole("alert");
            expect(alert).toHaveTextContent("screens.settings.catalogRefresh.errorRefresh");
        });
    });
    it("기존 listener가 새 LastRefresh를 emit하면 UI에 반영", async () => {
        let callback = null;
        onCatalogRefreshedMock.mockImplementation(async (cb) => {
            callback = cb;
            return () => { };
        });
        render(_jsx(CatalogRefreshPanel, {}));
        await waitFor(() => {
            expect(onCatalogRefreshedMock).toHaveBeenCalled();
        });
        expect(callback).toBeTruthy();
        callback({
            at_ms: 1234567890,
            fetched_count: 5,
            failed_count: 0,
            outcome: "ok",
        });
        await waitFor(() => {
            expect(screen.getByTestId("catalog-refresh-last")).toHaveTextContent("lastRefresh");
        });
    });
    it("a11y violations 없음", async () => {
        const { container } = render(_jsx(CatalogRefreshPanel, {}));
        await waitFor(() => {
            expect(screen.getByTestId("catalog-refresh-panel")).toBeInTheDocument();
        });
        const results = await axe(container, AXE_OPTIONS);
        expect(results.violations).toEqual([]);
    });
});
