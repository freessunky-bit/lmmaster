// xstate snapshot ↔ localStorage 영속화. Phase 1A.4.a 보강 §1, §9.
//
// 정책:
// - 동기 hydrate (Provider mount 전 module-scope에서 읽기) — race 회피.
// - install.running 서브상태는 저장 안 함 — 절반-끝난 install 재개 방지.
// - 완료 플래그는 별도 key — 미래 마이그레이션 시 onboarding 재트리거 안 되게.
// - 모든 IO 실패는 silent (localStorage 차단/SSR 등). 정상 폴백 = 처음부터 시작.

const STATE_KEY = "lmmaster.onboarding.v1";
const COMPLETED_KEY = "lmmaster.onboarding.completed";

/** 안전한 localStorage 읽기. JSON parse/접근 실패 시 undefined. */
export function loadSnapshot(): unknown | undefined {
  try {
    const raw = localStorage.getItem(STATE_KEY);
    if (!raw) return undefined;
    const parsed: unknown = JSON.parse(raw);
    return parsed;
  } catch {
    return undefined;
  }
}

/** 머신 transition 시 호출. install.running 같은 휘발 상태는 저장 직전에 caller가 필터링. */
export function saveSnapshot(snapshot: unknown): void {
  try {
    localStorage.setItem(STATE_KEY, JSON.stringify(snapshot));
  } catch {
    // localStorage가 막혀 있으면 무시 — 다음 mount 시 fresh start.
  }
}

/** 완료 시 한 번 set. 이후 onboarding 자체를 mount하지 않는다. */
export function markCompleted(): void {
  try {
    localStorage.setItem(COMPLETED_KEY, "1");
  } catch {
    // 무시.
  }
}

export function isOnboardingCompleted(): boolean {
  try {
    return localStorage.getItem(COMPLETED_KEY) === "1";
  } catch {
    return false;
  }
}

/** 디버그/QA용 — 완료 플래그 + 진행 snapshot 모두 제거. */
export function resetOnboarding(): void {
  try {
    localStorage.removeItem(STATE_KEY);
    localStorage.removeItem(COMPLETED_KEY);
  } catch {
    // 무시.
  }
}
