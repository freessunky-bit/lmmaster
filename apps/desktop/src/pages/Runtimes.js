import { jsx as _jsx, jsxs as _jsxs, Fragment as _Fragment } from "react/jsx-runtime";
// Runtimes — 어댑터 상태 + 모델 목록 페이지. Phase 4.c.
//
// 정책 (phase-4-screens-decision.md §1.1 runtimes, phase-4c-runtimes-decision.md):
// - 좌측 어댑터 카드 column (sm 320px) — Ollama / LM Studio.
//   각 카드는 <header(name + StatusPill + version + port)>
//             <body(model_count, last_ping_at)>
//             <footer(disabled stop / restart / 로그 보기)>.
// - 우측 main — 선택된 어댑터의 모델 VirtualList (24px row).
//   컬럼: name (mono, flex) | size (num) | digest 8자 prefix.
//   검색 input + 정렬 select (name / size).
//   빈 상태: "어떤 모델도 로드되지 않았어요" + 카탈로그 CTA.
// - start/stop/restart는 v1 disabled (외부 데몬이라 안전 위험 — §2.a 결정).
// - auto-refresh polling 거부 — 사용자 manual 새로고침으로 충분 (§2.c).
//
// a11y:
// - 어댑터 카드: <article role="region" aria-labelledby>.
// - virtual list row는 VirtualList 컴포넌트가 role="listitem" 처리.
import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { StatusPill, VirtualList } from "@lmmaster/design-system/react";
import "@lmmaster/design-system/react/pill.css";
import "@lmmaster/design-system/react/virtual-list.css";
import { listRuntimeModels, listRuntimeStatuses, } from "../ipc/runtimes";
import "./runtimes.css";
/** 카탈로그 nav로 이동시키는 custom event — App.tsx의 listener가 받는다. */
const NAV_EVENT = "lmmaster:navigate";
const RUNTIME_META = {
    ollama: { kind: "ollama", display_name: "Ollama", port: 11434 },
    "lm-studio": { kind: "lm-studio", display_name: "LM Studio", port: 1234 },
};
export function RuntimesPage() {
    const { t } = useTranslation();
    const [statuses, setStatuses] = useState([]);
    const [statusesLoaded, setStatusesLoaded] = useState(false);
    const [selectedKind, setSelectedKind] = useState(null);
    const [models, setModels] = useState([]);
    const [modelsLoading, setModelsLoading] = useState(false);
    const [modelsErrored, setModelsErrored] = useState(false);
    const [search, setSearch] = useState("");
    const [sort, setSort] = useState("name");
    // 1) 어댑터 합산 status — 한 번만 로드.
    useEffect(() => {
        let cancelled = false;
        listRuntimeStatuses()
            .then((rows) => {
            if (cancelled)
                return;
            setStatuses(rows);
            setStatusesLoaded(true);
            // 첫 카드 자동 선택 — 사용자가 빈 우측 main을 보지 않도록.
            if (rows.length > 0 && selectedKind == null) {
                setSelectedKind(rows[0].kind);
            }
        })
            .catch((e) => {
            if (cancelled)
                return;
            console.warn("listRuntimeStatuses failed:", e);
            setStatusesLoaded(true);
        });
        return () => {
            cancelled = true;
        };
        // selectedKind는 첫 진입 시 한 번만 결정 — 의존성 배열에서 제외 (eslint-disable는 미설정이라 주석으로 표기).
    }, [selectedKind]);
    // 2) 선택된 어댑터의 모델 — 선택 변경 시마다 fetch.
    useEffect(() => {
        if (selectedKind == null) {
            setModels([]);
            return;
        }
        let cancelled = false;
        setModelsLoading(true);
        setModelsErrored(false);
        listRuntimeModels(selectedKind)
            .then((rows) => {
            if (cancelled)
                return;
            setModels(rows);
            setModelsLoading(false);
        })
            .catch((e) => {
            if (cancelled)
                return;
            console.warn("listRuntimeModels failed:", e);
            setModels([]);
            setModelsLoading(false);
            setModelsErrored(true);
        });
        return () => {
            cancelled = true;
        };
    }, [selectedKind]);
    const summary = useMemo(() => {
        const total = statuses.length;
        const running = statuses.filter((s) => s.running).length;
        return { total, running };
    }, [statuses]);
    const visibleModels = useMemo(() => {
        let list = models;
        const q = search.trim().toLowerCase();
        if (q.length > 0) {
            list = list.filter((m) => m.id.toLowerCase().includes(q));
        }
        const copy = [...list];
        if (sort === "size") {
            copy.sort((a, b) => b.size_bytes - a.size_bytes || a.id.localeCompare(b.id, "ko"));
        }
        else {
            copy.sort((a, b) => a.id.localeCompare(b.id, "ko"));
        }
        return copy;
    }, [models, search, sort]);
    const onNavCatalog = useCallback(() => {
        if (typeof window !== "undefined") {
            window.dispatchEvent(new CustomEvent(NAV_EVENT, { detail: "catalog" }));
        }
    }, []);
    const selectedDisplayName = selectedKind
        ? RUNTIME_META[selectedKind]?.display_name ?? selectedKind
        : "";
    return (_jsxs("div", { className: "runtimes-root", children: [_jsxs("header", { className: "runtimes-page-header", children: [_jsxs("div", { className: "runtimes-page-header-text", children: [_jsx("h2", { className: "runtimes-page-title", children: t("screens.runtimes.title") }), _jsx("p", { className: "runtimes-page-subtitle", children: t("screens.runtimes.subtitle") })] }), _jsxs("div", { className: "runtimes-summary", role: "status", "aria-live": "polite", "aria-label": t("screens.runtimes.summary.runningCount", {
                            running: summary.running,
                            total: summary.total,
                        }), children: [_jsx("span", { className: "num", children: summary.running }), _jsx("span", { className: "runtimes-summary-sep", children: "/" }), _jsx("span", { className: "num", children: summary.total }), _jsx("span", { className: "runtimes-summary-label", children: t("screens.runtimes.summary.runningCount", {
                                    running: summary.running,
                                    total: summary.total,
                                })
                                    .split(/\s+/)
                                    .pop() })] })] }), _jsxs("div", { className: "runtimes-shell", children: [_jsx("aside", { className: "runtimes-sidebar", "aria-label": t("screens.runtimes.title"), children: !statusesLoaded ? (_jsx("p", { className: "runtimes-empty", children: "\u2026" })) : statuses.length === 0 ? (_jsx("p", { className: "runtimes-empty", children: "\u2026" })) : (statuses.map((s) => {
                            const meta = RUNTIME_META[s.kind];
                            const isActive = s.kind === selectedKind;
                            const cardId = `runtime-card-${s.kind}`;
                            return (_jsxs("article", { role: "region", "aria-labelledby": `${cardId}-title`, className: `runtimes-card${isActive ? " is-active" : ""}`, onClick: () => setSelectedKind(s.kind), "data-testid": `runtime-card-${s.kind}`, children: [_jsxs("header", { className: "runtimes-card-header", children: [_jsxs("div", { className: "runtimes-card-titlebar", children: [_jsx("h3", { id: `${cardId}-title`, className: "runtimes-card-title", children: meta?.display_name ?? s.kind }), _jsx(StatusPill, { status: statusToPill(s), label: pillLabel(t, s), detail: s.latency_ms != null ? `${s.latency_ms}ms` : null, size: "sm" })] }), _jsxs("div", { className: "runtimes-card-meta", children: [s.installed && s.version && (_jsxs("span", { className: "runtimes-card-version num", children: ["v", s.version] })), meta && (_jsxs("span", { className: "runtimes-card-port num", children: [":", meta.port] }))] })] }), _jsx("div", { className: "runtimes-card-body", children: !s.installed ? (_jsx("p", { className: "runtimes-card-line runtimes-card-line-warn", children: t("screens.runtimes.card.notInstalled") })) : (_jsxs(_Fragment, { children: [_jsx("p", { className: "runtimes-card-line", children: t("screens.runtimes.card.modelCount", {
                                                        count: s.model_count,
                                                    }) }), s.last_ping_at && (_jsx("p", { className: "runtimes-card-line runtimes-card-line-muted", children: t("screens.runtimes.card.lastPing", {
                                                        seconds: secondsAgo(s.last_ping_at),
                                                    }) }))] })) }), _jsxs("footer", { className: "runtimes-card-footer", "aria-label": t("screens.runtimes.card.actions.disabledHint"), children: [_jsx("button", { type: "button", className: "runtimes-card-action", disabled: true, title: t("screens.runtimes.card.actions.disabledHint"), children: t("screens.runtimes.card.actions.stop") }), _jsx("button", { type: "button", className: "runtimes-card-action", disabled: true, title: t("screens.runtimes.card.actions.disabledHint"), children: t("screens.runtimes.card.actions.restart") }), _jsx("button", { type: "button", className: "runtimes-card-action", disabled: true, title: t("screens.runtimes.card.actions.disabledHint"), children: t("screens.runtimes.card.actions.logs") })] })] }, s.kind));
                        })) }), _jsxs("main", { className: "runtimes-main", "aria-label": selectedDisplayName, children: [_jsxs("div", { className: "runtimes-toolbar", role: "toolbar", children: [_jsx("input", { type: "search", className: "runtimes-search", placeholder: t("screens.runtimes.models.search"), "aria-label": t("screens.runtimes.models.search"), value: search, onChange: (e) => setSearch(e.target.value) }), _jsxs("label", { className: "runtimes-sort", children: [_jsx("span", { className: "runtimes-sort-label", children: t("screens.runtimes.models.sort.name") }), _jsxs("select", { value: sort, onChange: (e) => setSort(e.target.value), "aria-label": t("screens.runtimes.models.sort.name"), children: [_jsx("option", { value: "name", children: t("screens.runtimes.models.sort.name") }), _jsx("option", { value: "size", children: t("screens.runtimes.models.sort.size") })] })] })] }), _jsxs("div", { className: "runtimes-table-header", role: "presentation", children: [_jsx("span", { className: "runtimes-col-name", children: t("screens.runtimes.models.column.name") }), _jsx("span", { className: "runtimes-col-size", children: t("screens.runtimes.models.column.size") }), _jsx("span", { className: "runtimes-col-digest", children: t("screens.runtimes.models.column.digest") })] }), modelsLoading ? (_jsx("div", { className: "runtimes-table-loading", children: "\u2026" })) : visibleModels.length === 0 ? (_jsx("div", { className: "runtimes-vlist-empty-wrap", children: _jsx(EmptyState, { title: t("screens.runtimes.models.empty.title"), body: t("screens.runtimes.models.empty.body"), cta: t("screens.runtimes.models.empty.cta"), onCta: onNavCatalog }) })) : (_jsx(VirtualList, { items: visibleModels, rowHeight: 24, keyOf: (m) => `${m.runtime_kind}:${m.id}`, ariaLabel: selectedDisplayName, renderRow: (m) => (_jsxs("div", { className: "runtimes-row", children: [_jsx("span", { className: "runtimes-cell-name mono", children: m.id }), _jsx("span", { className: "runtimes-cell-size num", children: formatSize(m.size_bytes) }), _jsx("span", { className: "runtimes-cell-digest mono", children: (m.digest || "").slice(0, 8) })] })), className: "runtimes-vlist", height: "100%" }))] })] })] }));
}
function EmptyState({ title, body, cta, onCta }) {
    return (_jsxs("div", { className: "runtimes-empty-state", role: "status", "aria-live": "polite", children: [_jsx("p", { className: "runtimes-empty-title", children: title }), _jsx("p", { className: "runtimes-empty-body", children: body }), _jsx("button", { type: "button", className: "runtimes-empty-cta", onClick: onCta, children: cta })] }));
}
function statusToPill(s) {
    if (!s.installed)
        return "idle";
    if (s.running)
        return "listening";
    return "failed";
}
function pillLabel(t, s) {
    if (!s.installed)
        return t("screens.runtimes.card.notInstalled");
    if (s.running)
        return t("gateway.status.listening");
    return t("gateway.status.failed");
}
function formatSize(bytes) {
    if (bytes <= 0)
        return "—";
    const units = ["B", "KB", "MB", "GB", "TB"];
    let n = bytes;
    let i = 0;
    while (n >= 1024 && i < units.length - 1) {
        n /= 1024;
        i += 1;
    }
    // 1 decimal for MB+, 0 for KB/B.
    if (i <= 1)
        return `${Math.round(n)} ${units[i]}`;
    return `${n.toFixed(1)} ${units[i]}`;
}
function secondsAgo(rfc3339) {
    const t = Date.parse(rfc3339);
    if (Number.isNaN(t))
        return 0;
    const diff = Math.max(0, Math.floor((Date.now() - t) / 1000));
    return diff;
}
