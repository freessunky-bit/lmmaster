import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// Projects — Phase 4.d.
//
// 정책 (phase-4-screens-decision.md §1.1 projects):
// - 같은 alias prefix를 가진 키들을 하나의 project 카드로 그룹화.
// - 카드: header(alias + StatusPill is-active|is-dim) + body(origin chips + 허용 모델 패턴 + 마지막 사용 시각) + footer(키 목록 + 사용량 펼치기).
// - 카드 클릭 → 우측 drawer로 24h sparkline mock + per-model top 3 + 키 회수.
// - Phase 6'에 real access log IPC로 교체. v1은 mock data (deterministic seed).
// - keys 화면(ApiKeysPanel)과 데이터 source 동일하지만 navigation 분리.
import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { StatusPill } from "@lmmaster/design-system/react";
import { listApiKeys, revokeApiKey, } from "../ipc/keys";
import "./projects.css";
/** alias의 첫 단어 또는 처음 공백 앞 — "내 블로그 프리뷰" → "내". */
function aliasPrefix(alias) {
    const trimmed = alias.trim();
    if (!trimmed)
        return "(unnamed)";
    // 공백 split 첫 토큰. 한국어 / 영어 모두 대응.
    return trimmed.split(/\s+/, 1)[0] ?? trimmed;
}
/** 키 목록을 alias prefix 기준으로 그룹화. */
function groupKeysIntoProjects(keys) {
    const map = new Map();
    for (const k of keys) {
        const prefix = aliasPrefix(k.alias);
        let g = map.get(prefix);
        if (!g) {
            g = {
                id: prefix,
                displayName: prefix,
                keys: [],
                hasActive: false,
                origins: [],
                modelPatterns: [],
                lastUsedAt: null,
            };
            map.set(prefix, g);
        }
        g.keys.push(k);
        if (!k.revoked_at)
            g.hasActive = true;
        for (const origin of k.scope.allowed_origins) {
            if (!g.origins.includes(origin))
                g.origins.push(origin);
        }
        for (const m of k.scope.models) {
            if (!g.modelPatterns.includes(m))
                g.modelPatterns.push(m);
        }
        if (k.last_used_at) {
            if (!g.lastUsedAt || k.last_used_at > g.lastUsedAt) {
                g.lastUsedAt = k.last_used_at;
            }
        }
    }
    // 정렬: 활성이 있는 그룹 먼저, 그 다음 alias prefix 알파벳.
    return Array.from(map.values()).sort((a, b) => {
        if (a.hasActive !== b.hasActive)
            return a.hasActive ? -1 : 1;
        return a.displayName.localeCompare(b.displayName, "ko");
    });
}
/** mock — 그룹 id로 deterministic seed → 24개 sample. */
function mockSparkline(groupId) {
    // TODO Phase 6': real access log IPC.
    // deterministic seed — 매 렌더링마다 같은 그래프.
    let h = 0;
    for (let i = 0; i < groupId.length; i++) {
        h = (h * 31 + groupId.charCodeAt(i)) >>> 0;
    }
    const out = [];
    for (let i = 0; i < 24; i++) {
        h = (h * 1103515245 + 12345) >>> 0;
        const v = (h % 100) / 100;
        // 시간대 가중 — 9~21시에 활성 더 높음.
        const hourWeight = i >= 9 && i <= 21 ? 1.2 : 0.6;
        out.push(Math.round(v * 80 * hourWeight));
    }
    return out;
}
/** mock — 카드별 deterministic top 3 모델. */
function mockTopModels(groupId, patterns) {
    // TODO Phase 6': real access log IPC.
    let h = 0;
    for (let i = 0; i < groupId.length; i++) {
        h = (h * 31 + groupId.charCodeAt(i)) >>> 0;
    }
    // pattern이 ["*"]면 가상 model 3개 fallback.
    const candidates = patterns.length === 0 || patterns.includes("*")
        ? ["exaone:1.2b", "qwen2.5:7b", "llama-3.2:3b"]
        : patterns.slice(0, 3);
    // % 분배 — deterministic 60/25/15 패턴.
    const pcts = [60, 25, 15];
    return candidates.map((model, i) => ({
        model,
        pct: pcts[i] ?? 0,
    }));
}
export function Projects() {
    const { t } = useTranslation();
    const [keys, setKeys] = useState([]);
    const [hasLoaded, setHasLoaded] = useState(false);
    const [error, setError] = useState(null);
    const [selectedId, setSelectedId] = useState(null);
    const refresh = useCallback(async () => {
        try {
            setError(null);
            const list = await listApiKeys();
            setKeys(list);
        }
        catch (e) {
            console.warn("listApiKeys failed:", e);
            // i18n key를 그대로 저장 — 렌더 시점에 t() 실행해 안정 ref 유지.
            setError("screens.projects.errors.loadFailed");
        }
        finally {
            setHasLoaded(true);
        }
    }, []);
    useEffect(() => {
        refresh();
    }, [refresh]);
    const projects = useMemo(() => groupKeysIntoProjects(keys), [keys]);
    const activeCount = useMemo(() => projects.filter((p) => p.hasActive).length, [projects]);
    const selected = useMemo(() => projects.find((p) => p.id === selectedId) ?? null, [projects, selectedId]);
    const handleRevoke = useCallback(async (id) => {
        // 한국어 confirm은 user-facing이지만 callback ref 안정성을 위해 i18n key direct.
        const ok = window.confirm(t("keys.actions.revokeConfirm"));
        if (!ok)
            return;
        try {
            await revokeApiKey(id);
            await refresh();
        }
        catch (e) {
            console.warn("revokeApiKey failed:", e);
            setError("keys.errors.revokeFailed");
        }
    }, 
    // t는 의도적으로 deps 제외 — useTranslation 객체가 매 렌더 새 ref라 deps에 두면 useEffect storm.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [refresh]);
    return (_jsxs("div", { className: "projects-root", children: [_jsxs("header", { className: "projects-topbar", children: [_jsxs("div", { className: "projects-topbar-titles", children: [_jsx("h2", { className: "projects-page-title", children: t("screens.projects.title") }), _jsx("p", { className: "projects-page-subtitle", children: t("screens.projects.subtitle") })] }), _jsx("span", { className: "projects-summary num", "aria-live": "polite", "data-testid": "projects-active-count", children: t("screens.projects.summary.activeCount", { count: activeCount }) })] }), error && (_jsx("p", { className: "projects-error", role: "alert", children: t(error) })), !hasLoaded ? (_jsx("p", { className: "projects-loading", children: t("screens.projects.loading") })) : projects.length === 0 ? (_jsx(ProjectsEmpty, {})) : (_jsx("ul", { className: "projects-grid", "aria-label": t("screens.projects.title"), children: projects.map((p) => (_jsx(ProjectCard, { project: p, selected: selectedId === p.id, onSelect: () => setSelectedId(p.id) }, p.id))) })), selected && (_jsx(ProjectDetailDrawer, { project: selected, onClose: () => setSelectedId(null), onRevoke: handleRevoke }))] }));
}
function ProjectCard({ project, selected, onSelect }) {
    const { t } = useTranslation();
    const lastUsed = project.lastUsedAt
        ? t("screens.projects.card.lastUsed", {
            when: formatDate(project.lastUsedAt),
        })
        : t("screens.projects.card.lastUsedNever");
    return (_jsxs("li", { className: `projects-card${selected ? " is-selected" : ""}${project.hasActive ? "" : " is-dim"}`, "aria-labelledby": `project-card-title-${project.id}`, children: [_jsxs("div", { className: "projects-card-header", children: [_jsx("h3", { id: `project-card-title-${project.id}`, className: "projects-card-title", children: project.displayName }), _jsx(StatusPill, { status: project.hasActive ? "listening" : "idle", size: "sm", label: project.hasActive
                            ? t("keys.status.active")
                            : t("keys.status.revoked") })] }), _jsxs("div", { className: "projects-card-body", children: [_jsxs("div", { className: "projects-card-origins", children: [project.origins.length === 0 ? (_jsx("span", { className: "projects-card-origin-empty", children: "\u2014" })) : (project.origins.slice(0, 3).map((o) => (_jsx("span", { className: "projects-card-origin-chip", children: o }, o)))), project.origins.length > 3 && (_jsxs("span", { className: "projects-card-origin-more num", children: ["+", project.origins.length - 3] }))] }), _jsxs("dl", { className: "projects-card-meta", children: [_jsxs("div", { className: "projects-card-meta-item", children: [_jsx("dt", { children: t("screens.projects.card.modelsAllowed") }), _jsx("dd", { className: "projects-card-meta-models", children: project.modelPatterns.join(", ") || "*" })] }), _jsxs("div", { className: "projects-card-meta-item", children: [_jsx("dt", { children: t("screens.projects.card.keys") }), _jsx("dd", { className: "num", children: project.keys.length })] }), _jsxs("div", { className: "projects-card-meta-item", children: [_jsx("dt", { children: t("screens.projects.card.lastUsedLabel") }), _jsx("dd", { children: lastUsed })] })] })] }), _jsx("div", { className: "projects-card-footer", children: _jsx("button", { type: "button", className: "projects-card-detail-btn", onClick: onSelect, "aria-expanded": selected, children: t("screens.projects.card.openDetail") }) })] }));
}
function ProjectDetailDrawer({ project, onClose, onRevoke, }) {
    const { t } = useTranslation();
    const sparkline = useMemo(() => mockSparkline(project.id), [project.id]);
    const topModels = useMemo(() => mockTopModels(project.id, project.modelPatterns), [project.id, project.modelPatterns]);
    const totalRequests = useMemo(() => sparkline.reduce((acc, v) => acc + v, 0), [sparkline]);
    // Esc로 닫기.
    useEffect(() => {
        const onKey = (e) => {
            if (e.key === "Escape")
                onClose();
        };
        window.addEventListener("keydown", onKey);
        return () => window.removeEventListener("keydown", onKey);
    }, [onClose]);
    return (_jsx("div", { className: "projects-drawer-backdrop", role: "presentation", onClick: onClose, children: _jsxs("aside", { className: "projects-drawer", role: "dialog", "aria-modal": "true", "aria-labelledby": "projects-drawer-title", onClick: (e) => e.stopPropagation(), children: [_jsxs("header", { className: "projects-drawer-header", children: [_jsx("h3", { id: "projects-drawer-title", className: "projects-drawer-title", children: project.displayName }), _jsx("button", { type: "button", className: "projects-drawer-close", onClick: onClose, "aria-label": t("screens.projects.detail.close"), children: "\u00D7" })] }), _jsxs("div", { className: "projects-drawer-body", children: [_jsxs("section", { className: "projects-drawer-section", "aria-labelledby": "projects-drawer-section-usage", children: [_jsx("h4", { id: "projects-drawer-section-usage", className: "projects-drawer-section-title", children: t("screens.projects.detail.last24h") }), _jsx("p", { className: "projects-drawer-text num", children: t("screens.projects.detail.totalRequests", {
                                        count: totalRequests,
                                    }) }), _jsx(Sparkline, { data: sparkline, ariaLabel: t("screens.projects.detail.sparklineAria", {
                                        count: totalRequests,
                                    }) })] }), _jsxs("section", { className: "projects-drawer-section", "aria-labelledby": "projects-drawer-section-models", children: [_jsx("h4", { id: "projects-drawer-section-models", className: "projects-drawer-section-title", children: t("screens.projects.detail.topModels") }), _jsx("ul", { className: "projects-drawer-models", children: topModels.map((m) => (_jsxs("li", { className: "projects-drawer-model-row", children: [_jsx("span", { className: "projects-drawer-model-name", children: m.model }), _jsx("div", { className: "projects-drawer-model-bar", role: "progressbar", "aria-valuenow": m.pct, "aria-valuemin": 0, "aria-valuemax": 100, children: _jsx("div", { className: "projects-drawer-model-bar-fill", style: { width: `${m.pct}%` } }) }), _jsxs("span", { className: "projects-drawer-model-pct num", children: [m.pct, "%"] })] }, m.model))) })] }), _jsxs("section", { className: "projects-drawer-section", "aria-labelledby": "projects-drawer-section-keys", children: [_jsx("h4", { id: "projects-drawer-section-keys", className: "projects-drawer-section-title", children: t("screens.projects.detail.keysSection") }), _jsx("ul", { className: "projects-drawer-keys", children: project.keys.map((k) => (_jsxs("li", { className: `projects-drawer-key-row${k.revoked_at ? " is-revoked" : ""}`, children: [_jsxs("div", { className: "projects-drawer-key-info", children: [_jsx("span", { className: "projects-drawer-key-alias", children: k.alias }), _jsx("span", { className: "projects-drawer-key-prefix num", children: k.key_prefix })] }), !k.revoked_at && (_jsx("button", { type: "button", className: "projects-drawer-revoke-btn", onClick: () => onRevoke(k.id), children: t("screens.projects.detail.revoke") }))] }, k.id))) })] })] })] }) }));
}
function Sparkline({ data, ariaLabel }) {
    const max = Math.max(1, ...data);
    const width = 240;
    const height = 56;
    const barWidth = width / data.length;
    return (_jsx("svg", { className: "projects-sparkline", viewBox: `0 0 ${width} ${height}`, role: "img", "aria-label": ariaLabel, "data-testid": "projects-sparkline", children: data.map((v, i) => {
            const h = Math.max(1, (v / max) * (height - 4));
            return (_jsx("rect", { x: i * barWidth + 1, y: height - h, width: Math.max(1, barWidth - 2), height: h, rx: 1, fill: "currentColor", opacity: 0.6 + (i / data.length) * 0.4 }, i));
        }) }));
}
function ProjectsEmpty() {
    const { t } = useTranslation();
    return (_jsxs("div", { className: "projects-empty", role: "region", "aria-label": "empty", children: [_jsx("h3", { className: "projects-empty-title", children: t("screens.projects.empty.title") }), _jsx("p", { className: "projects-empty-body", children: t("screens.projects.empty.body") }), _jsx("a", { href: "#keys", className: "projects-empty-cta", children: t("screens.projects.empty.cta") })] }));
}
function formatDate(iso) {
    // ISO 그대로 prefix만 — UI 단계 단순화. v1.x에 한국어 상대시각 (방금 / N분 전).
    return iso.slice(0, 10);
}
