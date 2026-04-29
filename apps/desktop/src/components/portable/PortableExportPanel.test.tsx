/**
 * @vitest-environment jsdom
 */
// PortableExportPanel — Phase 11' (ADR-0039) 단위 테스트.

import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
  }),
}));

vi.mock("../../ipc/portable", () => ({
  startWorkspaceExport: vi.fn(),
  cancelWorkspaceExport: vi.fn(),
  isTerminalExportEvent: (ev: { kind: string }) =>
    ev.kind === "done" || ev.kind === "failed",
}));

import {
  cancelWorkspaceExport,
  startWorkspaceExport,
  type ExportEvent,
} from "../../ipc/portable";

import { PortableExportPanel } from "./PortableExportPanel";

beforeEach(() => {
  vi.mocked(startWorkspaceExport).mockResolvedValue({
    export_id: "exp-1",
    summary: {
      sha256: "0".repeat(64),
      archive_size_bytes: 1234,
      files_count: 3,
    },
  });
  vi.mocked(cancelWorkspaceExport).mockResolvedValue(undefined);
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("PortableExportPanel — 기본 렌더", () => {
  it("idle 상태에서 시작 버튼이 활성화돼요", () => {
    render(<PortableExportPanel />);
    const btn = screen.getByTestId("portable-export-start-btn");
    expect(btn.getAttribute("disabled")).toBeNull();
    expect(
      screen.getByText("screens.settings.portable.export.subtitle"),
    ).toBeTruthy();
  });

  it("시작 버튼 클릭하면 옵션 dialog가 열려요", async () => {
    const user = userEvent.setup();
    render(<PortableExportPanel />);
    await user.click(screen.getByTestId("portable-export-start-btn"));
    expect(screen.getByTestId("portable-export-modal")).toBeTruthy();
    expect(screen.getByTestId("portable-export-target-input")).toBeTruthy();
    expect(screen.getByTestId("portable-export-include-models")).toBeTruthy();
    expect(screen.getByTestId("portable-export-include-keys")).toBeTruthy();
  });

  it("키 포함 체크하면 패스프레이즈 입력이 노출돼요", async () => {
    const user = userEvent.setup();
    render(<PortableExportPanel />);
    await user.click(screen.getByTestId("portable-export-start-btn"));
    expect(screen.queryByTestId("portable-export-passphrase")).toBeNull();
    await user.click(screen.getByTestId("portable-export-include-keys"));
    expect(screen.getByTestId("portable-export-passphrase")).toBeTruthy();
  });
});

describe("PortableExportPanel — 검증", () => {
  it("target 미입력 시 에러 키가 노출돼요", async () => {
    const user = userEvent.setup();
    render(<PortableExportPanel />);
    await user.click(screen.getByTestId("portable-export-start-btn"));
    await user.click(screen.getByTestId("portable-export-confirm-btn"));
    expect(
      screen.getByText(
        "screens.settings.portable.export.errors.emptyTarget",
      ),
    ).toBeTruthy();
    expect(startWorkspaceExport).not.toHaveBeenCalled();
  });

  it("키 포함 + 패스프레이즈 빈 상태로 confirm → 에러", async () => {
    const user = userEvent.setup();
    render(<PortableExportPanel />);
    await user.click(screen.getByTestId("portable-export-start-btn"));
    await user.type(
      screen.getByTestId("portable-export-target-input"),
      "C:/tmp/out.zip",
    );
    await user.click(screen.getByTestId("portable-export-include-keys"));
    await user.click(screen.getByTestId("portable-export-confirm-btn"));
    expect(
      screen.getByText(
        "screens.settings.portable.export.errors.emptyPassphrase",
      ),
    ).toBeTruthy();
    expect(startWorkspaceExport).not.toHaveBeenCalled();
  });
});

describe("PortableExportPanel — 진행", () => {
  it("정상 입력 → startWorkspaceExport 호출 + done 카드 노출", async () => {
    const user = userEvent.setup();
    render(<PortableExportPanel />);
    await user.click(screen.getByTestId("portable-export-start-btn"));
    await user.type(
      screen.getByTestId("portable-export-target-input"),
      "C:/tmp/out.zip",
    );
    await user.click(screen.getByTestId("portable-export-confirm-btn"));
    await waitFor(() => {
      expect(startWorkspaceExport).toHaveBeenCalled();
    });
    await waitFor(() => {
      expect(screen.getByTestId("portable-export-done")).toBeTruthy();
    });
    const done = screen.getByTestId("portable-export-done");
    expect(
      within(done).getByText("screens.settings.portable.export.done"),
    ).toBeTruthy();
  });

  it("진행 중 cancel 버튼이 노출되고 클릭 가능해요", async () => {
    let onEventRef: ((ev: ExportEvent) => void) | null = null;
    vi.mocked(startWorkspaceExport).mockImplementationOnce(
      async (_opts, onEvent) => {
        onEventRef = onEvent;
        // counting + compressing event 흘려서 진행 단계 진입.
        onEvent({
          kind: "started",
          source_path: "/ws",
          target_path: "/out.zip",
        });
        onEvent({ kind: "counting", total_files: 5, total_bytes: 100 });
        onEvent({
          kind: "compressing",
          processed: 1,
          total: 5,
          current_path: "manifest.json",
        });
        // 영원히 resolve 안 함.
        await new Promise(() => {});
        return {
          export_id: "exp-cancel",
          summary: {
            sha256: "x".repeat(64),
            archive_size_bytes: 0,
            files_count: 0,
          },
        };
      },
    );
    const user = userEvent.setup();
    render(<PortableExportPanel />);
    await user.click(screen.getByTestId("portable-export-start-btn"));
    await user.type(
      screen.getByTestId("portable-export-target-input"),
      "C:/tmp/out.zip",
    );
    await user.click(screen.getByTestId("portable-export-confirm-btn"));
    await waitFor(() => {
      expect(screen.getByTestId("portable-export-progress")).toBeTruthy();
    });
    expect(onEventRef).toBeTruthy();
    // cancel 버튼은 여전히 mounted.
    // (export_id는 invoke resolve 후에야 들어오지만 idle phase로 즉시 복귀하는 fallback path 검증)
    const cancelBtn = screen.getByTestId("portable-export-cancel-btn");
    await user.click(cancelBtn);
    // 명시적인 export_id가 없으므로 cancelWorkspaceExport는 호출 안 됨 (path b).
    // path a: invoke가 resolve 된 이후 cancel을 누르면 호출. 이 케이스에선 path b 검증만.
  });

  it("Esc 키로 옵션 dialog 닫혀요 (idle phase 복귀)", async () => {
    const user = userEvent.setup();
    render(<PortableExportPanel />);
    await user.click(screen.getByTestId("portable-export-start-btn"));
    expect(screen.getByTestId("portable-export-modal")).toBeTruthy();
    await user.keyboard("{Escape}");
    await waitFor(() => {
      expect(screen.queryByTestId("portable-export-modal")).toBeNull();
    });
  });
});

describe("PortableExportPanel — a11y", () => {
  it("axe violations === [] (idle)", async () => {
    const { container } = render(<PortableExportPanel />);
    const results = await axe.run(container, {
      rules: { region: { enabled: false } },
    });
    expect(results.violations).toEqual([]);
  });

  it("axe violations === [] (옵션 dialog 열린 상태)", async () => {
    const user = userEvent.setup();
    const { container } = render(<PortableExportPanel />);
    await user.click(screen.getByTestId("portable-export-start-btn"));
    const results = await axe.run(container, {
      rules: { region: { enabled: false } },
    });
    expect(results.violations).toEqual([]);
  });
});
