import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// PipelinesPanel — 게이트웨이 필터(Pipeline) 토글 + 감사 로그 노출. Phase 6'.c.
//
// 정책 (ADR-0025, phase-6p-updater-pipelines-decision.md §6):
// - 3종 v1 시드 토글 (pii-redact / token-quota / observability) — backend가 화이트리스트 검증.
// - 감사 로그는 backend ring buffer cap 200. 시간 역순(최신부터) 표시.
// - 한국어 해요체. design-system tokens만 사용.
// - a11y: switch role + aria-checked, log은 role="log" aria-live="polite", 키보드 친화.
// - 토글 실패 시 optimistic UI revert + 한국어 에러 메시지. 라벨/설명은 i18n 키 우선.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { clearAuditLog, getAuditLog, listPipelines, setPipelineEnabled, } from "../ipc/pipelines";
import "./pipelinesPanel.css";
const KNOWN_IDS = [
    "pii-redact",
    "token-quota",
    "observability",
];
function isKnownId(id) {
    return KNOWN_IDS.includes(id);
}
/** 알려진 pipeline id에 매핑되는 i18n 서브키 ("piiRedact" 등) — JSON 키 친화 형태. */
function i18nIdKey(id) {
    switch (id) {
        case "pii-redact":
            return "piiRedact";
        case "token-quota":
            return "tokenQuota";
        case "observability":
            return "observability";
    }
}
function classifyAction(action) {
    switch (action) {
        case "passed":
        case "modified":
        case "blocked":
            return action;
        default:
            return "other";
    }
}
/** action별 i18n 키 — fallback 한국어 라벨은 i18n 누락 시에만 노출. */
function actionLabelKey(action) {
    switch (action) {
        case "passed":
            return "screens.settings.pipelines.audit.actionPassed";
        case "modified":
            return "screens.settings.pipelines.audit.actionModified";
        case "blocked":
            return "screens.settings.pipelines.audit.actionBlocked";
        case "other":
            return "screens.settings.pipelines.audit.actionPassed";
    }
}
export function PipelinesPanel() {
    const { t } = useTranslation();
    const [pipelines, setPipelines] = useState([]);
    const [auditEntries, setAuditEntries] = useState([]);
    const [busyId, setBusyId] = useState(null);
    const [refreshing, setRefreshing] = useState(false);
    const [clearing, setClearing] = useState(false);
    const [error, setError] = useState(null);
    /** 마운트 후 첫 로드 완료 여부 — empty 상태 vs loading 구분. */
    const initialLoadDoneRef = useRef(false);
    // ── 첫 로드 ────────────────────────────────────────────────────────
    const refreshPipelines = useCallback(async () => {
        try {
            const list = await listPipelines();
            setPipelines(list);
        }
        catch (e) {
            console.warn("listPipelines failed:", e);
            setError("screens.settings.pipelines.errors.refreshFailed");
        }
    }, []);
    const refreshAudit = useCallback(async () => {
        setRefreshing(true);
        try {
            const entries = await getAuditLog(50);
            setAuditEntries(entries);
        }
        catch (e) {
            console.warn("getAuditLog failed:", e);
            setError("screens.settings.pipelines.errors.refreshFailed");
        }
        finally {
            setRefreshing(false);
        }
    }, []);
    useEffect(() => {
        let cancelled = false;
        void (async () => {
            await Promise.all([refreshPipelines(), refreshAudit()]);
            if (!cancelled) {
                initialLoadDoneRef.current = true;
            }
        })();
        return () => {
            cancelled = true;
        };
    }, [refreshPipelines, refreshAudit]);
    // ── Toggle 핸들러 — optimistic + revert ────────────────────────────
    const handleToggle = useCallback(async (id, currentEnabled) => {
        if (busyId !== null)
            return;
        setBusyId(id);
        setError(null);
        const next = !currentEnabled;
        // optimistic.
        setPipelines((prev) => prev.map((p) => (p.id === id ? { ...p, enabled: next } : p)));
        try {
            await setPipelineEnabled(id, next);
        }
        catch (e) {
            console.warn("setPipelineEnabled failed:", id, e);
            // revert.
            setPipelines((prev) => prev.map((p) => (p.id === id ? { ...p, enabled: currentEnabled } : p)));
            // backend 에러 kind에 따라 메시지 분기.
            if (typeof e === "object" &&
                e !== null &&
                "kind" in e &&
                e.kind === "unknown-pipeline") {
                setError("screens.settings.pipelines.errors.unknownPipeline");
            }
            else {
                setError("screens.settings.pipelines.errors.toggleFailed");
            }
        }
        finally {
            setBusyId(null);
        }
    }, [busyId]);
    const handleClear = useCallback(async () => {
        if (clearing)
            return;
        setClearing(true);
        setError(null);
        try {
            await clearAuditLog();
            setAuditEntries([]);
        }
        catch (e) {
            console.warn("clearAuditLog failed:", e);
            setError("screens.settings.pipelines.errors.clearFailed");
        }
        finally {
            setClearing(false);
        }
    }, [clearing]);
    const handleRefresh = useCallback(() => {
        void refreshAudit();
    }, [refreshAudit]);
    // ── i18n 라벨 helpers ──────────────────────────────────────────────
    const errorText = useMemo(() => (error ? t(error) : null), [error, t]);
    const localizedRows = useMemo(() => pipelines.map((p) => {
        const known = isKnownId(p.id) ? i18nIdKey(p.id) : null;
        const nameKey = known
            ? `screens.settings.pipelines.pipelines.${known}.name`
            : null;
        const descKey = known
            ? `screens.settings.pipelines.pipelines.${known}.desc`
            : null;
        return {
            ...p,
            displayName: nameKey ? t(nameKey) : p.display_name_ko,
            description: descKey ? t(descKey) : p.description_ko,
        };
    }), [pipelines, t]);
    return (_jsxs("fieldset", { className: "settings-fieldset", "data-testid": "pipelines-panel", children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.pipelines.title") }), _jsx("p", { className: "settings-hint pipelines-description", children: t("screens.settings.pipelines.description") }), _jsx("ul", { className: "pipelines-list", "aria-label": t("screens.settings.pipelines.title"), children: localizedRows.map((p) => {
                    const switchLabel = `${t("screens.settings.pipelines.header.enabled")}: ${p.displayName}`;
                    return (_jsxs("li", { className: "pipelines-row", "data-testid": `pipelines-row-${p.id}`, children: [_jsxs("div", { className: "pipelines-row-meta", children: [_jsx("span", { className: "pipelines-row-name", children: p.displayName }), _jsx("span", { className: "pipelines-row-desc", children: p.description })] }), _jsx("button", { type: "button", role: "switch", "aria-checked": p.enabled, "aria-label": switchLabel, disabled: busyId !== null && busyId !== p.id, className: `pipelines-toggle${p.enabled ? " is-on" : ""}${busyId === p.id ? " is-busy" : ""}`, onClick: () => void handleToggle(p.id, p.enabled), "data-testid": `pipelines-toggle-${p.id}`, children: _jsx("span", { className: "pipelines-toggle-track", "aria-hidden": true, children: _jsx("span", { className: "pipelines-toggle-thumb" }) }) })] }, p.id));
                }) }), _jsxs("section", { className: "pipelines-audit", role: "log", "aria-live": "polite", "aria-labelledby": "pipelines-audit-title", children: [_jsxs("header", { className: "pipelines-audit-header", children: [_jsx("h4", { id: "pipelines-audit-title", className: "pipelines-audit-title", children: t("screens.settings.pipelines.audit.title") }), _jsxs("div", { className: "pipelines-audit-actions", children: [_jsx("button", { type: "button", className: "settings-btn-secondary pipelines-btn-compact", onClick: handleRefresh, disabled: refreshing || clearing, "data-testid": "pipelines-audit-refresh", children: t("screens.settings.pipelines.audit.refresh") }), _jsx("button", { type: "button", className: "settings-btn-secondary pipelines-btn-compact", onClick: () => void handleClear(), disabled: clearing || refreshing || auditEntries.length === 0, "data-testid": "pipelines-audit-clear", children: t("screens.settings.pipelines.audit.clear") })] })] }), auditEntries.length === 0 ? (_jsx("p", { className: "pipelines-audit-empty", "data-testid": "pipelines-audit-empty", children: t("screens.settings.pipelines.audit.empty") })) : (_jsx("ol", { className: "pipelines-audit-list", "aria-label": t("screens.settings.pipelines.audit.title"), "data-testid": "pipelines-audit-list", children: auditEntries.map((entry, i) => {
                            const action = classifyAction(entry.action);
                            const known = isKnownId(entry.pipeline_id)
                                ? i18nIdKey(entry.pipeline_id)
                                : null;
                            const nameLabel = known
                                ? t(`screens.settings.pipelines.pipelines.${known}.name`)
                                : entry.pipeline_id;
                            return (_jsxs("li", { className: "pipelines-audit-entry", "data-testid": `pipelines-audit-entry-${i}`, children: [_jsx("span", { className: "pipelines-audit-pipeline", "data-testid": `pipelines-audit-entry-${i}-pipeline`, children: nameLabel }), _jsx("span", { className: `pipelines-audit-action is-${action}`, "data-testid": `pipelines-audit-entry-${i}-action`, children: t(actionLabelKey(action)) }), _jsx("time", { className: "pipelines-audit-timestamp num", dateTime: entry.timestamp_iso, children: `${t("screens.settings.pipelines.audit.timestampPrefix")} ${entry.timestamp_iso}` }), entry.details && (_jsx("span", { className: "pipelines-audit-details", "data-testid": `pipelines-audit-entry-${i}-details`, children: `${t("screens.settings.pipelines.audit.detailsLabel")}: ${truncate(entry.details, 160)}` }))] }, `${entry.timestamp_iso}-${i}`));
                        }) }))] }), errorText && (_jsx("p", { className: "settings-error", role: "alert", children: errorText }))] }));
}
/** 긴 details를 깔끔하게 자릅니다. UTF-16 length 기준 — 한국어/영어 동일 결. */
function truncate(s, max) {
    if (s.length <= max)
        return s;
    return `${s.slice(0, max)}…`;
}
