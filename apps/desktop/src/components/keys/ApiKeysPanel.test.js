import { jsx as _jsx } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// ApiKeysPanel + ApiKeyIssueModal 렌더 + 발급/회수 플로우 테스트.
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("react-i18next", () => ({
    useTranslation: () => ({
        t: (key, opts) => opts ? `${key}:${JSON.stringify(opts)}` : key,
        i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
    }),
}));
vi.mock("../../ipc/keys", () => ({
    listApiKeys: vi.fn(),
    revokeApiKey: vi.fn(),
    createApiKey: vi.fn(),
    defaultWebScope: (origin) => ({
        models: ["*"],
        endpoints: ["/v1/*"],
        allowed_origins: [origin],
        expires_at: null,
        project_id: null,
        rate_limit: null,
    }),
}));
import { createApiKey, listApiKeys, revokeApiKey, } from "../../ipc/keys";
import { ApiKeysPanel } from "./ApiKeysPanel";
function makeKey(id, alias, revoked = false) {
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
        revoked_at: revoked ? "2026-04-27T01:00:00Z" : null,
    };
}
beforeEach(() => {
    vi.mocked(listApiKeys).mockResolvedValue([]);
});
afterEach(() => {
    vi.clearAllMocks();
});
describe("ApiKeysPanel", () => {
    it("빈 목록 — empty 상태 메시지 노출", async () => {
        render(_jsx(ApiKeysPanel, {}));
        await waitFor(() => {
            expect(screen.getByText("keys.empty.title")).toBeTruthy();
        });
    });
    it("목록 있으면 테이블 렌더 + active/revoked 상태 표시", async () => {
        vi.mocked(listApiKeys).mockResolvedValueOnce([
            makeKey("active1", "blog"),
            makeKey("revoked1", "old", true),
        ]);
        render(_jsx(ApiKeysPanel, {}));
        await waitFor(() => {
            expect(screen.getByTestId("keys-table")).toBeTruthy();
            expect(screen.getByText("blog")).toBeTruthy();
            expect(screen.getByText("old")).toBeTruthy();
            expect(screen.getByText("keys.status.active")).toBeTruthy();
            expect(screen.getByText("keys.status.revoked")).toBeTruthy();
        });
    });
    it("revoked 키는 Revoke 버튼이 없다", async () => {
        vi.mocked(listApiKeys).mockResolvedValueOnce([
            makeKey("revoked1", "old", true),
        ]);
        render(_jsx(ApiKeysPanel, {}));
        await waitFor(() => {
            expect(screen.getByText("old")).toBeTruthy();
        });
        expect(screen.queryByText("keys.actions.revoke")).toBeNull();
    });
    it("Revoke 버튼 — confirm 후 호출", async () => {
        const user = userEvent.setup();
        vi.mocked(listApiKeys).mockResolvedValueOnce([makeKey("k1", "blog")]);
        vi.mocked(revokeApiKey).mockResolvedValueOnce(undefined);
        // 두 번째 list 호출 (revoke 후 refresh)을 위한 mock.
        vi.mocked(listApiKeys).mockResolvedValueOnce([makeKey("k1", "blog", true)]);
        const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
        render(_jsx(ApiKeysPanel, {}));
        await waitFor(() => {
            expect(screen.getByText("blog")).toBeTruthy();
        });
        const revokeBtn = screen.getByRole("button", { name: /keys\.actions\.revoke/ });
        await user.click(revokeBtn);
        await waitFor(() => {
            expect(revokeApiKey).toHaveBeenCalledWith("k1");
        });
        confirmSpy.mockRestore();
    });
    it("Revoke confirm 거부 시 호출 안 함", async () => {
        const user = userEvent.setup();
        vi.mocked(listApiKeys).mockResolvedValueOnce([makeKey("k1", "blog")]);
        const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);
        render(_jsx(ApiKeysPanel, {}));
        await waitFor(() => expect(screen.getByText("blog")).toBeTruthy());
        await user.click(screen.getByRole("button", { name: /keys\.actions\.revoke/ }));
        expect(revokeApiKey).not.toHaveBeenCalled();
        confirmSpy.mockRestore();
    });
    it("새 키 만들기 — modal 열림 + alias 빈 상태 거부", async () => {
        const user = userEvent.setup();
        render(_jsx(ApiKeysPanel, {}));
        await waitFor(() => expect(screen.getByText("keys.empty.title")).toBeTruthy());
        await user.click(screen.getByRole("button", { name: "keys.create" }));
        expect(screen.getByRole("dialog")).toBeTruthy();
        // alias 빈 상태로 submit → error.
        await user.click(screen.getByRole("button", { name: "keys.modal.submit" }));
        await waitFor(() => {
            expect(screen.getByText("keys.errors.emptyAlias")).toBeTruthy();
        });
        expect(createApiKey).not.toHaveBeenCalled();
    });
    it("alias + origin 채우면 발급 + reveal step 노출", async () => {
        const user = userEvent.setup();
        vi.mocked(createApiKey).mockResolvedValueOnce({
            id: "new-id",
            alias: "blog",
            key_prefix: "lm-aaaa1234",
            plaintext_once: "lm-aaaa1234XXXXSECRET24CHARS!",
            created_at: "2026-04-27T00:00:00Z",
        });
        render(_jsx(ApiKeysPanel, {}));
        await waitFor(() => expect(screen.getByText("keys.empty.title")).toBeTruthy());
        await user.click(screen.getByRole("button", { name: "keys.create" }));
        const dialog = screen.getByRole("dialog");
        const aliasInput = within(dialog).getAllByRole("textbox")[0];
        await user.type(aliasInput, "blog");
        const originInput = within(dialog).getAllByRole("textbox")[1];
        await user.type(originInput, "https://my-blog.com");
        await user.click(screen.getByRole("button", { name: "keys.modal.submit" }));
        await waitFor(() => {
            // reveal step — 평문 노출.
            expect(screen.getByTestId("keys-reveal-key")).toBeTruthy();
        });
        expect(createApiKey).toHaveBeenCalledWith({
            alias: "blog",
            scope: expect.objectContaining({
                allowed_origins: ["https://my-blog.com"],
                models: ["*"],
            }),
        });
    });
    it("발급 실패 시 에러 메시지 표시", async () => {
        const user = userEvent.setup();
        vi.mocked(createApiKey).mockRejectedValueOnce(new Error("boom"));
        const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => { });
        render(_jsx(ApiKeysPanel, {}));
        await waitFor(() => expect(screen.getByText("keys.empty.title")).toBeTruthy());
        await user.click(screen.getByRole("button", { name: "keys.create" }));
        const dialog = screen.getByRole("dialog");
        await user.type(within(dialog).getAllByRole("textbox")[0], "x");
        await user.type(within(dialog).getAllByRole("textbox")[1], "https://x.com");
        await user.click(screen.getByRole("button", { name: "keys.modal.submit" }));
        await waitFor(() => {
            expect(screen.getByText("keys.errors.createFailed")).toBeTruthy();
        });
        warnSpy.mockRestore();
    });
});
