/**
 * @vitest-environment jsdom
 */
// RagSeedStep — Phase 12'.b (ADR-0050) Stage 1 invariants.

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { axe } from "vitest-axe";

const { startIngestMock, useActiveWorkspaceMock } = vi.hoisted(() => ({
  startIngestMock: vi.fn(),
  useActiveWorkspaceMock: vi.fn(),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, defaultValue?: string | Record<string, unknown>) =>
      typeof defaultValue === "string" ? defaultValue : key,
    i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
  }),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("../../ipc/knowledge", async () => {
  const actual =
    await vi.importActual<typeof import("../../ipc/knowledge")>(
      "../../ipc/knowledge",
    );
  return {
    ...actual,
    startIngest: startIngestMock,
  };
});

vi.mock("../../contexts/ActiveWorkspaceContext", () => ({
  useActiveWorkspace: useActiveWorkspaceMock,
}));

import { RagSeedStep } from "./RagSeedStep";
import type { ModelEntry } from "../../ipc/catalog";

const FIXTURE_MODEL: ModelEntry = {
  id: "exaone",
  display_name: "EXAONE 3.5 7.8B",
  category: "agent-general",
  model_family: "exaone",
  source: { type: "hugging-face", repo: "x/y" },
  runner_compatibility: ["llama-cpp"],
  quantization_options: [],
  min_vram_mb: null,
  rec_vram_mb: null,
  min_ram_mb: 8192,
  rec_ram_mb: 16384,
  install_size_mb: 4900,
  context_guidance: "한국어 일반 비서",
  tool_support: true,
  vision_support: false,
  structured_output_support: true,
  license: "EXAONE Custom",
  maturity: "stable",
  portable_suitability: 7,
  on_device_suitability: 8,
  fine_tune_suitability: 8,
  verification: { tier: "verified" },
  use_case_examples: [],
  warnings: [],
};

const ACTIVE_WORKSPACE = {
  id: "ws-1",
  name: "Default",
  description: null,
  created_at: "2026-04-30",
  last_used_at: "2026-04-30",
};

beforeEach(() => {
  startIngestMock.mockReset();
  useActiveWorkspaceMock.mockReset();
  useActiveWorkspaceMock.mockReturnValue({
    active: ACTIVE_WORKSPACE,
    workspaces: [ACTIVE_WORKSPACE],
    loading: false,
    setActive: vi.fn(),
    refresh: vi.fn(),
    create: vi.fn(),
    rename: vi.fn(),
    remove: vi.fn(),
  });
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("RagSeedStep", () => {
  it("a11y: violations 없음 (일반 intent)", async () => {
    const { container } = render(
      <RagSeedStep model={FIXTURE_MODEL} intent="ko-rag" />,
    );
    const results = await axe(container);
    expect(results.violations).toEqual([]);
  });

  it("vision-image intent → graceful 안내, ingest UI 미렌더", () => {
    render(<RagSeedStep model={FIXTURE_MODEL} intent="vision-image" />);
    expect(
      screen.getByTestId("workbench-rag-seed-vision-deferred"),
    ).toBeDefined();
    expect(screen.queryByTestId("workbench-rag-seed-path")).toBeNull();
    expect(screen.queryByTestId("workbench-rag-seed-start")).toBeNull();
  });

  it("vision-multimodal intent도 graceful 안내", () => {
    render(
      <RagSeedStep model={FIXTURE_MODEL} intent="vision-multimodal" />,
    );
    expect(
      screen.getByTestId("workbench-rag-seed-vision-deferred"),
    ).toBeDefined();
  });

  it("ko-rag intent → KURE-v1 권장 메시지", () => {
    render(<RagSeedStep model={FIXTURE_MODEL} intent="ko-rag" />);
    const tip = screen.getByTestId("workbench-rag-seed-ko-rag-tip");
    expect(tip.textContent).toContain("KURE-v1");
  });

  it("intent=null 또는 ko-rag가 아닐 때 ko-rag tip 미렌더", () => {
    render(
      <RagSeedStep model={FIXTURE_MODEL} intent="coding-general" />,
    );
    expect(screen.queryByTestId("workbench-rag-seed-ko-rag-tip")).toBeNull();
  });

  it("path 입력 후 '지금 추가' 클릭 → startIngest 호출", async () => {
    startIngestMock.mockResolvedValue({
      ingest_id: "ingest-1",
      cancel: vi.fn(),
    });
    render(<RagSeedStep model={FIXTURE_MODEL} intent="ko-rag" />);
    const user = userEvent.setup();
    const input = screen.getByTestId("workbench-rag-seed-path");
    await user.type(input, "/notes");
    await user.click(screen.getByTestId("workbench-rag-seed-start"));
    expect(startIngestMock).toHaveBeenCalledTimes(1);
    const config = startIngestMock.mock.calls[0]?.[0];
    expect(config?.workspace_id).toBe("ws-1");
    expect(config?.path).toBe("/notes");
  });

  it("path가 비어있으면 '지금 추가' 비활성", () => {
    render(<RagSeedStep model={FIXTURE_MODEL} intent="ko-rag" />);
    const start = screen.getByTestId("workbench-rag-seed-start");
    expect(start).toHaveAttribute("disabled");
  });

  it("active workspace 없으면 안내 + 시작 비활성", () => {
    useActiveWorkspaceMock.mockReturnValue({
      active: null,
      workspaces: [],
      loading: false,
      setActive: vi.fn(),
      refresh: vi.fn(),
      create: vi.fn(),
      rename: vi.fn(),
      remove: vi.fn(),
    });
    render(<RagSeedStep model={FIXTURE_MODEL} intent="ko-rag" />);
    expect(
      screen.getByTestId("workbench-rag-seed-no-workspace"),
    ).toBeDefined();
    expect(screen.getByTestId("workbench-rag-seed-start")).toHaveAttribute(
      "disabled",
    );
  });

  it("'Workspace에서 관리' 클릭 → hash 라우팅", async () => {
    render(<RagSeedStep model={FIXTURE_MODEL} intent="ko-rag" />);
    const user = userEvent.setup();
    await user.click(screen.getByTestId("workbench-rag-seed-workspace-link"));
    expect(window.location.hash).toBe("#/workspace");
  });

  it("model.context_guidance가 있으면 모델 안내 노출", () => {
    render(<RagSeedStep model={FIXTURE_MODEL} intent="ko-rag" />);
    expect(screen.getByText(/한국어 일반 비서/)).toBeDefined();
  });
});
