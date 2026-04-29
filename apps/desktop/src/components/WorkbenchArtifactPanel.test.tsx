/**
 * @vitest-environment jsdom
 */
// WorkbenchArtifactPanel — Phase 8'.0.c. Workbench artifact retention 패널 단위 테스트.

import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) => {
      if (!opts) return key;
      return `${key} ${JSON.stringify(opts)}`;
    },
  }),
}));

vi.mock("../ipc/workbench", () => ({
  getArtifactStats: vi.fn(),
  cleanupArtifactsNow: vi.fn(),
}));

import {
  cleanupArtifactsNow,
  getArtifactStats,
  type ArtifactStats,
  type CleanupReport,
} from "../ipc/workbench";

import { WorkbenchArtifactPanel } from "./WorkbenchArtifactPanel";

const STATS_2_RUNS: ArtifactStats = {
  run_count: 2,
  total_bytes: 12_345_678,
  oldest_modified_unix: 1700000000,
  policy: {
    max_age_days: 30,
    max_total_size_bytes: 10 * 1024 * 1024 * 1024,
  },
};

const EMPTY_STATS: ArtifactStats = {
  run_count: 0,
  total_bytes: 0,
  oldest_modified_unix: 0,
  policy: STATS_2_RUNS.policy,
};

const CLEANUP_OK: CleanupReport = {
  removed_count: 1,
  freed_bytes: 5_000_000,
  kept_count: 1,
  remaining_bytes: 7_345_678,
};

beforeEach(() => {
  vi.mocked(getArtifactStats).mockResolvedValue(STATS_2_RUNS);
  vi.mocked(cleanupArtifactsNow).mockResolvedValue(CLEANUP_OK);
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("WorkbenchArtifactPanel — 기본 렌더", () => {
  it("title / description 노출", async () => {
    render(<WorkbenchArtifactPanel />);
    await waitFor(() => expect(getArtifactStats).toHaveBeenCalled());
    expect(
      screen.getByText("screens.settings.workbench.artifacts.title"),
    ).toBeTruthy();
    expect(
      screen.getByText("screens.settings.workbench.artifacts.description"),
    ).toBeTruthy();
  });

  it("stats 로드되면 run count + total bytes 표시", async () => {
    render(<WorkbenchArtifactPanel />);
    await waitFor(() => expect(getArtifactStats).toHaveBeenCalled());
    const panel = screen.getByTestId("workbench-artifact-panel");
    // run count 라벨 포함.
    await waitFor(() => {
      expect(
        within(panel).getByText(
          /screens\.settings\.workbench\.artifacts\.runCount/,
        ),
      ).toBeTruthy();
    });
  });

  it("로딩 중이면 loading hint 노출", async () => {
    // 완료 안 된 promise.
    let resolve: (v: ArtifactStats) => void = () => {};
    vi.mocked(getArtifactStats).mockReturnValueOnce(
      new Promise<ArtifactStats>((r) => {
        resolve = r;
      }),
    );
    render(<WorkbenchArtifactPanel />);
    expect(
      screen.getByText("screens.settings.workbench.artifacts.loading"),
    ).toBeTruthy();
    resolve(EMPTY_STATS);
    await waitFor(() => expect(getArtifactStats).toHaveBeenCalled());
  });

  it("로드 실패 시 loadFailed 에러", async () => {
    vi.mocked(getArtifactStats).mockRejectedValueOnce(new Error("ipc-fail"));
    render(<WorkbenchArtifactPanel />);
    await waitFor(() => {
      expect(screen.getByTestId("workbench-artifact-error")).toBeTruthy();
    });
  });
});

describe("WorkbenchArtifactPanel — cleanup", () => {
  it("'지금 정리할게요' 버튼 클릭 시 cleanupArtifactsNow 호출", async () => {
    render(<WorkbenchArtifactPanel />);
    await waitFor(() => expect(getArtifactStats).toHaveBeenCalled());
    const btn = screen.getByTestId("workbench-artifact-cleanup-btn");
    await userEvent.click(btn);
    await waitFor(() => expect(cleanupArtifactsNow).toHaveBeenCalled());
  });

  it("cleanup 성공 시 결과 메시지 노출 (aria-live)", async () => {
    render(<WorkbenchArtifactPanel />);
    await waitFor(() => expect(getArtifactStats).toHaveBeenCalled());
    const btn = screen.getByTestId("workbench-artifact-cleanup-btn");
    await userEvent.click(btn);
    await waitFor(() => {
      expect(screen.getByTestId("workbench-artifact-info")).toBeTruthy();
    });
  });

  it("cleanup 실패 시 에러 노출", async () => {
    vi.mocked(cleanupArtifactsNow).mockRejectedValueOnce(new Error("nope"));
    render(<WorkbenchArtifactPanel />);
    await waitFor(() => expect(getArtifactStats).toHaveBeenCalled());
    const btn = screen.getByTestId("workbench-artifact-cleanup-btn");
    await userEvent.click(btn);
    await waitFor(() => {
      expect(screen.getByTestId("workbench-artifact-error")).toBeTruthy();
    });
  });
});

describe("WorkbenchArtifactPanel — a11y", () => {
  it("axe 위반 0건", async () => {
    const { container } = render(<WorkbenchArtifactPanel />);
    await waitFor(() => expect(getArtifactStats).toHaveBeenCalled());
    const result = await axe.run(container);
    expect(result.violations).toEqual([]);
  });
});
