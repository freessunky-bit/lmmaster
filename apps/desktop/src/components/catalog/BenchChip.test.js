import { jsx as _jsx } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// BenchChip 상태별 렌더 + onMeasure/onCancel/onRetry 콜백 테스트.
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
vi.mock("react-i18next", () => ({
    useTranslation: () => ({
        t: (key, opts) => opts ? `${key}:${JSON.stringify(opts)}` : key,
        i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
    }),
}));
import { BenchChip } from "./BenchChip";
function makeReport(overrides = {}) {
    return {
        runtime_kind: "ollama",
        model_id: "exaone",
        quant_label: "Q4_K_M",
        host_fingerprint_short: "abcd",
        bench_at: null,
        digest_at_bench: null,
        tg_tps: 12.4,
        ttft_ms: 800,
        pp_tps: 80,
        e2e_ms: 5000,
        cold_load_ms: 50,
        peak_vram_mb: null,
        peak_ram_delta_mb: null,
        metrics_source: "native",
        sample_count: 6,
        prompts_used: ["bench-ko-chat"],
        timeout_hit: false,
        sample_text_excerpt: "응답",
        took_ms: 18000,
        error: null,
        ...overrides,
    };
}
describe("BenchChip", () => {
    it("idle 상태는 측정하기 CTA 버튼", async () => {
        const user = userEvent.setup();
        const onMeasure = vi.fn();
        render(_jsx(BenchChip, { state: { kind: "idle" }, onMeasure: onMeasure }));
        const btn = screen.getByRole("button", { name: /bench\.measure/ });
        await user.click(btn);
        expect(onMeasure).toHaveBeenCalledOnce();
    });
    it("running 상태는 spinner + 취소 CTA", async () => {
        const user = userEvent.setup();
        const onCancel = vi.fn();
        render(_jsx(BenchChip, { state: { kind: "running" }, onCancel: onCancel }));
        expect(screen.getByText("bench.running")).toBeTruthy();
        const cancelBtn = screen.getByRole("button", { name: /bench\.cancel/ });
        await user.click(cancelBtn);
        expect(onCancel).toHaveBeenCalledOnce();
    });
    it("측정 완료 ok 상태 — 한 줄 요약 텍스트", () => {
        render(_jsx(BenchChip, { state: { kind: "report", report: makeReport() } }));
        const ok = screen.getByTestId("bench-ok-chip");
        expect(ok).toBeTruthy();
        // tps=12.4, ttft=0.8 — i18n key + interpolation 검증.
        expect(ok.textContent).toContain("12.4");
    });
    it("WallclockEst인 LM Studio 결과는 추정 배지 노출", () => {
        render(_jsx(BenchChip, { state: {
                kind: "report",
                report: makeReport({ metrics_source: "wallclock-est" }),
            } }));
        expect(screen.getByText("bench.wallclockBadge")).toBeTruthy();
    });
    it("timeout + sample_count=0은 timeoutZero chip + retry", async () => {
        const user = userEvent.setup();
        const onRetry = vi.fn();
        render(_jsx(BenchChip, { state: {
                kind: "report",
                report: makeReport({ timeout_hit: true, sample_count: 0 }),
            }, onRetry: onRetry }));
        expect(screen.getByTestId("bench-timeout-chip")).toBeTruthy();
        await user.click(screen.getByRole("button", { name: /bench\.retry/ }));
        expect(onRetry).toHaveBeenCalledOnce();
    });
    it("timeout + sample_count>0은 partial chip", () => {
        render(_jsx(BenchChip, { state: {
                kind: "report",
                report: makeReport({ timeout_hit: true, sample_count: 2 }),
            } }));
        expect(screen.getByTestId("bench-partial-chip")).toBeTruthy();
    });
    it("error 상태는 error chip + retry 버튼 + 한국어 메시지", async () => {
        const user = userEvent.setup();
        const onRetry = vi.fn();
        render(_jsx(BenchChip, { state: {
                kind: "report",
                report: makeReport({
                    tg_tps: 0,
                    error: { kind: "runtime-unreachable", message: "offline" },
                }),
            }, onRetry: onRetry }));
        const chip = screen.getByTestId("bench-error-chip");
        expect(chip.textContent).toContain("bench.error.runtimeUnreachable");
        await user.click(screen.getByRole("button", { name: /bench\.retry/ }));
        expect(onRetry).toHaveBeenCalledOnce();
    });
    it("InsufficientVram 에러는 need/have 인자 포함", () => {
        render(_jsx(BenchChip, { state: {
                kind: "report",
                report: makeReport({
                    error: { kind: "insufficient-vram", need_mb: 12000, have_mb: 6000 },
                }),
            } }));
        const chip = screen.getByTestId("bench-error-chip");
        // GB 변환 단언.
        expect(chip.textContent).toContain("11.7 GB");
        expect(chip.textContent).toContain("5.9 GB");
    });
});
