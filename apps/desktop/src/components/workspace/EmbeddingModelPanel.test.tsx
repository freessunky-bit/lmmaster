/**
 * @vitest-environment jsdom
 */
// EmbeddingModelPanel — 카드 3장 렌더 + 다운로드 / 활성 / 에러 / a11y.

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

vi.mock("../../ipc/knowledge", () => {
  return {
    listEmbeddingModels: vi.fn(),
    setActiveEmbeddingModel: vi.fn(),
    startEmbeddingDownload: vi.fn(),
    cancelEmbeddingDownload: vi.fn(),
    isTerminalEmbeddingDownloadEvent: (ev: { kind: string }) =>
      ev.kind === "done" || ev.kind === "failed" || ev.kind === "cancelled",
  };
});

import {
  cancelEmbeddingDownload,
  listEmbeddingModels,
  setActiveEmbeddingModel,
  startEmbeddingDownload,
  type EmbeddingModelInfo,
  type DownloadEmbeddingHandle,
} from "../../ipc/knowledge";
import { EmbeddingModelPanel } from "./EmbeddingModelPanel";

const MODELS: EmbeddingModelInfo[] = [
  {
    kind: "bge-m3",
    dim: 1024,
    approx_size_mb: 580,
    korean_score: 0.85,
    downloaded: false,
    active: false,
  },
  {
    kind: "kure-v1",
    dim: 768,
    approx_size_mb: 450,
    korean_score: 1.0,
    downloaded: false,
    active: false,
  },
  {
    kind: "multilingual-e5-small",
    dim: 384,
    approx_size_mb: 120,
    korean_score: 0.65,
    downloaded: false,
    active: false,
  },
];

