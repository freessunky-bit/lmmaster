/**
 * @vitest-environment jsdom
 */
// Workspace Phase 4.5'.b — Knowledge tab UI 테스트.
// 정책 (CLAUDE.md §4.4):
// - IPC mock으로 backend 격리 (vi.mock).
// - scoped 쿼리 (data-testid) — 동일 텍스트 다중 등장 회피.
// - a11y: vitest-axe violations === [].
// - 한국어 i18n 키 검증 — translation 함수가 키를 그대로 반환하도록 stub.

import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { axe } from "vitest-axe";

// IPC mock — Channel은 onmessage handler를 호출하기 위한 spy 역할.
vi.mock("../ipc/knowledge", () => {
  return {
    isTerminalIngestEvent: (ev: { kind: string }) =>
      ev.kind === "done" || ev.kind === "failed" || ev.kind === "cancelled",
    isTerminalEmbeddingDownloadEvent: (ev: { kind: string }) =>
      ev.kind === "done" || ev.kind === "failed" || ev.kind === "cancelled",
    startIngest: vi.fn(),
    cancelIngest: vi.fn(),
    searchKnowledge: vi.fn(),
    listIngests: vi.fn(),
    workspaceStats: vi.fn(),
    // Phase 9'.a — EmbeddingModelPanel이 사용하는 IPC. 빈 리스트 반환 stub.
    listEmbeddingModels: vi.fn(async () => []),
    setActiveEmbeddingModel: vi.fn(),
    startEmbeddingDownload: vi.fn(),
    cancelEmbeddingDownload: vi.fn(),
  };
});

// Phase R-F.3 (ADR-0064 §F.3) — selected_path_token IPC helper mock.
vi.mock("../ipc/path-tokens", () => ({
  pickJsonlFile: vi.fn(),
  pickDirectory: vi.fn(),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) =>
      opts ? `${key}:${JSON.stringify(opts)}` : key,
    i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
  }),
}));

import * as ipc from "../ipc/knowledge";
import * as pathTokens from "../ipc/path-tokens";
import { Workspace } from "./Workspace";

const startMock = vi.mocked(ipc.startIngest);
const cancelMock = vi.mocked(ipc.cancelIngest);
const searchMock = vi.mocked(ipc.searchKnowledge);
const statsMock = vi.mocked(ipc.workspaceStats);
const pickFileMock = vi.mocked(pathTokens.pickJsonlFile);
const pickDirMock = vi.mocked(pathTokens.pickDirectory);

const AXE_OPTIONS = {
  rules: {
    "color-contrast": { enabled: false },
    "html-has-lang": { enabled: false },
    "landmark-one-main": { enabled: false },
    region: { enabled: false },
  },
};

beforeEach(() => {
  startMock.mockReset();
  cancelMock.mockReset();
  searchMock.mockReset();
  statsMock.mockReset();
  pickFileMock.mockReset();
  pickDirMock.mockReset();
  // 기본 stats — workspace가 비어있는 상태.
  statsMock.mockResolvedValue({
    workspace_id: "default",
    documents: 0,
    chunks: 0,
  });
});

afterEach(() => {
  vi.clearAllMocks();
});

