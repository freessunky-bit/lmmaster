import { jsx as _jsx } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// Projects 페이지 — 카드 그룹화 + drawer + revoke + a11y 테스트.
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import axe from "axe-core";
vi.mock("react-i18next", () => ({
    useTranslation: () => ({
        t: (key, opts) => opts ? `${key}:${JSON.stringify(opts)}` : key,
        i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
    }),
}));
vi.mock("../ipc/keys", () => ({
    listApiKeys: vi.fn(),
    revokeApiKey: vi.fn(),
}));
vi.mock("@lmmaster/design-system/react", () => ({
    StatusPill: ({ status, label }) => (_jsx("div", { "data-testid": "status-pill", "data-status": status, children: label })),
}));
import { listApiKeys, revokeApiKey } from "../ipc/keys";
import { Projects } from "./Projects";
function makeKey(id, alias, overrides = {}) {
    return {
        id,
        alias,
        key_prefix: `lm-${id.slice(0, 8)}`,
        scope: {
            models: ["*"],
            endpoints: ["/v1/*"],
            allowed_origins: ["https://blog.example.com"],
            expires_at: null,
            project_id: null,
            rate_limit: null,
        },
        created_at: "2026-04-27T00:00:00Z",
        last_used_at: null,
        revoked_at: null,
        ...overrides,
    };
}
beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(listApiKeys).mockResolvedValue([]);
});
describe("Projects 페이지", () => {
    it("같은 alias prefix를 가진 키들이 한 카드로 그룹화", async () => {
        vi.mocked(listApiKeys).mockResolvedValue([
            makeKey("a1", "블로그 메인", {
                scope: {
                    models: ["*"],
                    endpoints: ["/v1/*"],
                    allowed_origins: ["https://blog-main.com"],
                    expires_at: null,
                    project_id: null,
                    rate_limit: null,
                },
            }),
            makeKey("a2", "블로그 미러", {
                scope: {
                    models: ["exaone-*"],
                    endpoints: ["/v1/*"],
                    allowed_origins: ["https://blog-mirror.com"],
                    expires_at: null,
                    project_id: null,
                    rate_limit: null,
                },
            }),
            makeKey("b1", "쇼핑몰 메인", {
                scope: {
                    models: ["*"],
                    endpoints: ["/v1/*"],
                    allowed_origins: ["https://shop.com"],
                    expires_at: null,
                    project_id: null,
                    rate_limit: null,
                },
            }),
        ]);
        render(_jsx(Projects, {}));
        await waitFor(() => {
            const items = screen.getAllByRole("listitem");
            // 2개 그룹: "블로그", "쇼핑몰".
            expect(items.length).toBe(2);
        });
        expect(screen.getByText("블로그")).toBeTruthy();
        expect(screen.getByText("쇼핑몰")).toBeTruthy();
        // 활성 카운트 — 둘 다 활성.
        expect(screen.getByTestId("projects-active-count")).toHaveTextContent(/count.*2/);
    });
    it("빈 키 목록 — empty 상태 + CTA", async () => {
        vi.mocked(listApiKeys).mockResolvedValue([]);
        render(_jsx(Projects, {}));
        await waitFor(() => {
            expect(screen.getByText("screens.projects.empty.title")).toBeTruthy();
        });
        expect(screen.getByText("screens.projects.empty.body")).toBeTruthy();
        expect(screen.getByText("screens.projects.empty.cta")).toBeTruthy();
    });
    it("카드 클릭 → drawer 열림 + sparkline + top models 표시", async () => {
        const user = userEvent.setup();
        const fixture = [
            makeKey("a1", "블로그", {
                scope: {
                    models: ["exaone-*"],
                    endpoints: ["/v1/*"],
                    allowed_origins: ["https://x.com"],
                    expires_at: null,
                    project_id: null,
                    rate_limit: null,
                },
            }),
        ];
        vi.mocked(listApiKeys).mockResolvedValue(fixture);
        render(_jsx(Projects, {}));
        await waitFor(() => {
            expect(screen.getByText("블로그")).toBeTruthy();
        });
        const detailBtn = screen.getByRole("button", {
            name: "screens.projects.card.openDetail",
        });
        await user.click(detailBtn);
        // drawer 열림.
        await waitFor(() => {
            expect(screen.getByRole("dialog")).toBeTruthy();
        });
        expect(screen.getByTestId("projects-sparkline")).toBeTruthy();
        expect(screen.getByText("screens.projects.detail.topModels")).toBeTruthy();
    });
    it("revoke 버튼 — confirm 후 호출", async () => {
        const user = userEvent.setup();
        let revokedView = {};
        vi.mocked(listApiKeys).mockImplementation(async () => [
            makeKey("k1", "블로그", revokedView),
        ]);
        vi.mocked(revokeApiKey).mockImplementation(async () => {
            revokedView = { revoked_at: "2026-04-27T01:00:00Z" };
        });
        const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
        render(_jsx(Projects, {}));
        await waitFor(() => {
            expect(screen.getByText("블로그")).toBeTruthy();
        });
        // drawer 열기.
        await user.click(screen.getByRole("button", {
            name: "screens.projects.card.openDetail",
        }));
        // drawer 안의 revoke 버튼 click.
        const dialog = await screen.findByRole("dialog");
        const revokeBtn = within(dialog).getByRole("button", {
            name: "screens.projects.detail.revoke",
        });
        await user.click(revokeBtn);
        await waitFor(() => {
            expect(revokeApiKey).toHaveBeenCalledWith("k1");
        });
        confirmSpy.mockRestore();
    });
    it("revoked 키만 있는 그룹은 dim + StatusPill idle", async () => {
        vi.mocked(listApiKeys).mockResolvedValue([
            makeKey("r1", "블로그", { revoked_at: "2026-04-27T01:00:00Z" }),
        ]);
        render(_jsx(Projects, {}));
        await waitFor(() => {
            expect(screen.getByText("블로그")).toBeTruthy();
        });
        // 카드는 is-dim 클래스.
        const card = screen.getByRole("listitem");
        expect(card.className).toContain("is-dim");
        // StatusPill — idle 상태.
        const pill = within(card).getByTestId("status-pill");
        expect(pill).toHaveAttribute("data-status", "idle");
    });
    it("revoke confirm 거부 시 revoke 호출 안 함", async () => {
        const user = userEvent.setup();
        vi.mocked(listApiKeys).mockResolvedValue([makeKey("k1", "블로그")]);
        const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);
        render(_jsx(Projects, {}));
        await waitFor(() => {
            expect(screen.getByText("블로그")).toBeTruthy();
        });
        await user.click(screen.getByRole("button", {
            name: "screens.projects.card.openDetail",
        }));
        const dialog = await screen.findByRole("dialog");
        const revokeBtn = within(dialog).getByRole("button", {
            name: "screens.projects.detail.revoke",
        });
        await user.click(revokeBtn);
        expect(revokeApiKey).not.toHaveBeenCalled();
        confirmSpy.mockRestore();
    });
    it("Esc 누르면 drawer 닫힘", async () => {
        const user = userEvent.setup();
        vi.mocked(listApiKeys).mockResolvedValue([makeKey("k1", "블로그")]);
        render(_jsx(Projects, {}));
        await waitFor(() => expect(screen.getByText("블로그")).toBeTruthy());
        await user.click(screen.getByRole("button", {
            name: "screens.projects.card.openDetail",
        }));
        await waitFor(() => expect(screen.getByRole("dialog")).toBeTruthy());
        await user.keyboard("{Escape}");
        await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    });
    it("a11y — 위반 0건 (axe)", async () => {
        vi.mocked(listApiKeys).mockResolvedValue([
            makeKey("a1", "블로그"),
            makeKey("b1", "쇼핑몰", { revoked_at: "2026-04-27T01:00:00Z" }),
        ]);
        const { container } = render(_jsx(Projects, {}));
        await waitFor(() => {
            expect(screen.getByText("블로그")).toBeTruthy();
        });
        const results = await axe.run(container, {
            rules: {
                // jsdom region 룰은 main이 우리 책임 밖 (App shell 안에 placement 됨).
                region: { enabled: false },
            },
        });
        expect(results.violations).toEqual([]);
    });
});
