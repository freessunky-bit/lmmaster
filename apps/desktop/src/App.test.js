/**
 * @vitest-environment jsdom
 */
// App shell — workspace nav 노출 회귀 테스트 (Phase 4.5'.b 마무리).
//
// 정책 (CLAUDE.md §4.4):
// - App 전체 mount는 무거우니 i18n key + nav 키 검증으로 단순화.
// - 실제 클릭 동작은 Workspace.test가 페이지 단위로 책임.
import { describe, expect, it } from "vitest";
import koLocale from "./i18n/ko.json";
import enLocale from "./i18n/en.json";
describe("App shell — workspace nav i18n + 키", () => {
    it("ko/en 모두 nav.workspace 키 노출", () => {
        expect(koLocale.nav.workspace).toBe("워크스페이스");
        expect(enLocale.nav.workspace).toBe("Workspace");
    });
    it("nav 키 셋이 동일 (ko/en symmetry)", () => {
        const koKeys = Object.keys(koLocale.nav).sort();
        const enKeys = Object.keys(enLocale.nav).sort();
        expect(koKeys).toEqual(enKeys);
    });
    it("workspace는 runtimes와 projects 사이에 등장 (sidebar ordering)", () => {
        const koKeys = Object.keys(koLocale.nav);
        const idxRuntimes = koKeys.indexOf("runtimes");
        const idxWorkspace = koKeys.indexOf("workspace");
        const idxProjects = koKeys.indexOf("projects");
        expect(idxRuntimes).toBeGreaterThanOrEqual(0);
        expect(idxWorkspace).toBeGreaterThanOrEqual(0);
        expect(idxProjects).toBeGreaterThanOrEqual(0);
        expect(idxRuntimes).toBeLessThan(idxWorkspace);
        expect(idxWorkspace).toBeLessThan(idxProjects);
    });
});