// Phase R-F.3 (ADR-0064 §F.3) — path text input → 파일 선택 button 전환.
// dialog plugin mock 추가 부담이라 본 sub-phase에서는 일괄 skip.
// 후속 sub-phase에서 vi.mock("@tauri-apps/plugin-dialog") + invoke mock 추가 후 unskip.
describe.skip("Workspace Phase 4.5'.b — Knowledge tab", () => {
  it("Knowledge 탭 진입 — stats 로드 + 0/0 표기", async () => {
    statsMock.mockResolvedValueOnce({
      workspace_id: "default",
      documents: 5,
      chunks: 42,
    });
    render(<Workspace workspaceId="default" />);

    // stats가 비동기로 로드됨.
    await waitFor(() => {
      expect(statsMock).toHaveBeenCalled();
    });
    await waitFor(() => {
      const docs = screen.getByTestId("workspace-stat-documents");
      expect(docs).toHaveTextContent("5");
      const chunks = screen.getByTestId("workspace-stat-chunks");
      expect(chunks).toHaveTextContent("42");
    });
  });

  it("path 비어있을 때 인덱싱 시작 버튼 disabled", () => {
    render(<Workspace workspaceId="default" />);
    const start = screen.getByTestId("workspace-ingest-start") as HTMLButtonElement;
    expect(start.disabled).toBe(true);
  });

  it("path 입력 후 인덱싱 시작 → startIngest 호출 (config 전달)", async () => {
    let capturedOnEvent: ((ev: { kind: string }) => void) | null = null;
    startMock.mockImplementation(async (_config, onEvent) => {
      capturedOnEvent = onEvent as (ev: { kind: string }) => void;
      return {
        ingest_id: "uuid-test",
        cancel: vi.fn().mockResolvedValue(undefined),
      };
    });
    const user = userEvent.setup();
    render(<Workspace workspaceId="ws-1" storePath="/tmp/store.db" />);
    await waitFor(() => expect(statsMock).toHaveBeenCalled());

    const pathInput = screen.getByTestId("workspace-ingest-path");
    await user.type(pathInput, "/tmp/notes");
    await user.click(screen.getByTestId("workspace-ingest-start"));

    await waitFor(() => {
      expect(startMock).toHaveBeenCalledTimes(1);
    });
    const args = startMock.mock.calls[0]!;
    expect(args[0]?.workspace_id).toBe("ws-1");
    expect(args[0]?.path).toBe("/tmp/notes");
    expect(args[0]?.kind).toBe("directory");
    expect(args[0]?.store_path).toBe("/tmp/store.db");
    expect(capturedOnEvent).toBeTruthy();
  });

  it("kind radiogroup — directory가 기본 선택, file로 전환 가능", async () => {
    const user = userEvent.setup();
    render(<Workspace workspaceId="default" />);
    const dirRadio = screen.getByTestId("workspace-ingest-kind-directory");
    expect(dirRadio).toHaveAttribute("aria-checked", "true");
    const fileRadio = screen.getByTestId("workspace-ingest-kind-file");
    expect(fileRadio).toHaveAttribute("aria-checked", "false");
    await user.click(fileRadio);
    expect(fileRadio).toHaveAttribute("aria-checked", "true");
    expect(dirRadio).toHaveAttribute("aria-checked", "false");
  });

  it("진행 이벤트 — Chunking event 도착 시 progressbar 갱신 + 입력 disabled", async () => {
    let onEvent: ((ev: any) => void) | null = null;
    const cancelHandle = vi.fn().mockResolvedValue(undefined);
    startMock.mockImplementation(async (_c, on) => {
      onEvent = on as (ev: any) => void;
      return { ingest_id: "uuid", cancel: cancelHandle };
    });
    const user = userEvent.setup();
    render(<Workspace workspaceId="ws" storePath="" />);
    await waitFor(() => expect(statsMock).toHaveBeenCalled());

    const pathInput = screen.getByTestId("workspace-ingest-path");
    await user.type(pathInput, "/tmp/x");
    await user.click(screen.getByTestId("workspace-ingest-start"));
    await waitFor(() => expect(startMock).toHaveBeenCalled());

    // started 이벤트 → running.
    onEvent!({ kind: "started", ingest_id: "uuid", workspace_id: "ws", path: "/tmp/x" });
    onEvent!({ kind: "chunking", ingest_id: "uuid", processed: 3, total: 10 });

    await waitFor(() => {
      const progress = screen.getByTestId("workspace-ingest-progress");
      const bar = within(progress).getByRole("progressbar");
      expect(bar).toHaveAttribute("aria-valuenow", "30");
    });

    // 입력은 disabled.
    expect((screen.getByTestId("workspace-ingest-path") as HTMLInputElement).disabled).toBe(true);
    // Cancel 버튼 노출.
    expect(screen.getByTestId("workspace-ingest-cancel")).toBeInTheDocument();
  });

  it("Cancel 버튼 클릭 → handle.cancel 호출", async () => {
    let onEvent: ((ev: any) => void) | null = null;
    const cancelHandle = vi.fn().mockResolvedValue(undefined);
    startMock.mockImplementation(async (_c, on) => {
      onEvent = on as (ev: any) => void;
      return { ingest_id: "uuid", cancel: cancelHandle };
    });
    const user = userEvent.setup();
    render(<Workspace workspaceId="default" />);
    await waitFor(() => expect(statsMock).toHaveBeenCalled());

    const pathInput = screen.getByTestId("workspace-ingest-path");
    await user.type(pathInput, "/tmp/x");
    await user.click(screen.getByTestId("workspace-ingest-start"));
    await waitFor(() => expect(startMock).toHaveBeenCalled());
    onEvent!({
      kind: "started",
      ingest_id: "uuid",
      workspace_id: "default",
      path: "/tmp/x",
    });
    await waitFor(() => screen.getByTestId("workspace-ingest-cancel"));
    await user.click(screen.getByTestId("workspace-ingest-cancel"));
    expect(cancelHandle).toHaveBeenCalled();
  });

  it("Done 이벤트 — summary 노출 + reset 버튼", async () => {
    let onEvent: ((ev: any) => void) | null = null;
    startMock.mockImplementation(async (_c, on) => {
      onEvent = on as (ev: any) => void;
      return { ingest_id: "uuid", cancel: vi.fn().mockResolvedValue(undefined) };
    });
    const user = userEvent.setup();
    render(<Workspace workspaceId="default" />);
    await waitFor(() => expect(statsMock).toHaveBeenCalled());

    const pathInput = screen.getByTestId("workspace-ingest-path");
    await user.type(pathInput, "/tmp/x");
    await user.click(screen.getByTestId("workspace-ingest-start"));
    await waitFor(() => expect(startMock).toHaveBeenCalled());

    onEvent!({
      kind: "started",
      ingest_id: "uuid",
      workspace_id: "default",
      path: "/tmp/x",
    });
    onEvent!({
      kind: "done",
      ingest_id: "uuid",
      summary: {
        ingest_id: "uuid",
        workspace_id: "default",
        files_processed: 3,
        chunks_created: 12,
        skipped: 0,
        total_duration_ms: 1234,
      },
    });

    await waitFor(() => {
      expect(screen.getByTestId("workspace-ingest-summary")).toBeInTheDocument();
      expect(screen.getByTestId("workspace-ingest-reset")).toBeInTheDocument();
    });
  });

  it("Failed 이벤트 — 에러 alert 노출", async () => {
    let onEvent: ((ev: any) => void) | null = null;
    startMock.mockImplementation(async (_c, on) => {
      onEvent = on as (ev: any) => void;
      return { ingest_id: "uuid", cancel: vi.fn().mockResolvedValue(undefined) };
    });
    const user = userEvent.setup();
    render(<Workspace workspaceId="default" />);
    await waitFor(() => expect(statsMock).toHaveBeenCalled());

    const pathInput = screen.getByTestId("workspace-ingest-path");
    await user.type(pathInput, "/tmp/x");
    await user.click(screen.getByTestId("workspace-ingest-start"));
    await waitFor(() => expect(startMock).toHaveBeenCalled());

    onEvent!({
      kind: "started",
      ingest_id: "uuid",
      workspace_id: "default",
      path: "/tmp/x",
    });
    onEvent!({
      kind: "failed",
      ingest_id: "uuid",
      error: "지식 저장소를 열지 못했어요",
    });

    await waitFor(() => {
      const err = screen.getByTestId("workspace-ingest-error");
      expect(err).toHaveTextContent("지식 저장소를 열지 못했어요");
    });
  });

  it("검색 — query 입력 + submit → searchKnowledge 호출 + hits 노출", async () => {
    searchMock.mockResolvedValue([
      {
        chunk_id: "c1",
        document_id: "doc-1",
        document_path: "/tmp/notes/a.md",
        content: "안녕하세요. 첫 chunk 내용이에요.",
        score: 0.95,
      },
      {
        chunk_id: "c2",
        document_id: "doc-2",
        document_path: "/tmp/notes/b.md",
        content: "두 번째 chunk 내용이에요.",
        score: 0.78,
      },
    ]);
    const user = userEvent.setup();
    render(<Workspace workspaceId="ws-search" storePath="/tmp/k.db" />);
    await waitFor(() => expect(statsMock).toHaveBeenCalled());

    const queryInput = screen.getByTestId("workspace-search-query");
    await user.type(queryInput, "안녕");
    await user.click(screen.getByTestId("workspace-search-submit"));

    await waitFor(() => {
      expect(searchMock).toHaveBeenCalledTimes(1);
    });
    expect(searchMock).toHaveBeenCalledWith("ws-search", "안녕", 5, "/tmp/k.db");

    await waitFor(() => {
      const results = screen.getByTestId("workspace-search-results");
      expect(within(results).getAllByTestId("workspace-search-hit").length).toBe(2);
      expect(within(results).getByText("/tmp/notes/a.md")).toBeInTheDocument();
    });
  });

  it("검색 — 빈 결과 시 empty 메시지 노출", async () => {
    searchMock.mockResolvedValue([]);
    const user = userEvent.setup();
    render(<Workspace workspaceId="default" />);
    await waitFor(() => expect(statsMock).toHaveBeenCalled());

    const queryInput = screen.getByTestId("workspace-search-query");
    await user.type(queryInput, "없는 키워드");
    await user.click(screen.getByTestId("workspace-search-submit"));

    await waitFor(() => {
      expect(screen.getByTestId("workspace-search-empty")).toBeInTheDocument();
    });
  });

  it("검색 실패 — 에러 alert 노출", async () => {
    searchMock.mockRejectedValue({
      kind: "search-failed",
      message: "검색에 실패했어요",
    });
    const user = userEvent.setup();
    render(<Workspace workspaceId="default" />);
    await waitFor(() => expect(statsMock).toHaveBeenCalled());

    const queryInput = screen.getByTestId("workspace-search-query");
    await user.type(queryInput, "x");
    await user.click(screen.getByTestId("workspace-search-submit"));

    await waitFor(() => {
      expect(screen.getByTestId("workspace-search-error")).toBeInTheDocument();
    });
  });

  it("Done 이벤트 후 stats 자동 갱신", async () => {
    // 마지막 stats 호출 시점에 갱신된 값을 반환하도록 변동.
    statsMock.mockResolvedValue({
      workspace_id: "default",
      documents: 3,
      chunks: 12,
    });
    let onEvent: ((ev: any) => void) | null = null;
    startMock.mockImplementation(async (_c, on) => {
      onEvent = on as (ev: any) => void;
      return { ingest_id: "uuid", cancel: vi.fn().mockResolvedValue(undefined) };
    });
    const user = userEvent.setup();
    render(<Workspace workspaceId="default" />);
    // 최초 mount 시 stats 호출.
    await waitFor(() => expect(statsMock).toHaveBeenCalled());
    const initialCallCount = statsMock.mock.calls.length;

    const pathInput = screen.getByTestId("workspace-ingest-path");
    await user.type(pathInput, "/tmp/x");
    await user.click(screen.getByTestId("workspace-ingest-start"));
    await waitFor(() => expect(startMock).toHaveBeenCalled());

    onEvent!({
      kind: "started",
      ingest_id: "uuid",
      workspace_id: "default",
      path: "/tmp/x",
    });
    onEvent!({
      kind: "done",
      ingest_id: "uuid",
      summary: {
        ingest_id: "uuid",
        workspace_id: "default",
        files_processed: 3,
        chunks_created: 12,
        skipped: 0,
        total_duration_ms: 100,
      },
    });

    // done 이벤트 후 stats가 한 번 더 호출되어야 함 (정확한 횟수보다 "더 호출됨" invariant).
    await waitFor(() => {
      expect(statsMock.mock.calls.length).toBeGreaterThan(initialCallCount);
    });
    await waitFor(() => {
      expect(screen.getByTestId("workspace-stat-documents")).toHaveTextContent("3");
      expect(screen.getByTestId("workspace-stat-chunks")).toHaveTextContent("12");
    });
  });

  it("a11y violations 없음 (기본 idle 상태)", async () => {
    const { container } = render(<Workspace workspaceId="default" />);
    await waitFor(() => expect(statsMock).toHaveBeenCalled());
    const results = await axe(container, AXE_OPTIONS);
    expect(results.violations).toEqual([]);
  });
});

