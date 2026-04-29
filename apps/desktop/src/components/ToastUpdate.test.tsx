/**
 * @vitest-environment jsdom
 */
// ToastUpdate — Phase 6'.b 자동 갱신 토스트 테스트.

import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import axe from "axe-core";

import {
  ToastUpdate,
  cleanupOlderSkippedVersions,
  compareVersion,
} from "./ToastUpdate";
import type { ReleaseInfo } from "../ipc/updater";

// Phase 8'.b.2 — plugin-shell `open` 모킹. 테스트가 Tauri runtime 없이 호출 흐름만 검증.
const openExternalMock = vi.hoisted(() => vi.fn().mockResolvedValue(undefined));
vi.mock("@tauri-apps/plugin-shell", () => ({
  open: openExternalMock,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) =>
      opts ? `${key}:${JSON.stringify(opts)}` : key,
  }),
}));

const FIXTURE_RELEASE: ReleaseInfo = {
  version: "1.2.0",
  published_at_iso: "2026-04-15T08:30:00Z",
  url: "https://github.com/anthropics/lmmaster/releases/tag/v1.2.0",
  notes: "## 변경사항",
};

beforeEach(() => {
  if (typeof globalThis.localStorage !== "undefined") {
    globalThis.localStorage.clear();
  }
  openExternalMock.mockClear();
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("ToastUpdate", () => {
  it("새 버전 정보 + 한국어 날짜 포맷으로 렌더", () => {
    const onSkip = vi.fn();
    const onDismiss = vi.fn();
    render(
      <ToastUpdate
        release={FIXTURE_RELEASE}
        currentVersion="1.1.0"
        onSkip={onSkip}
        onDismiss={onDismiss}
      />,
    );
    // i18n key + opts mock — JSON stringify 안에 version 포함.
    expect(screen.getByTestId("toast-update")).toBeTruthy();
    expect(
      screen.getByText(/toast\.update\.title.*"version":"1\.2\.0"/),
    ).toBeTruthy();
    // detail에 한국어 날짜 (시간대에 따라 4월 15일 또는 16일이 될 수 있음)
    const detail = screen.getByText(/toast\.update\.detail/);
    expect(detail.textContent).toMatch(/2026년 4월 1[56]일/);
    expect(detail.textContent).toContain('"currentVersion":"1.1.0"');
  });

  it("이번 버전 건너뛰기 → onSkip 호출 + localStorage 저장", async () => {
    const user = userEvent.setup();
    const onSkip = vi.fn();
    const onDismiss = vi.fn();
    render(
      <ToastUpdate
        release={FIXTURE_RELEASE}
        currentVersion="1.1.0"
        onSkip={onSkip}
        onDismiss={onDismiss}
      />,
    );
    await user.click(screen.getByTestId("toast-update-skip"));
    expect(onSkip).toHaveBeenCalledWith("1.2.0");
    expect(globalThis.localStorage.getItem("lmmaster.update.skipped.1.2.0")).toBe(
      "true",
    );
  });

  it("'업데이트 보기' 클릭 → plugin-shell open으로 release URL 호출 (Phase 8'.b.2)", async () => {
    const user = userEvent.setup();
    const onSkip = vi.fn();
    const onDismiss = vi.fn();
    render(
      <ToastUpdate
        release={FIXTURE_RELEASE}
        currentVersion="1.1.0"
        onSkip={onSkip}
        onDismiss={onDismiss}
      />,
    );
    await user.click(screen.getByTestId("toast-update-view"));
    expect(openExternalMock).toHaveBeenCalledWith(FIXTURE_RELEASE.url);
  });

  it("X 버튼 → onDismiss 호출 (localStorage 미저장)", async () => {
    const user = userEvent.setup();
    const onSkip = vi.fn();
    const onDismiss = vi.fn();
    render(
      <ToastUpdate
        release={FIXTURE_RELEASE}
        currentVersion="1.1.0"
        onSkip={onSkip}
        onDismiss={onDismiss}
      />,
    );
    await user.click(screen.getByTestId("toast-update-close"));
    expect(onDismiss).toHaveBeenCalled();
    expect(onSkip).not.toHaveBeenCalled();
    expect(
      globalThis.localStorage.getItem("lmmaster.update.skipped.1.2.0"),
    ).toBeNull();
  });

  it("Esc 키 → onDismiss 호출", async () => {
    const onSkip = vi.fn();
    const onDismiss = vi.fn();
    render(
      <ToastUpdate
        release={FIXTURE_RELEASE}
        currentVersion="1.1.0"
        onSkip={onSkip}
        onDismiss={onDismiss}
      />,
    );
    const ev = new KeyboardEvent("keydown", { key: "Escape" });
    window.dispatchEvent(ev);
    await waitFor(() => {
      expect(onDismiss).toHaveBeenCalled();
    });
  });

  it("같은 version이 localStorage에 skipped면 마운트 즉시 onDismiss + null 렌더", async () => {
    globalThis.localStorage.setItem(
      "lmmaster.update.skipped.1.2.0",
      "true",
    );
    const onSkip = vi.fn();
    const onDismiss = vi.fn();
    const { container } = render(
      <ToastUpdate
        release={FIXTURE_RELEASE}
        currentVersion="1.1.0"
        onSkip={onSkip}
        onDismiss={onDismiss}
      />,
    );
    // 마운트 즉시 onDismiss 호출.
    await waitFor(() => {
      expect(onDismiss).toHaveBeenCalled();
    });
    // null 렌더 — toast-update 노드 없음.
    expect(container.querySelector("[data-testid='toast-update']")).toBeNull();
    expect(onSkip).not.toHaveBeenCalled();
  });

  it("close 버튼이 마운트 시 focus를 받음", async () => {
    const onSkip = vi.fn();
    const onDismiss = vi.fn();
    render(
      <ToastUpdate
        release={FIXTURE_RELEASE}
        currentVersion="1.1.0"
        onSkip={onSkip}
        onDismiss={onDismiss}
      />,
    );
    await waitFor(() => {
      expect(document.activeElement).toBe(screen.getByTestId("toast-update-close"));
    });
  });

  it("URL이 빈 문자열이면 '업데이트 보기' 버튼이 disabled", () => {
    const onSkip = vi.fn();
    const onDismiss = vi.fn();
    const release: ReleaseInfo = { ...FIXTURE_RELEASE, url: "" };
    render(
      <ToastUpdate
        release={release}
        currentVersion="1.1.0"
        onSkip={onSkip}
        onDismiss={onDismiss}
      />,
    );
    const btn = screen.getByTestId("toast-update-view") as HTMLButtonElement;
    expect(btn.disabled).toBe(true);
  });

  it("ISO 파싱 실패 시 원본 문자열 detail에 노출", () => {
    const onSkip = vi.fn();
    const onDismiss = vi.fn();
    const release: ReleaseInfo = {
      ...FIXTURE_RELEASE,
      published_at_iso: "not-an-iso",
    };
    render(
      <ToastUpdate
        release={release}
        currentVersion="1.1.0"
        onSkip={onSkip}
        onDismiss={onDismiss}
      />,
    );
    const detail = screen.getByText(/toast\.update\.detail/);
    expect(detail.textContent).toContain("not-an-iso");
  });

  it("a11y — 위반 0건 (axe)", async () => {
    const { container } = render(
      <ToastUpdate
        release={FIXTURE_RELEASE}
        currentVersion="1.1.0"
        onSkip={vi.fn()}
        onDismiss={vi.fn()}
      />,
    );
    const results = await axe.run(container, {
      rules: {
        // 토스트는 region 책임 밖.
        region: { enabled: false },
      },
    });
    expect(results.violations).toEqual([]);
  });
});

describe("compareVersion (Phase 8'.a.2)", () => {
  it("major.minor.patch 비교", () => {
    expect(compareVersion("1.0.0", "1.0.0")).toBe(0);
    expect(compareVersion("1.0.0", "1.0.1")).toBeLessThan(0);
    expect(compareVersion("1.0.1", "1.0.0")).toBeGreaterThan(0);
    expect(compareVersion("0.9.9", "1.0.0")).toBeLessThan(0);
    expect(compareVersion("2.0.0", "1.99.99")).toBeGreaterThan(0);
  });

  it("v 접두사는 비교에 영향 없음", () => {
    // v1.2.0 < v2.0.0 — v 접두사가 있어도 numeric ordering 유지.
    expect(compareVersion("v1.0.0", "v2.0.0")).toBeLessThan(0);
    // v 접두사가 한쪽에만 있어도 major.minor.patch 비교는 동일 결과.
    expect(compareVersion("v1.0.0", "2.0.0")).toBeLessThan(0);
    expect(compareVersion("1.0.0", "v2.0.0")).toBeLessThan(0);
  });

  it("pre-release는 core 버전 기준 무시", () => {
    expect(compareVersion("1.2.3-rc1", "1.2.4")).toBeLessThan(0);
  });
});

describe("cleanupOlderSkippedVersions (Phase 8'.a.2)", () => {
  beforeEach(() => {
    globalThis.localStorage.clear();
  });

  it("새 버전 도착 시 이전(낮은) 버전 키 제거", () => {
    globalThis.localStorage.setItem("lmmaster.update.skipped.1.0.0", "true");
    globalThis.localStorage.setItem("lmmaster.update.skipped.1.0.5", "true");
    cleanupOlderSkippedVersions("1.1.0");
    expect(
      globalThis.localStorage.getItem("lmmaster.update.skipped.1.0.0"),
    ).toBeNull();
    expect(
      globalThis.localStorage.getItem("lmmaster.update.skipped.1.0.5"),
    ).toBeNull();
  });

  it("같은 버전 또는 더 높은 버전 키는 보존 (downgrade 안전)", () => {
    globalThis.localStorage.setItem("lmmaster.update.skipped.1.2.0", "true");
    globalThis.localStorage.setItem("lmmaster.update.skipped.2.0.0", "true");
    cleanupOlderSkippedVersions("1.2.0");
    // 같은 버전은 보존.
    expect(
      globalThis.localStorage.getItem("lmmaster.update.skipped.1.2.0"),
    ).toBe("true");
    // 더 높은 버전도 보존 (downgrade 시나리오 — 사용자가 1.2.0을 받지 않고 2.0.0을 본 경우).
    expect(
      globalThis.localStorage.getItem("lmmaster.update.skipped.2.0.0"),
    ).toBe("true");
  });

  it("다른 prefix 키는 건드리지 않음", () => {
    globalThis.localStorage.setItem("lmmaster.update.skipped.1.0.0", "true");
    globalThis.localStorage.setItem("lmmaster.unrelated.value", "keep");
    globalThis.localStorage.setItem("other.app.skipped.1.0.0", "keep");
    cleanupOlderSkippedVersions("2.0.0");
    expect(
      globalThis.localStorage.getItem("lmmaster.update.skipped.1.0.0"),
    ).toBeNull();
    expect(globalThis.localStorage.getItem("lmmaster.unrelated.value")).toBe(
      "keep",
    );
    expect(globalThis.localStorage.getItem("other.app.skipped.1.0.0")).toBe(
      "keep",
    );
  });

  it("ToastUpdate 마운트 시 자동 정리 — 1.0.0 skipped + 1.2.0 도착 → 1.0.0 키 제거", async () => {
    globalThis.localStorage.setItem("lmmaster.update.skipped.1.0.0", "true");
    const { container: _ } = render(
      <ToastUpdate
        release={FIXTURE_RELEASE}
        currentVersion="1.1.0"
        onSkip={vi.fn()}
        onDismiss={vi.fn()}
      />,
    );
    await waitFor(() => {
      expect(
        globalThis.localStorage.getItem("lmmaster.update.skipped.1.0.0"),
      ).toBeNull();
    });
  });
});
