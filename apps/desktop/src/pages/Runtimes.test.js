import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// Runtimes 페이지 — Phase 4.c.
//
// 테스트 invariant (phase-4-screens-decision.md §4 + phase-4c-runtimes-decision.md §4):
// 1. listRuntimeStatuses → 좌측 카드 2개 (Ollama / LM Studio).
// 2. 첫 카드 클릭 → 우측 모델 목록 fetch + 표시.
// 3. 검색 input 입력 → 모델 필터링.
// 4. 빈 모델 → 빈 상태 + CTA.
// 5. start/stop/restart 버튼은 disabled.
// 6. axe 0 violation.
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { axe } from "vitest-axe";
vi.mock("react-i18next", () => ({
    useTranslation: () => ({
        t: (key, opts) => opts ? `${key}:${JSON.stringify(opts)}` : key,
        i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
    }),
}));
vi.mock("../ipc/runtimes", () => ({
    listRuntimeStatuses: vi.fn(),
    listRuntimeModels: vi.fn(),
}));
// VirtualList 자체는 Phase 4.a에서 테스트 완료 — 여기선 jsdom에서 ResizeObserver/scrollHeight
// 측정이 안 돼 row 0개로 그려지는 문제를 피하려고 단순 list로 mock.
vi.mock("@lmmaster/design-system/react", () => ({
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    StatusPill: ({ label, detail }) => (_jsxs("span", { "data-testid": "status-pill", children: [label, detail ? _jsx("span", { className: "num", children: detail }) : null] })),
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    VirtualList: (props) => (_jsx("div", { role: "list", "aria-label": props.ariaLabel, children: props.items.map((item, idx) => (_jsx("div", { role: "listitem", className: "ds-vlist-row", style: { height: props.rowHeight ?? 24 }, children: props.renderRow(item, idx) }, props.keyOf(item)))) })),
}));
import { listRuntimeModels, listRuntimeStatuses, } from "../ipc/runtimes";
import { RuntimesPage } from "./Runtimes";
// ResizeObserver — @tanstack/react-virtual 의존. jsdom 미구현이라 stub.
class ResizeObserverStub {
    observe() { }
    unobserve() { }
    disconnect() { }
}
if (typeof globalThis.ResizeObserver === "undefined") {
    globalThis.ResizeObserver =
        ResizeObserverStub;
}
const FIXTURE_STATUSES = [
    {
        kind: "ollama",
        installed: true,
        version: "0.4.0",
        running: true,
        latency_ms: 12,
        model_count: 3,
        last_ping_at: "2026-04-27T00:00:00Z",
    },
    {
        kind: "lm-studio",
        installed: true,
        version: null,
        running: false,
        latency_ms: null,
        model_count: 0,
        last_ping_at: "2026-04-27T00:00:00Z",
    },
];
const FIXTURE_OLLAMA_MODELS = [
    {
        runtime_kind: "ollama",
        id: "exaone:1.2b",
        size_bytes: 800_000_000,
        digest: "abc12345fedcba",
    },
    {
        runtime_kind: "ollama",
        id: "qwen2.5:3b",
        size_bytes: 2_000_000_000,
        digest: "def98765abcdef",
    },
    {
        runtime_kind: "ollama",
        id: "deepseek-coder:6.7b",
        size_bytes: 4_000_000_000,
        digest: "1234567890abcd",
    },
];
const FIXTURE_LMSTUDIO_EMPTY = [];
beforeEach(() => {
    vi.mocked(listRuntimeStatuses).mockResolvedValue(FIXTURE_STATUSES);
    vi.mocked(listRuntimeModels).mockImplementation(async (kind) => {
        if (kind === "ollama")
            return FIXTURE_OLLAMA_MODELS;
        return FIXTURE_LMSTUDIO_EMPTY;
    });
});
afterEach(() => {
    vi.clearAllMocks();
});
const AXE_OPTIONS = {
    rules: {
        "color-contrast": { enabled: false },
        "html-has-lang": { enabled: false },
        "landmark-one-main": { enabled: false },
        region: { enabled: false },
    },
};
describe("RuntimesPage 렌더", () => {
    it("listRuntimeStatuses → 좌측 카드 2개", async () => {
        render(_jsx(RuntimesPage, {}));
        await waitFor(() => {
            expect(screen.getByTestId("runtime-card-ollama")).toBeTruthy();
            expect(screen.getByTestId("runtime-card-lm-studio")).toBeTruthy();
        });
        expect(screen.getByText("Ollama")).toBeTruthy();
        expect(screen.getByText("LM Studio")).toBeTruthy();
    });
    it("첫 카드 자동 선택 → 우측 모델 목록 fetch + 표시", async () => {
        render(_jsx(RuntimesPage, {}));
        await waitFor(() => {
            expect(screen.getByTestId("runtime-card-ollama")).toBeTruthy();
        });
        // 첫 카드(Ollama)가 자동 선택되어 listRuntimeModels("ollama") 호출됨.
        await waitFor(() => {
            expect(listRuntimeModels).toHaveBeenCalledWith("ollama");
        });
        await waitFor(() => {
            expect(screen.getByText("exaone:1.2b")).toBeTruthy();
            expect(screen.getByText("qwen2.5:3b")).toBeTruthy();
            expect(screen.getByText("deepseek-coder:6.7b")).toBeTruthy();
        });
    });
    it("검색 input 입력 → 모델 필터링", async () => {
        const user = userEvent.setup();
        render(_jsx(RuntimesPage, {}));
        await waitFor(() => {
            expect(screen.getByText("exaone:1.2b")).toBeTruthy();
        });
        const input = screen.getByPlaceholderText("screens.runtimes.models.search");
        await user.type(input, "exaone");
        await waitFor(() => {
            expect(screen.getByText("exaone:1.2b")).toBeTruthy();
            expect(screen.queryByText("qwen2.5:3b")).toBeNull();
            expect(screen.queryByText("deepseek-coder:6.7b")).toBeNull();
        });
    });
    it("LM Studio 카드 클릭 → 모델 0개 → 빈 상태 + CTA 표시", async () => {
        const user = userEvent.setup();
        render(_jsx(RuntimesPage, {}));
        await waitFor(() => {
            expect(screen.getByTestId("runtime-card-lm-studio")).toBeTruthy();
        });
        const lmCard = screen.getByTestId("runtime-card-lm-studio");
        await user.click(lmCard);
        await waitFor(() => {
            expect(listRuntimeModels).toHaveBeenCalledWith("lm-studio");
        });
        await waitFor(() => {
            expect(screen.getByText("screens.runtimes.models.empty.title")).toBeTruthy();
            expect(screen.getByText("screens.runtimes.models.empty.cta")).toBeTruthy();
        });
    });
    it("빈 상태 CTA 클릭 → catalog navigate event 발생", async () => {
        const user = userEvent.setup();
        const dispatchSpy = vi.spyOn(window, "dispatchEvent");
        render(_jsx(RuntimesPage, {}));
        await waitFor(() => {
            expect(screen.getByTestId("runtime-card-lm-studio")).toBeTruthy();
        });
        await user.click(screen.getByTestId("runtime-card-lm-studio"));
        await waitFor(() => {
            expect(screen.getByText("screens.runtimes.models.empty.cta")).toBeTruthy();
        });
        await user.click(screen.getByText("screens.runtimes.models.empty.cta"));
        const navEvents = dispatchSpy.mock.calls
            .map((c) => c[0])
            .filter((e) => e.type === "lmmaster:navigate");
        expect(navEvents.length).toBeGreaterThan(0);
        const detail = navEvents[0].detail;
        expect(detail).toBe("catalog");
        dispatchSpy.mockRestore();
    });
    it("start/stop/restart/logs 버튼 모두 disabled", async () => {
        render(_jsx(RuntimesPage, {}));
        await waitFor(() => {
            expect(screen.getByTestId("runtime-card-ollama")).toBeTruthy();
        });
        const ollamaCard = screen.getByTestId("runtime-card-ollama");
        const stopBtn = within(ollamaCard).getByText("screens.runtimes.card.actions.stop");
        const restartBtn = within(ollamaCard).getByText("screens.runtimes.card.actions.restart");
        const logsBtn = within(ollamaCard).getByText("screens.runtimes.card.actions.logs");
        expect(stopBtn).toHaveProperty("disabled", true);
        expect(restartBtn).toHaveProperty("disabled", true);
        expect(logsBtn).toHaveProperty("disabled", true);
    });
    it("정렬 select — 이름순 → 크기 순 변경", async () => {
        const user = userEvent.setup();
        render(_jsx(RuntimesPage, {}));
        await waitFor(() => {
            expect(screen.getByText("exaone:1.2b")).toBeTruthy();
        });
        const sortSelect = screen.getByRole("combobox");
        await user.selectOptions(sortSelect, "size");
        await waitFor(() => {
            // 크기 큰 순으로 정렬 — deepseek-coder(4GB) > qwen(2GB) > exaone(800MB).
            const rows = screen.getAllByRole("listitem");
            const ids = rows
                .map((r) => r.querySelector(".runtimes-cell-name")?.textContent ?? "")
                .filter((s) => s.length > 0);
            expect(ids[0]).toContain("deepseek-coder");
        });
    });
    it("axe — 페이지 axe violations 0", async () => {
        const { container } = render(_jsx(RuntimesPage, {}));
        await waitFor(() => {
            expect(screen.getByText("exaone:1.2b")).toBeTruthy();
        });
        const results = await axe(container, AXE_OPTIONS);
        expect(results.violations).toEqual([]);
    });
});
