import { jsx as _jsx } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// PortableImportPanel — Phase 11' (ADR-0039) 단위 테스트.
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("react-i18next", () => ({
    useTranslation: () => ({
        t: (key) => key,
        i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
    }),
}));
vi.mock("../../ipc/portable", () => ({
    startWorkspaceImport: vi.fn(),
    cancelWorkspaceImport: vi.fn(),
    verifyWorkspaceArchive: vi.fn(),
    isTerminalImportEvent: (ev) => ev.kind === "done" || ev.kind === "failed",
}));
import { cancelWorkspaceImport, startWorkspaceImport, verifyWorkspaceArchive, } from "../../ipc/portable";
import { PortableImportPanel } from "./PortableImportPanel";
const PREVIEW_META_ONLY = {
    manifest_summary: "워크스페이스 ws-1 (windows/x86_64) · 만든 시각 2026-04-28 · 런타임 0개 · 모델 0개",
    source_fingerprint: {
        os: "windows",
        arch: "x86_64",
        gpu_class: "nvidia",
        vram_bucket_mb: 16384,
        ram_bucket_mb: 65536,
        fingerprint_hash: "abcdef0123456789",
    },
    size_bytes: 4096,
    has_models: false,
    has_keys: false,
    entries_count: 5,
};
const PREVIEW_WITH_KEYS = {
    ...PREVIEW_META_ONLY,
    has_keys: true,
};
beforeEach(() => {
    vi.mocked(verifyWorkspaceArchive).mockResolvedValue(PREVIEW_META_ONLY);
    vi.mocked(startWorkspaceImport).mockResolvedValue({
        import_id: "imp-1",
        summary: {
            repair_tier: "green",
            source_fingerprint: PREVIEW_META_ONLY.source_fingerprint,
            manifest_summary: PREVIEW_META_ONLY.manifest_summary,
        },
    });
    vi.mocked(cancelWorkspaceImport).mockResolvedValue(undefined);
});
afterEach(() => {
    vi.clearAllMocks();
});
describe("PortableImportPanel — 기본 렌더", () => {
    it("idle 상태에서 시작 버튼이 활성화돼요", () => {
        render(_jsx(PortableImportPanel, {}));
        const btn = screen.getByTestId("portable-import-start-btn");
        expect(btn.getAttribute("disabled")).toBeNull();
    });
    it("시작 버튼 클릭하면 파일 선택 dialog가 열려요", async () => {
        const user = userEvent.setup();
        render(_jsx(PortableImportPanel, {}));
        await user.click(screen.getByTestId("portable-import-start-btn"));
        expect(screen.getByTestId("portable-import-modal")).toBeTruthy();
        expect(screen.getByTestId("portable-import-source-input")).toBeTruthy();
    });
});
describe("PortableImportPanel — verify + preview", () => {
    it("source 미입력 시 검증 시도하면 에러 키 노출", async () => {
        const user = userEvent.setup();
        render(_jsx(PortableImportPanel, {}));
        await user.click(screen.getByTestId("portable-import-start-btn"));
        await user.click(screen.getByTestId("portable-import-verify-btn"));
        expect(screen.getByText("screens.settings.portable.import.errors.emptySource")).toBeTruthy();
        expect(verifyWorkspaceArchive).not.toHaveBeenCalled();
    });
    it("정상 경로 입력 → verify → preview 노출", async () => {
        const user = userEvent.setup();
        render(_jsx(PortableImportPanel, {}));
        await user.click(screen.getByTestId("portable-import-start-btn"));
        await user.type(screen.getByTestId("portable-import-source-input"), "C:/tmp/in.zip");
        await user.click(screen.getByTestId("portable-import-verify-btn"));
        await waitFor(() => {
            expect(verifyWorkspaceArchive).toHaveBeenCalledWith("C:/tmp/in.zip");
        });
        await waitFor(() => {
            expect(screen.getByTestId("portable-import-preview")).toBeTruthy();
        });
        const preview = screen.getByTestId("portable-import-preview");
        expect(within(preview).getByText(/ws-1/)).toBeTruthy();
    });
    it("키 포함 archive → 패스프레이즈 입력 노출", async () => {
        vi.mocked(verifyWorkspaceArchive).mockResolvedValueOnce(PREVIEW_WITH_KEYS);
        const user = userEvent.setup();
        render(_jsx(PortableImportPanel, {}));
        await user.click(screen.getByTestId("portable-import-start-btn"));
        await user.type(screen.getByTestId("portable-import-source-input"), "C:/tmp/with-keys.zip");
        await user.click(screen.getByTestId("portable-import-verify-btn"));
        await waitFor(() => {
            expect(screen.getByTestId("portable-import-passphrase")).toBeTruthy();
        });
    });
});
describe("PortableImportPanel — 정책 + 진행", () => {
    it("conflict_policy 라디오 3개 모두 렌더돼요", async () => {
        const user = userEvent.setup();
        render(_jsx(PortableImportPanel, {}));
        await user.click(screen.getByTestId("portable-import-start-btn"));
        await user.type(screen.getByTestId("portable-import-source-input"), "C:/tmp/in.zip");
        await user.click(screen.getByTestId("portable-import-verify-btn"));
        await waitFor(() => {
            expect(screen.getByTestId("portable-import-policy-skip")).toBeTruthy();
        });
        expect(screen.getByTestId("portable-import-policy-overwrite")).toBeTruthy();
        expect(screen.getByTestId("portable-import-policy-rename")).toBeTruthy();
        // default = rename.
        const renameRadio = screen.getByTestId("portable-import-policy-rename");
        expect(renameRadio.checked).toBe(true);
    });
    it("정상 import → done 카드 + tier 노출", async () => {
        const user = userEvent.setup();
        render(_jsx(PortableImportPanel, {}));
        await user.click(screen.getByTestId("portable-import-start-btn"));
        await user.type(screen.getByTestId("portable-import-source-input"), "C:/tmp/in.zip");
        await user.click(screen.getByTestId("portable-import-verify-btn"));
        await waitFor(() => {
            expect(screen.getByTestId("portable-import-preview")).toBeTruthy();
        });
        await user.click(screen.getByTestId("portable-import-confirm-btn"));
        await waitFor(() => {
            expect(startWorkspaceImport).toHaveBeenCalled();
        });
        await waitFor(() => {
            expect(screen.getByTestId("portable-import-done")).toBeTruthy();
        });
        const tier = screen.getByTestId("portable-import-tier");
        expect(tier.textContent).toContain("screens.settings.portable.import.tier.green");
    });
    it("키 포함 + 패스프레이즈 빈 상태로 confirm → 에러", async () => {
        vi.mocked(verifyWorkspaceArchive).mockResolvedValueOnce(PREVIEW_WITH_KEYS);
        const user = userEvent.setup();
        render(_jsx(PortableImportPanel, {}));
        await user.click(screen.getByTestId("portable-import-start-btn"));
        await user.type(screen.getByTestId("portable-import-source-input"), "C:/tmp/with-keys.zip");
        await user.click(screen.getByTestId("portable-import-verify-btn"));
        await waitFor(() => {
            expect(screen.getByTestId("portable-import-passphrase")).toBeTruthy();
        });
        await user.click(screen.getByTestId("portable-import-confirm-btn"));
        expect(screen.getByText("screens.settings.portable.import.errors.emptyPassphrase")).toBeTruthy();
        expect(startWorkspaceImport).not.toHaveBeenCalled();
    });
    it("Esc 키로 preview dialog가 닫혀요", async () => {
        const user = userEvent.setup();
        render(_jsx(PortableImportPanel, {}));
        await user.click(screen.getByTestId("portable-import-start-btn"));
        expect(screen.getByTestId("portable-import-modal")).toBeTruthy();
        await user.keyboard("{Escape}");
        await waitFor(() => {
            expect(screen.queryByTestId("portable-import-modal")).toBeNull();
        });
    });
});
describe("PortableImportPanel — a11y", () => {
    it("axe violations === [] (idle)", async () => {
        const { container } = render(_jsx(PortableImportPanel, {}));
        const results = await axe.run(container, {
            rules: { region: { enabled: false } },
        });
        expect(results.violations).toEqual([]);
    });
    it("axe violations === [] (preview dialog 열린 상태)", async () => {
        const user = userEvent.setup();
        const { container } = render(_jsx(PortableImportPanel, {}));
        await user.click(screen.getByTestId("portable-import-start-btn"));
        const results = await axe.run(container, {
            rules: { region: { enabled: false } },
        });
        expect(results.violations).toEqual([]);
    });
});
