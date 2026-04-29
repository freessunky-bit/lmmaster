/**
 * @vitest-environment jsdom
 */
// persistence (localStorage 헬퍼) 단위 테스트. Phase 1A.4.d.1.
import { beforeEach, describe, expect, it } from "vitest";
import { isOnboardingCompleted, loadSnapshot, markCompleted, resetOnboarding, saveSnapshot, } from "./persistence";
beforeEach(() => {
    localStorage.clear();
});
describe("persistence", () => {
    it("snapshot save → load round-trip", () => {
        saveSnapshot({ value: "language", context: { lang: "ko" } });
        expect(loadSnapshot()).toEqual({ value: "language", context: { lang: "ko" } });
    });
    it("loadSnapshot returns undefined when missing", () => {
        expect(loadSnapshot()).toBeUndefined();
    });
    it("loadSnapshot returns undefined on JSON parse error", () => {
        localStorage.setItem("lmmaster.onboarding.v1", "{not-json");
        expect(loadSnapshot()).toBeUndefined();
    });
    it("markCompleted + isOnboardingCompleted", () => {
        expect(isOnboardingCompleted()).toBe(false);
        markCompleted();
        expect(isOnboardingCompleted()).toBe(true);
    });
    it("isOnboardingCompleted returns false for arbitrary value", () => {
        localStorage.setItem("lmmaster.onboarding.completed", "0");
        expect(isOnboardingCompleted()).toBe(false);
        localStorage.setItem("lmmaster.onboarding.completed", "true");
        expect(isOnboardingCompleted()).toBe(false); // 정확히 "1"만 true
    });
    it("resetOnboarding clears both keys", () => {
        saveSnapshot({ x: 1 });
        markCompleted();
        resetOnboarding();
        expect(loadSnapshot()).toBeUndefined();
        expect(isOnboardingCompleted()).toBe(false);
    });
});
