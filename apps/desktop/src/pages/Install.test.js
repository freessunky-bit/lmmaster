import { jsx as _jsx } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// Install 페이지 — Phase 4.b 테스트.
//
// 검증 (CLAUDE.md §4.4 + 결정 노트 §4):
// 1) 두 카드 (Ollama + LM Studio) 렌더 + alias / license / reason 표시.
// 2) 카드 클릭 → drawer 열림 (role="dialog") + Esc 닫힘.
// 3) 두 런타임 모두 설치 → 빈 상태 + 카탈로그 CTA.
// 4) 설치 진행 중 → InstallProgress compact + 취소 버튼.
// 5) a11y: vitest-axe violations.toEqual([])  (color-contrast, html-has-lang, region 비활성).
// 6) scoped 쿼리 — within(scope).getByText 사용.
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
vi.mock("../ipc/environment", async () => {
    const actual = await vi.importActual("../ipc/environment");
    return {
        ...actual,
        detectEnvironment: vi.fn(),
    };
});
vi.mock("../ipc/install", () => ({
    installApp: vi.fn(),
    cancelInstall: vi.fn().mockResolvedValue(undefined),
}));
import { detectEnvironment } from "../ipc/environment";
import { cancelInstall, installApp } from "../ipc/install";
import { InstallPage } from "./Install";
const FAKE_HW = {
    os: { family: "windows", version: "11", arch: "x86_64", kernel: "10" },
    cpu: {
        brand: "Intel",
        vendor_id: "GI",
        physical_cores: 8,
        logical_cores: 16,
        frequency_mhz: 3000,
    },
    mem: { total_bytes: 16 * 1024 ** 3, available_bytes: 8 * 1024 ** 3 },
    disks: [
        {
            mount_point: "C:",
            kind: "ssd",
            total_bytes: 500e9,
            available_bytes: 250e9,
        },
    ],
    gpus: [],
    runtimes: {},
    probed_at: "2026-04-27T00:00:00Z",
    probe_ms: 100,
};
function makeEnv(ollama, lmStudio) {
    return {
        hardware: FAKE_HW,
        runtimes: [
            { runtime: "ollama", status: ollama },
            { runtime: "lm-studio", status: lmStudio },
        ],
    };
}
const AXE_OPTIONS = {
    rules: {
        "color-contrast": { enabled: false },
        "html-has-lang": { enabled: false },
        "landmark-one-main": { enabled: false },
        region: { enabled: false },
    },
};
beforeEach(() => {
    vi.mocked(detectEnvironment).mockResolvedValue(makeEnv("not-installed", "not-installed"));
    vi.mocked(installApp).mockImplementation(() => new Promise(() => { }));
    vi.mocked(cancelInstall).mockResolvedValue(undefined);
});
afterEach(() => {
    vi.clearAllMocks();
});
describe("InstallPage 렌더", () => {
    it("두 카드 (Ollama + LM Studio) 렌더 + alias / license / reason 표시", async () => {
        const { container } = render(_jsx(InstallPage, {}));
        // Ollama 카드.
        await waitFor(() => {
            const cards = container.querySelectorAll(".install-card");
            expect(cards.length).toBe(2);
        });
        const cards = container.querySelectorAll(".install-card");
        const ollamaCard = cards[0];
        const lmCard = cards[1];
        // 별칭 (alias) — i18n key 미러.
        expect(within(ollamaCard).getByText("screens.install.cards.ollama.name")).toBeTruthy();
        expect(within(ollamaCard).getByText("screens.install.cards.ollama.license")).toBeTruthy();
        expect(within(ollamaCard).getByText("screens.install.cards.ollama.reason")).toBeTruthy();
        // LM Studio.
        expect(within(lmCard).getByText("screens.install.cards.lmStudio.name")).toBeTruthy();
        expect(within(lmCard).getByText("screens.install.cards.lmStudio.license")).toBeTruthy();
        expect(within(lmCard).getByText("screens.install.cards.lmStudio.reason")).toBeTruthy();
    });
    it("not-installed 카드는 '받을게요' 버튼 노출", async () => {
        render(_jsx(InstallPage, {}));
        await waitFor(() => {
            // 카드는 role=button, footer에도 button — 모두 install button이 두 개 (카드당 1).
            const installButtons = screen.getAllByRole("button", {
                name: /screens\.install\.actions\.install/,
            });
            expect(installButtons.length).toBe(2);
        });
    });
});
describe("InstallPage drawer", () => {
    it("카드 details 클릭 → drawer 열림 (role='dialog') + Esc 닫힘", async () => {
        const user = userEvent.setup();
        const { container } = render(_jsx(InstallPage, {}));
        await waitFor(() => {
            expect(container.querySelectorAll(".install-card").length).toBe(2);
        });
        const cards = container.querySelectorAll(".install-card");
        const detailsBtn = within(cards[0]).getByRole("button", {
            name: /screens\.install\.actions\.details/,
        });
        await user.click(detailsBtn);
        await waitFor(() => {
            expect(screen.getByRole("dialog")).toBeTruthy();
        });
        // Esc로 닫기.
        await user.keyboard("{Escape}");
        await waitFor(() => {
            expect(screen.queryByRole("dialog")).toBeNull();
        });
    });
    it("drawer에 license / install size / homepage 노출", async () => {
        const user = userEvent.setup();
        const { container } = render(_jsx(InstallPage, {}));
        await waitFor(() => {
            expect(container.querySelectorAll(".install-card").length).toBe(2);
        });
        const cards = container.querySelectorAll(".install-card");
        const detailsBtn = within(cards[0]).getByRole("button", {
            name: /screens\.install\.actions\.details/,
        });
        await user.click(detailsBtn);
        await waitFor(() => {
            const dialog = screen.getByRole("dialog");
            expect(within(dialog).getByText("screens.install.drawer.licenseFull")).toBeTruthy();
            expect(within(dialog).getByText("screens.install.drawer.installSize")).toBeTruthy();
            expect(within(dialog).getByText("screens.install.drawer.homepage")).toBeTruthy();
        });
    });
});
describe("InstallPage 빈 상태", () => {
    it("두 런타임 모두 running일 때 빈 상태 + CTA", async () => {
        vi.mocked(detectEnvironment).mockResolvedValue(makeEnv("running", "running"));
        const onNavigate = vi.fn();
        render(_jsx(InstallPage, { onNavigate: onNavigate }));
        await waitFor(() => {
            expect(screen.getByText("screens.install.empty.title")).toBeTruthy();
        });
        const cta = screen.getByRole("button", {
            name: /screens\.install\.empty\.cta/,
        });
        const user = userEvent.setup();
        await user.click(cta);
        expect(onNavigate).toHaveBeenCalledWith("catalog");
    });
    it("한 런타임만 installed면 빈 상태 안 나옴", async () => {
        vi.mocked(detectEnvironment).mockResolvedValue(makeEnv("installed", "not-installed"));
        render(_jsx(InstallPage, {}));
        await waitFor(() => {
            expect(screen.queryByText("screens.install.empty.title")).toBeNull();
        });
    });
});
describe("InstallPage 진행 패널", () => {
    it("설치 시작 시 InstallProgress compact + 취소 버튼 노출", async () => {
        // installApp는 onEvent로 download:progress 이벤트 emit, 그 후 영원히 pending.
        let emitter = null;
        vi.mocked(installApp).mockImplementation((_id, options) => {
            emitter = options.onEvent;
            return new Promise(() => { });
        });
        const user = userEvent.setup();
        const { container } = render(_jsx(InstallPage, {}));
        await waitFor(() => {
            expect(container.querySelectorAll(".install-card").length).toBe(2);
        });
        // 첫 카드의 footer install 버튼 클릭.
        const cards = container.querySelectorAll(".install-card");
        const firstCardInstall = within(cards[0]).getAllByRole("button", { name: /screens\.install\.actions\.install/ });
        await user.click(firstCardInstall[0]);
        // download progress emit.
        emitter({
            kind: "download",
            download: {
                kind: "progress",
                downloaded: 200,
                total: 1000,
                speed_bps: 100 * 1024,
            },
        });
        // 진행 패널이 노출되고 취소 버튼이 있어야 함.
        await waitFor(() => {
            const panel = container.querySelector(".install-progress-panel");
            expect(panel).toBeTruthy();
            const cancelBtn = within(panel).getByRole("button", {
                name: /onboarding\.install\.cancel/,
            });
            expect(cancelBtn).toBeTruthy();
        });
        // 취소 버튼 클릭 시 cancelInstall("ollama") 호출.
        const panel = container.querySelector(".install-progress-panel");
        const cancelBtn = within(panel).getByRole("button", {
            name: /onboarding\.install\.cancel/,
        });
        await user.click(cancelBtn);
        expect(cancelInstall).toHaveBeenCalledWith("ollama");
    });
});
describe("InstallPage a11y", () => {
    it("WCAG 위반 없음 (idle, 카드 두 개)", async () => {
        const { container } = render(_jsx(InstallPage, {}));
        await waitFor(() => {
            expect(container.querySelectorAll(".install-card").length).toBe(2);
        });
        const results = await axe(container, AXE_OPTIONS);
        expect(results.violations).toEqual([]);
    });
    it("WCAG 위반 없음 (drawer 열린 상태)", async () => {
        const user = userEvent.setup();
        const { container } = render(_jsx(InstallPage, {}));
        await waitFor(() => {
            expect(container.querySelectorAll(".install-card").length).toBe(2);
        });
        const cards = container.querySelectorAll(".install-card");
        const detailsBtn = within(cards[0]).getByRole("button", {
            name: /screens\.install\.actions\.details/,
        });
        await user.click(detailsBtn);
        await waitFor(() => {
            expect(screen.getByRole("dialog")).toBeTruthy();
        });
        const results = await axe(container, AXE_OPTIONS);
        expect(results.violations).toEqual([]);
    });
});
