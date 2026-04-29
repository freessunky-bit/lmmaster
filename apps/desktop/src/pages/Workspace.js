import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// Workspace — Phase 4.5'.b. Knowledge tab (ingest + search).
//
// 정책 (phase-5pb-4p5b-ipc-reinforcement.md, CLAUDE.md §4.1, §4.3):
// - 한국어 해요체. 디자인 토큰만.
// - a11y: 탭은 semantic <button>, role=tab + aria-selected. progressbar role/aria-valuenow.
// - Channel<IngestEvent> 기반 진행 상태 — terminal까지 stage 라벨 갱신.
// - workspace 단위 직렬화 — backend가 AlreadyIngesting 거부.
// - Stats display: 문서 N개 / 청크 N개. Knowledge IPC workspaceStats() 호출.
// - Search panel: top-k cosine, 결과 list + 빈 상태.
import { useCallback, useEffect, useMemo, useReducer, useRef, useState, } from "react";
import { useTranslation } from "react-i18next";
import { HelpButton } from "../components/HelpButton";
import { useActiveWorkspaceOptional } from "../contexts/ActiveWorkspaceContext";
import { cancelIngest, isTerminalIngestEvent, searchKnowledge, startIngest, workspaceStats, } from "../ipc/knowledge";
import "./workspace.css";
// ── 상수 ─────────────────────────────────────────────────────────────
const TAB_KEYS = ["knowledge"];
const DEFAULT_K = 5;
const MAX_K = 20;
const INITIAL_INGEST = {
    status: "idle",
    stage: null,
    percent: 0,
    currentPath: null,
    handle: null,
    ingestId: null,
    error: null,
    filesProcessed: 0,
    chunksCreated: 0,
};
function ingestReducer(state, action) {
    switch (action.type) {
        case "START":
            return {
                ...INITIAL_INGEST,
                status: "running",
                handle: action.handle,
                ingestId: action.handle.ingest_id,
                stage: "reading",
            };
        case "START_FAILED":
            return {
                ...INITIAL_INGEST,
                status: "failed",
                error: action.message,
            };
        case "EVENT":
            return applyIngestEvent(state, action.event);
        case "RESET":
            return INITIAL_INGEST;
    }
}
function applyIngestEvent(state, event) {
    switch (event.kind) {
        case "started":
            return {
                ...state,
                status: "running",
                stage: "reading",
                percent: 0,
                ingestId: event.ingest_id,
            };
        case "reading":
            return {
                ...state,
                stage: "reading",
                currentPath: event.current_path,
            };
        case "chunking":
        case "embedding":
        case "writing": {
            const total = event.total > 0 ? event.total : 1;
            const percent = Math.min(100, Math.round((event.processed / total) * 100));
            return {
                ...state,
                stage: event.kind,
                percent,
            };
        }
        case "done":
            return {
                ...state,
                status: "done",
                stage: "done",
                percent: 100,
                handle: null,
                filesProcessed: event.summary.files_processed,
                chunksCreated: event.summary.chunks_created,
            };
        case "failed":
            return {
                ...state,
                status: "failed",
                error: event.error,
                handle: null,
            };
        case "cancelled":
            return {
                ...state,
                status: "cancelled",
                handle: null,
            };
    }
}
export function Workspace({ workspaceId, storePath = "" }) {
    const { t } = useTranslation();
    const [activeTab, setActiveTab] = useState("knowledge");
    // 우선순위: 명시 prop → context.active.id → null(로딩).
    // Provider가 없는 테스트 환경에선 기존 호환을 위해 prop이 없으면 null 처리되어 loading skeleton 노출.
    const ctx = useActiveWorkspaceOptional();
    const effectiveWorkspaceId = workspaceId ?? ctx?.active?.id ?? null;
    return (_jsxs("div", { className: "workspace-root", "data-testid": "workspace-page", children: [_jsx("header", { className: "workspace-topbar", children: _jsxs("div", { children: [_jsxs("div", { className: "workspace-title-row", children: [_jsx("h2", { className: "workspace-page-title", children: t("screens.workspace.title") }), _jsx(HelpButton, { sectionId: "knowledge", hint: t("screens.help.workspace") ?? undefined, testId: "workspace-help" })] }), _jsx("p", { className: "workspace-page-subtitle", children: t("screens.workspace.subtitle") })] }) }), _jsx("div", { role: "tablist", "aria-label": t("screens.workspace.title"), className: "workspace-tabs", children: TAB_KEYS.map((key) => {
                    const selected = activeTab === key;
                    return (_jsx("button", { type: "button", role: "tab", id: `workspace-tab-${key}`, "aria-selected": selected, "aria-controls": `workspace-panel-${key}`, tabIndex: selected ? 0 : -1, "data-testid": `workspace-tab-${key}`, className: "workspace-tab-trigger", onClick: () => setActiveTab(key), children: t(`screens.workspace.tabs.${key}`) }, key));
                }) }), activeTab === "knowledge" && (_jsx("div", { role: "tabpanel", id: "workspace-panel-knowledge", "aria-labelledby": "workspace-tab-knowledge", children: effectiveWorkspaceId ? (_jsx(KnowledgeTab, { workspaceId: effectiveWorkspaceId, storePath: storePath })) : (_jsx("p", { className: "workspace-empty", "data-testid": "workspace-loading", "aria-live": "polite", children: t("screens.workspace.loading") })) }))] }));
}
function KnowledgeTab({ workspaceId, storePath }) {
    const { t } = useTranslation();
    const [stats, setStats] = useState(null);
    const [statsError, setStatsError] = useState(null);
    const [ingest, dispatch] = useReducer(ingestReducer, INITIAL_INGEST);
    const handleRef = useRef(null);
    // ingest config form state.
    const [path, setPath] = useState("");
    const [kind, setKind] = useState("directory");
    // search panel state.
    const [query, setQuery] = useState("");
    const [k, setK] = useState(DEFAULT_K);
    const [hits, setHits] = useState(null);
    const [searching, setSearching] = useState(false);
    const [searchError, setSearchError] = useState(null);
    // handle ref — 언마운트 cleanup용.
    useEffect(() => {
        handleRef.current = ingest.handle;
    }, [ingest.handle]);
    useEffect(() => {
        return () => {
            const h = handleRef.current;
            if (h) {
                void h.cancel().catch(() => {
                    /* idempotent */
                });
            }
        };
    }, []);
    // stats 로드 — workspaceId/storePath 변경 시 + ingest done 시.
    const refreshStats = useCallback(async () => {
        try {
            setStatsError(null);
            const s = await workspaceStats(workspaceId, storePath);
            setStats(s);
        }
        catch (e) {
            setStats(null);
            setStatsError(extractErrorMessage(e, t("screens.workspace.knowledge.errors.statsFailed")));
        }
    }, [workspaceId, storePath, t]);
    useEffect(() => {
        void refreshStats();
    }, [refreshStats]);
    useEffect(() => {
        if (ingest.status === "done") {
            void refreshStats();
        }
    }, [ingest.status, refreshStats]);
    const handleStart = useCallback(async () => {
        if (ingest.status === "running")
            return;
        if (!path.trim()) {
            dispatch({
                type: "START_FAILED",
                message: t("screens.workspace.knowledge.errors.missingPath"),
            });
            return;
        }
        const config = {
            workspace_id: workspaceId,
            path: path.trim(),
            kind,
            store_path: storePath,
        };
        try {
            const handle = await startIngest(config, (ev) => {
                dispatch({ type: "EVENT", event: ev });
                if (isTerminalIngestEvent(ev)) {
                    handleRef.current = null;
                }
            });
            dispatch({ type: "START", handle });
        }
        catch (e) {
            const msg = extractErrorMessage(e, t("screens.workspace.knowledge.errors.startFailed"));
            dispatch({ type: "START_FAILED", message: msg });
        }
    }, [ingest.status, kind, path, storePath, t, workspaceId]);
    const handleCancel = useCallback(async () => {
        if (ingest.handle) {
            try {
                await ingest.handle.cancel();
            }
            catch {
                // idempotent.
            }
        }
        else {
            try {
                await cancelIngest(workspaceId);
            }
            catch {
                // idempotent.
            }
        }
    }, [ingest.handle, workspaceId]);
    const handleSearch = useCallback(async () => {
        const q = query.trim();
        if (!q) {
            setHits([]);
            setSearchError(null);
            return;
        }
        setSearching(true);
        setSearchError(null);
        try {
            const results = await searchKnowledge(workspaceId, q, k, storePath);
            setHits(results);
        }
        catch (e) {
            setHits(null);
            setSearchError(extractErrorMessage(e, t("screens.workspace.knowledge.errors.searchFailed")));
        }
        finally {
            setSearching(false);
        }
    }, [k, query, storePath, t, workspaceId]);
    const stageLabel = useMemo(() => {
        if (ingest.status === "idle")
            return null;
        if (ingest.status === "done")
            return t("screens.workspace.knowledge.stage.done");
        if (ingest.status === "failed")
            return t("screens.workspace.knowledge.stage.failed");
        if (ingest.status === "cancelled")
            return t("screens.workspace.knowledge.stage.cancelled");
        if (ingest.stage)
            return t(`screens.workspace.knowledge.stage.${ingest.stage}`);
        return t("screens.workspace.knowledge.stage.reading");
    }, [ingest.stage, ingest.status, t]);
    const inputsDisabled = ingest.status === "running";
    return (_jsxs("section", { className: "workspace-section", role: "region", "aria-labelledby": "workspace-knowledge-title", children: [_jsxs("header", { className: "workspace-section-header", children: [_jsx("h3", { id: "workspace-knowledge-title", className: "workspace-section-title", children: t("screens.workspace.knowledge.title") }), _jsx("p", { className: "workspace-section-subtitle", children: t("screens.workspace.knowledge.subtitle") })] }), statsError && (_jsx("p", { className: "workspace-error", role: "alert", "data-testid": "workspace-stats-error", children: statsError })), _jsxs("dl", { className: "workspace-stats num", "aria-label": t("screens.workspace.knowledge.statsAria"), "data-testid": "workspace-stats", children: [_jsxs("div", { className: "workspace-stat-item", children: [_jsx("dt", { className: "workspace-stat-label", children: t("screens.workspace.knowledge.stats.documents") }), _jsx("dd", { className: "workspace-stat-value", "data-testid": "workspace-stat-documents", children: stats ? stats.documents : 0 })] }), _jsxs("div", { className: "workspace-stat-item", children: [_jsx("dt", { className: "workspace-stat-label", children: t("screens.workspace.knowledge.stats.chunks") }), _jsx("dd", { className: "workspace-stat-value", "data-testid": "workspace-stat-chunks", children: stats ? stats.chunks : 0 })] })] }), _jsxs("section", { className: "workspace-panel", role: "region", "aria-labelledby": "workspace-ingest-title", "data-testid": "workspace-ingest-panel", children: [_jsxs("header", { className: "workspace-section-header", children: [_jsx("h4", { id: "workspace-ingest-title", className: "workspace-section-title", children: t("screens.workspace.knowledge.ingest.title") }), _jsx("p", { className: "workspace-section-subtitle", children: t("screens.workspace.knowledge.ingest.subtitle") })] }), _jsxs("div", { className: "workspace-form", children: [_jsxs("label", { className: "workspace-field", children: [_jsx("span", { className: "workspace-field-label", children: t("screens.workspace.knowledge.ingest.pathLabel") }), _jsx("input", { type: "text", value: path, onChange: (e) => setPath(e.target.value), disabled: inputsDisabled, className: "workspace-input", placeholder: t("screens.workspace.knowledge.ingest.pathPlaceholder"), "data-testid": "workspace-ingest-path" }), _jsx("span", { className: "workspace-field-hint", children: t("screens.workspace.knowledge.ingest.pathHint") })] }), _jsxs("fieldset", { className: "workspace-radiogroup", role: "radiogroup", "aria-labelledby": "workspace-ingest-kind-label", children: [_jsx("legend", { id: "workspace-ingest-kind-label", children: t("screens.workspace.knowledge.ingest.kindLabel") }), ["file", "directory"].map((opt) => {
                                        const checked = kind === opt;
                                        return (_jsx("button", { type: "button", role: "radio", "aria-checked": checked, className: `workspace-radio${checked ? " is-checked" : ""}`, onClick: () => setKind(opt), disabled: inputsDisabled, "data-testid": `workspace-ingest-kind-${opt}`, children: t(`screens.workspace.knowledge.ingest.kind.${opt}`) }, opt));
                                    })] })] }), ingest.status !== "idle" && (_jsxs("div", { className: "workspace-progress", "data-testid": "workspace-ingest-progress", children: [_jsxs("div", { className: "workspace-progress-meta num", children: [_jsx("span", { children: _jsx("span", { className: "workspace-stage-badge", "data-testid": "workspace-stage-badge", children: stageLabel }) }), _jsxs("span", { children: [ingest.percent, "%"] })] }), _jsx("div", { role: "progressbar", "aria-label": t("screens.workspace.knowledge.ingest.progressAria"), "aria-valuenow": ingest.percent, "aria-valuemin": 0, "aria-valuemax": 100, className: "workspace-progress-bar", children: _jsx("div", { className: "workspace-progress-fill", style: { width: `${ingest.percent}%` } }) }), ingest.currentPath && (_jsx("p", { className: "workspace-progress-message", "aria-live": "polite", children: t("screens.workspace.knowledge.ingest.currentPath", {
                                    path: ingest.currentPath,
                                }) })), ingest.status === "done" && (_jsx("p", { className: "workspace-progress-message", "aria-live": "polite", "data-testid": "workspace-ingest-summary", children: t("screens.workspace.knowledge.ingest.summary", {
                                    files: ingest.filesProcessed,
                                    chunks: ingest.chunksCreated,
                                }) }))] })), ingest.error && (_jsx("p", { className: "workspace-error", role: "alert", "data-testid": "workspace-ingest-error", children: ingest.error })), _jsxs("div", { className: "workspace-actions", children: [ingest.status !== "running" && (_jsx("button", { type: "button", className: "workspace-button workspace-button-primary", onClick: handleStart, "data-testid": "workspace-ingest-start", disabled: !path.trim(), children: t("screens.workspace.knowledge.ingest.start") })), ingest.status === "running" && (_jsx("button", { type: "button", className: "workspace-button workspace-button-secondary", onClick: handleCancel, "data-testid": "workspace-ingest-cancel", children: t("screens.workspace.knowledge.ingest.cancel") })), (ingest.status === "done" ||
                                ingest.status === "failed" ||
                                ingest.status === "cancelled") && (_jsx("button", { type: "button", className: "workspace-button workspace-button-secondary", onClick: () => dispatch({ type: "RESET" }), "data-testid": "workspace-ingest-reset", children: t("screens.workspace.knowledge.ingest.reset") }))] })] }), _jsxs("section", { className: "workspace-panel", role: "region", "aria-labelledby": "workspace-search-title", "data-testid": "workspace-search-panel", children: [_jsxs("header", { className: "workspace-section-header", children: [_jsx("h4", { id: "workspace-search-title", className: "workspace-section-title", children: t("screens.workspace.knowledge.search.title") }), _jsx("p", { className: "workspace-section-subtitle", children: t("screens.workspace.knowledge.search.subtitle") })] }), _jsxs("div", { className: "workspace-form", children: [_jsxs("label", { className: "workspace-field", children: [_jsx("span", { className: "workspace-field-label", children: t("screens.workspace.knowledge.search.queryLabel") }), _jsx("input", { type: "text", value: query, onChange: (e) => setQuery(e.target.value), className: "workspace-input", placeholder: t("screens.workspace.knowledge.search.queryPlaceholder"), "data-testid": "workspace-search-query" })] }), _jsxs("label", { className: "workspace-field", children: [_jsx("span", { className: "workspace-field-label", children: t("screens.workspace.knowledge.search.kLabel") }), _jsx("input", { type: "number", min: 1, max: MAX_K, value: k, onChange: (e) => setK(Math.min(MAX_K, Math.max(1, Number(e.target.value) || DEFAULT_K))), className: "workspace-input num", "data-testid": "workspace-search-k" }), _jsx("span", { className: "workspace-field-hint", children: t("screens.workspace.knowledge.search.kHint", { max: MAX_K }) })] })] }), _jsx("div", { className: "workspace-actions", children: _jsx("button", { type: "button", className: "workspace-button workspace-button-primary", onClick: handleSearch, disabled: searching || !query.trim(), "data-testid": "workspace-search-submit", children: t("screens.workspace.knowledge.search.submit") }) }), searchError && (_jsx("p", { className: "workspace-error", role: "alert", "data-testid": "workspace-search-error", children: searchError })), hits !== null && hits.length === 0 && (_jsx("p", { className: "workspace-empty", "data-testid": "workspace-search-empty", children: t("screens.workspace.knowledge.search.empty") })), hits !== null && hits.length > 0 && (_jsx("ul", { className: "workspace-results", "aria-label": t("screens.workspace.knowledge.search.title"), "data-testid": "workspace-search-results", children: hits.map((h) => (_jsxs("li", { className: "workspace-result-item", "data-testid": "workspace-search-hit", children: [_jsxs("div", { className: "workspace-result-meta", children: [_jsx("span", { className: "workspace-result-path", children: h.document_path }), _jsxs("span", { className: "workspace-result-score num", children: [Math.round(h.score * 100), "%"] })] }), _jsx("p", { className: "workspace-result-content", children: h.content })] }, h.chunk_id))) }))] })] }));
}
// ── helpers ─────────────────────────────────────────────────────────
function extractErrorMessage(err, fallback) {
    if (err && typeof err === "object") {
        const e = err;
        if (typeof e.message === "string" && e.message.length > 0)
            return e.message;
        if (typeof e.kind === "string")
            return `${e.kind}`;
    }
    if (err instanceof Error)
        return err.message || fallback;
    if (typeof err === "string")
        return err;
    return fallback;
}
export default Workspace;
