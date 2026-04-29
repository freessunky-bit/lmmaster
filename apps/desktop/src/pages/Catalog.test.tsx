/**
 * @vitest-environment jsdom
 */
// Catalog 페이지 렌더 + 필터 + 카드 클릭 → Drawer 테스트.

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

vi.mock("../ipc/catalog", () => ({
  getCatalog: vi.fn(),
  getRecommendation: vi.fn(),
}));

vi.mock("../ipc/catalog-refresh", () => ({
  refreshCatalogNow: vi.fn(),
  getLastCatalogRefresh: vi.fn().mockResolvedValue(null),
  onCatalogRefreshed: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("../ipc/bench", () => ({
  startBench: vi.fn().mockResolvedValue(null),
  cancelBench: vi.fn().mockResolvedValue(undefined),
  getLastBenchReport: vi.fn().mockResolvedValue(null),
  onBenchStarted: vi.fn().mockResolvedValue(() => {}),
  onBenchFinished: vi.fn().mockResolvedValue(() => {}),
}));

// Phase 8'.b.1 — CustomModelsSection이 listCustomModels를 호출. 빈 list로 mock.
vi.mock("../ipc/workbench", () => ({
  listCustomModels: vi.fn().mockResolvedValue([]),
}));

vi.mock("../contexts/ActiveWorkspaceContext", () => ({
  useActiveWorkspaceOptional: () => null,
}));

import { getCatalog, getRecommendation } from "../ipc/catalog";
import type { ModelEntry, Recommendation } from "../ipc/catalog";
import { CatalogPage } from "./Catalog";

function makeEntry(
  id: string,
  overrides: Partial<ModelEntry> = {},
): ModelEntry {
  return {
    id,
    display_name: id,
    category: "agent-general",
    model_family: "x",
    source: { type: "direct-url", url: "https://x" },
    runner_compatibility: ["llama-cpp"],
    quantization_options: [
      {
        label: "Q4_K_M",
        size_mb: 760,
        sha256: "0".repeat(64),
      },
    ],
    min_vram_mb: null,
    rec_vram_mb: 2048,
    min_ram_mb: 4096,
    rec_ram_mb: 8192,
    install_size_mb: 760,
    language_strength: 9,
    tool_support: true,
    vision_support: false,
    structured_output_support: true,
    license: "MIT",
    maturity: "stable",
    portable_suitability: 9,
    on_device_suitability: 9,
    fine_tune_suitability: 6,
    verification: { tier: "verified" },
    use_case_examples: ["한국어 일상 대화"],
    warnings: [],
    ...overrides,
  } as ModelEntry;
}

const FIXTURE_ENTRIES: ModelEntry[] = [
  makeEntry("exaone-1.2b", { display_name: "EXAONE 4.0 1.2B" }),
  makeEntry("qwen-coder", {
    display_name: "Qwen 2.5 Coder",
    category: "coding",
    coding_strength: 9,
  }),
  makeEntry("polyglot", {
    display_name: "Polyglot-Ko",
    category: "roleplay",
    roleplay_strength: 9,
    install_size_mb: 7700,
  }),
];

const FIXTURE_REC: Recommendation = {
  best_choice: "exaone-1.2b",
  balanced_choice: "exaone-1.2b",
  lightweight_choice: "exaone-1.2b",
  fallback_choice: "exaone-1.2b",
  excluded: [
    {
      kind: "insufficient-vram",
      id: "polyglot",
      need_mb: 10240,
      have_mb: 6144,
    },
  ],
  expected_tradeoffs: [],
};

beforeEach(() => {
  vi.mocked(getCatalog).mockResolvedValue({
    entries: FIXTURE_ENTRIES,
    recommendation: null,
  });
  vi.mocked(getRecommendation).mockResolvedValue(FIXTURE_REC);
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("CatalogPage 렌더", () => {
  it("카탈로그 entries 모두 렌더 + Best 슬롯에 추천 모델 이름 표시", async () => {
    render(<CatalogPage />);
    await waitFor(() => {
      expect(screen.getAllByText("EXAONE 4.0 1.2B").length).toBeGreaterThan(0);
      expect(screen.getByText("Qwen 2.5 Coder")).toBeTruthy();
      expect(screen.getByText("Polyglot-Ko")).toBeTruthy();
    });
    // 추천 strip — Best 라벨 + 모델 이름.
    await waitFor(() => {
      expect(screen.getByText("recommendation.best.label")).toBeTruthy();
    });
  });

  it("카테고리 변경 시 그리드 필터링", async () => {
    const user = userEvent.setup();
    const { container } = render(<CatalogPage />);
    await waitFor(() => {
      expect(screen.getAllByText("EXAONE 4.0 1.2B").length).toBeGreaterThan(0);
    });

    const codingTab = screen.getByRole("radio", {
      name: /catalog\.category\.coding/,
    });
    await user.click(codingTab);

    // coding 카테고리만 그리드에 — 그리드 안에서만 카드 검사.
    await waitFor(() => {
      const grid = container.querySelector(".catalog-grid")!;
      const titles = within(grid as HTMLElement).getAllByRole("listitem");
      expect(titles.length).toBe(1);
      expect(within(titles[0] as HTMLElement).getByText("Qwen 2.5 Coder")).toBeTruthy();
    });
  });

  it("excluded 모델은 dim + reason chip 표시", async () => {
    render(<CatalogPage />);
    await waitFor(() => {
      // polyglot은 excluded — exclude-chip이 보여야 함.
      const cards = screen.getAllByTestId("exclude-chip");
      expect(cards.length).toBeGreaterThan(0);
    });
  });

  it("카드 클릭 → Drawer 열리고 quant_options 표시", async () => {
    const user = userEvent.setup();
    render(<CatalogPage />);
    await waitFor(() => {
      expect(screen.getAllByText("EXAONE 4.0 1.2B").length).toBeGreaterThan(0);
    });

    // .catalog-card 안의 EXAONE을 찾아 부모 클릭.
    const cards = screen
      .getAllByText("EXAONE 4.0 1.2B")
      .map((el) => el.closest(".catalog-card"))
      .filter((x): x is Element => !!x);
    expect(cards.length).toBeGreaterThan(0);
    await user.click(cards[0]!);

    await waitFor(() => {
      expect(screen.getByRole("dialog")).toBeTruthy();
      expect(screen.getByText("Q4_K_M")).toBeTruthy();
      expect(screen.getByText("drawer.quantRecommended")).toBeTruthy();
    });
  });

  it("Drawer에서 Esc로 닫기", async () => {
    const user = userEvent.setup();
    render(<CatalogPage />);
    await waitFor(() => {
      expect(screen.getAllByText("EXAONE 4.0 1.2B").length).toBeGreaterThan(0);
    });
    const cards = screen
      .getAllByText("EXAONE 4.0 1.2B")
      .map((el) => el.closest(".catalog-card"))
      .filter((x): x is Element => !!x);
    await user.click(cards[0]!);
    await waitFor(() => expect(screen.getByRole("dialog")).toBeTruthy());
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it('"추천만" 토글하면 그리드가 추천 4슬롯 모델만 남김', async () => {
    const user = userEvent.setup();
    const { container } = render(<CatalogPage />);
    await waitFor(() => {
      expect(screen.getAllByText("EXAONE 4.0 1.2B").length).toBeGreaterThan(0);
    });

    const recOnly = screen.getByRole("button", {
      name: /catalog\.filter\.recommendedOnly/,
    });
    await user.click(recOnly);

    await waitFor(() => {
      // 4슬롯 모두 exaone-1.2b — 그리드는 카드 1개만.
      const grid = container.querySelector(".catalog-grid")!;
      const titles = within(grid as HTMLElement).getAllByRole("listitem");
      expect(titles.length).toBe(1);
      expect(within(titles[0] as HTMLElement).getByText("EXAONE 4.0 1.2B")).toBeTruthy();
    });
  });

  it("getRecommendation 실패 시 strip이 추천을 안 보여줌", async () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    vi.mocked(getRecommendation).mockRejectedValueOnce(new Error("host probe failed"));
    render(<CatalogPage />);
    await waitFor(() => {
      expect(screen.getAllByText("EXAONE 4.0 1.2B").length).toBeGreaterThan(0);
    });
    // strip은 null이므로 best 라벨이 없어야 함.
    await waitFor(() => {
      expect(screen.queryByText("recommendation.best.label")).toBeNull();
    });
    warnSpy.mockRestore();
  });

  it("카탈로그 빈 상태 — entries=[]", async () => {
    vi.mocked(getCatalog).mockResolvedValueOnce({
      entries: [],
      recommendation: null,
    });
    render(<CatalogPage />);
    await waitFor(() => {
      expect(screen.getByText(/catalog\.empty/)).toBeTruthy();
    });
  });
});

describe("CatalogPage 정렬", () => {
  it("'설치 크기 작은순'으로 정렬 변경", async () => {
    const user = userEvent.setup();
    render(<CatalogPage />);
    await waitFor(() => {
      expect(screen.getAllByText("EXAONE 4.0 1.2B").length).toBeGreaterThan(0);
    });

    const sortSelect = screen.getByRole("combobox");
    await user.selectOptions(sortSelect, "size");

    // 카드 순서 — 작은 것부터 (exaone 760 < qwen 760(동률, 알파벳) < polyglot 7700).
    await waitFor(() => {
      const titles = screen
        .getAllByRole("listitem")
        .map((el) => within(el as HTMLElement).queryByRole("button")?.textContent ?? "")
        .filter((t) => t.includes("EXAONE") || t.includes("Qwen") || t.includes("Polyglot"));
      // Polyglot이 마지막이어야 함.
      expect(titles[titles.length - 1]).toContain("Polyglot");
    });
  });
});