beforeEach(() => {
  vi.clearAllMocks();
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("EmbeddingModelPanel", () => {
  it("3 카드를 모두 렌더링하고 한국어 점수 정렬을 적용해요", async () => {
    vi.mocked(listEmbeddingModels).mockResolvedValueOnce(MODELS);
    render(<EmbeddingModelPanel refreshIntervalMs={0} />);

    await waitFor(() =>
      expect(
        screen.getByTestId("workspace-embed-list"),
      ).toBeInTheDocument(),
    );

    const list = screen.getByTestId("workspace-embed-list");
    const items = within(list).getAllByRole("radio");
    expect(items).toHaveLength(3);
    // 정렬: korean_score 내림차순 — kure-v1(1.0) → bge-m3(0.85) → e5-small(0.65).
    expect(items[0]).toHaveAttribute(
      "data-testid",
      "workspace-embed-card-kure-v1",
    );
    expect(items[1]).toHaveAttribute(
      "data-testid",
      "workspace-embed-card-bge-m3",
    );
    expect(items[2]).toHaveAttribute(
      "data-testid",
      "workspace-embed-card-multilingual-e5-small",
    );
  });

  it("loadFailed 에러는 alert role로 노출돼요", async () => {
    vi.mocked(listEmbeddingModels).mockRejectedValueOnce(new Error("boom"));
    render(<EmbeddingModelPanel refreshIntervalMs={0} />);

    await waitFor(() =>
      expect(
        screen.getByTestId("workspace-embed-load-error"),
      ).toBeInTheDocument(),
    );
    expect(screen.getByTestId("workspace-embed-load-error")).toHaveAttribute(
      "role",
      "alert",
    );
  });

  it("'받을게요' 버튼은 startEmbeddingDownload를 호출해요", async () => {
    vi.mocked(listEmbeddingModels).mockResolvedValue(MODELS);
    const handle: DownloadEmbeddingHandle = {
      kind: "bge-m3",
      cancel: vi.fn(async () => {}),
    };
    vi.mocked(startEmbeddingDownload).mockResolvedValueOnce(handle);

    render(<EmbeddingModelPanel refreshIntervalMs={0} />);
    await waitFor(() =>
      expect(
        screen.getByTestId("workspace-embed-download-bge-m3"),
      ).toBeInTheDocument(),
    );

    const user = userEvent.setup();
    await user.click(screen.getByTestId("workspace-embed-download-bge-m3"));

    expect(startEmbeddingDownload).toHaveBeenCalledWith(
      "bge-m3",
      expect.any(Function),
    );
  });

  it("진행률 progressbar는 percent를 반영해요", async () => {
    vi.mocked(listEmbeddingModels).mockResolvedValue(MODELS);
    // noop default — 실제 listener는 mockImplementationOnce가 덮어씀.
    // TS가 let의 narrow를 closure에 적용 못해 `never`로 추론하는 문제 회피.
    let onEvent: (ev: unknown) => void = () => {};
    vi.mocked(startEmbeddingDownload).mockImplementationOnce(
      async (_kind, listener) => {
        onEvent = listener as unknown as (ev: unknown) => void;
        return { kind: "bge-m3", cancel: vi.fn(async () => {}) };
      },
    );

    render(<EmbeddingModelPanel refreshIntervalMs={0} />);
    await waitFor(() =>
      expect(
        screen.getByTestId("workspace-embed-download-bge-m3"),
      ).toBeInTheDocument(),
    );

    const user = userEvent.setup();
    await user.click(screen.getByTestId("workspace-embed-download-bge-m3"));

    // started + progress 50% 시뮬레이션.
    onEvent?.({
      kind: "started",
      model_kind: "bge-m3",
      file: "model.onnx",
      total_bytes: 1000,
    });
    onEvent?.({
      kind: "progress",
      model_kind: "bge-m3",
      file: "model.onnx",
      downloaded: 500,
      total: 1000,
    });

    await waitFor(() => {
      const bar = within(
        screen.getByTestId("workspace-embed-progress-bge-m3"),
      ).getByRole("progressbar");
      expect(bar).toHaveAttribute("aria-valuenow", "50");
    });
  });

  it("실패 이벤트는 카드별 error alert를 표시해요", async () => {
    vi.mocked(listEmbeddingModels).mockResolvedValue(MODELS);
    // noop default — 실제 listener는 mockImplementationOnce가 덮어씀.
    // TS가 let의 narrow를 closure에 적용 못해 `never`로 추론하는 문제 회피.
    let onEvent: (ev: unknown) => void = () => {};
    vi.mocked(startEmbeddingDownload).mockImplementationOnce(
      async (_kind, listener) => {
        onEvent = listener as unknown as (ev: unknown) => void;
        return { kind: "kure-v1", cancel: vi.fn(async () => {}) };
      },
    );

    render(<EmbeddingModelPanel refreshIntervalMs={0} />);
    await waitFor(() =>
      expect(
        screen.getByTestId("workspace-embed-download-kure-v1"),
      ).toBeInTheDocument(),
    );

    const user = userEvent.setup();
    await user.click(screen.getByTestId("workspace-embed-download-kure-v1"));

    onEvent?.({
      kind: "failed",
      model_kind: "kure-v1",
      error: "다운로드 도중 끊겼어요",
    });

    await waitFor(() => {
      const err = screen.getByTestId("workspace-embed-error-kure-v1");
      expect(err).toHaveTextContent("다운로드 도중 끊겼어요");
      expect(err).toHaveAttribute("role", "alert");
    });
  });

  it("이미 다운로드된 모델에는 'Activate' 버튼이 노출되고 클릭 시 setActive를 호출해요", async () => {
    const downloaded = MODELS.map((m) =>
      m.kind === "bge-m3" ? { ...m, downloaded: true } : m,
    );
    vi.mocked(listEmbeddingModels).mockResolvedValue(downloaded);
    vi.mocked(setActiveEmbeddingModel).mockResolvedValueOnce(undefined);

    render(<EmbeddingModelPanel refreshIntervalMs={0} />);
    await waitFor(() =>
      expect(
        screen.getByTestId("workspace-embed-activate-bge-m3"),
      ).toBeInTheDocument(),
    );

    const user = userEvent.setup();
    await user.click(screen.getByTestId("workspace-embed-activate-bge-m3"));

    expect(setActiveEmbeddingModel).toHaveBeenCalledWith("bge-m3");
  });

  it("active 모델에는 active 뱃지가 노출돼요 (radiogroup aria-checked)", async () => {
    const activeList = MODELS.map((m) =>
      m.kind === "kure-v1" ? { ...m, downloaded: true, active: true } : m,
    );
    vi.mocked(listEmbeddingModels).mockResolvedValue(activeList);

    render(<EmbeddingModelPanel refreshIntervalMs={0} />);
    await waitFor(() =>
      expect(
        screen.getByTestId("workspace-embed-active-kure-v1"),
      ).toBeInTheDocument(),
    );

    const card = screen.getByTestId("workspace-embed-card-kure-v1");
    expect(card).toHaveAttribute("aria-checked", "true");
    // 다른 카드는 false.
    const other = screen.getByTestId("workspace-embed-card-bge-m3");
    expect(other).toHaveAttribute("aria-checked", "false");
  });

  it("진행 중에는 'Cancel' 버튼만 노출되고 cancel 호출 시 IPC가 호출돼요", async () => {
    vi.mocked(listEmbeddingModels).mockResolvedValue(MODELS);
    // noop default — 실제 listener는 mockImplementationOnce가 덮어씀.
    let onEvent: (ev: any) => void = () => {};
    const handle: DownloadEmbeddingHandle = {
      kind: "bge-m3",
      cancel: vi.fn(async () => {}),
    };
    vi.mocked(startEmbeddingDownload).mockImplementationOnce(
      async (_kind, listener) => {
        onEvent = listener as (ev: any) => void;
        return handle;
      },
    );
    vi.mocked(cancelEmbeddingDownload).mockResolvedValueOnce(undefined);

    render(<EmbeddingModelPanel refreshIntervalMs={0} />);
    await waitFor(() =>
      expect(
        screen.getByTestId("workspace-embed-download-bge-m3"),
      ).toBeInTheDocument(),
    );

    const user = userEvent.setup();
    await user.click(screen.getByTestId("workspace-embed-download-bge-m3"));

    // started로 status=running으로 진입.
    onEvent?.({
      kind: "started",
      model_kind: "bge-m3",
      file: "model.onnx",
      total_bytes: null,
    });

    await waitFor(() =>
      expect(
        screen.getByTestId("workspace-embed-cancel-bge-m3"),
      ).toBeInTheDocument(),
    );
    // download 버튼은 사라져야 해요.
    expect(
      screen.queryByTestId("workspace-embed-download-bge-m3"),
    ).not.toBeInTheDocument();

    await user.click(screen.getByTestId("workspace-embed-cancel-bge-m3"));
    // handle.cancel이 우선 호출 — handle이 set돼 있으면 그쪽 경로.
    expect(handle.cancel).toHaveBeenCalled();
  });

  it("activate 실패는 panel-level error로 노출돼요", async () => {
    const downloaded = MODELS.map((m) =>
      m.kind === "bge-m3" ? { ...m, downloaded: true } : m,
    );
    vi.mocked(listEmbeddingModels).mockResolvedValue(downloaded);
    vi.mocked(setActiveEmbeddingModel).mockRejectedValueOnce(
      new Error("activate boom"),
    );

    render(<EmbeddingModelPanel refreshIntervalMs={0} />);
    await waitFor(() =>
      expect(
        screen.getByTestId("workspace-embed-activate-bge-m3"),
      ).toBeInTheDocument(),
    );

    const user = userEvent.setup();
    await user.click(screen.getByTestId("workspace-embed-activate-bge-m3"));

    await waitFor(() =>
      expect(
        screen.getByTestId("workspace-embed-activate-error"),
      ).toBeInTheDocument(),
    );
  });

  it("빈 리스트는 empty 상태를 노출해요", async () => {
    vi.mocked(listEmbeddingModels).mockResolvedValueOnce([]);
    render(<EmbeddingModelPanel refreshIntervalMs={0} />);
    await waitFor(() =>
      expect(screen.getByTestId("workspace-embed-empty")).toBeInTheDocument(),
    );
  });
});
