import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// HelpButton — Phase 12'.b. 페이지 헤더에 노출되는 ? 도움말 버튼.
//
// 정책 (phase-8p-9p-10p-residual-plan.md §1.9):
// - 클릭 시 popover (focus trap + Esc + role=dialog + aria-modal=true).
// - 짧은 hint 1~2줄 + "전체 가이드 보기" 링크 → Guide page deep link 진입.
// - design-system token, prefers-reduced-motion 존중.
// - sectionId는 Guide.tsx의 SECTION_IDS와 일치해야 해요.
import { useCallback, useEffect, useId, useRef, useState, } from "react";
import { useTranslation } from "react-i18next";
import "./helpButton.css";
const NAV_EVENT = "lmmaster:navigate";
const GUIDE_OPEN_EVENT = "lmmaster:guide:open";
export function HelpButton({ sectionId, hint, ariaLabel, testId, }) {
    const { t } = useTranslation();
    const [open, setOpen] = useState(false);
    const triggerRef = useRef(null);
    const popoverRef = useRef(null);
    const closeBtnRef = useRef(null);
    const linkBtnRef = useRef(null);
    const popoverId = useId();
    // popover 열릴 때 close 버튼에 포커스.
    useEffect(() => {
        if (open) {
            // 마이크로 task 후 — DOM 마운트 완료 대기.
            const id = globalThis.requestAnimationFrame?.(() => {
                closeBtnRef.current?.focus();
            });
            return () => {
                if (id !== undefined)
                    globalThis.cancelAnimationFrame?.(id);
            };
        }
        // 닫혔을 때 트리거로 포커스 복원.
        triggerRef.current?.focus({ preventScroll: true });
        return undefined;
    }, [open]);
    // Esc + 외부 클릭 닫기.
    useEffect(() => {
        if (!open)
            return undefined;
        const onKey = (e) => {
            if (e.key === "Escape") {
                e.preventDefault();
                setOpen(false);
            }
        };
        const onClick = (e) => {
            const root = popoverRef.current;
            const trigger = triggerRef.current;
            if (!root)
                return;
            const target = e.target;
            if (target && !root.contains(target) && !trigger?.contains(target)) {
                setOpen(false);
            }
        };
        globalThis.window?.addEventListener("keydown", onKey);
        globalThis.window?.addEventListener("mousedown", onClick);
        return () => {
            globalThis.window?.removeEventListener("keydown", onKey);
            globalThis.window?.removeEventListener("mousedown", onClick);
        };
    }, [open]);
    const handleToggle = useCallback(() => {
        setOpen((v) => !v);
    }, []);
    const handleOpenGuide = useCallback(() => {
        // 1) Guide page로 nav.
        try {
            globalThis.window?.dispatchEvent(new CustomEvent(NAV_EVENT, { detail: "guide" }));
        }
        catch {
            /* noop */
        }
        // 2) 어떤 섹션으로 이동할지 알림 (Guide가 listen).
        try {
            globalThis.window?.dispatchEvent(new CustomEvent(GUIDE_OPEN_EVENT, { detail: { section: sectionId } }));
        }
        catch {
            /* noop */
        }
        setOpen(false);
    }, [sectionId]);
    // Tab focus trap 안.
    const handleKeyDown = useCallback((e) => {
        if (e.key !== "Tab")
            return;
        const root = popoverRef.current;
        if (!root)
            return;
        const focusable = root.querySelectorAll('button, [href], input, [tabindex]:not([tabindex="-1"])');
        if (focusable.length === 0)
            return;
        const first = focusable.item(0);
        const last = focusable.item(focusable.length - 1);
        if (!first || !last)
            return;
        const active = globalThis.document.activeElement;
        if (e.shiftKey && active === first) {
            e.preventDefault();
            last.focus();
        }
        else if (!e.shiftKey && active === last) {
            e.preventDefault();
            first.focus();
        }
    }, []);
    const tid = testId ?? `help-${sectionId}`;
    const trigLabel = ariaLabel ?? t("screens.help.triggerAria");
    return (_jsxs("span", { className: "help-button-wrap", children: [_jsx("button", { type: "button", ref: triggerRef, className: "help-button-trigger", "aria-label": trigLabel, "aria-haspopup": "dialog", "aria-expanded": open, "aria-controls": open ? popoverId : undefined, onClick: handleToggle, "data-testid": tid, children: _jsx("span", { "aria-hidden": "true", children: "?" }) }), open && (_jsxs("div", { ref: popoverRef, id: popoverId, role: "dialog", "aria-modal": "true", "aria-label": t("screens.help.popoverAria") ?? undefined, className: "help-button-popover", "data-testid": `${tid}-popover`, onKeyDown: handleKeyDown, children: [_jsx("p", { className: "help-button-hint", "data-testid": `${tid}-hint`, children: hint ?? t("screens.help.defaultHint") }), _jsxs("div", { className: "help-button-actions", children: [_jsx("button", { type: "button", ref: linkBtnRef, className: "help-button-link", onClick: handleOpenGuide, "data-testid": `${tid}-open-guide`, children: t("screens.help.openGuide") }), _jsx("button", { type: "button", ref: closeBtnRef, className: "help-button-close", onClick: () => setOpen(false), "data-testid": `${tid}-close`, children: t("screens.help.close") })] })] }))] }));
}
