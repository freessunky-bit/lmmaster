/**
 * @vitest-environment jsdom
 */
// Settings 페이지 — 4 카테고리 nav + form panels + i18n + a11y 테스트.

import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import axe from "axe-core";

const changeLanguageMock = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) =>
      opts ? `${key}:${JSON.stringify(opts)}` : key,
    i18n: {
      changeLanguage: changeLanguageMock,
      resolvedLanguage: "ko",
    },
  }),
}));

vi.mock("../ipc/workspace", () => ({
  getWorkspaceFingerprint: vi.fn(),
  checkWorkspaceRepair: vi.fn(),
}));

vi.mock("../ipc/updater", () => ({
  checkForUpdate: vi.fn(),
  getAutoUpdateStatus: vi.fn(),
  startAutoUpdatePoller: vi.fn(),
  stopAutoUpdatePoller: vi.fn(),
}));

vi.mock("../ipc/catalog-refresh", () => ({
  refreshCatalogNow: vi.fn().mockResolvedValue({
    at_ms: 0,
    fetched_count: 0,
    failed_count: 0,
    outcome: "ok",
  }),
  getLastCatalogRefresh: vi.fn().mockResolvedValue(null),
  onCatalogRefreshed: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("../ipc/pipelines", () => ({
  listPipelines: vi.fn().mockResolvedValue([
    {
      id: "pii-redact",
      display_name_ko: "개인정보 보호 필터",
      description_ko: "주민·휴대폰·카드·이메일을 가려요",
      enabled: true,
    },
    {
      id: "token-quota",
      display_name_ko: "토큰 한도 관리",
      description_ko: "초과 요청을 막아요",
      enabled: true,
    },
    {
      id: "observability",
      display_name_ko: "관찰성 로그",
      description_ko: "요청 메타를 남겨요",
      enabled: true,
    },
  ]),
  getAuditLog: vi.fn().mockResolvedValue([]),
  setPipelineEnabled: vi.fn().mockResolvedValue(undefined),
  clearAuditLog: vi.fn().mockResolvedValue(undefined),
  getPipelinesConfig: vi.fn().mockResolvedValue({
    pii_redact_enabled: true,
    token_quota_enabled: true,
    observability_enabled: true,
  }),
}));

import {
  checkWorkspaceRepair,
  getWorkspaceFingerprint,
  type WorkspaceStatus,
} from "../ipc/workspace";
import {
  checkForUpdate,
  getAutoUpdateStatus,
  startAutoUpdatePoller,
  stopAutoUpdatePoller,
  type PollerStatus,
  type UpdateEvent,
} from "../ipc/updater";
import { Settings } from "./Settings";

const FIXTURE_WORKSPACE: WorkspaceStatus = {
  fingerprint: {
    os: "windows",
    arch: "x86_64",
    gpu_class: "nvidia",
    vram_bucket_mb: 8192,
    ram_bucket_mb: 16384,
    fingerprint_hash: "abcdef0123",
  },
  previous: null,
  tier: "green",
  workspace_root: "C:\\Users\\me\\AppData\\Local\\LMmaster",
};

const IDLE_POLLER: PollerStatus = {
  active: false,
  repo: null,
  interval_secs: null,
  last_check_iso: null,
};

const ACTIVE_POLLER: PollerStatus = {
  active: true,
  repo: "anthropics/lmmaster",
  interval_secs: 6 * 3600,
  last_check_iso: "2026-04-28T01:23:45Z",
};

beforeEach(() => {
  vi.mocked(getWorkspaceFingerprint).mockResolvedValue(FIXTURE_WORKSPACE);
  vi.mocked(getAutoUpdateStatus).mockResolvedValue(IDLE_POLLER);
  vi.mocked(startAutoUpdatePoller).mockResolvedValue();
  vi.mocked(stopAutoUpdatePoller).mockResolvedValue();
  vi.mocked(checkForUpdate).mockResolvedValue({
    check_id: "c1",
    cancel: vi.fn().mockResolvedValue(undefined),
  });
  changeLanguageMock.mockClear();
});

afterEach(() => {
  vi.clearAllMocks();
  if (typeof globalThis.localStorage !== "undefined") {
    globalThis.localStorage.clear();
  }
});

describe("Settings 페이지", () => {
  it("4 카테고리 nav 모두 렌더 + 일반이 default", async () => {
    render(<Settings />);
    expect(screen.getByTestId("settings-category-general")).toBeTruthy();
    expect(screen.getByTestId("settings-category-workspace")).toBeTruthy();
    expect(screen.getByTestId("settings-category-catalog")).toBeTruthy();
    expect(screen.getByTestId("settings-category-advanced")).toBeTruthy();
    // 일반이 active.
    expect(
      screen.getByTestId("settings-category-general").getAttribute("aria-checked"),
    ).toBe("true");
    // 일반 panel — language fieldset 노출.
    expect(screen.getByText("screens.settings.general.language")).toBeTruthy();
  });

  it("카테고리 변경 시 form panel 변경", async () => {
    const user = userEvent.setup();
    render(<Settings />);
    expect(screen.getByText("screens.settings.general.language")).toBeTruthy();

    await user.click(screen.getByTestId("settings-category-workspace"));
    await waitFor(() => {
      expect(screen.getByText("screens.settings.workspace.path")).toBeTruthy();
    });
    expect(screen.queryByText("screens.settings.general.language")).toBeNull();

    await user.click(screen.getByTestId("settings-category-catalog"));
    await waitFor(() => {
      expect(screen.getByText("screens.settings.catalog.registryUrl")).toBeTruthy();
    });

    await user.click(screen.getByTestId("settings-category-advanced"));
    await waitFor(() => {
      expect(screen.getByText("screens.settings.advanced.gemini")).toBeTruthy();
    });
  });

  it("일반 → 자가스캔 주기 라디오 → setScanInterval + localStorage 갱신", async () => {
    const user = userEvent.setup();
    render(<Settings />);

    // 60분이 default (localStorage 비어있음).
    const radio15 = screen.getByLabelText("screens.settings.general.scanInterval.15m");
    expect((radio15 as HTMLInputElement).checked).toBe(false);

    await user.click(radio15);

    await waitFor(() => {
      expect((radio15 as HTMLInputElement).checked).toBe(true);
    });
    expect(globalThis.localStorage.getItem("lmmaster.settings.general.scan_interval_min")).toBe(
      "15",
    );

    const radioOff = screen.getByLabelText("screens.settings.general.scanInterval.off");
    await user.click(radioOff);
    await waitFor(() => {
      expect((radioOff as HTMLInputElement).checked).toBe(true);
    });
    expect(globalThis.localStorage.getItem("lmmaster.settings.general.scan_interval_min")).toBe(
      "0",
    );
  });

  it("일반 → 언어 라디오 → i18n.changeLanguage 호출", async () => {
    const user = userEvent.setup();
    render(<Settings />);

    const en = screen.getByLabelText("screens.settings.general.language.en");
    await user.click(en);

    expect(changeLanguageMock).toHaveBeenCalledWith("en");
  });

  it("일반 → 테마 light 라디오는 disabled (lock 아이콘 + selectable 불가)", async () => {
    render(<Settings />);
    const light = screen.getByLabelText(
      /screens\.settings\.general\.theme\.light/,
    ) as HTMLInputElement;
    expect(light.disabled).toBe(true);
    const dark = screen.getByLabelText(
      "screens.settings.general.theme.dark",
    ) as HTMLInputElement;
    expect(dark.checked).toBe(true);
  });

  it("워크스페이스 → '지금 정리할게요' 클릭 → checkWorkspaceRepair 호출", async () => {
    const user = userEvent.setup();
    vi.mocked(checkWorkspaceRepair).mockResolvedValueOnce({
      tier: "green",
      invalidated_caches: ["bench", "scan"],
      invalidated_runtimes: 0,
      models_preserved: 5,
    });
    render(<Settings />);
    await user.click(screen.getByTestId("settings-category-workspace"));
    await waitFor(() => {
      expect(screen.getByTestId("settings-workspace-repair-btn")).toBeTruthy();
    });
    await user.click(screen.getByTestId("settings-workspace-repair-btn"));
    await waitFor(() => {
      expect(checkWorkspaceRepair).toHaveBeenCalled();
    });
    // 결과 메시지 표시.
    await waitFor(() => {
      expect(
        screen.getByText(/screens\.settings\.workspace\.repairDone/),
      ).toBeTruthy();
    });
  });

  it("워크스페이스 → 경로 표시 + 'relocate'는 disabled", async () => {
    const user = userEvent.setup();
    render(<Settings />);
    await user.click(screen.getByTestId("settings-category-workspace"));
    await waitFor(() => {
      expect(
        screen.getByText("C:\\Users\\me\\AppData\\Local\\LMmaster"),
      ).toBeTruthy();
    });
    const relocateBtn = screen.getByRole("button", {
      name: /screens\.settings\.workspace\.relocate$/,
    });
    expect((relocateBtn as HTMLButtonElement).disabled).toBe(true);
  });

  it("카탈로그 → registryUrl read-only", async () => {
    const user = userEvent.setup();
    render(<Settings />);
    await user.click(screen.getByTestId("settings-category-catalog"));
    await waitFor(() => {
      expect(screen.getByTestId("settings-registry-url")).toBeTruthy();
    });
    const input = screen.getByTestId("settings-registry-url") as HTMLInputElement;
    expect(input.readOnly).toBe(true);
    expect(input.value).toContain("lmmaster://");
  });

  it("고급 → Gemini / 로그 export 모두 disabled + comingSoon 텍스트", async () => {
    const user = userEvent.setup();
    render(<Settings />);
    await user.click(screen.getByTestId("settings-category-advanced"));
    await waitFor(() => {
      expect(screen.getByText("screens.settings.advanced.gemini")).toBeTruthy();
    });
    // Gemini 토글 — disabled.
    const geminiToggle = screen.getByRole("switch", {
      name: "screens.settings.advanced.gemini",
    });
    expect((geminiToggle as HTMLButtonElement).disabled).toBe(true);
    expect(geminiToggle.getAttribute("aria-checked")).toBe("false");
    // comingSoon 텍스트.
    expect(
      screen.getByText("screens.settings.advanced.gemini.comingSoon"),
    ).toBeTruthy();
    // 진단 로그 export — disabled 버튼.
    const exportBtn = screen.getByRole("button", {
      name: /screens\.settings\.advanced\.exportLogs$/,
    });
    expect((exportBtn as HTMLButtonElement).disabled).toBe(true);
    // 빌드 정보.
    expect(screen.getByText("screens.settings.advanced.buildInfo")).toBeTruthy();
  });

  it("고급 → SQLCipher env hint 표시 (LMMASTER_ENCRYPT_DB)", async () => {
    const user = userEvent.setup();
    render(<Settings />);
    await user.click(screen.getByTestId("settings-category-advanced"));
    await waitFor(() => {
      expect(
        screen.getByText("screens.settings.advanced.sqlcipher"),
      ).toBeTruthy();
    });
    expect(
      screen.getByText(/LMMASTER_ENCRYPT_DB=/),
    ).toBeTruthy();
    expect(
      screen.getByText("screens.settings.advanced.sqlcipher.envHint"),
    ).toBeTruthy();
  });

  it("a11y — 위반 0건 (axe)", async () => {
    const user = userEvent.setup();
    const { container } = render(<Settings />);
    // 일반 panel 부터 — workspace로도 테스트.
    const results = await axe.run(container, {
      rules: {
        // 카테고리 단일 페이지 안에 main이 1개 — App shell 책임 밖.
        region: { enabled: false },
      },
    });
    expect(results.violations).toEqual([]);
    // 다른 카테고리도 빠르게.
    await user.click(screen.getByTestId("settings-category-workspace"));
    await waitFor(() => {
      expect(screen.getByText("screens.settings.workspace.path")).toBeTruthy();
    });
    const results2 = await axe.run(container, {
      rules: { region: { enabled: false } },
    });
    expect(results2.violations).toEqual([]);
  });
});

describe("Settings — initial localStorage hydration", () => {
  it("자가스캔 주기 — localStorage에 저장된 값이 있으면 그것을 active로", async () => {
    globalThis.localStorage.setItem(
      "lmmaster.settings.general.scan_interval_min",
      "15",
    );
    render(<Settings />);
    await waitFor(() => {
      const radio15 = screen.getByLabelText(
        "screens.settings.general.scanInterval.15m",
      ) as HTMLInputElement;
      expect(radio15.checked).toBe(true);
    });
  });
});

describe("Settings — workspace repair 실패", () => {
  it("checkWorkspaceRepair reject 시 에러 메시지", async () => {
    const user = userEvent.setup();
    vi.mocked(checkWorkspaceRepair).mockRejectedValueOnce(new Error("disk"));
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    render(<Settings />);
    await user.click(screen.getByTestId("settings-category-workspace"));
    await waitFor(() => {
      expect(screen.getByTestId("settings-workspace-repair-btn")).toBeTruthy();
    });
    await user.click(screen.getByTestId("settings-workspace-repair-btn"));
    await waitFor(() => {
      expect(
        screen.getByText("screens.settings.workspace.errorRepair"),
      ).toBeTruthy();
    });
    warnSpy.mockRestore();
  });
});

describe("Settings — 자동 갱신 (Phase 6'.b)", () => {
  it("일반 패널에 자동 갱신 섹션 + idle 상태 노출", async () => {
    render(<Settings />);
    await waitFor(() => {
      expect(getAutoUpdateStatus).toHaveBeenCalled();
    });
    expect(screen.getByText("screens.settings.autoUpdate.title")).toBeTruthy();
    expect(screen.getByText("screens.settings.autoUpdate.toggleOff")).toBeTruthy();
    expect(screen.getByText("screens.settings.autoUpdate.neverChecked")).toBeTruthy();
  });

  it("토글 ON → startAutoUpdatePoller 호출 (6h 기본)", async () => {
    const user = userEvent.setup();
    render(<Settings />);
    await waitFor(() => {
      expect(getAutoUpdateStatus).toHaveBeenCalled();
    });
    // status 갱신 두 번째 콜이 active로 응답하도록.
    vi.mocked(getAutoUpdateStatus).mockResolvedValueOnce(ACTIVE_POLLER);

    const toggle = screen.getByRole("switch", {
      name: "screens.settings.autoUpdate.toggleLabel",
    });
    await user.click(toggle);

    await waitFor(() => {
      expect(startAutoUpdatePoller).toHaveBeenCalledWith(
        "anthropics/lmmaster",
        expect.any(String),
        6 * 3600,
        expect.any(Function),
      );
    });
  });

  it("active 상태에서 토글 OFF → stopAutoUpdatePoller 호출", async () => {
    const user = userEvent.setup();
    vi.mocked(getAutoUpdateStatus).mockResolvedValue(ACTIVE_POLLER);
    render(<Settings />);
    await waitFor(() => {
      expect(
        screen.getByText("screens.settings.autoUpdate.toggleOn"),
      ).toBeTruthy();
    });
    // 토글 OFF.
    vi.mocked(getAutoUpdateStatus).mockResolvedValueOnce(IDLE_POLLER);
    await user.click(
      screen.getByRole("switch", {
        name: "screens.settings.autoUpdate.toggleLabel",
      }),
    );
    await waitFor(() => {
      expect(stopAutoUpdatePoller).toHaveBeenCalled();
    });
  });

  it("interval radio 토글 비활성 상태에선 disabled", async () => {
    render(<Settings />);
    await waitFor(() => {
      expect(getAutoUpdateStatus).toHaveBeenCalled();
    });
    // 1h interval 라디오 — disabled.
    const r1h = screen.getByLabelText(
      "screens.settings.autoUpdate.interval.1h",
    ) as HTMLInputElement;
    expect(r1h.disabled).toBe(true);
  });

  it("interval radio active 상태에선 클릭 → restart (stop + start)", async () => {
    const user = userEvent.setup();
    vi.mocked(getAutoUpdateStatus).mockResolvedValue(ACTIVE_POLLER);
    render(<Settings />);
    await waitFor(() => {
      expect(
        screen.getByText("screens.settings.autoUpdate.toggleOn"),
      ).toBeTruthy();
    });
    const r12h = screen.getByLabelText(
      "screens.settings.autoUpdate.interval.12h",
    ) as HTMLInputElement;
    expect(r12h.disabled).toBe(false);
    await user.click(r12h);
    await waitFor(() => {
      expect(stopAutoUpdatePoller).toHaveBeenCalled();
    });
    await waitFor(() => {
      expect(startAutoUpdatePoller).toHaveBeenLastCalledWith(
        "anthropics/lmmaster",
        expect.any(String),
        12 * 3600,
        expect.any(Function),
      );
    });
  });

  it("'지금 확인할게요' 클릭 → checkForUpdate 호출", async () => {
    const user = userEvent.setup();
    render(<Settings />);
    await waitFor(() => {
      expect(getAutoUpdateStatus).toHaveBeenCalled();
    });
    await user.click(screen.getByTestId("settings-autoupdate-check-now"));
    await waitFor(() => {
      expect(checkForUpdate).toHaveBeenCalled();
    });
  });

  it("Outdated event → ToastUpdate 인라인 렌더", async () => {
    const user = userEvent.setup();
    vi.mocked(checkForUpdate).mockImplementation(async (_repo, _cur, onEvent) => {
      // 즉시 outdated emit (테스트는 callback synchronously 호출 — IPC 시뮬레이션).
      const ev: UpdateEvent = {
        kind: "outdated",
        check_id: "c1",
        current_version: "0.1.0",
        latest: {
          version: "0.2.0",
          published_at_iso: "2026-04-15T00:00:00Z",
          url: "https://example.com/r",
          notes: null,
        },
      };
      onEvent(ev);
      return { check_id: "c1", cancel: vi.fn().mockResolvedValue(undefined) };
    });
    render(<Settings />);
    await waitFor(() => {
      expect(getAutoUpdateStatus).toHaveBeenCalled();
    });
    await user.click(screen.getByTestId("settings-autoupdate-check-now"));
    await waitFor(() => {
      expect(screen.getByTestId("toast-update")).toBeTruthy();
    });
  });

  it("UpToDate event → '최신 버전이에요' info 메시지", async () => {
    const user = userEvent.setup();
    vi.mocked(checkForUpdate).mockImplementation(async (_repo, _cur, onEvent) => {
      const ev: UpdateEvent = {
        kind: "up-to-date",
        check_id: "c1",
        current_version: "0.1.0",
      };
      onEvent(ev);
      return { check_id: "c1", cancel: vi.fn().mockResolvedValue(undefined) };
    });
    render(<Settings />);
    await waitFor(() => {
      expect(getAutoUpdateStatus).toHaveBeenCalled();
    });
    await user.click(screen.getByTestId("settings-autoupdate-check-now"));
    await waitFor(() => {
      expect(
        screen.getByText("screens.settings.autoUpdate.upToDate"),
      ).toBeTruthy();
    });
  });

  it("Failed event → 한국어 에러 메시지", async () => {
    const user = userEvent.setup();
    vi.mocked(checkForUpdate).mockImplementation(async (_repo, _cur, onEvent) => {
      const ev: UpdateEvent = {
        kind: "failed",
        check_id: "c1",
        error: "네트워크 끊겼어요",
      };
      onEvent(ev);
      return { check_id: "c1", cancel: vi.fn().mockResolvedValue(undefined) };
    });
    render(<Settings />);
    await waitFor(() => {
      expect(getAutoUpdateStatus).toHaveBeenCalled();
    });
    await user.click(screen.getByTestId("settings-autoupdate-check-now"));
    await waitFor(() => {
      // errorText는 i18n key + suffix detail.
      expect(
        screen.getByText(/screens\.settings\.autoUpdate\.errorCheck.*네트워크/),
      ).toBeTruthy();
    });
  });

  it("status에서 last_check_iso 있으면 lastChecked 메시지", async () => {
    vi.mocked(getAutoUpdateStatus).mockResolvedValue(ACTIVE_POLLER);
    render(<Settings />);
    await waitFor(() => {
      expect(
        screen.getByText(
          /screens\.settings\.autoUpdate\.lastChecked.*2026-04-28T01:23:45Z/,
        ),
      ).toBeTruthy();
    });
  });

  it("startAutoUpdatePoller reject 시 에러 메시지 + 토글 비활성 유지", async () => {
    const user = userEvent.setup();
    vi.mocked(startAutoUpdatePoller).mockRejectedValueOnce({
      kind: "interval-out-of-range",
      got: 0,
    });
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    render(<Settings />);
    await waitFor(() => {
      expect(getAutoUpdateStatus).toHaveBeenCalled();
    });
    await user.click(
      screen.getByRole("switch", {
        name: "screens.settings.autoUpdate.toggleLabel",
      }),
    );
    await waitFor(() => {
      expect(
        screen.getByText("screens.settings.autoUpdate.errorStart"),
      ).toBeTruthy();
    });
    warnSpy.mockRestore();
  });

  it("a11y — auto-update fieldset 위반 0건", async () => {
    const { container } = render(<Settings />);
    await waitFor(() => {
      expect(getAutoUpdateStatus).toHaveBeenCalled();
    });
    const results = await axe.run(container, {
      rules: {
        region: { enabled: false },
      },
    });
    expect(results.violations).toEqual([]);
  });
});

describe("Settings — Pipelines 섹션 (Phase 6'.c)", () => {
  it("일반 패널에 PipelinesPanel 렌더 + 3종 토글 노출", async () => {
    render(<Settings />);
    await waitFor(() => {
      expect(screen.getByTestId("pipelines-panel")).toBeTruthy();
    });
    await waitFor(() => {
      expect(screen.getByTestId("pipelines-toggle-pii-redact")).toBeTruthy();
      expect(screen.getByTestId("pipelines-toggle-token-quota")).toBeTruthy();
      expect(screen.getByTestId("pipelines-toggle-observability")).toBeTruthy();
    });
  });

  it("Pipelines 섹션 — 감사 로그 영역 + empty 상태", async () => {
    render(<Settings />);
    await waitFor(() => {
      expect(screen.getByTestId("pipelines-audit-empty")).toBeTruthy();
    });
  });
});
