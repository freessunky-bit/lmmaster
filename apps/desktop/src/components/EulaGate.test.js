import { jsx as _jsx } from "react/jsx-runtime";
/**
 * @vitest-environment jsdom
 */
// EulaGate — Phase 7'.a 첫 실행 동의 게이트 단위 테스트.
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("react-i18next", () => ({
    useTranslation: () => ({
        t: (key, opts) => opts ? `${key}:${JSON.stringify(opts)}` : key,
        i18n: { changeLanguage: vi.fn(), resolvedLanguage: "ko" },
    }),
}));
// `?raw` import는 Vite 전용 — vitest에서 모킹 필요.
vi.mock("../i18n/eula-ko-v1.md?raw", () => ({
    default: "# LMmaster 사용자 동의서\n\n## 1. 개요\n\n한국어 EULA 본문이에요. KOREAN_EULA_MARKER를 포함하고 있어요.\n",
}));
vi.mock("../i18n/eula-en-v1.md?raw", () => ({
    default: "# LMmaster EULA\n\n## 1. Overview\n\nEnglish body. Contains ENGLISH_EULA_MARKER.\n",
}));
import { EulaGate, renderMarkdown } from "./EulaGate";
const VERSION = "1.0.0";
const STORAGE_KEY = `lmmaster.eula.accepted.${VERSION}`;
beforeEach(() => {
    globalThis.localStorage.clear();
});
afterEach(() => {
    vi.restoreAllMocks();
});
/** 본문 스크롤이 끝까지 도달했음을 시뮬레이션. jsdom은 layout이 0이라 강제로 상태를 주입. */
function simulateScrollEnd() {
    const body = screen.getByTestId("eula-body");
    Object.defineProperty(body, "scrollHeight", { value: 1000, configurable: true });
    Object.defineProperty(body, "clientHeight", { value: 200, configurable: true });
    Object.defineProperty(body, "scrollTop", { value: 800, configurable: true });
    body.dispatchEvent(new Event("scroll"));
}
describe("EulaGate — 첫 렌더 + 동의 흐름", () => {
    it("동의 안 한 상태 — dialog 노출 + 한국어 본문 + 동의 버튼 disabled", async () => {
        render(_jsx(EulaGate, { eulaVersion: VERSION, onAccept: vi.fn(), children: _jsx("div", { "data-testid": "protected-child", children: "main app" }) }));
        expect(screen.getByTestId("eula-gate-dialog")).toBeTruthy();
        // 자식은 노출 X.
        expect(screen.queryByTestId("protected-child")).toBeNull();
        // 한국어 본문 default.
        await waitFor(() => {
            expect(screen.getByTestId("eula-body").textContent ?? "").toContain("KOREAN_EULA_MARKER");
        });
        const accept = screen.getByTestId("eula-accept");
        expect(accept.disabled).toBe(true);
    });
    it("스크롤 끝 도달 → 동의 버튼 활성화", async () => {
        render(_jsx(EulaGate, { eulaVersion: VERSION, onAccept: vi.fn(), children: _jsx("div", { children: "child" }) }));
        const accept = screen.getByTestId("eula-accept");
        expect(accept.disabled).toBe(true);
        simulateScrollEnd();
        await waitFor(() => {
            expect(screen.getByTestId("eula-accept").disabled).toBe(false);
        });
    });
    it("동의 클릭 → localStorage 저장 + onAccept 호출 + 자식 노출", async () => {
        const user = userEvent.setup();
        const onAccept = vi.fn();
        render(_jsx(EulaGate, { eulaVersion: VERSION, onAccept: onAccept, children: _jsx("div", { "data-testid": "protected-child", children: "main app" }) }));
        simulateScrollEnd();
        await waitFor(() => {
            expect(screen.getByTestId("eula-accept").disabled).toBe(false);
        });
        await user.click(screen.getByTestId("eula-accept"));
        expect(onAccept).toHaveBeenCalledTimes(1);
        expect(globalThis.localStorage.getItem(STORAGE_KEY)).toBe("true");
        await waitFor(() => {
            expect(screen.getByTestId("protected-child")).toBeTruthy();
        });
        expect(screen.queryByTestId("eula-gate-dialog")).toBeNull();
    });
    it("같은 버전 재진입 → dialog 스킵 + 자식 즉시 노출", () => {
        globalThis.localStorage.setItem(STORAGE_KEY, "true");
        render(_jsx(EulaGate, { eulaVersion: VERSION, onAccept: vi.fn(), children: _jsx("div", { "data-testid": "protected-child", children: "main app" }) }));
        expect(screen.queryByTestId("eula-gate-dialog")).toBeNull();
        expect(screen.getByTestId("protected-child")).toBeTruthy();
    });
    it("English 토글 → 영어 본문 노출 + 스크롤 상태 reset", async () => {
        const user = userEvent.setup();
        render(_jsx(EulaGate, { eulaVersion: VERSION, onAccept: vi.fn(), children: _jsx("div", { children: "child" }) }));
        // 먼저 한국어에서 스크롤 끝.
        simulateScrollEnd();
        await waitFor(() => {
            expect(screen.getByTestId("eula-accept").disabled).toBe(false);
        });
        // English 토글 → 본문 변경 + 스크롤 reset.
        await user.click(screen.getByTestId("eula-lang-en"));
        await waitFor(() => {
            expect(screen.getByTestId("eula-body").textContent ?? "").toContain("ENGLISH_EULA_MARKER");
        });
        // 스크롤 상태 reset → 다시 disabled.
        await waitFor(() => {
            expect(screen.getByTestId("eula-accept").disabled).toBe(true);
        });
    });
    it("거절 → 확인 dialog 노출 → 종료 클릭 시 window.close 호출", async () => {
        const user = userEvent.setup();
        const closeSpy = vi.fn();
        Object.defineProperty(globalThis.window, "close", {
            value: closeSpy,
            writable: true,
            configurable: true,
        });
        render(_jsx(EulaGate, { eulaVersion: VERSION, onAccept: vi.fn(), children: _jsx("div", { children: "child" }) }));
        await user.click(screen.getByTestId("eula-decline"));
        expect(screen.getByTestId("eula-decline-confirm")).toBeTruthy();
        await user.click(screen.getByTestId("eula-decline-exit"));
        expect(closeSpy).toHaveBeenCalledTimes(1);
    });
    it("거절 확인 dialog의 취소 → 동의 화면으로 복귀", async () => {
        const user = userEvent.setup();
        render(_jsx(EulaGate, { eulaVersion: VERSION, onAccept: vi.fn(), children: _jsx("div", { children: "child" }) }));
        await user.click(screen.getByTestId("eula-decline"));
        expect(screen.getByTestId("eula-decline-confirm")).toBeTruthy();
        await user.click(screen.getByTestId("eula-decline-cancel"));
        await waitFor(() => {
            expect(screen.queryByTestId("eula-decline-confirm")).toBeNull();
        });
        expect(screen.getByTestId("eula-gate-dialog")).toBeTruthy();
    });
    it("dialog는 role=dialog + aria-modal=true + aria-labelledby", () => {
        render(_jsx(EulaGate, { eulaVersion: VERSION, onAccept: vi.fn(), children: _jsx("div", { children: "child" }) }));
        const dialog = screen.getByTestId("eula-gate-dialog");
        expect(dialog.getAttribute("role")).toBe("dialog");
        expect(dialog.getAttribute("aria-modal")).toBe("true");
        expect(dialog.getAttribute("aria-labelledby")).toBe("eula-title");
    });
});
describe("EulaGate — a11y", () => {
    it("axe violations === [] (기본 OFF)", async () => {
        const { container } = render(_jsx(EulaGate, { eulaVersion: VERSION, onAccept: vi.fn(), children: _jsx("div", { children: "child" }) }));
        const results = await axe.run(container, {
            rules: { region: { enabled: false } },
        });
        expect(results.violations).toEqual([]);
    });
});
describe("renderMarkdown helper", () => {
    it("# / ## / - / **bold**를 HTML로 변환", () => {
        const html = renderMarkdown("# Title\n\n## Sub\n\n- item one\n- item two\n\n**bold** body");
        expect(html).toContain("<h1>Title</h1>");
        expect(html).toContain("<h2>Sub</h2>");
        expect(html).toContain("<ul>");
        expect(html).toContain("<li>item one</li>");
        expect(html).toContain("<strong>bold</strong>");
    });
    it("HTML inject은 escape돼요 (XSS 방어)", () => {
        const html = renderMarkdown("악성 <script>alert(1)</script> 입력");
        expect(html).not.toContain("<script>");
        expect(html).toContain("&lt;script&gt;");
    });
});
