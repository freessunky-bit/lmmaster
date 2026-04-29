import { jsx as _jsx } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// Guide — Phase 12'.a 단위 테스트.
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("react-i18next", () => ({
    useTranslation: () => ({
        t: (key, fallback) => fallback ?? key,
        i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
    }),
}));
// 테스트는 작은 마크다운만 — 실 가이드 콘텐츠와 분리.
vi.mock("../i18n/guide-ko-v1.md?raw", () => ({
    default: [
        "<!-- section: getting-started -->",
        "# 시작하기",
        "",
        "EULA와 마법사 4단계 안내해드려요",
        "",
        "---",
        "",
        "<!-- section: catalog -->",
        "# 모델 카탈로그",
        "",
        "추천 strip이 PC에 잘 맞는 모델을 보여줘요",
        "",
        "---",
        "",
        "<!-- section: workbench -->",
        "# 워크벤치",
        "",
        "5단계 흐름이에요",
        "",
        "---",
        "",
        "<!-- section: knowledge -->",
        "# 자료 인덱싱 (RAG)",
        "",
        "RAG 본문",
        "",
        "---",
        "",
        "<!-- section: api-keys -->",
        "# API 키",
        "",
        "키 발급 흐름이에요",
        "",
        "---",
        "",
        "<!-- section: portable -->",
        "# 포터블",
        "",
        "내보내기/가져오기 흐름이에요",
        "",
        "---",
        "",
        "<!-- section: diagnostics -->",
        "# 진단",
        "",
        "자가 점검 본문이에요",
        "",
        "---",
        "",
        "<!-- section: faq -->",
        "# FAQ",
        "",
        "흔한 문제 해결 본문",
    ].join("\n"),
}));
vi.mock("../i18n/guide-en-v1.md?raw", () => ({
    default: "<!-- section: getting-started -->\n# Getting Started\n\nEnglish body.",
}));
import { Guide } from "./Guide";
beforeEach(() => {
    globalThis.localStorage.clear();
});
afterEach(() => {
    vi.restoreAllMocks();
});
describe("Guide — 기본 렌더", () => {
    it("페이지가 렌더되고 8개 섹션 nav가 보여요", () => {
        render(_jsx(Guide, {}));
        expect(screen.getByTestId("guide-page")).toBeTruthy();
        expect(screen.getByTestId("guide-section-getting-started")).toBeTruthy();
        expect(screen.getByTestId("guide-section-catalog")).toBeTruthy();
        expect(screen.getByTestId("guide-section-workbench")).toBeTruthy();
        expect(screen.getByTestId("guide-section-knowledge")).toBeTruthy();
        expect(screen.getByTestId("guide-section-api-keys")).toBeTruthy();
        expect(screen.getByTestId("guide-section-portable")).toBeTruthy();
        expect(screen.getByTestId("guide-section-diagnostics")).toBeTruthy();
        expect(screen.getByTestId("guide-section-faq")).toBeTruthy();
    });
    it("초기 active 섹션은 getting-started — 본문이 노출돼요", () => {
        render(_jsx(Guide, {}));
        expect(screen.getByTestId("guide-active-getting-started")).toBeTruthy();
        expect(screen.getByTestId("guide-main").textContent ?? "").toContain("EULA와 마법사 4단계");
    });
    it("a11y violations === []", async () => {
        const { container } = render(_jsx(Guide, {}));
        const results = await axe(container);
        expect(results.violations).toEqual([]);
    });
});
describe("Guide — 섹션 navigation", () => {
    it("섹션 버튼 클릭 → 본문 전환", async () => {
        const user = userEvent.setup();
        render(_jsx(Guide, {}));
        await user.click(screen.getByTestId("guide-section-workbench"));
        expect(screen.getByTestId("guide-active-workbench")).toBeTruthy();
        expect(screen.getByTestId("guide-main").textContent ?? "").toContain("5단계 흐름");
    });
    it("active 섹션은 aria-current=page", async () => {
        const user = userEvent.setup();
        render(_jsx(Guide, {}));
        await user.click(screen.getByTestId("guide-section-catalog"));
        const btn = screen.getByTestId("guide-section-catalog");
        expect(btn.getAttribute("aria-current")).toBe("page");
    });
});
describe("Guide — 검색", () => {
    it("검색어 입력 시 매치되는 섹션만 노출", async () => {
        const user = userEvent.setup();
        render(_jsx(Guide, {}));
        const searchInput = screen.getByTestId("guide-search");
        await user.type(searchInput, "워크벤치");
        // 워크벤치만 매칭, 나머지는 제거.
        await waitFor(() => {
            expect(screen.getByTestId("guide-section-workbench")).toBeTruthy();
            expect(screen.queryByTestId("guide-section-catalog")).toBeNull();
        });
    });
    it("매치 없는 query → 빈 상태 노출", async () => {
        const user = userEvent.setup();
        render(_jsx(Guide, {}));
        await user.type(screen.getByTestId("guide-search"), "존재하지않는키워드xyz");
        await waitFor(() => {
            expect(screen.getByTestId("guide-no-results")).toBeTruthy();
        });
    });
    it("키워드 cheat (한국어 jamo) 매칭", async () => {
        const user = userEvent.setup();
        render(_jsx(Guide, {}));
        // SECTION_KEYWORDS에 등록된 ㅁㄷ는 catalog 매칭.
        await user.type(screen.getByTestId("guide-search"), "ㅁㄷ");
        await waitFor(() => {
            expect(screen.getByTestId("guide-section-catalog")).toBeTruthy();
        });
    });
});
describe("Guide — CTA 버튼", () => {
    it("'이 기능 사용해 볼게요' CTA 클릭 시 lmmaster:navigate 이벤트", async () => {
        const user = userEvent.setup();
        render(_jsx(Guide, {}));
        await user.click(screen.getByTestId("guide-section-workbench"));
        const events = [];
        const handler = (e) => {
            const detail = e.detail;
            if (typeof detail === "string")
                events.push(detail);
        };
        window.addEventListener("lmmaster:navigate", handler);
        try {
            await user.click(screen.getByTestId("guide-cta-try"));
            await waitFor(() => {
                expect(events).toContain("workbench");
            });
        }
        finally {
            window.removeEventListener("lmmaster:navigate", handler);
        }
    });
});
describe("Guide — deep link via initialSection", () => {
    it("initialSection='knowledge' 진입 시 해당 섹션 활성", () => {
        render(_jsx(Guide, { initialSection: "knowledge" }));
        expect(screen.getByTestId("guide-active-knowledge")).toBeTruthy();
    });
    it("외부 dispatch lmmaster:guide:open 시 섹션 전환", async () => {
        render(_jsx(Guide, {}));
        expect(screen.getByTestId("guide-active-getting-started")).toBeTruthy();
        window.dispatchEvent(new CustomEvent("lmmaster:guide:open", {
            detail: { section: "api-keys" },
        }));
        await waitFor(() => {
            expect(screen.getByTestId("guide-active-api-keys")).toBeTruthy();
        });
    });
});
