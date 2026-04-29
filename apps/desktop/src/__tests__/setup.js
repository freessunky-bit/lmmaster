// Vitest 글로벌 setup — 모든 테스트 파일에서 자동 로드.
// 정책 (Phase 1A.4.d.1 보강 §1):
// - jest-dom 매처 등록 (toBeInTheDocument 등). 컴포넌트 테스트가 jsdom env에서 자동 활성.
// - install-bridge 싱글톤 cross-test bleed 방지 — beforeEach reset.
// - jsdom env에서 localStorage clear.
import "@testing-library/jest-dom/vitest";
import { afterEach, beforeEach } from "vitest";
import { setInstallEventBridge } from "../onboarding/install-bridge";
// 주: vitest-axe 매처는 vitest 2.x의 Assertion 타입과 type-param이 충돌해 augmentation 불가.
// 테스트 측에서 `axe(container)`의 결과의 `.violations` 배열을 직접 검사한다.
// jsdom polyfills for framer-motion + reduced-motion 감지.
if (typeof globalThis.IntersectionObserver === "undefined") {
    // @ts-expect-error — minimal stub.
    globalThis.IntersectionObserver = class {
        observe() { }
        unobserve() { }
        disconnect() { }
        takeRecords() {
            return [];
        }
    };
}
if (typeof globalThis.matchMedia === "undefined") {
    // jsdom 미구현. framer-motion MotionConfig 사용.
    globalThis.matchMedia = (query) => ({
        matches: false,
        media: query,
        onchange: null,
        addEventListener: () => { },
        removeEventListener: () => { },
        addListener: () => { },
        removeListener: () => { },
        dispatchEvent: () => false,
    });
}
beforeEach(() => {
    // install-bridge 싱글톤 초기화.
    setInstallEventBridge(null);
    // jsdom env에서만 localStorage 존재.
    if (typeof globalThis.localStorage !== "undefined") {
        globalThis.localStorage.clear();
    }
});
afterEach(async () => {
    // RTL cleanup은 jsdom에서만 의미. cleanup() 호출은 동적 import로 — node env에서 모듈 로드 회피.
    if (typeof globalThis.document !== "undefined") {
        const rtl = await import("@testing-library/react");
        rtl.cleanup();
    }
});
