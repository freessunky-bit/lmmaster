/**
 * @vitest-environment jsdom
 */
// 접근성 자동 검증 — vitest-axe로 마법사 단계별 + CommandPalette WCAG 검사. Phase 1A.4.d.3.
//
// 정책:
// - color-contrast 룰은 jsdom이 실제 색을 계산 못 해서 자동 disable.
// - 한국어 lang attribute는 root html 차원이라 컨테이너에선 적용 안 함 (잘못된 lang 룰 disable).
// - 컴포넌트별로 mock context로 강제 substate 설정 후 axe.

import { render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { axe } from "vitest-axe";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) =>
      opts ? `${key}:${JSON.stringify(opts)}` : key,
    i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
  }),
}));

vi.mock("../onboarding/context", () => ({
  useOnboardingLang: vi.fn(() => "ko"),
  useOnboardingSend: vi.fn(() => vi.fn()),
  useOnboardingEnv: vi.fn(),
  useOnboardingScanError: vi.fn(),
  useOnboardingScanSub: vi.fn(),
  useOnboardingInstallError: vi.fn(),
  useOnboardingInstallLatest: vi.fn(),
  useOnboardingInstallLog: vi.fn(() => []),
  useOnboardingInstallOutcome: vi.fn(),
  useOnboardingInstallProgress: vi.fn(),
  useOnboardingInstallSub: vi.fn(),
  useOnboardingModelId: vi.fn(),
  useOnboardingRetryAttempt: vi.fn(),
}));

import * as ctx from "../onboarding/context";
import { Step1Language } from "../onboarding/steps/Step1Language";
import { Step2Scan } from "../onboarding/steps/Step2Scan";
import { Step3Install } from "../onboarding/steps/Step3Install";
import { Step4Done } from "../onboarding/steps/Step4Done";
import {
  CommandPaletteProvider,
  useCommandPalette,
  useCommandRegistration,
} from "../components/command-palette/context";
import { CommandPalette } from "../components/command-palette/CommandPalette";
import type { Command } from "../components/command-palette/types";
import { useEffect } from "react";

const FAKE_ENV = {
  hardware: {
    os: { family: "windows" as const, version: "11", arch: "x86_64", kernel: "10" },
    cpu: { brand: "Intel", vendor_id: "GI", physical_cores: 8, logical_cores: 16, frequency_mhz: 3000 },
    mem: { total_bytes: 16 * 1024 ** 3, available_bytes: 8 * 1024 ** 3 },
    disks: [{ mount_point: "C:", kind: "ssd" as const, total_bytes: 500e9, available_bytes: 250e9 }],
    gpus: [{ vendor: "nvidia" as const, name: "RTX 4080", vram_bytes: 16 * 1024 ** 3 }],
    runtimes: {},
    probed_at: "2026-04-27T00:00:00Z",
    probe_ms: 100,
  },
  runtimes: [
    { runtime: "ollama" as const, status: "not-installed" as const },
    { runtime: "lm-studio" as const, status: "not-installed" as const },
  ],
};

// jsdom은 실제 색 계산 안 함 → color-contrast 비활성. lang 룰은 root에서만 의미.
const AXE_OPTIONS = {
  rules: {
    "color-contrast": { enabled: false },
    "html-has-lang": { enabled: false },
    "landmark-one-main": { enabled: false },
    region: { enabled: false },
  },
};

