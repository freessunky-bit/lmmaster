// CommandPalette filter 단위 테스트 — substring match + jamo cheat keywords + group ordering.
// Phase 1A.4.d.1.
import { describe, expect, it } from "vitest";
import { groupCommands, matchesQuery } from "./filter";
const cmd = (id, group, label, keywords = []) => ({ id, group, label, keywords, perform: () => { } });
describe("matchesQuery", () => {
    it("빈 쿼리는 모두 매칭", () => {
        expect(matchesQuery(cmd("a", "wizard", "Foo"), "")).toBe(true);
        expect(matchesQuery(cmd("a", "wizard", "Foo"), "  ")).toBe(true);
    });
    it("label substring 매칭 — 한국어", () => {
        expect(matchesQuery(cmd("a", "wizard", "환경 다시 점검"), "환경")).toBe(true);
        expect(matchesQuery(cmd("a", "wizard", "환경 다시 점검"), "다시")).toBe(true);
    });
    it("label substring 매칭 — case-insensitive", () => {
        expect(matchesQuery(cmd("a", "wizard", "Restart Wizard"), "wizard")).toBe(true);
        expect(matchesQuery(cmd("a", "wizard", "Restart Wizard"), "WIZARD")).toBe(true);
        expect(matchesQuery(cmd("a", "wizard", "Restart Wizard"), "ReSt")).toBe(true);
    });
    it("keywords 매칭 — EN alias", () => {
        const c = cmd("a", "wizard", "환경 다시 점검", ["scan", "environment"]);
        expect(matchesQuery(c, "scan")).toBe(true);
        expect(matchesQuery(c, "environment")).toBe(true);
    });
    it("keywords 매칭 — jamo cheat (cho-only)", () => {
        const c = cmd("a", "wizard", "환경 다시 점검", ["ㅎㄱㅈㄱ"]);
        expect(matchesQuery(c, "ㅎㄱㅈㄱ")).toBe(true);
    });
    it("매치 없음", () => {
        expect(matchesQuery(cmd("a", "wizard", "홈으로"), "환경")).toBe(false);
        expect(matchesQuery(cmd("a", "wizard", "홈으로", ["home"]), "scan")).toBe(false);
    });
});
describe("groupCommands", () => {
    it("그룹 순서: wizard → navigation → system → diagnostics", () => {
        const list = [
            cmd("d", "diagnostics", "Diag"),
            cmd("s", "system", "Sys"),
            cmd("w", "wizard", "Wiz"),
            cmd("n", "navigation", "Nav"),
        ];
        const out = groupCommands(list);
        expect(out.map(([g]) => g)).toEqual([
            "wizard",
            "navigation",
            "system",
            "diagnostics",
        ]);
    });
    it("그룹 내 삽입 순서 보존", () => {
        const list = [
            cmd("w1", "wizard", "First"),
            cmd("w2", "wizard", "Second"),
            cmd("w3", "wizard", "Third"),
        ];
        const out = groupCommands(list);
        const first = out[0];
        expect(first).toBeDefined();
        expect(first[1].map((c) => c.id)).toEqual(["w1", "w2", "w3"]);
    });
    it("빈 그룹 skip", () => {
        const list = [cmd("w", "wizard", "X")];
        const out = groupCommands(list);
        expect(out.map(([g]) => g)).toEqual(["wizard"]);
    });
    it("빈 입력 → 빈 결과", () => {
        expect(groupCommands([])).toEqual([]);
    });
});
