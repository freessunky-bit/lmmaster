import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// WorkspaceSwitcher — Phase 8'.1.
//
// 사이드바 상단 dropdown — 현재 active workspace 표시 + 전환 + 생성 + 이름변경 + 삭제.
//
// 정책 (ADR-0038, CLAUDE.md §4.3):
// - role="menu" + aria-haspopup="menu" + aria-expanded.
// - Esc / 배경 클릭 / 다른 곳 click으로 dropdown 닫기.
// - 모달은 role="dialog" aria-modal="true" + focus trap (첫 input 자동 focus).
// - 한국어 해요체.
// - design-system tokens.
// - 삭제는 confirmation dialog 의무 — 사용자 인지.
import { useCallback, useEffect, useRef, useState, } from "react";
import { useTranslation } from "react-i18next";
import { useActiveWorkspace } from "../contexts/ActiveWorkspaceContext";
import "./workspaceSwitcher.css";
export function WorkspaceSwitcher() {
    const { t } = useTranslation();
    const { active, workspaces, setActive, create, rename, remove } = useActiveWorkspace();
    const [open, setOpen] = useState(false);
    const [modal, setModal] = useState({ kind: "none" });
    const triggerRef = useRef(null);
    const menuRef = useRef(null);
    // ── Dropdown — Esc / 외부 click 닫기 ─────────────────────────────────
    useEffect(() => {
        if (!open)
            return;
        const onKey = (e) => {
            if (e.key === "Escape") {
                setOpen(false);
                triggerRef.current?.focus();
            }
        };
        const onClick = (e) => {
            const target = e.target;
            if (menuRef.current &&
                !menuRef.current.contains(target) &&
                triggerRef.current &&
                !triggerRef.current.contains(target)) {
                setOpen(false);
            }
        };
        window.addEventListener("keydown", onKey);
        // mousedown으로 잡으면 click 직전에 닫혀서 트리거가 다시 열림. click을 사용.
        window.addEventListener("click", onClick);
        return () => {
            window.removeEventListener("keydown", onKey);
            window.removeEventListener("click", onClick);
        };
    }, [open]);
    const handleSelect = useCallback(async (id) => {
        if (active && active.id === id) {
            setOpen(false);
            return;
        }
        try {
            await setActive(id);
        }
        catch (e) {
            console.warn("setActive 실패:", e);
        }
        setOpen(false);
        triggerRef.current?.focus();
    }, [active, setActive]);
    const openCreate = useCallback(() => {
        setOpen(false);
        setModal({ kind: "create" });
    }, []);
    const openRename = useCallback((target) => {
        setOpen(false);
        setModal({ kind: "rename", target });
    }, []);
    const openDelete = useCallback((target) => {
        setOpen(false);
        setModal({ kind: "delete", target });
    }, []);
    const closeModal = useCallback(() => {
        setModal({ kind: "none" });
        triggerRef.current?.focus();
    }, []);
    // ── 표시명 ───────────────────────────────────────────────────────────
    const displayName = active?.name ?? t("screens.workspaceSwitcher.loading");
    return (_jsxs("div", { className: "workspace-switcher", "data-testid": "workspace-switcher", children: [_jsxs("button", { ref: triggerRef, type: "button", className: "workspace-switcher-trigger", "aria-haspopup": "menu", "aria-expanded": open, "aria-label": t("screens.workspaceSwitcher.triggerAria", {
                    name: displayName,
                }), "data-testid": "workspace-switcher-trigger", onClick: (e) => {
                    // 외부 click handler가 잡지 않도록 stop.
                    e.stopPropagation();
                    setOpen((v) => !v);
                }, children: [_jsxs("span", { className: "workspace-switcher-trigger-content", children: [_jsx("span", { className: "workspace-switcher-eyebrow", children: t("screens.workspaceSwitcher.current") }), _jsx("span", { className: "workspace-switcher-name", children: displayName })] }), _jsx("span", { className: "workspace-switcher-chevron", "aria-hidden": "true", children: "\u25BE" })] }), open && (_jsxs("div", { ref: menuRef, role: "menu", "aria-label": t("screens.workspaceSwitcher.menuAria"), className: "workspace-switcher-menu", "data-testid": "workspace-switcher-menu", onClick: (e) => e.stopPropagation(), children: [_jsxs("div", { className: "workspace-switcher-menu-section", role: "none", children: [workspaces.length === 0 && (_jsx("p", { className: "workspace-switcher-empty", "data-testid": "workspace-switcher-empty", role: "none", children: t("screens.workspaceSwitcher.empty") })), workspaces.map((w) => {
                                const isActive = active && active.id === w.id;
                                // 한 row = menuitemradio (전환) + 두 menuitem (rename/delete).
                                // 외부 div는 role="none"으로 ARIA 트리에서 제외 — 자식 menuitems는 menu의 직접 자손으로 인식.
                                return (_jsxs("div", { className: "workspace-switcher-row", role: "none", children: [_jsxs("button", { type: "button", role: "menuitemradio", "aria-checked": isActive ?? false, "aria-current": isActive ? "true" : undefined, className: "workspace-switcher-item", "data-testid": `workspace-switcher-item-${w.id}`, onClick: () => handleSelect(w.id), children: [_jsx("span", { className: "workspace-switcher-item-check", "aria-hidden": "true", children: isActive ? "✓" : "" }), _jsx("span", { className: "workspace-switcher-item-name", children: w.name })] }), _jsx("button", { type: "button", role: "menuitem", className: "workspace-switcher-action", onClick: () => openRename(w), "data-testid": `workspace-switcher-rename-${w.id}`, "aria-label": t("screens.workspaceSwitcher.renameAria", {
                                                name: w.name,
                                            }), children: t("screens.workspaceSwitcher.rename") }), _jsx("button", { type: "button", role: "menuitem", className: "workspace-switcher-action is-danger", onClick: () => openDelete(w), "data-testid": `workspace-switcher-delete-${w.id}`, "aria-label": t("screens.workspaceSwitcher.deleteAria", {
                                                name: w.name,
                                            }), disabled: workspaces.length <= 1, children: t("screens.workspaceSwitcher.delete") })] }, w.id));
                            })] }), _jsx("div", { className: "workspace-switcher-menu-section", role: "none", children: _jsx("button", { type: "button", role: "menuitem", className: "workspace-switcher-create", "data-testid": "workspace-switcher-create", onClick: openCreate, children: t("screens.workspaceSwitcher.create") }) })] })), modal.kind === "create" && (_jsx(CreateModal, { onClose: closeModal, onCreate: create })), modal.kind === "rename" && (_jsx(RenameModal, { target: modal.target, onClose: closeModal, onRename: rename })), modal.kind === "delete" && (_jsx(DeleteModal, { target: modal.target, onClose: closeModal, onDelete: remove }))] }));
}
function CreateModal({ onClose, onCreate }) {
    const { t } = useTranslation();
    const [name, setName] = useState("");
    const [description, setDescription] = useState("");
    const [error, setError] = useState(null);
    const [submitting, setSubmitting] = useState(false);
    const inputRef = useRef(null);
    useEffect(() => {
        inputRef.current?.focus();
    }, []);
    useEffect(() => {
        const onKey = (e) => {
            if (e.key === "Escape")
                onClose();
        };
        window.addEventListener("keydown", onKey);
        return () => window.removeEventListener("keydown", onKey);
    }, [onClose]);
    const handleSubmit = useCallback(async (e) => {
        e.preventDefault();
        setError(null);
        const trimmed = name.trim();
        if (trimmed.length === 0) {
            setError(t("screens.workspaceSwitcher.errors.empty"));
            return;
        }
        setSubmitting(true);
        try {
            await onCreate(trimmed, description.trim() || undefined);
            onClose();
        }
        catch (e) {
            setError(extractKoreanError(e, t));
        }
        finally {
            setSubmitting(false);
        }
    }, [description, name, onClose, onCreate, t]);
    return (_jsx("div", { className: "workspace-switcher-modal-backdrop", role: "presentation", onClick: onClose, "data-testid": "workspace-switcher-create-modal", children: _jsxs("form", { role: "dialog", "aria-modal": "true", "aria-labelledby": "workspace-switcher-create-title", className: "workspace-switcher-modal", onClick: (e) => e.stopPropagation(), onSubmit: handleSubmit, children: [_jsx("h3", { id: "workspace-switcher-create-title", className: "workspace-switcher-modal-title", children: t("screens.workspaceSwitcher.createTitle") }), _jsxs("label", { className: "workspace-switcher-modal-field", children: [_jsx("span", { className: "workspace-switcher-modal-label", children: t("screens.workspaceSwitcher.nameLabel") }), _jsx("input", { ref: inputRef, type: "text", className: "workspace-switcher-modal-input", placeholder: t("screens.workspaceSwitcher.namePlaceholder"), value: name, onChange: (e) => setName(e.target.value), "data-testid": "workspace-switcher-create-name" })] }), _jsxs("label", { className: "workspace-switcher-modal-field", children: [_jsx("span", { className: "workspace-switcher-modal-label", children: t("screens.workspaceSwitcher.descriptionLabel") }), _jsx("input", { type: "text", className: "workspace-switcher-modal-input", placeholder: t("screens.workspaceSwitcher.descriptionPlaceholder"), value: description, onChange: (e) => setDescription(e.target.value), "data-testid": "workspace-switcher-create-desc" })] }), error && (_jsx("p", { className: "workspace-switcher-modal-error", role: "alert", "data-testid": "workspace-switcher-create-error", children: error })), _jsxs("footer", { className: "workspace-switcher-modal-footer", children: [_jsx("button", { type: "button", className: "workspace-switcher-modal-button", onClick: onClose, disabled: submitting, "data-testid": "workspace-switcher-create-cancel", children: t("screens.workspaceSwitcher.cancel") }), _jsx("button", { type: "submit", className: "workspace-switcher-modal-button is-primary", disabled: submitting, "data-testid": "workspace-switcher-create-submit", children: t("screens.workspaceSwitcher.submit") })] })] }) }));
}
function RenameModal({ target, onClose, onRename }) {
    const { t } = useTranslation();
    const [name, setName] = useState(target.name);
    const [error, setError] = useState(null);
    const [submitting, setSubmitting] = useState(false);
    const inputRef = useRef(null);
    useEffect(() => {
        inputRef.current?.focus();
        inputRef.current?.select();
    }, []);
    useEffect(() => {
        const onKey = (e) => {
            if (e.key === "Escape")
                onClose();
        };
        window.addEventListener("keydown", onKey);
        return () => window.removeEventListener("keydown", onKey);
    }, [onClose]);
    const handleSubmit = useCallback(async (e) => {
        e.preventDefault();
        setError(null);
        const trimmed = name.trim();
        if (trimmed.length === 0) {
            setError(t("screens.workspaceSwitcher.errors.empty"));
            return;
        }
        if (trimmed === target.name) {
            onClose();
            return;
        }
        setSubmitting(true);
        try {
            await onRename(target.id, trimmed);
            onClose();
        }
        catch (e) {
            setError(extractKoreanError(e, t));
        }
        finally {
            setSubmitting(false);
        }
    }, [name, onClose, onRename, t, target.id, target.name]);
    return (_jsx("div", { className: "workspace-switcher-modal-backdrop", role: "presentation", onClick: onClose, "data-testid": "workspace-switcher-rename-modal", children: _jsxs("form", { role: "dialog", "aria-modal": "true", "aria-labelledby": "workspace-switcher-rename-title", className: "workspace-switcher-modal", onClick: (e) => e.stopPropagation(), onSubmit: handleSubmit, children: [_jsx("h3", { id: "workspace-switcher-rename-title", className: "workspace-switcher-modal-title", children: t("screens.workspaceSwitcher.renameTitle") }), _jsxs("label", { className: "workspace-switcher-modal-field", children: [_jsx("span", { className: "workspace-switcher-modal-label", children: t("screens.workspaceSwitcher.nameLabel") }), _jsx("input", { ref: inputRef, type: "text", className: "workspace-switcher-modal-input", placeholder: t("screens.workspaceSwitcher.namePlaceholder"), value: name, onChange: (e) => setName(e.target.value), "data-testid": "workspace-switcher-rename-name" })] }), error && (_jsx("p", { className: "workspace-switcher-modal-error", role: "alert", "data-testid": "workspace-switcher-rename-error", children: error })), _jsxs("footer", { className: "workspace-switcher-modal-footer", children: [_jsx("button", { type: "button", className: "workspace-switcher-modal-button", onClick: onClose, disabled: submitting, "data-testid": "workspace-switcher-rename-cancel", children: t("screens.workspaceSwitcher.cancel") }), _jsx("button", { type: "submit", className: "workspace-switcher-modal-button is-primary", disabled: submitting, "data-testid": "workspace-switcher-rename-submit", children: t("screens.workspaceSwitcher.submit") })] })] }) }));
}
function DeleteModal({ target, onClose, onDelete }) {
    const { t } = useTranslation();
    const [error, setError] = useState(null);
    const [submitting, setSubmitting] = useState(false);
    const cancelRef = useRef(null);
    useEffect(() => {
        // 안전 default — 취소 버튼에 focus.
        cancelRef.current?.focus();
    }, []);
    useEffect(() => {
        const onKey = (e) => {
            if (e.key === "Escape")
                onClose();
        };
        window.addEventListener("keydown", onKey);
        return () => window.removeEventListener("keydown", onKey);
    }, [onClose]);
    const handleConfirm = useCallback(async () => {
        setError(null);
        setSubmitting(true);
        try {
            await onDelete(target.id);
            onClose();
        }
        catch (e) {
            setError(extractKoreanError(e, t));
        }
        finally {
            setSubmitting(false);
        }
    }, [onClose, onDelete, t, target.id]);
    return (_jsx("div", { className: "workspace-switcher-modal-backdrop", role: "presentation", onClick: onClose, "data-testid": "workspace-switcher-delete-modal", children: _jsxs("div", { role: "dialog", "aria-modal": "true", "aria-labelledby": "workspace-switcher-delete-title", "aria-describedby": "workspace-switcher-delete-body", className: "workspace-switcher-modal", onClick: (e) => e.stopPropagation(), children: [_jsx("h3", { id: "workspace-switcher-delete-title", className: "workspace-switcher-modal-title", children: t("screens.workspaceSwitcher.deleteConfirmTitle", {
                        name: target.name,
                    }) }), _jsx("p", { id: "workspace-switcher-delete-body", className: "workspace-switcher-modal-body", children: t("screens.workspaceSwitcher.deleteConfirmBody", {
                        name: target.name,
                    }) }), error && (_jsx("p", { className: "workspace-switcher-modal-error", role: "alert", "data-testid": "workspace-switcher-delete-error", children: error })), _jsxs("footer", { className: "workspace-switcher-modal-footer", children: [_jsx("button", { ref: cancelRef, type: "button", className: "workspace-switcher-modal-button", onClick: onClose, disabled: submitting, "data-testid": "workspace-switcher-delete-cancel", children: t("screens.workspaceSwitcher.deleteCancel") }), _jsx("button", { type: "button", className: "workspace-switcher-modal-button is-danger", onClick: handleConfirm, disabled: submitting, "data-testid": "workspace-switcher-delete-confirm", children: t("screens.workspaceSwitcher.deleteConfirm") })] })] }) }));
}
// ── helpers ──────────────────────────────────────────────────────────
/** WorkspacesApiError를 한국어 메시지로 변환. fallback은 i18n key. */
function extractKoreanError(err, t) {
    if (err && typeof err === "object" && "kind" in err) {
        const e = err;
        switch (e.kind) {
            case "duplicate-name":
                return t("screens.workspaceSwitcher.errors.duplicate", { name: e.name });
            case "empty-name":
                return t("screens.workspaceSwitcher.errors.empty");
            case "not-found":
                return t("screens.workspaceSwitcher.errors.notFound");
            case "cannot-delete-only-workspace":
                return t("screens.workspaceSwitcher.errors.cannotDeleteOnly");
            case "persist":
                return t("screens.workspaceSwitcher.errors.persist", {
                    message: e.message,
                });
            case "internal":
                return t("screens.workspaceSwitcher.errors.internal", {
                    message: e.message,
                });
        }
    }
    if (err instanceof Error)
        return err.message;
    return t("screens.workspaceSwitcher.errors.unknown");
}
