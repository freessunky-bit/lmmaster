/**
 * @vitest-environment jsdom
 */
// ApiKeysPanel + ApiKeyIssueModal 렌더 + 발급/회수 플로우 테스트.

import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) =>
      opts ? `${key}:${JSON.stringify(opts)}` : key,
    i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
  }),
}));

vi.mock("../../ipc/keys", () => ({
  listApiKeys: vi.fn(),
  revokeApiKey: vi.fn(),
  createApiKey: vi.fn(),
  updateApiKeyPipelines: vi.fn(),
  defaultWebScope: (origin: string) => ({
    models: ["*"],
    endpoints: ["/v1/*"],
    allowed_origins: [origin],
    expires_at: null,
    project_id: null,
    rate_limit: null,
    enabled_pipelines: null,
  }),
  SEED_PIPELINE_IDS: [
    "pii-redact",
    "token-quota",
    "observability",
    "prompt-sanitize",
  ],
}));

import {
  createApiKey,
  listApiKeys,
  revokeApiKey,
  type ApiKeyView,
} from "../../ipc/keys";
import { ApiKeysPanel } from "./ApiKeysPanel";

function makeKey(id: string, alias: string, revoked = false): ApiKeyView {
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
    render(<ApiKeysPanel />);
    await waitFor(() => {
      expect(screen.getByText("keys.empty.title")).toBeTruthy();
    });
  });

  it("목록 있으면 테이블 렌더 + active/revoked 상태 표시", async () => {
    vi.mocked(listApiKeys).mockResolvedValueOnce([
      makeKey("active1", "blog"),
      makeKey("revoked1", "old", true),
    ]);
    render(<ApiKeysPanel />);
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
    render(<ApiKeysPanel />);
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

    render(<ApiKeysPanel />);
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

    render(<ApiKeysPanel />);
    await waitFor(() => expect(screen.getByText("blog")).toBeTruthy());
    await user.click(screen.getByRole("button", { name: /keys\.actions\.revoke/ }));
    expect(revokeApiKey).not.toHaveBeenCalled();
    confirmSpy.mockRestore();
  });

  it("새 키 만들기 — modal 열림 + alias 빈 상태 거부", async () => {
    const user = userEvent.setup();
    render(<ApiKeysPanel />);
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
    render(<ApiKeysPanel />);
    await waitFor(() => expect(screen.getByText("keys.empty.title")).toBeTruthy());
    await user.click(screen.getByRole("button", { name: "keys.create" }));

    const dialog = screen.getByRole("dialog");
    const aliasInput = within(dialog).getAllByRole("textbox")[0]!;
    await user.type(aliasInput, "blog");
    const originInput = within(dialog).getAllByRole("textbox")[1]!;
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

  // ── Phase 8'.c.3 — per-key Pipelines override fieldset ─────────

  it("modal — pipelines fieldset 노출 + 'use global' 기본 체크", async () => {
    const user = userEvent.setup();
    render(<ApiKeysPanel />);
    await waitFor(() => expect(screen.getByText("keys.empty.title")).toBeTruthy());
    await user.click(screen.getByRole("button", { name: "keys.create" }));

    expect(screen.getByTestId("keys-pipelines-fieldset")).toBeTruthy();
    const useGlobal = screen.getByTestId("keys-pipelines-use-global") as HTMLInputElement;
    expect(useGlobal.checked).toBe(true);
    // 4종 시드 체크박스 모두 노출.
    expect(screen.getByTestId("keys-pipelines-cb-pii-redact")).toBeTruthy();
    expect(screen.getByTestId("keys-pipelines-cb-token-quota")).toBeTruthy();
    expect(screen.getByTestId("keys-pipelines-cb-observability")).toBeTruthy();
    expect(screen.getByTestId("keys-pipelines-cb-prompt-sanitize")).toBeTruthy();
  });

  it("'use global' 체크 시 enabled_pipelines = null로 발급", async () => {
    const user = userEvent.setup();
    vi.mocked(createApiKey).mockResolvedValueOnce({
      id: "id1",
      alias: "x",
      key_prefix: "lm-x",
      plaintext_once: "lm-x000000000000000000000000",
      created_at: "2026-04-28T00:00:00Z",
    });
    render(<ApiKeysPanel />);
    await waitFor(() => expect(screen.getByText("keys.empty.title")).toBeTruthy());
    await user.click(screen.getByRole("button", { name: "keys.create" }));

    const dialog = screen.getByRole("dialog");
    await user.type(within(dialog).getAllByRole("textbox")[0]!, "x");
    await user.type(within(dialog).getAllByRole("textbox")[1]!, "https://x.com");
    await user.click(screen.getByRole("button", { name: "keys.modal.submit" }));

    await waitFor(() => {
      expect(createApiKey).toHaveBeenCalled();
    });
    const callArg = vi.mocked(createApiKey).mock.calls[0]?.[0];
    expect(callArg?.scope.enabled_pipelines).toBeNull();
  });

  it("'use global' 해제 후 일부 끄면 enabled_pipelines가 화이트리스트로 발급", async () => {
    const user = userEvent.setup();
    vi.mocked(createApiKey).mockResolvedValueOnce({
      id: "id2",
      alias: "x",
      key_prefix: "lm-x",
      plaintext_once: "lm-x000000000000000000000000",
      created_at: "2026-04-28T00:00:00Z",
    });
    render(<ApiKeysPanel />);
    await waitFor(() => expect(screen.getByText("keys.empty.title")).toBeTruthy());
    await user.click(screen.getByRole("button", { name: "keys.create" }));

    const dialog = screen.getByRole("dialog");
    await user.type(within(dialog).getAllByRole("textbox")[0]!, "x");
    await user.type(within(dialog).getAllByRole("textbox")[1]!, "https://x.com");

    // use-global 해제.
    await user.click(screen.getByTestId("keys-pipelines-use-global"));
    // observability 체크박스 해제 (default ON).
    await user.click(screen.getByTestId("keys-pipelines-cb-observability"));
    // token-quota도 해제.
    await user.click(screen.getByTestId("keys-pipelines-cb-token-quota"));

    await user.click(screen.getByRole("button", { name: "keys.modal.submit" }));

    await waitFor(() => {
      expect(createApiKey).toHaveBeenCalled();
    });
    const callArg = vi.mocked(createApiKey).mock.calls[0]?.[0];
    // pii-redact + prompt-sanitize만 남음.
    expect(callArg?.scope.enabled_pipelines).toEqual([
      "pii-redact",
      "prompt-sanitize",
    ]);
  });

  it("모든 체크 해제 시 빈 vec 경고 노출", async () => {
    const user = userEvent.setup();
    render(<ApiKeysPanel />);
    await waitFor(() => expect(screen.getByText("keys.empty.title")).toBeTruthy());
    await user.click(screen.getByRole("button", { name: "keys.create" }));
    // use-global 해제.
    await user.click(screen.getByTestId("keys-pipelines-use-global"));
    // 4 체크박스 모두 해제.
    for (const id of [
      "pii-redact",
      "token-quota",
      "observability",
      "prompt-sanitize",
    ]) {
      await user.click(screen.getByTestId(`keys-pipelines-cb-${id}`));
    }
    expect(screen.getByTestId("keys-pipelines-warn-empty")).toBeTruthy();
  });

  it("발급 실패 시 에러 메시지 표시", async () => {
    const user = userEvent.setup();
    vi.mocked(createApiKey).mockRejectedValueOnce(new Error("boom"));
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    render(<ApiKeysPanel />);
    await waitFor(() => expect(screen.getByText("keys.empty.title")).toBeTruthy());
    await user.click(screen.getByRole("button", { name: "keys.create" }));
    const dialog = screen.getByRole("dialog");
    await user.type(within(dialog).getAllByRole("textbox")[0]!, "x");
    await user.type(within(dialog).getAllByRole("textbox")[1]!, "https://x.com");
    await user.click(screen.getByRole("button", { name: "keys.modal.submit" }));
    await waitFor(() => {
      expect(screen.getByText("keys.errors.createFailed")).toBeTruthy();
    });
    warnSpy.mockRestore();
  });
});
