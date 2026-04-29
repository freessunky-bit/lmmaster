/**
 * @vitest-environment jsdom
 */
// PipelinesPanel — 토글 / 감사 로그 / 에러 / a11y 단위 테스트.

import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) =>
      opts ? `${key}:${JSON.stringify(opts)}` : key,
    i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
  }),
}));

vi.mock("../ipc/pipelines", () => ({
  listPipelines: vi.fn(),
  getAuditLog: vi.fn(),
  setPipelineEnabled: vi.fn(),
  clearAuditLog: vi.fn(),
  getPipelinesConfig: vi.fn(),
}));

import {
  clearAuditLog,
  getAuditLog,
  listPipelines,
  setPipelineEnabled,
  type AuditEntry,
  type PipelineDescriptor,
} from "../ipc/pipelines";

import { PipelinesPanel } from "./PipelinesPanel";

const FIXTURE_DESCRIPTORS: PipelineDescriptor[] = [
  {
    id: "pii-redact",
    display_name_ko: "개인정보 보호 필터",
    description_ko: "주민·휴대폰·카드·이메일을 자동으로 가려요.",
    enabled: true,
  },
  {
    id: "token-quota",
    display_name_ko: "토큰 한도 관리",
    description_ko: "키별 토큰 한도를 추적하고 초과 요청을 막아 드려요.",
    enabled: true,
  },
  {
    id: "observability",
    display_name_ko: "관찰성 로그",
    description_ko: "요청·응답 메타를 진단 로그에 남겨드려요.",
    enabled: true,
  },
];

function makeAudit(
  pipelineId: string,
  action: "passed" | "modified" | "blocked",
  details: string | null = null,
  iso = "2026-04-28T01:23:45Z",
): AuditEntry {
  return {
    pipeline_id: pipelineId,
    action,
    timestamp_iso: iso,
    details,
  };
}

