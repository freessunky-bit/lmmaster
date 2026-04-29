/**
 * @vitest-environment jsdom
 */
// ActiveWorkspaceContext Phase 8'.1 — initial fetch / setActive optimistic /
// workspaces://changed 이벤트 listener 검증.

import { render, screen, waitFor, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/workspaces", () => ({
  listWorkspaces: vi.fn(),
  getActiveWorkspace: vi.fn(),
  createWorkspace: vi.fn(),
  renameWorkspace: vi.fn(),
  deleteWorkspace: vi.fn(),
  setActiveWorkspace: vi.fn(),
  onWorkspacesChanged: vi.fn(),
}));

import * as ipc from "../ipc/workspaces";
import {
  ActiveWorkspaceProvider,
  useActiveWorkspace,
} from "./ActiveWorkspaceContext";

const listMock = vi.mocked(ipc.listWorkspaces);
const getActiveMock = vi.mocked(ipc.getActiveWorkspace);
const setActiveMock = vi.mocked(ipc.setActiveWorkspace);
const onChangedMock = vi.mocked(ipc.onWorkspacesChanged);

const W1 = {
  id: "w-1",
  name: "첫 번째",
  description: null,
  created_at_iso: "2026-04-01T00:00:00Z",
  last_used_iso: "2026-04-28T00:00:00Z",
};
const W2 = {
  id: "w-2",
  name: "두 번째",
  description: null,
  created_at_iso: "2026-04-02T00:00:00Z",
  last_used_iso: null,
};

beforeEach(() => {
  listMock.mockReset();
  getActiveMock.mockReset();
  setActiveMock.mockReset();
  onChangedMock.mockReset();
  // localStorage clear.
  try {
    window.localStorage.clear();
  } catch {
    /* ignore */
  }
});

afterEach(() => {
  vi.clearAllMocks();
});

function HookProbe() {
  const { active, workspaces, loading, setActive } = useActiveWorkspace();
  return (
    <div>
      <div data-testid="active-id">{active?.id ?? "none"}</div>
      <div data-testid="active-name">{active?.name ?? ""}</div>
      <div data-testid="ws-count">{workspaces.length}</div>
      <div data-testid="loading">{loading ? "yes" : "no"}</div>
      <button
        type="button"
        data-testid="switch-w2"
        onClick={() => {
          void setActive(W2.id);
        }}
      >
        switch
      </button>
    </div>
  );
}

describe("ActiveWorkspaceContext Phase 8'.1", () => {
  it("초기 마운트 — list/getActive 호출 + active/list 갱신 + loading=false", async () => {
    listMock.mockResolvedValue([W1, W2]);
    getActiveMock.mockResolvedValue(W1);
    onChangedMock.mockResolvedValue(() => {
      /* unlisten noop */
    });

    render(
      <ActiveWorkspaceProvider>
        <HookProbe />
      </ActiveWorkspaceProvider>,
    );

    await waitFor(() => {
      expect(listMock).toHaveBeenCalled();
      expect(getActiveMock).toHaveBeenCalled();
    });
    await waitFor(() => {
      expect(screen.getByTestId("active-id")).toHaveTextContent(W1.id);
      expect(screen.getByTestId("active-name")).toHaveTextContent(W1.name);
      expect(screen.getByTestId("ws-count")).toHaveTextContent("2");
      expect(screen.getByTestId("loading")).toHaveTextContent("no");
    });
  });

  it("초기 fetch 실패 — loading=false + active=null", async () => {
    listMock.mockRejectedValue(new Error("network"));
    getActiveMock.mockRejectedValue(new Error("network"));
    onChangedMock.mockResolvedValue(() => {
      /* unlisten noop */
    });

    render(
      <ActiveWorkspaceProvider>
        <HookProbe />
      </ActiveWorkspaceProvider>,
    );

    await waitFor(() => {
      expect(screen.getByTestId("loading")).toHaveTextContent("no");
    });
    expect(screen.getByTestId("active-id")).toHaveTextContent("none");
  });

  it("setActive 호출 → setActiveWorkspace IPC 호출 + 즉시 active 반영", async () => {
    listMock.mockResolvedValue([W1, W2]);
    getActiveMock.mockResolvedValue(W1);
    setActiveMock.mockResolvedValue(undefined);
    onChangedMock.mockResolvedValue(() => {
      /* unlisten noop */
    });

    const user = userEvent.setup();
    render(
      <ActiveWorkspaceProvider>
        <HookProbe />
      </ActiveWorkspaceProvider>,
    );
    await waitFor(() => {
      expect(screen.getByTestId("active-id")).toHaveTextContent(W1.id);
    });
    await user.click(screen.getByTestId("switch-w2"));
    await waitFor(() => {
      expect(setActiveMock).toHaveBeenCalledWith(W2.id);
    });
    // optimistic — active가 W2로 즉시 갱신 (이벤트 도착 이전이라도 OK).
    await waitFor(() => {
      expect(screen.getByTestId("active-id")).toHaveTextContent(W2.id);
    });
  });

  it("workspaces://changed 이벤트 도착 시 active/list 재구독", async () => {
    listMock.mockResolvedValue([W1, W2]);
    getActiveMock.mockResolvedValue(W1);
    type WInfo = typeof W1 | typeof W2;
    let eventCallback:
      | ((p: { active_id: string; workspaces: WInfo[] }) => void)
      | null = null;
    onChangedMock.mockImplementation(async (cb) => {
      eventCallback = cb;
      return () => {
        /* unlisten noop */
      };
    });

    render(
      <ActiveWorkspaceProvider>
        <HookProbe />
      </ActiveWorkspaceProvider>,
    );
    await waitFor(() => {
      expect(onChangedMock).toHaveBeenCalled();
      expect(eventCallback).toBeTruthy();
    });
    // 이벤트 시뮬레이션 — active를 W2로 변경.
    act(() => {
      eventCallback!({ active_id: W2.id, workspaces: [W1, W2] });
    });
    await waitFor(() => {
      expect(screen.getByTestId("active-id")).toHaveTextContent(W2.id);
    });
  });

  it("localStorage hydration — 초기 마운트 시 hint 사용", async () => {
    try {
      window.localStorage.setItem("lmmaster.active_workspace_id", W2.id);
    } catch {
      /* ignore */
    }
    listMock.mockResolvedValue([W1, W2]);
    getActiveMock.mockResolvedValue(W2);
    onChangedMock.mockResolvedValue(() => {
      /* unlisten noop */
    });

    render(
      <ActiveWorkspaceProvider>
        <HookProbe />
      </ActiveWorkspaceProvider>,
    );
    // 초기 hydration — backend 호출 전에도 active.id가 W2.
    await waitFor(() => {
      expect(screen.getByTestId("active-id")).toHaveTextContent(W2.id);
    });
  });

  it("Provider 없이 useActiveWorkspace 호출 → 에러", () => {
    const errSpy = vi.spyOn(console, "error").mockImplementation(() => {
      /* swallow */
    });
    expect(() => {
      render(<HookProbe />);
    }).toThrow();
    errSpy.mockRestore();
  });
});