beforeEach(() => {
  vi.mocked(ctx.useOnboardingLang).mockReturnValue("ko");
  vi.mocked(ctx.useOnboardingSend).mockReturnValue(vi.fn());
  vi.mocked(ctx.useOnboardingInstallLog).mockReturnValue([]);
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("접근성 — 마법사 step 컴포넌트", () => {
  it("Step1Language WCAG 위반 없음", async () => {
    const { container } = render(<Step1Language />);
    const results = await axe(container, AXE_OPTIONS);
    expect(results.violations).toEqual([]);
  });

  it("Step2Scan running 상태 WCAG 위반 없음", async () => {
    vi.mocked(ctx.useOnboardingScanSub).mockReturnValue("running");
    vi.mocked(ctx.useOnboardingEnv).mockReturnValue(undefined);
    const { container } = render(<Step2Scan />);
    const results = await axe(container, AXE_OPTIONS);
    expect(results.violations).toEqual([]);
  });

  it("Step2Scan done 상태 (4 카드) WCAG 위반 없음", async () => {
    vi.mocked(ctx.useOnboardingScanSub).mockReturnValue("done");
    vi.mocked(ctx.useOnboardingEnv).mockReturnValue(FAKE_ENV);
    const { container } = render(<Step2Scan />);
    const results = await axe(container, AXE_OPTIONS);
    expect(results.violations).toEqual([]);
  });

  it("Step2Scan failed 상태 WCAG 위반 없음", async () => {
    vi.mocked(ctx.useOnboardingScanSub).mockReturnValue("failed");
    vi.mocked(ctx.useOnboardingScanError).mockReturnValue("network down");
    const { container } = render(<Step2Scan />);
    const results = await axe(container, AXE_OPTIONS);
    expect(results.violations).toEqual([]);
  });

  it("Step3Install idle 상태 WCAG 위반 없음", async () => {
    vi.mocked(ctx.useOnboardingInstallSub).mockReturnValue("idle");
    vi.mocked(ctx.useOnboardingEnv).mockReturnValue(FAKE_ENV);
    const { container } = render(<Step3Install />);
    const results = await axe(container, AXE_OPTIONS);
    expect(results.violations).toEqual([]);
  });

  it("Step3Install running 상태 WCAG 위반 없음", async () => {
    vi.mocked(ctx.useOnboardingInstallSub).mockReturnValue("running");
    vi.mocked(ctx.useOnboardingModelId).mockReturnValue("ollama");
    vi.mocked(ctx.useOnboardingInstallProgress).mockReturnValue({
      downloaded: 500,
      total: 1000,
      speed_bps: 100 * 1024,
    });
    vi.mocked(ctx.useOnboardingInstallLatest).mockReturnValue({
      kind: "download",
      download: {
        kind: "progress",
        downloaded: 500,
        total: 1000,
        speed_bps: 100 * 1024,
      },
    });
    const { container } = render(<Step3Install />);
    const results = await axe(container, AXE_OPTIONS);
    expect(results.violations).toEqual([]);
  });

  it("Step3Install failed 상태 WCAG 위반 없음", async () => {
    vi.mocked(ctx.useOnboardingInstallSub).mockReturnValue("failed");
    vi.mocked(ctx.useOnboardingInstallError).mockReturnValue({
      code: "download-failed",
      message: "network down",
    });
    const { container } = render(<Step3Install />);
    const results = await axe(container, AXE_OPTIONS);
    expect(results.violations).toEqual([]);
  });

  it("Step3Install openedUrl 상태 WCAG 위반 없음", async () => {
    vi.mocked(ctx.useOnboardingInstallSub).mockReturnValue("openedUrl");
    vi.mocked(ctx.useOnboardingInstallOutcome).mockReturnValue({
      kind: "opened-url",
      url: "https://lmstudio.ai/",
    });
    const { container } = render(<Step3Install />);
    const results = await axe(container, AXE_OPTIONS);
    expect(results.violations).toEqual([]);
  });

  it("Step4Done WCAG 위반 없음", async () => {
    const { container } = render(<Step4Done onFinish={vi.fn()} />);
    const results = await axe(container, AXE_OPTIONS);
    expect(results.violations).toEqual([]);
  });
});

// ── CommandPalette ───────────────────────────────────────────────────

function CommandsRegister({ commands }: { commands: Command[] }) {
  useCommandRegistration(commands);
  return null;
}
function OpenController({ open }: { open: boolean }) {
  const { setOpen } = useCommandPalette();
  useEffect(() => setOpen(open), [open, setOpen]);
  return null;
}

describe("접근성 — CommandPalette", () => {
  it("열린 상태 WCAG 위반 없음", async () => {
    const cmds: Command[] = [
      {
        id: "test.foo",
        group: "wizard",
        label: "Foo",
        keywords: [],
        perform: vi.fn(),
      },
      {
        id: "test.bar",
        group: "navigation",
        label: "Bar",
        keywords: [],
        perform: vi.fn(),
      },
    ];
    const { container } = render(
      <CommandPaletteProvider>
        <OpenController open />
        <CommandsRegister commands={cmds} />
        <CommandPalette />
      </CommandPaletteProvider>,
    );
    // dialog가 portal로 body에 마운트되므로 document.body 전체 검사.
    const results = await axe(document.body, AXE_OPTIONS);
    expect(results.violations).toEqual([]);
    void container;
  });
});
