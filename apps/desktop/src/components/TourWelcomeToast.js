import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// TourWelcomeToast — Phase 12'.c. 첫 실행 마법사 끝나고 나타나는 toast.
//
// 정책 (phase-8p-9p-10p-residual-plan.md §1.9):
// - 우측 하단 toast — "처음이세요? 가이드 둘러볼래요?".
// - "지금 볼게요" → Guide page `getting-started`.
// - "다음에 할게요" → localStorage `lmmaster.tour.skipped = true`.
// - 1회만 표시 — 본 적 있거나 skipped면 안 띄움.
// - design-system token, framer-motion slide-in (reduced-motion 비활성).
// - role=status — accessible 알림.
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { AnimatePresence, motion } from "framer-motion";
import "./tourWelcomeToast.css";
const STORAGE_KEY_SKIPPED = "lmmaster.tour.skipped";
const STORAGE_KEY_SHOWN = "lmmaster.tour.shown";
const NAV_EVENT = "lmmaster:navigate";
const GUIDE_OPEN_EVENT = "lmmaster:guide:open";
const EASE = [0.16, 1, 0.3, 1];
/** 이미 본 적 있거나 skip했는지. SSR 안전. */
function isAlreadyHandled() {
    try {
        const skipped = globalThis.localStorage?.getItem(STORAGE_KEY_SKIPPED) === "true";
        const shown = globalThis.localStorage?.getItem(STORAGE_KEY_SHOWN) === "true";
        return skipped || shown;
    }
    catch {
        // localStorage 접근 실패 → toast를 띄우지 않음 (silent skip).
        return true;
    }
}
function persistSkipped() {
    try {
        globalThis.localStorage?.setItem(STORAGE_KEY_SKIPPED, "true");
    }
    catch {
        /* noop */
    }
}
function persistShown() {
    try {
        globalThis.localStorage?.setItem(STORAGE_KEY_SHOWN, "true");
    }
    catch {
        /* noop */
    }
}
export function TourWelcomeToast({ trigger, onDismiss, }) {
    const { t } = useTranslation();
    const [visible, setVisible] = useState(false);
    // trigger가 true가 되었을 때 1회만 표시.
    useEffect(() => {
        if (!trigger)
            return;
        if (isAlreadyHandled())
            return;
        setVisible(true);
        persistShown();
    }, [trigger]);
    const handleAccept = useCallback(() => {
        setVisible(false);
        // 1) Guide page로 nav.
        try {
            globalThis.window?.dispatchEvent(new CustomEvent(NAV_EVENT, { detail: "guide" }));
        }
        catch {
            /* noop */
        }
        // 2) getting-started 섹션으로 진입 알림.
        try {
            globalThis.window?.dispatchEvent(new CustomEvent(GUIDE_OPEN_EVENT, {
                detail: { section: "getting-started" },
            }));
        }
        catch {
            /* noop */
        }
        onDismiss?.("accepted");
    }, [onDismiss]);
    const handleDecline = useCallback(() => {
        setVisible(false);
        persistSkipped();
        onDismiss?.("declined");
    }, [onDismiss]);
    return (_jsx(AnimatePresence, { children: visible && (_jsxs(motion.div, { className: "tour-welcome-toast", role: "status", "aria-live": "polite", "aria-label": t("screens.tour.title") ?? undefined, "data-testid": "tour-welcome-toast", initial: { opacity: 0, y: 16 }, animate: { opacity: 1, y: 0 }, exit: { opacity: 0, y: 16 }, transition: { duration: 0.2, ease: EASE }, children: [_jsxs("div", { className: "tour-welcome-content", children: [_jsx("h3", { className: "tour-welcome-title", children: t("screens.tour.title") }), _jsx("p", { className: "tour-welcome-body", children: t("screens.tour.body") })] }), _jsxs("div", { className: "tour-welcome-actions", children: [_jsx("button", { type: "button", className: "tour-welcome-decline", onClick: handleDecline, "data-testid": "tour-welcome-decline", children: t("screens.tour.decline") }), _jsx("button", { type: "button", className: "tour-welcome-accept", onClick: handleAccept, "data-testid": "tour-welcome-accept", autoFocus: true, children: t("screens.tour.accept") })] })] })) }));
}
