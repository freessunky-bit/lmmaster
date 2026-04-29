// install-bridge 싱글톤 단위 테스트. Phase 1A.4.d.1.
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getInstallEventBridge, setInstallEventBridge } from "./install-bridge";
beforeEach(() => {
    // setup.ts가 글로벌 reset하지만 명시적으로도 한 번 더 — 격리 보강.
    setInstallEventBridge(null);
});
describe("install-bridge", () => {
    it("초기 상태는 null", () => {
        expect(getInstallEventBridge()).toBeNull();
    });
    it("set 후 같은 함수 반환", () => {
        const fn = vi.fn();
        setInstallEventBridge(fn);
        expect(getInstallEventBridge()).toBe(fn);
    });
    it("null로 다시 set하면 clear", () => {
        setInstallEventBridge(vi.fn());
        setInstallEventBridge(null);
        expect(getInstallEventBridge()).toBeNull();
    });
    it("교체 — 두 번째 set이 첫 번째를 대체", () => {
        const a = vi.fn();
        const b = vi.fn();
        setInstallEventBridge(a);
        setInstallEventBridge(b);
        expect(getInstallEventBridge()).toBe(b);
        expect(a).not.toHaveBeenCalled();
    });
});
