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
    network_scope: null,
  }),
  // Phase 8'.c.4 (ADR-0066) — origin 없이 발급할 때 쓰는 default scope.
  defaultNoOriginScope: (network: "localhost" | "lan" | "any") => ({
    models: ["*"],
    endpoints: ["/v1/*"],
    allowed_origins: [],
    expires_at: null,
    project_id: null,
    rate_limit: null,
    enabled_pipelines: null,
    network_scope: network,
  }),
  SEED_PIPELINE_IDS: [
    "pii-redact",
    "token-quota",
    "observability",
    "prompt-sanitize",
  ],
}));

// Phase 8'.c.4 — ApiKeyIssueModal이 useEffect에서 호출하는 IPC들.
vi.mock("../../ipc/chat", () => ({
  listLocalLlamaCppModels: vi.fn(async () => [] as string[]),
}));

vi.mock("../../ipc/runtimes", () => ({
  listRuntimeModels: vi.fn(async () => []),
}));

vi.mock("../../ipc/gateway-settings", () => ({
  getGatewayAllowExternal: vi.fn(async () => false),
  listLanAddresses: vi.fn(async () => [] as string[]),
  setGatewayAllowExternal: vi.fn(),
}));

vi.mock("../../ipc/gateway", () => ({
  getGatewayStatus: vi.fn(async () => ({
    port: 8788,
    status: "listening",
    error: null,
  })),
}));

import {
  createApiKey,
  listApiKeys,
  revokeApiKey,
  type ApiKeyView,
} from "../../ipc/keys";
import { ApiKeysPanel } from "./ApiKeysPanel";