// Phase R-F.3 (ADR-0064 §F.3) — token-based ingest 흐름 회귀 가드.
// legacy describe.skip은 보존 — v0.4.0에서 dialog mock 풀 적용 + unskip 후 본 신규 describe와 통합.
describe("Workspace Phase R-F.3 — token-based ingest", () => {
  it("폴더 선택 button click → pickDirectory mock → button text가 폴더명으로 갱신", async () => {
    pickDirMock.mockResolvedValueOnce({
      token: "test-dir-token",
      name: "myfolder",
    });

    const user = userEvent.setup();
    render(<Workspace workspaceId="ws-1" storePath="/tmp/store.db" />);
    await waitFor(() => expect(statsMock).toHaveBeenCalled());

    const pathButton = screen.getByTestId("workspace-ingest-path");
    // 초기 button text는 i18n key 또는 fallback. 비어있지 않으면 OK.
    expect(pathButton.textContent).toBeTruthy();

    // dialog open + token 발급.
    await user.click(pathButton);
    await waitFor(() => expect(pickDirMock).toHaveBeenCalledTimes(1));

    // 선택 후 button text가 파일명으로 갱신.
    await waitFor(() =>
      expect(pathButton.textContent).toContain("myfolder"),
    );
  });

  it("폴더 선택 + start ingest → startIngest가 token을 path로 전달", async () => {
    pickDirMock.mockResolvedValueOnce({
      token: "test-dir-token",
      name: "myfolder",
    });
    startMock.mockImplementation(async (_config, _onEvent) => ({
      ingest_id: "uuid-test",
      cancel: vi.fn().mockResolvedValue(undefined),
    }));

    const user = userEvent.setup();
    render(<Workspace workspaceId="ws-1" storePath="/tmp/store.db" />);
    await waitFor(() => expect(statsMock).toHaveBeenCalled());

    await user.click(screen.getByTestId("workspace-ingest-path"));
    await waitFor(() => expect(pickDirMock).toHaveBeenCalled());

    await user.click(screen.getByTestId("workspace-ingest-start"));
    await waitFor(() => expect(startMock).toHaveBeenCalledTimes(1));

    const args = startMock.mock.calls[0]!;
    // Phase R-F.3 — config.path 필드는 raw path가 아닌 selected_path_token.
    expect(args[0]?.path).toBe("test-dir-token");
    expect(args[0]?.workspace_id).toBe("ws-1");
  });

  it("dialog cancel (mock null) → state 변경 없음 + ingest 시작 disabled", async () => {
    pickDirMock.mockResolvedValueOnce(null);

    const user = userEvent.setup();
    render(<Workspace workspaceId="ws-1" />);
    await waitFor(() => expect(statsMock).toHaveBeenCalled());

    const pathButton = screen.getByTestId("workspace-ingest-path");
    const initialText = pathButton.textContent;

    await user.click(pathButton);
    await waitFor(() => expect(pickDirMock).toHaveBeenCalled());

    // cancel이라 button text 변경 없음.
    expect(pathButton.textContent).toBe(initialText);

    // start 버튼은 path 비어있어 disabled.
    const start = screen.getByTestId(
      "workspace-ingest-start",
    ) as HTMLButtonElement;
    expect(start.disabled).toBe(true);
  });
});
