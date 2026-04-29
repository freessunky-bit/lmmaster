import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// ShortcutsModal — Phase 12'.c. F1 / Shift+? 글로벌 hotkey로 표시되는 단축키 도움말.
//
// 정책 (phase-8p-9p-10p-residual-plan.md §1.9):
// - F1 또는 Shift+?로 표시. Esc / 닫기 / 외부 클릭으로 닫힘.
// - role=dialog + aria-modal=true + focus trap.
// - 표 형식 단축키 목록 — Ctrl+K / F1 / Ctrl+1~9 / Esc.
// - input/textarea focus 시 hotkey 비활성 (글로벌 충돌 회피).
// - design-system token. prefers-reduced-motion 존중.
import { useCallback, useEffect, useRef, useState, } from "react";
import { useTranslation } from "react-i18next";
import "./shortcutsModal.css";
const SHORTCUT_ROWS = [
    { key: "palette", win: ["Ctrl", "K"], mac: ["⌘", "K"] },
    { key: "shortcuts", win: ["F1"], mac: ["F1"] },
    { key: "shortcutsAlt", win: ["Shift", "?"], mac: ["Shift", "?"] },
    { key: "navHome", win: ["Ctrl", "1"], mac: ["⌘", "1"] },
    { key: "navCatalog", win: ["Ctrl", "2"], mac: ["⌘", "2"] },
    { key: "navInstall", win: ["Ctrl", "3"], mac: ["⌘", "3"] },
    { key: "navRuntimes", win: ["Ctrl", "4"], mac: ["⌘", "4"] },
    { key: "navWorkspace", win: ["Ctrl", "5"], mac: ["⌘", "5"] },
    { key: "navProjects", win: ["Ctrl", "6"], mac: ["⌘", "6"] },
    { key: "navKeys", win: ["Ctrl", "7"], mac: ["⌘", "7"] },
    { key: "navWorkbench", win: ["Ctrl", "8"], mac: ["⌘", "8"] },
    { key: "navDiagnostics", win: ["Ctrl", "9"], mac: ["⌘", "9"] },
    { key: "escape", win: ["Esc"], mac: ["Esc"] },
];
export function ShortcutsModal({ open, onClose }) {
    const { t } = useTranslation();
    const dialogRef = useRef(null);
    const closeBtnRef = useRef(null);
    // 첫 렌더 후 close 버튼에 포커스.
    useEffect(() => {
        if (open) {
            const id = globalThis.requestAnimationFrame?.(() => {
                closeBtnRef.current?.focus();
            });
            return () => {
                if (id !== undefined)
                    globalThis.cancelAnimationFrame?.(id);
            };
        }
        return undefined;
    }, [open]);
    // Esc 닫기.
    useEffect(() => {
        if (!open)
            return undefined;
        const onKey = (e) => {
            if (e.key === "Escape") {
                e.preventDefault();
                onClose();
            }
        };
        globalThis.window?.addEventListener("keydown", onKey);
        return () => {
            globalThis.window?.removeEventListener("keydown", onKey);
        };
    }, [open, onClose]);
    // Focus trap.
    const handleKeyDown = useCallback((e) => {
        if (e.key !== "Tab")
            return;
        const root = dialogRef.current;
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
    if (!open)
        return null;
    // 플랫폼 추정 (jsdom에서는 navigator 보조).
    const isMac = (() => {
        try {
            const platform = globalThis.navigator?.platform ?? globalThis.navigator?.userAgent ?? "";
            return /Mac|iPhone|iPad/i.test(platform);
        }
        catch {
            return false;
        }
    })();
    return (_jsx("div", { className: "shortcuts-backdrop", onClick: (e) => {
            if (e.target === e.currentTarget)
                onClose();
        }, "data-testid": "shortcuts-backdrop", children: _jsxs("div", { ref: dialogRef, role: "dialog", "aria-modal": "true", "aria-labelledby": "shortcuts-modal-title", className: "shortcuts-modal", "data-testid": "shortcuts-modal", onKeyDown: handleKeyDown, children: [_jsxs("header", { className: "shortcuts-modal-header", children: [_jsx("h2", { id: "shortcuts-modal-title", className: "shortcuts-modal-title", children: t("screens.shortcuts.title") }), _jsx("button", { type: "button", ref: closeBtnRef, className: "shortcuts-modal-close", onClick: onClose, "aria-label": t("screens.shortcuts.close"), "data-testid": "shortcuts-close", children: "\u00D7" })] }), _jsx("div", { className: "shortcuts-modal-body", children: _jsxs("table", { className: "shortcuts-table", "aria-label": t("screens.shortcuts.title") ?? undefined, children: [_jsx("thead", { children: _jsxs("tr", { children: [_jsx("th", { scope: "col", children: t("screens.shortcuts.col.action") }), _jsx("th", { scope: "col", children: t("screens.shortcuts.col.shortcut") })] }) }), _jsx("tbody", { children: SHORTCUT_ROWS.map((row) => {
                                    const keys = isMac ? row.mac : row.win;
                                    return (_jsxs("tr", { "data-testid": `shortcuts-row-${row.key}`, children: [_jsx("td", { className: "shortcuts-action", children: t(`screens.shortcuts.rows.${row.key}`) }), _jsx("td", { className: "shortcuts-keys", children: keys.map((k, idx) => (_jsxs("span", { className: "shortcuts-kbd-wrap", children: [_jsx("kbd", { className: "shortcuts-kbd", children: k }), idx < keys.length - 1 && (_jsx("span", { "aria-hidden": "true", className: "shortcuts-kbd-sep", children: "+" }))] }, idx))) })] }, row.key));
                                }) })] }) })] }) }));
}
/**
 * 글로벌 hotkey:
 * - F1 / Shift+? — modal toggle.
 * - Ctrl+1~9 / ⌘1~9 — NAV 이동.
 * - input / textarea / contenteditable focus 시 비활성.
 *
 * App.tsx의 useEffect에서 직접 호출해 사용해요.
 */
export function isFormControlActive() {
    const el = globalThis.document?.activeElement;
    if (!el)
        return false;
    const tag = el.tagName?.toLowerCase();
    if (tag === "input" || tag === "textarea" || tag === "select")
        return true;
    if (el.isContentEditable)
        return true;
    return false;
}
/** Ctrl+숫자 키를 NAV 키로 매핑. 9개 항목이라 Ctrl+10 매핑 없음. */
const NAV_NUMBER_MAP = {
    "1": "home",
    "2": "catalog",
    "3": "install",
    "4": "runtimes",
    "5": "workspace",
    "6": "projects",
    "7": "keys",
    "8": "workbench",
    "9": "diagnostics",
};
export function useShortcutsHotkey(opts) {
    const { open, setOpen, onNav } = opts;
    useEffect(() => {
        const onKey = (e) => {
            if (e.repeat)
                return;
            // input focus 시 비활성 — 사용자 타이핑과 충돌 회피.
            if (isFormControlActive())
                return;
            // F1 또는 Shift+?로 modal toggle.
            if (e.key === "F1") {
                e.preventDefault();
                setOpen(!open);
                return;
            }
            if (e.key === "?" && e.shiftKey) {
                e.preventDefault();
                setOpen(!open);
                return;
            }
            // Ctrl/⌘ + 1~9 NAV 이동 — modal 안에서도 동작 OK.
            if ((e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey) {
                const target = NAV_NUMBER_MAP[e.key];
                if (target) {
                    e.preventDefault();
                    onNav(target);
                }
            }
        };
        globalThis.window?.addEventListener("keydown", onKey);
        return () => {
            globalThis.window?.removeEventListener("keydown", onKey);
        };
    }, [open, setOpen, onNav]);
}
// 필요 시 임포터에서 단순 useShortcutsState 헬퍼 사용 가능.
export function useShortcutsState() {
    const [open, setOpen] = useState(false);
    return { open, setOpen };
}
