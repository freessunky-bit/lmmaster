import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// Step 4 — 완료. Phase 1A.4.a.
//
// 머신은 이미 final 상태에 도달 — 본 컴포넌트는 사용자에게 완료 메시지 + CTA 노출.
// CTA 클릭 시 OnboardingApp이 onComplete 콜백을 호출 → markCompleted → MainShell 전환.
import { useTranslation } from "react-i18next";
export function Step4Done({ onFinish }) {
    const { t } = useTranslation();
    return (_jsxs("section", { className: "onb-step onb-step-done", "aria-labelledby": "onb-step4-title", children: [_jsx("div", { className: "onb-done-mark", "aria-hidden": true, children: "\u2713" }), _jsxs("header", { className: "onb-step-header", children: [_jsx("h1", { id: "onb-step4-title", className: "onb-step-title", children: t("onboarding.done.title") }), _jsx("p", { className: "onb-step-subtitle", children: t("onboarding.done.subtitle") })] }), _jsx("div", { className: "onb-step-actions", children: _jsx("button", { type: "button", className: "onb-button onb-button-primary onb-button-wide", onClick: onFinish, autoFocus: true, children: t("onboarding.done.cta") }) })] }));
}
