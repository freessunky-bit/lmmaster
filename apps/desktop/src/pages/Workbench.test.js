import { jsx as _jsx } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// Workbench Phase 5'.b — 5단계 작업대 UI 테스트.
// 정책 (CLAUDE.md §4.4):
// - IPC mock으로 backend 격리.
// - scoped 쿼리 (data-testid) — 동일 텍스트 다중 등장 회피.
// - a11y: vitest-axe violations === [].
// - 한국어 i18n key 검증 — translation 함수가 키를 그대로 반환하도록 stub.
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { axe } from "vitest-axe";
// IPC mock — Channel은 onmessage handler를 호출하기 위한 spy 역할.
vi.mock("../ipc/workbench", () => {
    return {
        isTerminalEvent: (ev) => ev.kind === "completed" || ev.kind === "failed" || ev.kind === "cancelled",
        startWorkbenchRun: vi.fn(),
        cancelWorkbenchRun: vi.fn(),
        listWorkbenchRuns: vi.fn(),
        previewJsonl: vi.fn(),
        serializeExamples: vi.fn(),
    };
});
vi.mock("react-i18next", () => ({
    useTranslation: () => ({
        t: (key, opts) => opts ? `${key}:${JSON.stringify(opts)}` : key,
        i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
    }),
}));
import * as ipc from "../ipc/workbench";
import { Workbench } from "./Workbench";
const startMock = vi.mocked(ipc.startWorkbenchRun);
const cancelMock = vi.mocked(ipc.cancelWorkbenchRun);
const previewMock = vi.mocked(ipc.previewJsonl);
const AXE_OPTIONS = {
    rules: {
        "color-contrast": { enabled: false },
        "html-has-lang": { enabled: false },
        "landmark-one-main": { enabled: false },
        region: { enabled: false },
    },
};
beforeEach(() => {
    startMock.mockReset();
    cancelMock.mockReset();
    previewMock.mockReset();
    globalThis.localStorage.clear();
});
afterEach(() => {
    vi.clearAllMocks();
});
describe("Workbench Phase 5'.b — 5단계 작업대", () => {
    it("Step 1 (data) 진입 — 기본 base_model 입력 + 시작 버튼", () => {
        render(_jsx(Workbench, {}));
        const baseInput = screen.getByTestId("wb-input-base-model");
        expect(baseInput.value).toBe("Qwen2.5-3B");
        expect(screen.getByTestId("workbench-start")).toBeInTheDocument();
    });
    it("Step 1 — dataset path 입력 시 previewJsonl 호출 (디바운스 후)", async () => {
        previewMock.mockResolvedValue([
            {
                messages: [
                    { role: "user", content: "hi" },
                    { role: "assistant", content: "hello" },
                ],
            },
        ]);
        const user = userEvent.setup();
        render(_jsx(Workbench, {}));
        const pathInput = screen.getByTestId("wb-input-dataset-path");
        await user.type(pathInput, "/tmp/x.jsonl");
        // 디바운스 250ms — 짧게 wait.
        await waitFor(() => {
            expect(previewMock).toHaveBeenCalled();
        }, { timeout: 1000 });
        // preview 결과가 화면에 노출.
        await waitFor(() => {
            const preview = screen.getByTestId("wb-preview");
            expect(within(preview).getByText("hi")).toBeInTheDocument();
            expect(within(preview).getByText("hello")).toBeInTheDocument();
        });
    });
    it("시작 버튼 클릭 → startWorkbenchRun 호출 (config 전달)", async () => {
        let capturedOnEvent = null;
        startMock.mockImplementation(async (_config, opts) => {
            capturedOnEvent = opts.onEvent;
            return {
                run_id: "uuid-test",
                cancel: vi.fn().mockResolvedValue(undefined),
            };
        });
        const user = userEvent.setup();
        render(_jsx(Workbench, {}));
        await user.click(screen.getByTestId("workbench-start"));
        await waitFor(() => {
            expect(startMock).toHaveBeenCalledTimes(1);
        });
        expect(capturedOnEvent).toBeTruthy();
        const args = startMock.mock.calls[0];
        expect(args[0]?.base_model_id).toBe("Qwen2.5-3B");
        expect(args[0]?.quant_type).toBe("Q4_K_M");
    });
    it("running 상태 — Started/StageProgress event 도착 시 UI 갱신", async () => {
        let onEvent = null;
        const handle = {
            run_id: "uuid",
            cancel: vi.fn().mockResolvedValue(undefined),
        };
        startMock.mockImplementation(async (config, opts) => {
            onEvent = opts.onEvent;
            return handle;
        });
        const user = userEvent.setup();
        render(_jsx(Workbench, {}));
        await user.click(screen.getByTestId("workbench-start"));
        await waitFor(() => expect(startMock).toHaveBeenCalled());
        // 이벤트 푸시.
        onEvent({
            kind: "started",
            run_id: "uuid",
            config: {
                base_model_id: "Qwen2.5-3B",
                data_jsonl_path: "",
                quant_type: "Q4_K_M",
                lora_epochs: 3,
                korean_preset: true,
                register_to_ollama: true,
            },
        });
        onEvent({
            kind: "stage-progress",
            run_id: "uuid",
            progress: { stage: "quantize", percent: 50, label: "quantizing", message: "진행 중이에요" },
        });
        await waitFor(() => {
            const progress = screen.getByTestId("workbench-progress");
            expect(within(progress).getByRole("progressbar")).toHaveAttribute("aria-valuenow", "50");
            expect(within(progress).getByText("진행 중이에요")).toBeInTheDocument();
        });
        // Cancel 버튼 노출.
        expect(screen.getByTestId("workbench-cancel")).toBeInTheDocument();
    });
    it("Cancel 버튼 → handle.cancel 호출", async () => {
        let onEvent = null;
        const cancelHandle = vi.fn().mockResolvedValue(undefined);
        startMock.mockImplementation(async (_c, opts) => {
            onEvent = opts.onEvent;
            return { run_id: "uuid", cancel: cancelHandle };
        });
        const user = userEvent.setup();
        render(_jsx(Workbench, {}));
        await user.click(screen.getByTestId("workbench-start"));
        await waitFor(() => expect(startMock).toHaveBeenCalled());
        onEvent({ kind: "started", run_id: "uuid", config: {} });
        await waitFor(() => screen.getByTestId("workbench-cancel"));
        await user.click(screen.getByTestId("workbench-cancel"));
        expect(cancelHandle).toHaveBeenCalled();
    });
    it("Failed event — 한국어 에러 메시지 alert 노출", async () => {
        let onEvent = null;
        startMock.mockImplementation(async (_c, opts) => {
            onEvent = opts.onEvent;
            return { run_id: "uuid", cancel: vi.fn().mockResolvedValue(undefined) };
        });
        const user = userEvent.setup();
        render(_jsx(Workbench, {}));
        await user.click(screen.getByTestId("workbench-start"));
        await waitFor(() => expect(startMock).toHaveBeenCalled());
        onEvent({ kind: "started", run_id: "uuid", config: {} });
        onEvent({
            kind: "failed",
            run_id: "uuid",
            error: "양자화 단계에서 오류가 났어요",
        });
        await waitFor(() => {
            const alert = screen.getByRole("alert");
            expect(alert).toHaveTextContent("양자화 단계에서 오류가 났어요");
        });
        // retry 버튼 노출.
        expect(screen.getByTestId("workbench-retry")).toBeInTheDocument();
    });
    it("Completed event — summary + Modelfile preview 노출", async () => {
        let onEvent = null;
        startMock.mockImplementation(async (_c, opts) => {
            onEvent = opts.onEvent;
            return { run_id: "uuid", cancel: vi.fn().mockResolvedValue(undefined) };
        });
        const user = userEvent.setup();
        render(_jsx(Workbench, {}));
        await user.click(screen.getByTestId("workbench-start"));
        await waitFor(() => expect(startMock).toHaveBeenCalled());
        onEvent({ kind: "started", run_id: "uuid", config: {} });
        onEvent({
            kind: "completed",
            run_id: "uuid",
            summary: {
                run_id: "uuid",
                total_duration_ms: 12345,
                artifact_paths: ["a.gguf", "b/adapter"],
                eval_passed: 9,
                eval_total: 10,
                modelfile_preview: "FROM ./test.gguf\nSYSTEM \"\"\"한국어 helper\"\"\"\n",
            },
        });
        await waitFor(() => {
            const status = screen.getByTestId("workbench-status");
            expect(status).toHaveAttribute("data-status", "completed");
        });
        // preview는 register step에서 노출 — currentStep이 register로 자동 이동.
        await waitFor(() => {
            const mf = screen.getByTestId("wb-modelfile-preview");
            expect(mf).toHaveTextContent("FROM ./test.gguf");
        });
        expect(screen.getByTestId("wb-summary")).toBeInTheDocument();
        expect(screen.getByTestId("workbench-new-run")).toBeInTheDocument();
    });
    it("Quantize step — 4가지 quant type radiogroup + 선택", async () => {
        const user = userEvent.setup();
        render(_jsx(Workbench, {}));
        // data step에서 next로 이동 — workbench-stepper에서 quantize trigger 클릭.
        const triggers = screen.getAllByRole("button", {
            name: (n) => n.includes("screens.workbench.stepper"),
        });
        // quantize는 두 번째.
        await user.click(triggers[1]);
        // 4가지 radio 노출.
        for (const q of ["Q4_K_M", "Q5_K_M", "Q8_0", "FP16"]) {
            expect(screen.getByTestId(`wb-quant-${q}`)).toBeInTheDocument();
        }
        // 기본 Q4_K_M 선택됨.
        expect(screen.getByTestId("wb-quant-Q4_K_M")).toHaveAttribute("aria-checked", "true");
        // Q5_K_M 클릭 → checked 전환.
        await user.click(screen.getByTestId("wb-quant-Q5_K_M"));
        expect(screen.getByTestId("wb-quant-Q5_K_M")).toHaveAttribute("aria-checked", "true");
        expect(screen.getByTestId("wb-quant-Q4_K_M")).toHaveAttribute("aria-checked", "false");
    });
    it("LoRA step — Korean preset toggle 동작", async () => {
        const user = userEvent.setup();
        render(_jsx(Workbench, {}));
        const triggers = screen.getAllByRole("button", {
            name: (n) => n.includes("screens.workbench.stepper"),
        });
        await user.click(triggers[2]); // lora
        const toggle = screen.getByTestId("wb-toggle-korean-preset");
        expect(toggle).toHaveAttribute("aria-checked", "true");
        await user.click(toggle);
        expect(toggle).toHaveAttribute("aria-checked", "false");
    });
    it("base_model 빈 문자열로 시작 시도 → 에러 노출", async () => {
        startMock.mockResolvedValue({
            run_id: "uuid",
            cancel: vi.fn().mockResolvedValue(undefined),
        });
        const user = userEvent.setup();
        render(_jsx(Workbench, {}));
        const baseInput = screen.getByTestId("wb-input-base-model");
        await user.clear(baseInput);
        await user.click(screen.getByTestId("workbench-start"));
        await waitFor(() => {
            expect(screen.getByTestId("workbench-error")).toBeInTheDocument();
        });
        // backend 호출은 일어나지 않아야 함.
        expect(startMock).not.toHaveBeenCalled();
    });
    it("Stepper trigger는 running 상태에서 disabled", async () => {
        let onEvent = null;
        startMock.mockImplementation(async (_c, opts) => {
            onEvent = opts.onEvent;
            return { run_id: "uuid", cancel: vi.fn().mockResolvedValue(undefined) };
        });
        const user = userEvent.setup();
        render(_jsx(Workbench, {}));
        await user.click(screen.getByTestId("workbench-start"));
        await waitFor(() => expect(startMock).toHaveBeenCalled());
        onEvent({ kind: "started", run_id: "uuid", config: {} });
        await waitFor(() => {
            const triggers = screen.getAllByRole("button", {
                name: (n) => n.includes("screens.workbench.stepper"),
            });
            expect(triggers.every((t) => t.disabled)).toBe(true);
        });
    });
    it("a11y violations 없음 (idle 상태)", async () => {
        const { container } = render(_jsx(Workbench, {}));
        const results = await axe(container, AXE_OPTIONS);
        expect(results.violations).toEqual([]);
    });
    // ── Phase 5'.e — Runtime selector ────────────────────────────────
    it("Runtime selector — 3개 옵션 (mock/ollama/lm-studio)", () => {
        render(_jsx(Workbench, {}));
        expect(screen.getByTestId("wb-runtime-mock")).toBeInTheDocument();
        expect(screen.getByTestId("wb-runtime-ollama")).toBeInTheDocument();
        expect(screen.getByTestId("wb-runtime-lm-studio")).toBeInTheDocument();
        // 기본은 mock 선택.
        expect(screen.getByTestId("wb-runtime-mock")).toHaveAttribute("aria-checked", "true");
    });
    it("Runtime selector — Mock 기본 상태에서 base URL 입력 숨김", () => {
        render(_jsx(Workbench, {}));
        expect(screen.queryByTestId("wb-runtime-http-fields")).not.toBeInTheDocument();
    });
    it("Runtime selector — Ollama 선택 시 base URL/model id 입력 노출", async () => {
        const user = userEvent.setup();
        render(_jsx(Workbench, {}));
        await user.click(screen.getByTestId("wb-runtime-ollama"));
        expect(screen.getByTestId("wb-runtime-ollama")).toHaveAttribute("aria-checked", "true");
        expect(screen.getByTestId("wb-runtime-http-fields")).toBeInTheDocument();
        expect(screen.getByTestId("wb-input-runtime-base-url")).toBeInTheDocument();
        expect(screen.getByTestId("wb-input-runtime-model-id")).toBeInTheDocument();
        // ollama 선택 시 ollama-create 토글 노출.
        expect(screen.getByTestId("wb-toggle-ollama-create")).toBeInTheDocument();
    });
    it("Runtime selector — LM Studio 선택 시 base URL 노출 + ollama-create 토글 숨김", async () => {
        const user = userEvent.setup();
        render(_jsx(Workbench, {}));
        await user.click(screen.getByTestId("wb-runtime-lm-studio"));
        expect(screen.getByTestId("wb-runtime-lm-studio")).toHaveAttribute("aria-checked", "true");
        expect(screen.getByTestId("wb-runtime-http-fields")).toBeInTheDocument();
        // LM Studio일 때는 ollama-create 토글 미노출.
        expect(screen.queryByTestId("wb-toggle-ollama-create")).not.toBeInTheDocument();
    });
    it("ollama-create-progress 이벤트 — 라인이 누적되어 노출", async () => {
        let onEvent = null;
        startMock.mockImplementation(async (_c, opts) => {
            onEvent = opts.onEvent;
            return { run_id: "uuid", cancel: vi.fn().mockResolvedValue(undefined) };
        });
        const user = userEvent.setup();
        render(_jsx(Workbench, {}));
        // 시작 후 register 단계 도달 시뮬레이션.
        await user.click(screen.getByTestId("workbench-start"));
        await waitFor(() => expect(startMock).toHaveBeenCalled());
        onEvent({ kind: "started", run_id: "uuid", config: {} });
        onEvent({
            kind: "ollama-create-started",
            run_id: "uuid",
            output_name: "lmmaster-qwen-12345678",
        });
        onEvent({
            kind: "ollama-create-progress",
            run_id: "uuid",
            line: "transferring model data",
        });
        onEvent({
            kind: "ollama-create-progress",
            run_id: "uuid",
            line: "writing manifest",
        });
        onEvent({
            kind: "completed",
            run_id: "uuid",
            summary: {
                run_id: "uuid",
                total_duration_ms: 100,
                artifact_paths: [],
                eval_passed: 10,
                eval_total: 10,
                modelfile_preview: "FROM ./x.gguf",
            },
        });
        await waitFor(() => {
            expect(screen.getByTestId("wb-ollama-output-name")).toHaveTextContent("lmmaster-qwen-12345678");
            expect(screen.getByTestId("wb-ollama-create-log")).toHaveTextContent("transferring model data");
            expect(screen.getByTestId("wb-ollama-create-log")).toHaveTextContent("writing manifest");
        });
    });
});
