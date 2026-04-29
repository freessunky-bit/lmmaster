import { jsx as _jsx } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// CustomModelsSection — Phase 8'.b.1 사용자 정의 모델 섹션 테스트.
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("react-i18next", () => ({
    useTranslation: () => ({
        t: (key, opts) => opts ? `${key}:${JSON.stringify(opts)}` : key,
    }),
}));
vi.mock("../../ipc/workbench", async () => {
    const actual = await vi.importActual("../../ipc/workbench");
    return {
        ...actual,
        listCustomModels: vi.fn(),
    };
});
// active workspace context — context 없이 호출 시 null 반환하는 useActiveWorkspaceOptional 사용 흐름.
vi.mock("../../contexts/ActiveWorkspaceContext", () => ({
    useActiveWorkspaceOptional: () => null,
}));
import { listCustomModels } from "../../ipc/workbench";
import { CustomModelsSection } from "./CustomModelsSection";
const FIXTURE_MODELS = [
    {
        id: "my-coder-q4",
        base_model: "qwen-coder-7b",
        quant_type: "Q4_K_M",
        lora_adapter: null,
        modelfile: "FROM qwen-coder-7b",
        created_at: "2026-04-28T00:00:00Z",
        eval_passed: 9,
        eval_total: 10,
        artifact_paths: ["/tmp/a.gguf"],
    },
    {
        id: "my-roleplay",
        base_model: "polyglot-ko",
        quant_type: "Q5_K_M",
        lora_adapter: null,
        modelfile: "FROM polyglot-ko",
        created_at: "2026-04-27T00:00:00Z",
        eval_passed: 0,
        eval_total: 0,
        artifact_paths: [],
    },
];
beforeEach(() => {
    vi.mocked(listCustomModels).mockReset();
});
afterEach(() => {
    vi.clearAllMocks();
});
describe("CustomModelsSection (Phase 8'.b.1)", () => {
    it("로딩 상태 → 모델 목록 렌더 + badge 표시", async () => {
        vi.mocked(listCustomModels).mockResolvedValue(FIXTURE_MODELS);
        render(_jsx(CustomModelsSection, {}));
        // 로딩 메시지가 잠깐 보이고 사라짐.
        await waitFor(() => {
            expect(screen.getByTestId("custom-models-section")).toBeTruthy();
        });
        await waitFor(() => {
            const cards = screen.getAllByTestId("custom-model-card");
            expect(cards.length).toBe(2);
        });
        // 첫 카드의 badge + name + meta 확인.
        const cards = screen.getAllByTestId("custom-model-card");
        const first = cards[0];
        expect(within(first).getByTestId("custom-model-badge")).toBeTruthy();
        expect(within(first).getByText("my-coder-q4")).toBeTruthy();
        // basedOn에 base_model 인자.
        expect(within(first).getByText(/screens\.catalog\.custom\.basedOn.*qwen-coder-7b/)).toBeTruthy();
    });
    it("eval_total=0인 모델은 evalSummary 미노출", async () => {
        vi.mocked(listCustomModels).mockResolvedValue([FIXTURE_MODELS[1]]);
        render(_jsx(CustomModelsSection, {}));
        await waitFor(() => {
            expect(screen.getByTestId("custom-model-card")).toBeTruthy();
        });
        expect(screen.queryByText(/screens\.catalog\.custom\.evalSummary/)).toBeNull();
    });
    it("빈 상태 — 'Workbench로 가기' CTA가 보이고 클릭 시 lmmaster:navigate=workbench dispatch", async () => {
        vi.mocked(listCustomModels).mockResolvedValue([]);
        const navHandler = vi.fn();
        window.addEventListener("lmmaster:navigate", navHandler);
        try {
            const user = userEvent.setup();
            render(_jsx(CustomModelsSection, {}));
            await waitFor(() => {
                expect(screen.getByTestId("custom-models-empty")).toBeTruthy();
            });
            const cta = screen.getByTestId("custom-models-empty-cta");
            await user.click(cta);
            await waitFor(() => {
                expect(navHandler).toHaveBeenCalled();
            });
            const ev = navHandler.mock.calls[0][0];
            expect(ev.detail).toBe("workbench");
        }
        finally {
            window.removeEventListener("lmmaster:navigate", navHandler);
        }
    });
    it("listCustomModels 실패 시 빈 상태로 graceful fallback", async () => {
        const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => { });
        vi.mocked(listCustomModels).mockRejectedValue(new Error("backend down"));
        render(_jsx(CustomModelsSection, {}));
        await waitFor(() => {
            expect(screen.getByTestId("custom-models-empty")).toBeTruthy();
        });
        expect(warnSpy).toHaveBeenCalled();
        warnSpy.mockRestore();
    });
    it("카드 클릭 → onSelect 콜백이 CustomModel 인자로 호출", async () => {
        vi.mocked(listCustomModels).mockResolvedValue(FIXTURE_MODELS);
        const onSelect = vi.fn();
        const user = userEvent.setup();
        render(_jsx(CustomModelsSection, { onSelect: onSelect }));
        await waitFor(() => {
            expect(screen.getAllByTestId("custom-model-card").length).toBe(2);
        });
        const cards = screen.getAllByTestId("custom-model-card");
        await user.click(cards[0]);
        expect(onSelect).toHaveBeenCalledTimes(1);
        expect(onSelect.mock.calls[0][0]).toEqual(FIXTURE_MODELS[0]);
    });
    it("scoped 쿼리 — section 안에서 grid 카드만 카운트 (Korean badge가 다른 곳과 충돌 X)", async () => {
        vi.mocked(listCustomModels).mockResolvedValue(FIXTURE_MODELS);
        const { container } = render(_jsx(CustomModelsSection, {}));
        await waitFor(() => {
            expect(screen.getAllByTestId("custom-model-card").length).toBe(2);
        });
        const section = container.querySelector(".catalog-custom-section");
        const cards = within(section).getAllByTestId("custom-model-card");
        expect(cards.length).toBe(2);
    });
});
