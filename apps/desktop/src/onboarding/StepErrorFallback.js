import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// 에러 경계 fallback — react-error-boundary v4. Phase 1A.4.a 보강 §3.
//
// 정책: per-step 경계 + resetKeys=[step]로 transition 시 자동 reset.
// 한국어 해요체 — "문제가 생겼어요" / "다시 시도".
import { useTranslation } from "react-i18next";
export function StepErrorFallback({ error, resetErrorBoundary }) {
    const { t } = useTranslation();
    const message = error instanceof Error ? error.message : String(error ?? "");
    return (_jsxs("div", { role: "alert", className: "onb-error", children: [_jsx("h2", { className: "onb-error-title", children: t("onboarding.error.title") }), _jsx("p", { className: "onb-error-body", children: t("onboarding.error.body") }), message && (_jsx("pre", { className: "onb-error-detail", "aria-label": "error detail", children: message })), _jsx("div", { className: "onb-error-actions", children: _jsx("button", { type: "button", className: "onb-button onb-button-primary", onClick: resetErrorBoundary, children: t("onboarding.error.retry") }) })] }));
}
