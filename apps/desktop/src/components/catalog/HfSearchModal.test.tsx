/**
 * @vitest-environment jsdom
 */
// HfSearchModal — Phase 11'.c (ADR-0049) a11y + 검색/등록/큐레이션 흐름 invariants.

import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { axe } from "vitest-axe";

// vi.mock factory는 파일 최상단으로 hoisted — 외부 변수 참조 시 vi.hoisted 사용 필수.
const { openExternalMock, searchHfModelsMock, registerHfModelMock } =
  vi.hoisted(() => ({
    openExternalMock: vi.fn(async (_url: string) => {}),
    searchHfModelsMock: vi.fn(),
    registerHfModelMock: vi.fn(),
  }));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, defaultValue?: string | Record<string, unknown>) =>
      typeof defaultValue === "string" ? defaultValue : key,
    i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
  }),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-shell", () => ({
  open: openExternalMock,
}));

vi.mock("../../ipc/hf_search", async () => {
  const actual =
    await vi.importActual<typeof import("../../ipc/hf_search")>(
      "../../ipc/hf_search",
    );
  return {
    ...actual,
    searchHfModels: searchHfModelsMock,
    registerHfModel: registerHfModelMock,
  };
});

import { HfSearchModal } from "./HfSearchModal";
import type { HfSearchHit } from "../../ipc/hf_search";

const FIXTURE_HITS: HfSearchHit[] = [
  {
    repo: "elyza/Llama-3-ELYZA-JP-8B",
    downloads: 12345,
    likes: 247,
    last_modified: new Date(Date.now() - 3 * 24 * 60 * 60_000).toISOString(),
    pipeline_tag: "text-generation",
    library_name: "transformers",
  },
  {
    repo: "test-org/another-model",
    downloads: 50,
    likes: 5,
    last_modified: new Date().toISOString(),
    pipeline_tag: null,
    library_name: null,
  },
];

beforeEach(() => {
  searchHfModelsMock.mockReset();
  registerHfModelMock.mockReset();
  openExternalMock.mockReset();
  searchHfModelsMock.mockResolvedValue(FIXTURE_HITS);
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("HfSearchModal", () => {
  it("isOpen=false → 렌더 안 함 (검색 호출 X)", () => {
    render(
      <HfSearchModal isOpen={false} query="anything" onClose={() => {}} />,
    );
    expect(screen.queryByTestId("hf-search-modal")).toBeNull();
    expect(searchHfModelsMock).not.toHaveBeenCalled();
  });

  it("isOpen=true 마운트 시 query로 검색 실행 + 결과 노출", async () => {
    render(<HfSearchModal isOpen query="elyza" onClose={() => {}} />);
    await waitFor(() => expect(searchHfModelsMock).toHaveBeenCalledWith("elyza"));
    await screen.findByTestId("hf-search-hit-elyza/Llama-3-ELYZA-JP-8B");
    expect(
      screen.getAllByTestId("hf-search-unsupported").length,
    ).toBeGreaterThan(0);
  });

  it("a11y: dialog role + violations 없음", async () => {
    const { container } = render(
      <HfSearchModal isOpen query="test" onClose={() => {}} />,
    );
    await screen.findByTestId("hf-search-modal");
    expect(screen.getByRole("dialog")).toHaveAttribute("aria-modal", "true");
    const results = await axe(container);
    expect(results.violations).toEqual([]);
  });

  it("Esc 키 → onClose", async () => {
    const onClose = vi.fn();
    render(<HfSearchModal isOpen query="x" onClose={onClose} />);
    await screen.findByTestId("hf-search-modal");
    const user = userEvent.setup();
    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalled();
  });

  it("배경 클릭 → onClose, modal body 클릭은 닫지 않음", async () => {
    const onClose = vi.fn();
    render(<HfSearchModal isOpen query="x" onClose={onClose} />);
    await screen.findByTestId("hf-search-modal");
    const user = userEvent.setup();
    // 배경 클릭.
    await user.click(screen.getByTestId("hf-search-backdrop"));
    expect(onClose).toHaveBeenCalledTimes(1);
    // modal body 클릭 — onClose 추가 호출 X.
    await user.click(screen.getByTestId("hf-search-modal"));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("'지금 시도' → registerHfModel + onRegistered + 모달 닫힘", async () => {
    const onClose = vi.fn();
    const onRegistered = vi.fn();
    registerHfModelMock.mockResolvedValueOnce({
      id: "hf-elyza-Llama-3-ELYZA-JP-8B",
      base_model: "hf.co/elyza/Llama-3-ELYZA-JP-8B",
      quant_type: "auto",
      modelfile: "FROM hf.co/elyza/Llama-3-ELYZA-JP-8B",
      created_at: new Date().toISOString(),
      eval_passed: 0,
      eval_total: 0,
      artifact_paths: [],
    });
    render(
      <HfSearchModal
        isOpen
        query="elyza"
        onClose={onClose}
        onRegistered={onRegistered}
      />,
    );
    await screen.findByTestId("hf-search-hit-elyza/Llama-3-ELYZA-JP-8B");
    const user = userEvent.setup();
    await user.click(
      screen.getByTestId("hf-search-try-elyza/Llama-3-ELYZA-JP-8B"),
    );
    await waitFor(() =>
      expect(registerHfModelMock).toHaveBeenCalledWith(
        "elyza/Llama-3-ELYZA-JP-8B",
      ),
    );
    expect(onRegistered).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it("'큐레이션 추가 요청' → openExternal(GitHub Issue URL)", async () => {
    render(<HfSearchModal isOpen query="x" onClose={() => {}} />);
    await screen.findByTestId("hf-search-hit-elyza/Llama-3-ELYZA-JP-8B");
    const user = userEvent.setup();
    await user.click(
      screen.getByTestId("hf-search-curate-elyza/Llama-3-ELYZA-JP-8B"),
    );
    await waitFor(() => expect(openExternalMock).toHaveBeenCalled());
    const firstCall = openExternalMock.mock.calls[0];
    expect(firstCall).toBeDefined();
    const url = firstCall![0] as string;
    expect(url).toContain("github.com");
    // URLSearchParams는 공백을 '+'로 인코딩 — URL 객체로 디코딩 후 검증.
    const params = new URL(url).searchParams;
    expect(params.get("template")).toBe("curation-request.yml");
    expect(params.get("title")).toContain("[큐레이션 요청]");
    expect(params.get("title")).toContain("elyza/Llama-3-ELYZA-JP-8B");
    expect(params.get("body")).toContain("huggingface.co/elyza/Llama-3-ELYZA-JP-8B");
  });

  it("검색 결과 빈 배열 → empty 메시지", async () => {
    searchHfModelsMock.mockResolvedValueOnce([]);
    render(<HfSearchModal isOpen query="zzzz-no-result" onClose={() => {}} />);
    await screen.findByTestId("hf-search-empty");
  });

  it("HF API 에러 → role=alert + 한국어 메시지", async () => {
    searchHfModelsMock.mockRejectedValueOnce({
      kind: "upstream",
      status: 503,
      message: "HuggingFace 서버 오류 (503)",
    });
    render(<HfSearchModal isOpen query="x" onClose={() => {}} />);
    const error = await screen.findByTestId("hf-search-error");
    expect(error).toHaveAttribute("role", "alert");
    expect(error.textContent).toContain("503");
  });

  it("배너에 '큐레이션 외' 워닝 노출", async () => {
    render(<HfSearchModal isOpen query="x" onClose={() => {}} />);
    const banner = await screen.findByTestId("hf-search-banner");
    expect(banner.textContent).toContain("큐레이션 외");
  });
});
