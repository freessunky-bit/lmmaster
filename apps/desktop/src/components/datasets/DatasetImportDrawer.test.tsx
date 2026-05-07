/**
 * @vitest-environment jsdom
 */
// DatasetImportDrawer — Phase 23'.c.2.d.4.5.
// a11y (vitest-axe) + scoped 쿼리 + 한국어 카피 + EULA 분기 invariant.

import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { axe } from "vitest-axe";

// react-i18next mock — fallback string 그대로 노출 (CLAUDE.md 한국어 우선).
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, defaultValue?: string | Record<string, unknown>) =>
      typeof defaultValue === "string" ? defaultValue : key,
    i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
  }),
}));

// Tauri Channel mock — Drawer는 new Channel<T>()를 만들고 invoke에 넘김. axe + UI 단위
// 검증에는 invoke 호출까지 안 가도 OK이라 단순 stub.
vi.mock("@tauri-apps/api/core", () => ({
  Channel: class {
    onmessage: ((ev: unknown) => void) | null = null;
  },
  invoke: vi.fn(),
}));

vi.mock("../../ipc/dataset-import", () => ({
  startDatasetImport: vi.fn(),
  cancelDatasetImport: vi.fn(),
  defaultSampleStrategy: () => ({
    kind: "stratified",
    n: 10000,
    by: ["province", "occupation"],
  }),
  sampleStrategyLabel: (s: { kind: string; n?: number }) => {
    if (s.kind === "full") return "전체 가져오기 (대용량은 시간 오래 걸려요)";
    if (s.kind === "first") return `처음 ${s.n}행 미리보기`;
    return `${s.n}명 균등 분포 (province × occupation)`;
  },
}));

import { DatasetImportDrawer } from "./DatasetImportDrawer";
import type { DatasetEntry } from "../../ipc/datasets";

const SAMPLE_DATASET_BASE: DatasetEntry = {
  id: "test-dataset",
  display_name: "테스트 데이터셋",
  category: "persona-seed",
  source: { type: "hugging-face", repo: "test/repo" },
  size_mb: 100,
  languages: ["ko"],
  license: "Apache-2.0",
  commercial: true,
  format: "parquet",
  use_case: { kind: "persona-seed", narrative_field: "persona" },
  curator_note_ko: "테스트용 데이터셋입니다.",
};

const NSFW_DATASET: DatasetEntry = {
  ...SAMPLE_DATASET_BASE,
  id: "nsfw-dataset",
  display_name: "NSFW 테스트",
  content_warning: "rp-explicit",
};

const NONCOMMERCIAL_DATASET: DatasetEntry = {
  ...SAMPLE_DATASET_BASE,
  id: "noncommercial-dataset",
  display_name: "비상업 테스트",
  license: "CC-BY-NC-4.0",
  commercial: false,
};

describe("DatasetImportDrawer", () => {
  it("dataset이 null이면 렌더링 X", () => {
    const { container } = render(
      <DatasetImportDrawer dataset={null} onClose={vi.fn()} />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("기본 dataset — 라이선스 + 샘플 + start 버튼 표시 (한국어 해요체)", () => {
    render(
      <DatasetImportDrawer dataset={SAMPLE_DATASET_BASE} onClose={vi.fn()} />,
    );
    const dialog = screen.getByRole("dialog");
    // dialog 안 scoped query — getByText 화면 전체 단언 X (CLAUDE.md §4.4).
    expect(within(dialog).getByText("테스트 데이터셋")).toBeTruthy();
    expect(within(dialog).getByText(/Apache-2\.0/)).toBeTruthy();
    expect(within(dialog).getByText(/샘플 크기/)).toBeTruthy();
    expect(
      within(dialog).getByRole("button", { name: /가져올게요/ }),
    ).toBeTruthy();
  });

  it("샘플 preset 3종이 radiogroup으로 노출", () => {
    render(
      <DatasetImportDrawer dataset={SAMPLE_DATASET_BASE} onClose={vi.fn()} />,
    );
    const radioGroup = screen.getByRole("radiogroup");
    const radios = within(radioGroup).getAllByRole("radio");
    expect(radios.length).toBe(3);
    // 권장 (10000명 균등) 라디오는 default checked.
    const recommended = radios.find(
      (r) => (r as HTMLInputElement).checked,
    ) as HTMLInputElement;
    expect(recommended).toBeTruthy();
  });

  it("NSFW dataset — 미성년 보호 동의 체크박스 + 체크 안 했으면 start disabled", async () => {
    render(<DatasetImportDrawer dataset={NSFW_DATASET} onClose={vi.fn()} />);
    const dialog = screen.getByRole("dialog");
    // 미성년 보호 동의 섹션 헤딩 — within으로 scope.
    expect(
      within(dialog).getByRole("heading", { name: /미성년 보호 동의/ }),
    ).toBeTruthy();
    const startBtn = within(dialog).getByRole("button", {
      name: /가져올게요/,
    }) as HTMLButtonElement;
    expect(startBtn.disabled).toBe(true);
    // 체크박스 클릭 후 disabled 해제 검증.
    const checkbox = within(dialog).getByRole("checkbox");
    await userEvent.click(checkbox);
    expect(startBtn.disabled).toBe(false);
  });

  it("비상업 라이선스 dataset — 별도 동의 체크박스", async () => {
    render(
      <DatasetImportDrawer
        dataset={NONCOMMERCIAL_DATASET}
        onClose={vi.fn()}
      />,
    );
    const dialog = screen.getByRole("dialog");
    expect(
      within(dialog).getByRole("heading", { name: /비상업 라이선스 동의/ }),
    ).toBeTruthy();
    expect(within(dialog).getByText(/비상업 전용/)).toBeTruthy();
    // 비상업 동의 전 start disabled.
    const startBtn = within(dialog).getByRole("button", {
      name: /가져올게요/,
    }) as HTMLButtonElement;
    expect(startBtn.disabled).toBe(true);
  });

  it("일반 dataset — axe critical/serious violations 0", async () => {
    const { container } = render(
      <DatasetImportDrawer dataset={SAMPLE_DATASET_BASE} onClose={vi.fn()} />,
    );
    const results = await axe(container);
    // critical / serious만 의무 (best-practice는 디자인 시스템 차원 일부 허용).
    const blocking = results.violations.filter(
      (v) => v.impact === "critical" || v.impact === "serious",
    );
    expect(blocking).toEqual([]);
  });

  it("dialog에 role + aria-modal + aria-labelledby 3종 세트", () => {
    render(
      <DatasetImportDrawer dataset={SAMPLE_DATASET_BASE} onClose={vi.fn()} />,
    );
    const dialog = screen.getByRole("dialog");
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(dialog.getAttribute("aria-labelledby")).toBeTruthy();
    // 첫 close 버튼이 자동 focus.
    expect(document.activeElement?.getAttribute("aria-label")).toBe("닫기");
  });

  it("닫기 버튼 클릭 시 onClose 호출", async () => {
    const onClose = vi.fn();
    render(
      <DatasetImportDrawer dataset={SAMPLE_DATASET_BASE} onClose={onClose} />,
    );
    const closeBtn = screen.getByRole("button", { name: "닫기" });
    await userEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("Esc 키 누르면 onClose (config 단계)", async () => {
    const onClose = vi.fn();
    render(
      <DatasetImportDrawer dataset={SAMPLE_DATASET_BASE} onClose={onClose} />,
    );
    await userEvent.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalled();
  });
});
