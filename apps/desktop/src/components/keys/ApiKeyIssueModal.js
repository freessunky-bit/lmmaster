import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// ApiKeyIssueModal — 신규 발급 + 1회 reveal 모달.
//
// 정책 (ADR-0022 §10):
// - alias 필수.
// - allowed_origins 1개 이상.
// - 발급 후 평문 표시 단계: 8초 자동 mask + 5분 후 modal auto-close.
// - 클립보드 카피 버튼.
// - Esc / 배경 클릭으로 닫기 (단 reveal 단계는 명시 close 버튼만).
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { createApiKey, defaultWebScope, } from "../../ipc/keys";
const AUTOMASK_SECONDS = 8;
const AUTOCLOSE_SECONDS = 300;
export function ApiKeyIssueModal({ onClose, onCreated }) {
    const { t } = useTranslation();
    const [alias, setAlias] = useState("");
    const [origins, setOrigins] = useState([""]);
    const [models, setModels] = useState("*");
    const [submitting, setSubmitting] = useState(false);
    const [error, setError] = useState(null);
    const [created, setCreated] = useState(null);
    const aliasRef = useRef(null);
    const closeRef = useRef(null);
    const isReveal = created !== null;
    useEffect(() => {
        if (isReveal) {
            closeRef.current?.focus();
        }
        else {
            aliasRef.current?.focus();
        }
    }, [isReveal]);
    useEffect(() => {
        const onKey = (e) => {
            if (e.key === "Escape" && !isReveal) {
                onClose();
            }
        };
        window.addEventListener("keydown", onKey);
        return () => window.removeEventListener("keydown", onKey);
    }, [isReveal, onClose]);
    const setOrigin = (idx, value) => {
        setOrigins((prev) => prev.map((o, i) => (i === idx ? value : o)));
    };
    const addOrigin = () => setOrigins((prev) => [...prev, ""]);
    const removeOrigin = (idx) => setOrigins((prev) => prev.filter((_, i) => i !== idx));
    const handleSubmit = useCallback(async () => {
        setError(null);
        if (alias.trim().length === 0) {
            setError(t("keys.errors.emptyAlias"));
            return;
        }
        const cleanedOrigins = origins.map((o) => o.trim()).filter((o) => o.length > 0);
        if (cleanedOrigins.length === 0) {
            setError(t("keys.errors.emptyOrigin"));
            return;
        }
        const cleanedModels = models
            .split(",")
            .map((s) => s.trim())
            .filter((s) => s.length > 0);
        const scope = {
            ...defaultWebScope(cleanedOrigins[0]),
            models: cleanedModels.length > 0 ? cleanedModels : ["*"],
            allowed_origins: cleanedOrigins,
        };
        setSubmitting(true);
        try {
            const issued = await createApiKey({ alias: alias.trim(), scope });
            setCreated(issued);
            onCreated(issued);
        }
        catch (e) {
            console.warn("createApiKey failed:", e);
            setError(t("keys.errors.createFailed"));
        }
        finally {
            setSubmitting(false);
        }
    }, [alias, origins, models, t, onCreated]);
    if (isReveal && created) {
        return (_jsx(RevealStep, { plaintext: created.plaintext_once, onClose: onClose, closeRef: closeRef }));
    }
    return (_jsx("div", { className: "keys-modal-backdrop", role: "presentation", onClick: onClose, children: _jsxs("div", { className: "keys-modal", role: "dialog", "aria-modal": "true", "aria-labelledby": "keys-modal-title", onClick: (e) => e.stopPropagation(), children: [_jsx("header", { className: "keys-modal-header", children: _jsx("h3", { id: "keys-modal-title", className: "keys-modal-title", children: t("keys.modal.createTitle") }) }), _jsxs("div", { className: "keys-modal-body", children: [_jsxs("label", { className: "keys-field", children: [_jsx("span", { className: "keys-field-label", children: t("keys.modal.aliasLabel") }), _jsx("input", { ref: aliasRef, type: "text", className: "keys-input", placeholder: t("keys.modal.aliasPlaceholder"), value: alias, onChange: (e) => setAlias(e.target.value) })] }), _jsxs("fieldset", { className: "keys-field", children: [_jsx("legend", { className: "keys-field-label", children: t("keys.modal.originLabel") }), _jsx("p", { className: "keys-field-hint", children: t("keys.modal.originHint") }), _jsxs("div", { className: "keys-origin-list", children: [origins.map((o, i) => (_jsxs("div", { className: "keys-origin-row", children: [_jsx("input", { type: "text", className: "keys-input", placeholder: t("keys.modal.originPlaceholder"), value: o, onChange: (e) => setOrigin(i, e.target.value) }), origins.length > 1 && (_jsx("button", { type: "button", className: "keys-button-secondary", onClick: () => removeOrigin(i), "aria-label": t("keys.modal.removeOrigin"), children: "\u00D7" }))] }, i))), _jsx("button", { type: "button", className: "keys-button-secondary", onClick: addOrigin, children: t("keys.modal.addOrigin") })] })] }), _jsxs("label", { className: "keys-field", children: [_jsx("span", { className: "keys-field-label", children: t("keys.modal.modelsLabel") }), _jsx("p", { className: "keys-field-hint", children: t("keys.modal.modelsHint") }), _jsx("input", { type: "text", className: "keys-input", value: models, onChange: (e) => setModels(e.target.value) })] }), error && (_jsx("p", { className: "keys-error", role: "alert", children: error }))] }), _jsxs("footer", { className: "keys-modal-footer", children: [_jsx("button", { type: "button", className: "keys-button-secondary", onClick: onClose, disabled: submitting, children: t("keys.modal.cancel") }), _jsx("button", { type: "button", className: "keys-button-primary", onClick: handleSubmit, disabled: submitting, children: t("keys.modal.submit") })] })] }) }));
}
function RevealStep({ plaintext, onClose, closeRef }) {
    const { t } = useTranslation();
    const [maskedAt, setMaskedAt] = useState(null);
    const [copied, setCopied] = useState(false);
    const [secondsLeft, setSecondsLeft] = useState(AUTOMASK_SECONDS);
    // 8초 카운트다운 + auto-mask.
    useEffect(() => {
        if (maskedAt !== null)
            return;
        if (secondsLeft <= 0) {
            setMaskedAt(Date.now());
            return;
        }
        const id = window.setTimeout(() => setSecondsLeft(secondsLeft - 1), 1000);
        return () => window.clearTimeout(id);
    }, [secondsLeft, maskedAt]);
    // 5분 auto-close.
    useEffect(() => {
        const id = window.setTimeout(onClose, AUTOCLOSE_SECONDS * 1000);
        return () => window.clearTimeout(id);
    }, [onClose]);
    const handleCopy = useCallback(async () => {
        try {
            await navigator.clipboard.writeText(plaintext);
            setCopied(true);
            window.setTimeout(() => setCopied(false), 2000);
        }
        catch (e) {
            console.warn("clipboard write failed:", e);
        }
    }, [plaintext]);
    const masked = maskedAt !== null;
    const display = masked
        ? plaintext.slice(0, 11) + "·".repeat(Math.max(0, plaintext.length - 11))
        : plaintext;
    return (_jsx("div", { className: "keys-modal-backdrop", role: "presentation", children: _jsxs("div", { className: "keys-modal keys-reveal", role: "dialog", "aria-modal": "true", "aria-labelledby": "keys-reveal-title", children: [_jsx("header", { className: "keys-modal-header", children: _jsx("h3", { id: "keys-reveal-title", className: "keys-modal-title", children: t("keys.modal.revealTitle") }) }), _jsxs("div", { className: "keys-modal-body", children: [_jsx("p", { className: "keys-reveal-body", children: t("keys.modal.revealBody") }), _jsx("div", { className: "keys-reveal-key num", "data-testid": "keys-reveal-key", children: display }), !masked && (_jsx("p", { className: "keys-reveal-countdown", "aria-live": "polite", children: t("keys.modal.revealAutomask", { seconds: secondsLeft }) }))] }), _jsxs("footer", { className: "keys-modal-footer", children: [_jsx("button", { type: "button", className: "keys-button-secondary", onClick: handleCopy, disabled: masked, children: copied ? t("keys.modal.revealCopied") : t("keys.modal.revealCopy") }), _jsx("button", { ref: closeRef, type: "button", className: "keys-button-primary", onClick: onClose, children: t("keys.modal.revealClose") })] })] }) }));
}
