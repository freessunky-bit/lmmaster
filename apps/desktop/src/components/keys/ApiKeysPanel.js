import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// ApiKeysPanel — Settings 화면의 키 목록 + 발급 + 회수.
//
// 정책 (ADR-0022 §5, §10):
// - prefix만 노출. 평문은 발급 시 1회만 modal에서 노출.
// - revoke는 confirm 후 idempotent.
// - 빈 상태 + 회수된 키는 dim 표시.
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listApiKeys, revokeApiKey, } from "../../ipc/keys";
import { HelpButton } from "../HelpButton";
import { ApiKeyIssueModal } from "./ApiKeyIssueModal";
import "./keys.css";
export function ApiKeysPanel() {
    const { t } = useTranslation();
    const [keys, setKeys] = useState([]);
    const [showModal, setShowModal] = useState(false);
    const [error, setError] = useState(null);
    const refresh = useCallback(async () => {
        try {
            const list = await listApiKeys();
            setKeys(list);
        }
        catch (e) {
            console.warn("listApiKeys failed:", e);
        }
    }, []);
    useEffect(() => {
        refresh();
    }, [refresh]);
    const handleRevoke = useCallback(async (id) => {
        const ok = window.confirm(t("keys.actions.revokeConfirm"));
        if (!ok)
            return;
        try {
            await revokeApiKey(id);
            await refresh();
        }
        catch (e) {
            console.warn("revokeApiKey failed:", e);
            setError(t("keys.errors.revokeFailed"));
        }
    }, [refresh, t]);
    const handleCreated = useCallback((_k) => {
        // refresh는 modal close 후. created 키도 목록에 표시.
        refresh();
    }, [refresh]);
    return (_jsxs("section", { className: "keys-panel", "aria-labelledby": "keys-panel-title", children: [_jsxs("header", { className: "keys-panel-header", children: [_jsxs("div", { children: [_jsxs("div", { className: "keys-title-row", children: [_jsx("h2", { id: "keys-panel-title", className: "keys-panel-title", children: t("keys.title") }), _jsx(HelpButton, { sectionId: "api-keys", hint: t("screens.help.apiKeys") ?? undefined, testId: "keys-help" })] }), _jsx("p", { className: "keys-panel-subtitle", children: t("keys.subtitle") })] }), _jsx("button", { type: "button", className: "keys-button-primary", onClick: () => setShowModal(true), children: t("keys.create") })] }), error && (_jsx("p", { className: "keys-error", role: "alert", children: error })), keys.length === 0 ? (_jsxs("div", { className: "keys-empty", children: [_jsx("h3", { className: "keys-empty-title", children: t("keys.empty.title") }), _jsx("p", { className: "keys-empty-body", children: t("keys.empty.body") })] })) : (_jsxs("table", { className: "keys-table", "data-testid": "keys-table", children: [_jsx("thead", { children: _jsxs("tr", { children: [_jsx("th", { children: t("keys.table.alias") }), _jsx("th", { children: t("keys.table.prefix") }), _jsx("th", { children: t("keys.table.scope") }), _jsx("th", { children: t("keys.table.created") }), _jsx("th", { children: t("keys.table.lastUsed") }), _jsx("th", { children: t("keys.table.status") }), _jsx("th", { "aria-label": "actions" })] }) }), _jsx("tbody", { children: keys.map((k) => (_jsxs("tr", { className: k.revoked_at ? "keys-row is-revoked" : "keys-row", children: [_jsx("td", { children: k.alias }), _jsx("td", { className: "num", children: k.key_prefix }), _jsx("td", { className: "keys-scope-cell", children: k.scope.allowed_origins.join(", ") || "—" }), _jsx("td", { children: formatDate(k.created_at) }), _jsx("td", { children: k.last_used_at ? formatDate(k.last_used_at) : t("keys.neverUsed") }), _jsx("td", { children: _jsx("span", { className: `keys-status keys-status-${k.revoked_at ? "revoked" : "active"}`, children: k.revoked_at
                                            ? t("keys.status.revoked")
                                            : t("keys.status.active") }) }), _jsx("td", { children: !k.revoked_at && (_jsx("button", { type: "button", className: "keys-button-secondary", onClick: () => handleRevoke(k.id), children: t("keys.actions.revoke") })) })] }, k.id))) })] })), showModal && (_jsx(ApiKeyIssueModal, { onClose: () => setShowModal(false), onCreated: handleCreated }))] }));
}
function formatDate(iso) {
    // ISO 그대로 표시 — UI 단계 단순화. v1.x에 한국어 상대시각 (방금 / N분 전).
    return iso.slice(0, 10);
}