function makeKey(
  id: string,
  alias: string,
  revoked = false,
  scopeOverrides: Partial<ApiKeyView["scope"]> = {},
): ApiKeyView {
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
      ...scopeOverrides,
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

  it("alias만 입력하면 origin 없이도 발급 + reveal step 노출 (Phase 8'.c.4)", async () => {
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
    const aliasInput = within(dialog).getByTestId("keys-modal-alias");
    await user.type(aliasInput, "blog");
    // Origin 입력 없이 — default radio = "localhost" + "전체" 모델로 발급.

    await user.click(screen.getByRole("button", { name: "keys.modal.submit" }));

    await waitFor(() => {
      expect(screen.getByTestId("keys-reveal-key")).toBeTruthy();
    });
    expect(createApiKey).toHaveBeenCalledWith({
      alias: "blog",
      scope: expect.objectContaining({
        allowed_origins: [],
        models: ["*"],
        network_scope: "localhost",
      }),
    });
  });

  // ── Phase 8'.c.4 — network_scope radio + advanced collapse ─────

  it("network scope 라디오 — default localhost", async () => {
    const user = userEvent.setup();
    render(<ApiKeysPanel />);
    await waitFor(() => expect(screen.getByText("keys.empty.title")).toBeTruthy());
    await user.click(screen.getByRole("button", { name: "keys.create" }));

    const localhostRadio = screen.getByTestId(
      "keys-network-scope-radio-localhost",
    ) as HTMLLabelElement;
    const radioInput = localhostRadio.querySelector(
      "input[type='radio']",
    ) as HTMLInputElement;
    expect(radioInput.checked).toBe(true);
  });

  it("network scope 'lan' 선택 + allow_external false → 'lan' scope로 발급", async () => {
    const user = userEvent.setup();
    vi.mocked(createApiKey).mockResolvedValueOnce({
      id: "lan-id",
      alias: "x",
      key_prefix: "lm-lan",
      plaintext_once: "lm-lan000000000000000000000",
      created_at: "2026-05-09T00:00:00Z",
    });
    render(<ApiKeysPanel />);
    await waitFor(() => expect(screen.getByText("keys.empty.title")).toBeTruthy());
    await user.click(screen.getByRole("button", { name: "keys.create" }));

    await user.type(screen.getByTestId("keys-modal-alias"), "x");
    const lanLabel = screen.getByTestId(
      "keys-network-scope-radio-lan",
    ) as HTMLLabelElement;
    const lanInput = lanLabel.querySelector(
      "input[type='radio']",
    ) as HTMLInputElement;
    await user.click(lanInput);

    await user.click(screen.getByRole("button", { name: "keys.modal.submit" }));
    await waitFor(() => {
      expect(createApiKey).toHaveBeenCalled();
    });
    const callArg = vi.mocked(createApiKey).mock.calls[0]?.[0];
    expect(callArg?.scope.network_scope).toBe("lan");
  });

  it("network scope 'any' 선택 시 경고 카피 노출", async () => {
    const user = userEvent.setup();
    render(<ApiKeysPanel />);
    await waitFor(() => expect(screen.getByText("keys.empty.title")).toBeTruthy());
    await user.click(screen.getByRole("button", { name: "keys.create" }));

    const anyLabel = screen.getByTestId(
      "keys-network-scope-radio-any",
    ) as HTMLLabelElement;
    const anyInput = anyLabel.querySelector(
      "input[type='radio']",
    ) as HTMLInputElement;
    await user.click(anyInput);

    await waitFor(() => {
      expect(
        screen.getByTestId("keys-network-scope-any-warning"),
      ).toBeTruthy();
    });
  });

  it("'전체' 모델 sentinel — default ON + 발급 시 models = ['*']", async () => {
    const user = userEvent.setup();
    vi.mocked(createApiKey).mockResolvedValueOnce({
      id: "all-id",
      alias: "x",
      key_prefix: "lm-all",
      plaintext_once: "lm-all000000000000000000000",
      created_at: "2026-05-09T00:00:00Z",
    });
    render(<ApiKeysPanel />);
    await waitFor(() => expect(screen.getByText("keys.empty.title")).toBeTruthy());
    await user.click(screen.getByRole("button", { name: "keys.create" }));

    const selectAll = screen.getByTestId("keys-models-select-all") as HTMLInputElement;
    expect(selectAll.checked).toBe(true);

    await user.type(screen.getByTestId("keys-modal-alias"), "x");
    await user.click(screen.getByRole("button", { name: "keys.modal.submit" }));

    await waitFor(() => {
      expect(createApiKey).toHaveBeenCalled();
    });
    const callArg = vi.mocked(createApiKey).mock.calls[0]?.[0];
    expect(callArg?.scope.models).toEqual(["*"]);
  });

  it("'전체' 해제 + 모델 0개 선택 시 발급 거부 (emptyModels error)", async () => {
    const user = userEvent.setup();
    render(<ApiKeysPanel />);
    await waitFor(() => expect(screen.getByText("keys.empty.title")).toBeTruthy());
    await user.click(screen.getByRole("button", { name: "keys.create" }));

    await user.type(screen.getByTestId("keys-modal-alias"), "x");
    await user.click(screen.getByTestId("keys-models-select-all")); // 해제

    await user.click(screen.getByRole("button", { name: "keys.modal.submit" }));
    await waitFor(() => {
      expect(screen.getByText("keys.errors.emptyModels")).toBeTruthy();
    });
    expect(createApiKey).not.toHaveBeenCalled();
  });

  it("고급 설정 collapse — default 접힘 + 클릭 시 origin 입력 노출", async () => {
    const user = userEvent.setup();
    render(<ApiKeysPanel />);
    await waitFor(() => expect(screen.getByText("keys.empty.title")).toBeTruthy());
    await user.click(screen.getByRole("button", { name: "keys.create" }));

    // default 접힘 — body 보이지 않음.
    expect(screen.queryByTestId("keys-advanced-body")).toBeNull();

    await user.click(screen.getByTestId("keys-advanced-toggle"));
    expect(screen.getByTestId("keys-advanced-body")).toBeTruthy();
    // 펼친 후 origin / pipelines 모두 등장.
    expect(screen.getByTestId("keys-pipelines-fieldset")).toBeTruthy();
  });

  // ── Phase 8'.c.3 — per-key Pipelines override fieldset (now in 고급 설정) ─

  it("modal — pipelines fieldset은 고급 설정 안에서만 노출", async () => {
    const user = userEvent.setup();
    render(<ApiKeysPanel />);
    await waitFor(() => expect(screen.getByText("keys.empty.title")).toBeTruthy());
    await user.click(screen.getByRole("button", { name: "keys.create" }));

    // 고급 펼치기 전엔 보이지 않음.
    expect(screen.queryByTestId("keys-pipelines-fieldset")).toBeNull();

    await user.click(screen.getByTestId("keys-advanced-toggle"));
    expect(screen.getByTestId("keys-pipelines-fieldset")).toBeTruthy();
    const useGlobal = screen.getByTestId("keys-pipelines-use-global") as HTMLInputElement;
    expect(useGlobal.checked).toBe(true);
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

    await user.type(screen.getByTestId("keys-modal-alias"), "x");
    // 고급 미펼친 상태에서도 default = useGlobal=true → null.
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

    await user.type(screen.getByTestId("keys-modal-alias"), "x");
    // 고급 펼치기.
    await user.click(screen.getByTestId("keys-advanced-toggle"));
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
    // 고급 펼치기 (Phase 8'.c.4).
    await user.click(screen.getByTestId("keys-advanced-toggle"));
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

  // ── Phase 8'.c.4 (ADR-0066) — network_scope 뱃지 ───────────────

  it("network_scope 뱃지 — null/localhost는 '이 PC만'으로 노출", async () => {
    vi.mocked(listApiKeys).mockResolvedValueOnce([
      makeKey("k1", "blog"),
      makeKey("k2", "lan-key", false, { network_scope: "lan" }),
      makeKey("k3", "any-key", false, { network_scope: "any" }),
    ]);
    render(<ApiKeysPanel />);
    await waitFor(() => {
      expect(screen.getByText("blog")).toBeTruthy();
    });
    // k1 (network_scope null) → localhost 뱃지로 fallback.
    expect(screen.getByTestId("keys-network-badge-localhost")).toBeTruthy();
    expect(screen.getByTestId("keys-network-badge-lan")).toBeTruthy();
    expect(screen.getByTestId("keys-network-badge-any")).toBeTruthy();
  });

  it("발급 실패 시 에러 메시지 표시", async () => {
    const user = userEvent.setup();
    vi.mocked(createApiKey).mockRejectedValueOnce(new Error("boom"));
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});

    render(<ApiKeysPanel />);
    await waitFor(() => expect(screen.getByText("keys.empty.title")).toBeTruthy());
    await user.click(screen.getByRole("button", { name: "keys.create" }));
    await user.type(screen.getByTestId("keys-modal-alias"), "x");
    // origin 없이도 발급 시도 — Phase 8'.c.4 default 흐름.
    await user.click(screen.getByRole("button", { name: "keys.modal.submit" }));
    await waitFor(() => {
      expect(screen.getByText("keys.errors.createFailed")).toBeTruthy();
    });
    warnSpy.mockRestore();
  });
});