beforeEach(() => {
  vi.mocked(listPipelines).mockResolvedValue(FIXTURE_DESCRIPTORS);
  vi.mocked(getAuditLog).mockResolvedValue([]);
  vi.mocked(setPipelineEnabled).mockResolvedValue(undefined);
  vi.mocked(clearAuditLog).mockResolvedValue(undefined);
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("PipelinesPanel — 기본 렌더 + 토글", () => {
  it("3종 토글이 default enabled 상태로 렌더돼요", async () => {
    render(<PipelinesPanel />);
    await waitFor(() => {
      expect(listPipelines).toHaveBeenCalled();
    });
    await waitFor(() => {
      expect(screen.getByTestId("pipelines-toggle-pii-redact")).toBeTruthy();
    });
    const piiToggle = screen.getByTestId("pipelines-toggle-pii-redact");
    const tqToggle = screen.getByTestId("pipelines-toggle-token-quota");
    const obsToggle = screen.getByTestId("pipelines-toggle-observability");
    expect(piiToggle.getAttribute("aria-checked")).toBe("true");
    expect(tqToggle.getAttribute("aria-checked")).toBe("true");
    expect(obsToggle.getAttribute("aria-checked")).toBe("true");
  });

  it("i18n 라벨 키가 노출돼요 (이름 + 설명)", async () => {
    render(<PipelinesPanel />);
    await waitFor(() => {
      expect(
        screen.getByText("screens.settings.pipelines.pipelines.piiRedact.name"),
      ).toBeTruthy();
    });
    expect(
      screen.getByText("screens.settings.pipelines.pipelines.tokenQuota.name"),
    ).toBeTruthy();
    expect(
      screen.getByText(
        "screens.settings.pipelines.pipelines.observability.name",
      ),
    ).toBeTruthy();
    expect(
      screen.getByText("screens.settings.pipelines.pipelines.piiRedact.desc"),
    ).toBeTruthy();
  });

  it("토글 클릭 → setPipelineEnabled(false) 호출 + aria-checked flip", async () => {
    const user = userEvent.setup();
    render(<PipelinesPanel />);
    await waitFor(() => {
      expect(screen.getByTestId("pipelines-toggle-pii-redact")).toBeTruthy();
    });
    const toggle = screen.getByTestId("pipelines-toggle-pii-redact");
    expect(toggle.getAttribute("aria-checked")).toBe("true");
    await user.click(toggle);
    await waitFor(() => {
      expect(setPipelineEnabled).toHaveBeenCalledWith("pii-redact", false);
    });
    await waitFor(() => {
      expect(
        screen.getByTestId("pipelines-toggle-pii-redact").getAttribute("aria-checked"),
      ).toBe("false");
    });
  });

  it("토글 실패 → optimistic UI revert + 한국어 에러 키 노출", async () => {
    const user = userEvent.setup();
    vi.mocked(setPipelineEnabled).mockRejectedValueOnce(new Error("ipc fail"));
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    render(<PipelinesPanel />);
    await waitFor(() => {
      expect(screen.getByTestId("pipelines-toggle-token-quota")).toBeTruthy();
    });
    const toggle = screen.getByTestId("pipelines-toggle-token-quota");
    await user.click(toggle);
    await waitFor(() => {
      expect(setPipelineEnabled).toHaveBeenCalled();
    });
    // revert: aria-checked는 다시 true.
    await waitFor(() => {
      expect(toggle.getAttribute("aria-checked")).toBe("true");
    });
    // 에러 메시지 — i18n 키 노출.
    await waitFor(() => {
      expect(
        screen.getByText("screens.settings.pipelines.errors.toggleFailed"),
      ).toBeTruthy();
    });
    warnSpy.mockRestore();
  });

  it("unknown-pipeline backend 에러 — 전용 메시지 노출", async () => {
    const user = userEvent.setup();
    vi.mocked(setPipelineEnabled).mockRejectedValueOnce({
      kind: "unknown-pipeline",
      pipeline_id: "x",
    });
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    render(<PipelinesPanel />);
    await waitFor(() => {
      expect(screen.getByTestId("pipelines-toggle-observability")).toBeTruthy();
    });
    await user.click(screen.getByTestId("pipelines-toggle-observability"));
    await waitFor(() => {
      expect(
        screen.getByText(
          "screens.settings.pipelines.errors.unknownPipeline",
        ),
      ).toBeTruthy();
    });
    warnSpy.mockRestore();
  });
});

describe("PipelinesPanel — 감사 로그", () => {
  it("빈 로그 — empty 상태 i18n 키 노출", async () => {
    render(<PipelinesPanel />);
    await waitFor(() => {
      expect(getAuditLog).toHaveBeenCalled();
    });
    await waitFor(() => {
      expect(screen.getByTestId("pipelines-audit-empty")).toBeTruthy();
    });
    expect(
      screen.getByText("screens.settings.pipelines.audit.empty"),
    ).toBeTruthy();
  });

  it("entry 있을 때 — pipeline 이름 + action 라벨 + timestamp 표시", async () => {
    vi.mocked(getAuditLog).mockResolvedValueOnce([
      makeAudit("pii-redact", "modified", "redacted 2 PII"),
      makeAudit("token-quota", "blocked", "projected 1500 > 1000"),
      makeAudit("observability", "passed", null),
    ]);
    render(<PipelinesPanel />);
    await waitFor(() => {
      expect(screen.getByTestId("pipelines-audit-list")).toBeTruthy();
    });
    const list = screen.getByTestId("pipelines-audit-list");
    // 3개 entry.
    expect(within(list).getAllByRole("listitem").length).toBe(3);
    // action 라벨 키.
    expect(
      within(list).getByText("screens.settings.pipelines.audit.actionModified"),
    ).toBeTruthy();
    expect(
      within(list).getByText("screens.settings.pipelines.audit.actionBlocked"),
    ).toBeTruthy();
    expect(
      within(list).getByText("screens.settings.pipelines.audit.actionPassed"),
    ).toBeTruthy();
  });

  it("action variant 별 클래스 (is-passed / is-modified / is-blocked)", async () => {
    vi.mocked(getAuditLog).mockResolvedValueOnce([
      makeAudit("pii-redact", "modified"),
      makeAudit("token-quota", "blocked"),
    ]);
    render(<PipelinesPanel />);
    await waitFor(() => {
      expect(screen.getByTestId("pipelines-audit-list")).toBeTruthy();
    });
    const m = screen.getByTestId("pipelines-audit-entry-0-action");
    expect(m.className).toContain("is-modified");
    const b = screen.getByTestId("pipelines-audit-entry-1-action");
    expect(b.className).toContain("is-blocked");
  });

  it("clear 버튼 → clearAuditLog 호출 + list 비워짐", async () => {
    const user = userEvent.setup();
    vi.mocked(getAuditLog).mockResolvedValueOnce([
      makeAudit("pii-redact", "modified", "redacted 1"),
    ]);
    render(<PipelinesPanel />);
    await waitFor(() => {
      expect(screen.getByTestId("pipelines-audit-list")).toBeTruthy();
    });
    const clearBtn = screen.getByTestId("pipelines-audit-clear");
    expect((clearBtn as HTMLButtonElement).disabled).toBe(false);
    await user.click(clearBtn);
    await waitFor(() => {
      expect(clearAuditLog).toHaveBeenCalled();
    });
    await waitFor(() => {
      expect(screen.getByTestId("pipelines-audit-empty")).toBeTruthy();
    });
  });

  it("refresh 버튼 → getAuditLog 다시 호출", async () => {
    const user = userEvent.setup();
    render(<PipelinesPanel />);
    await waitFor(() => {
      expect(getAuditLog).toHaveBeenCalledTimes(1);
    });
    // 두 번째 호출은 새 entry 응답.
    vi.mocked(getAuditLog).mockResolvedValueOnce([
      makeAudit("pii-redact", "passed", null),
    ]);
    await user.click(screen.getByTestId("pipelines-audit-refresh"));
    await waitFor(() => {
      expect(getAuditLog).toHaveBeenCalledTimes(2);
    });
    await waitFor(() => {
      expect(screen.getByTestId("pipelines-audit-list")).toBeTruthy();
    });
  });

  it("clearAuditLog 실패 → 한국어 에러 키 노출", async () => {
    const user = userEvent.setup();
    vi.mocked(getAuditLog).mockResolvedValueOnce([
      makeAudit("pii-redact", "modified"),
    ]);
    vi.mocked(clearAuditLog).mockRejectedValueOnce(new Error("boom"));
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    render(<PipelinesPanel />);
    await waitFor(() => {
      expect(screen.getByTestId("pipelines-audit-list")).toBeTruthy();
    });
    await user.click(screen.getByTestId("pipelines-audit-clear"));
    await waitFor(() => {
      expect(
        screen.getByText("screens.settings.pipelines.errors.clearFailed"),
      ).toBeTruthy();
    });
    warnSpy.mockRestore();
  });

  it("details 노출 + 긴 details는 truncate", async () => {
    const long = "a".repeat(500);
    vi.mocked(getAuditLog).mockResolvedValueOnce([
      makeAudit("pii-redact", "modified", long),
    ]);
    render(<PipelinesPanel />);
    await waitFor(() => {
      expect(screen.getByTestId("pipelines-audit-list")).toBeTruthy();
    });
    const detailsEl = screen.getByTestId("pipelines-audit-entry-0-details");
    expect(detailsEl.textContent ?? "").toContain("…");
    // truncate cap 160 + 라벨 키 + ": " 정도 — 전체 길이가 원본보다 짧음.
    expect((detailsEl.textContent ?? "").length).toBeLessThan(long.length);
  });
});

describe("PipelinesPanel — a11y", () => {
  it("axe violations === [] (빈 로그)", async () => {
    const { container } = render(<PipelinesPanel />);
    await waitFor(() => {
      expect(screen.getByTestId("pipelines-panel")).toBeTruthy();
    });
    await waitFor(() => {
      expect(screen.getByTestId("pipelines-audit-empty")).toBeTruthy();
    });
    const results = await axe.run(container, {
      rules: { region: { enabled: false } },
    });
    expect(results.violations).toEqual([]);
  });

  it("axe violations === [] (entries 있을 때)", async () => {
    vi.mocked(getAuditLog).mockResolvedValueOnce([
      makeAudit("pii-redact", "modified", "redacted 1 PII"),
      makeAudit("token-quota", "blocked", "budget 1000"),
    ]);
    const { container } = render(<PipelinesPanel />);
    await waitFor(() => {
      expect(screen.getByTestId("pipelines-audit-list")).toBeTruthy();
    });
    const results = await axe.run(container, {
      rules: { region: { enabled: false } },
    });
    expect(results.violations).toEqual([]);
  });
});
