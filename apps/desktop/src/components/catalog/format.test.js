// format.test — 카탈로그 포맷터 단위 테스트.
import { describe, expect, it } from "vitest";
import { compatOf, formatSize, idOf, languageStars, modelHasFlag, } from "./format";
function makeModel(overrides = {}) {
    return {
        id: "m1",
        display_name: "M1",
        category: "agent-general",
        model_family: "x",
        source: { type: "direct-url", url: "https://x" },
        runner_compatibility: ["llama-cpp"],
        quantization_options: [],
        min_vram_mb: null,
        rec_vram_mb: null,
        min_ram_mb: 1024,
        rec_ram_mb: 2048,
        install_size_mb: 100,
        tool_support: false,
        vision_support: false,
        structured_output_support: false,
        license: "MIT",
        maturity: "stable",
        portable_suitability: 5,
        on_device_suitability: 5,
        fine_tune_suitability: 5,
        verification: { tier: "community" },
        use_case_examples: [],
        warnings: [],
        ...overrides,
    };
}
describe("formatSize", () => {
    it("MB 단위 그대로", () => {
        expect(formatSize(500)).toBe("500 MB");
    });
    it("1024 이상은 GB로 변환", () => {
        expect(formatSize(2048)).toBe("2.0 GB");
        expect(formatSize(7700)).toBe("7.5 GB");
    });
    it("10GB 이상은 정수로", () => {
        expect(formatSize(19500)).toBe("19 GB");
    });
    it("null/undefined → 대시", () => {
        expect(formatSize(null)).toBe("—");
        expect(formatSize(undefined)).toBe("—");
    });
});
describe("languageStars", () => {
    it("0~10을 0~5 별로 환산", () => {
        expect(languageStars(0)).toBe("☆☆☆☆☆");
        expect(languageStars(10)).toBe("★★★★★");
        expect(languageStars(5)).toBe("★★★☆☆");
    });
    it("null/undefined은 0으로", () => {
        expect(languageStars(null)).toBe("☆☆☆☆☆");
        expect(languageStars(undefined)).toBe("☆☆☆☆☆");
    });
});
describe("compatOf", () => {
    it("recommendation 없으면 fit", () => {
        expect(compatOf(makeModel(), null)).toBe("fit");
    });
    it("excluded면 unfit", () => {
        const rec = {
            best_choice: null,
            balanced_choice: null,
            lightweight_choice: null,
            fallback_choice: null,
            excluded: [
                {
                    kind: "insufficient-vram",
                    id: "m1",
                    need_mb: 8000,
                    have_mb: 4000,
                },
            ],
            expected_tradeoffs: [],
        };
        expect(compatOf(makeModel(), rec)).toBe("unfit");
    });
    it("best/lightweight 안에 있으면 fit/exceeds", () => {
        const rec = {
            best_choice: "m1",
            balanced_choice: null,
            lightweight_choice: "m1",
            fallback_choice: null,
            excluded: [],
            expected_tradeoffs: [],
        };
        expect(compatOf(makeModel({ install_size_mb: 100 }), rec)).toBe("fit");
        expect(compatOf(makeModel({ install_size_mb: 8000 }), rec)).toBe("exceeds");
    });
});
describe("idOf", () => {
    it("모든 variant에서 id 추출", () => {
        const reasons = [
            { kind: "insufficient-vram", id: "a", need_mb: 1, have_mb: 0 },
            { kind: "insufficient-ram", id: "b", need_mb: 1, have_mb: 0 },
            { kind: "incompatible-runtime", id: "c" },
            { kind: "deprecated", id: "d" },
        ];
        expect(reasons.map(idOf)).toEqual(["a", "b", "c", "d"]);
    });
});
describe("modelHasFlag", () => {
    it("tool/vision/structured 플래그 정확히 매핑", () => {
        const m = makeModel({
            tool_support: true,
            vision_support: false,
            structured_output_support: true,
        });
        expect(modelHasFlag(m, "tool")).toBe(true);
        expect(modelHasFlag(m, "vision")).toBe(false);
        expect(modelHasFlag(m, "structured")).toBe(true);
    });
});
