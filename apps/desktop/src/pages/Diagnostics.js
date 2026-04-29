import { jsx as _jsx, jsxs as _jsxs, Fragment as _Fragment } from "react/jsx-runtime";
// Diagnostics — Phase 4.f.
//
// 정책 (phase-4-screens-decision.md §1.1 diagnostics):
// - 4 섹션 grid 2x2 (자가스캔 / 게이트웨이 / 벤치 / 워크스페이스).
// - 종합 health score = 4 섹션 status 합산 (deterministic, server 측 계산 거부).
// - mock 데이터: 게이트웨이 latency / 활성 키 / 최근 요청 / 벤치 막대 / repair history.
//   v1.x에서 실 데이터 IPC 연결 시 // MOCK 마커 영역 교체.
// - a11y: <section aria-labelledby> × 4. 차트는 role="img" + aria-label로 textual 요약.
// - 합산 health는 role="status" aria-live="polite".
import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { StatusPill } from "@lmmaster/design-system/react";
import { getGatewayStatus, } from "../ipc/gateway";
import { getLastScan, startScan, } from "../ipc/scanner";
import { getWorkspaceFingerprint, } from "../ipc/workspace";
import "./diagnostics.css";
/**
 * Phase 4.f 통합 시점에 App.tsx가 listen해서 nav 전환에 사용할 수 있는 custom event 이름.
 * window.dispatchEvent(new CustomEvent("lmmaster:navigate", { detail: "catalog" }))
 */
const NAV_EVENT = "lmmaster:navigate";
// ── MOCK 데이터 (v1.x에서 IPC로 교체) ───────────────────────────────
/** MOCK: 게이트웨이 60s latency sparkline. v1.x: gateway latency IPC. */
const MOCK_GATEWAY_LATENCY_MS = [
    18, 22, 19, 25, 24, 21, 19, 23, 28, 24, 22, 20, 19, 21, 24, 23, 25, 22, 20, 21,
    24, 26, 23, 22, 20, 19, 18, 20, 22, 24,
];
/** MOCK: 활성 키 수. v1.x: listApiKeys() 결과 length. */
const MOCK_ACTIVE_KEY_COUNT = 3;
/** MOCK: 마지막 5 request log. v1.x: 게이트웨이 access log SQLite IPC. */
const MOCK_RECENT_REQUESTS = [
    { ts: "13:42:08", method: "POST", path: "/v1/chat/completions", status: 200, ms: 412 },
    { ts: "13:42:01", method: "GET", path: "/v1/models", status: 200, ms: 14 },
    { ts: "13:41:54", method: "POST", path: "/v1/chat/completions", status: 200, ms: 1102 },
    { ts: "13:41:38", method: "POST", path: "/v1/chat/completions", status: 200, ms: 587 },
    { ts: "13:41:21", method: "GET", path: "/v1/models", status: 200, ms: 11 },
];
/** MOCK: 가장 최근 측정한 모델 5개 token/sec. v1.x: getLastBenchReport batch. */
const MOCK_BENCH_ENTRIES = [
    { modelId: "exaone:1.2b", displayName: "EXAONE 4.0 1.2B", tps: 142.3 },
    { modelId: "qwen2.5:3b", displayName: "Qwen 2.5 3B", tps: 86.5 },
    { modelId: "hyperclova-x-seed:8b", displayName: "HyperCLOVA-X SEED 8B", tps: 38.1 },
    { modelId: "deepseek-coder:6.7b", displayName: "DeepSeek Coder 6.7B", tps: 44.2 },
    { modelId: "qwen2.5-coder:7b", displayName: "Qwen 2.5 Coder 7B", tps: 41.0 },
];
/** MOCK: repair history. v1.x: workspace repair log IPC. */
const MOCK_REPAIR_HISTORY = [
    { date: "2026-04-25", tier: "yellow", invalidatedCaches: 2 },
    { date: "2026-03-12", tier: "yellow", invalidatedCaches: 1 },
];
// ── 헬퍼 ────────────────────────────────────────────────────────────
function severityToTier(severity) {
    if (severity === "error")
        return "red";
    if (severity === "warn")
        return "yellow";
    return "green";
}
function gatewayToTier(gw) {
    if (gw.status === "failed")
        return "red";
    if (gw.status === "stopping" || gw.status === "booting")
        return "yellow";
    return "green";
}
function workspaceToTier(ws) {
    if (!ws)
        return "yellow";
    return ws.tier;
}
function combineTiers(tiers) {
    if (tiers.includes("red"))
        return "red";
    if (tiers.includes("yellow"))
        return "yellow";
    return "green";
}
function checksToWorstTier(checks) {
    if (checks.length === 0)
        return "yellow";
    const tiers = checks.map((c) => severityToTier(c.severity));
    return combineTiers(tiers);
}
function gatewayPillStatus(gw) {
    // listening / booting / failed / stopping은 PillStatus 그대로.
    return gw.status;
}
// ── 메인 ────────────────────────────────────────────────────────────
export function Diagnostics() {
    const { t } = useTranslation();
    const [scan, setScan] = useState(null);
    const [gw, setGw] = useState({ port: null, status: "booting", error: null });
    const [ws, setWs] = useState(null);
    const [scanLoading, setScanLoading] = useState(false);
    // 첫 마운트 — IPC 일괄 fetch.
    useEffect(() => {
        let cancelled = false;
        (async () => {
            try {
                const cached = await getLastScan();
                if (!cancelled)
                    setScan(cached);
            }
            catch (e) {
                console.warn("getLastScan failed:", e);
            }
            try {
                const snap = await getGatewayStatus();
                if (!cancelled)
                    setGw(snap);
            }
            catch (e) {
                console.warn("getGatewayStatus failed:", e);
            }
            try {
                const fp = await getWorkspaceFingerprint();
                if (!cancelled)
                    setWs(fp);
            }
            catch (e) {
                console.warn("getWorkspaceFingerprint failed:", e);
            }
        })();
        return () => {
            cancelled = true;
        };
    }, []);
    const onRescan = useCallback(async () => {
        setScanLoading(true);
        try {
            const fresh = await startScan();
            setScan(fresh);
        }
        catch (e) {
            console.warn("startScan failed:", e);
        }
        finally {
            setScanLoading(false);
        }
    }, []);
    const onStartNewBench = useCallback(() => {
        // App.tsx가 listen해 nav 전환. 통합 전엔 noop.
        if (typeof window !== "undefined") {
            window.dispatchEvent(new CustomEvent(NAV_EVENT, { detail: "catalog" }));
        }
    }, []);
    const scanTier = useMemo(() => {
        if (!scan)
            return "yellow";
        return checksToWorstTier(scan.checks);
    }, [scan]);
    const gatewayTier = useMemo(() => gatewayToTier(gw), [gw]);
    const wsTier = useMemo(() => workspaceToTier(ws), [ws]);
    // bench는 mock — 항상 green (실 데이터 도착 후 v1.x에서 평가).
    const benchTier = "green";
    const overallTier = useMemo(() => combineTiers([scanTier, gatewayTier, wsTier, benchTier]), [scanTier, gatewayTier, wsTier, benchTier]);
    return (_jsxs("div", { className: "diag-root", children: [_jsxs("header", { className: "diag-topbar", children: [_jsxs("div", { className: "diag-topbar-text", children: [_jsx("h2", { className: "diag-page-title", children: t("screens.diagnostics.title") }), _jsx("p", { className: "diag-page-subtitle", children: t("screens.diagnostics.subtitle") })] }), _jsxs("div", { className: `diag-health diag-health-${overallTier}`, role: "status", "aria-live": "polite", "data-testid": "diag-overall-health", children: [_jsx("span", { className: "diag-health-dot", "aria-hidden": true }), _jsx("span", { className: "diag-health-label", children: t(`screens.diagnostics.health.${overallTier}`) })] })] }), _jsxs("div", { className: "diag-grid", children: [_jsx(ScanSection, { scan: scan, loading: scanLoading, tier: scanTier, onRescan: onRescan }), _jsx(GatewaySection, { gw: gw, tier: gatewayTier }), _jsx(BenchSection, { entries: MOCK_BENCH_ENTRIES, onStartNewBench: onStartNewBench }), _jsx(WorkspaceSection, { ws: ws, tier: wsTier, history: MOCK_REPAIR_HISTORY })] })] }));
}
function ScanSection({ scan, loading, tier, onRescan }) {
    const { t } = useTranslation();
    const titleId = "diag-section-scan-title";
    return (_jsxs("section", { className: "diag-section", "aria-labelledby": titleId, "data-testid": "diag-section-scan", "data-tier": tier, children: [_jsxs("header", { className: "diag-section-header", children: [_jsx("h3", { id: titleId, className: "diag-section-title", children: t("screens.diagnostics.sections.scan.title") }), _jsx("button", { type: "button", className: "diag-section-action", onClick: () => void onRescan(), disabled: loading, children: t("screens.diagnostics.sections.scan.rescan") })] }), _jsx("div", { className: "diag-section-body", children: scan ? (_jsxs(_Fragment, { children: [_jsx("p", { className: "diag-scan-summary", children: scan.summary_korean }), _jsx("ul", { className: "diag-scan-checks", role: "list", children: scan.checks.slice(0, 6).map((c) => (_jsxs("li", { className: "diag-scan-check", role: "listitem", children: [_jsx(StatusPill, { size: "sm", status: c.severity === "error" ? "failed" : c.severity === "warn" ? "stopping" : "listening", label: c.title_ko }), _jsx("span", { className: "diag-scan-check-detail", children: c.detail_ko })] }, c.id))) })] })) : (_jsx("p", { className: "diag-empty", children: t("screens.diagnostics.sections.scan.empty") })) })] }));
}
function GatewaySection({ gw, tier }) {
    const { t } = useTranslation();
    const titleId = "diag-section-gateway-title";
    const detail = gw.port != null ? `:${gw.port}` : null;
    return (_jsxs("section", { className: "diag-section", "aria-labelledby": titleId, "data-testid": "diag-section-gateway", "data-tier": tier, children: [_jsxs("header", { className: "diag-section-header", children: [_jsx("h3", { id: titleId, className: "diag-section-title", children: t("screens.diagnostics.sections.gateway.title") }), _jsx(StatusPill, { size: "sm", status: gatewayPillStatus(gw), label: t(`gateway.status.${gw.status}`), detail: detail })] }), _jsxs("div", { className: "diag-section-body", children: [_jsx("div", { className: "diag-gateway-meta", children: _jsx("span", { className: "diag-gateway-keys num", children: t("screens.diagnostics.sections.gateway.activeKeys", {
                                count: MOCK_ACTIVE_KEY_COUNT,
                            }) }) }), _jsx(LatencySparkline, { samples: MOCK_GATEWAY_LATENCY_MS }), _jsx("h4", { className: "diag-section-subtitle", children: t("screens.diagnostics.sections.gateway.recentRequests") }), MOCK_RECENT_REQUESTS.length === 0 ? (_jsx("p", { className: "diag-empty", children: t("screens.diagnostics.sections.gateway.noRequests") })) : (_jsx("ul", { className: "diag-gateway-requests", role: "list", children: MOCK_RECENT_REQUESTS.map((r, idx) => (_jsxs("li", { role: "listitem", className: "diag-gateway-request num", children: [_jsx("span", { className: "diag-req-ts", children: r.ts }), _jsx("span", { className: "diag-req-method", children: r.method }), _jsx("span", { className: "diag-req-path", children: r.path }), _jsx("span", { className: "diag-req-status", children: r.status }), _jsxs("span", { className: "diag-req-ms", children: [r.ms, "ms"] })] }, `${r.ts}-${idx}`))) }))] })] }));
}
function LatencySparkline({ samples }) {
    const { t } = useTranslation();
    const max = Math.max(...samples, 1);
    const min = Math.min(...samples, 0);
    const w = 240;
    const h = 40;
    const stepX = samples.length > 1 ? w / (samples.length - 1) : 0;
    const points = samples
        .map((v, i) => {
        const norm = (v - min) / (max - min || 1);
        const x = i * stepX;
        const y = h - norm * (h - 4) - 2;
        return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
        .join(" ");
    const avg = samples.reduce((a, b) => a + b, 0) / Math.max(samples.length, 1);
    const ariaLabel = `${t("screens.diagnostics.sections.gateway.latency")} — 평균 ${avg.toFixed(0)}ms, 최대 ${max}ms`;
    return (_jsxs("div", { className: "diag-sparkline-wrap", children: [_jsx("span", { className: "diag-sparkline-label", children: t("screens.diagnostics.sections.gateway.latency") }), _jsx("svg", { className: "diag-sparkline-svg", width: w, height: h, viewBox: `0 0 ${w} ${h}`, role: "img", "aria-label": ariaLabel, "data-testid": "diag-gateway-sparkline", children: _jsx("polyline", { fill: "none", stroke: "currentColor", strokeWidth: "1.4", strokeLinecap: "round", strokeLinejoin: "round", points: points }) })] }));
}
function BenchSection({ entries, onStartNewBench }) {
    const { t } = useTranslation();
    const titleId = "diag-section-bench-title";
    const max = Math.max(...entries.map((e) => e.tps), 1);
    const ariaLabel = entries.length === 0
        ? t("screens.diagnostics.sections.bench.empty")
        : entries.map((e) => `${e.displayName} ${e.tps.toFixed(1)} 토큰/초`).join(", ");
    return (_jsxs("section", { className: "diag-section", "aria-labelledby": titleId, "data-testid": "diag-section-bench", children: [_jsxs("header", { className: "diag-section-header", children: [_jsx("h3", { id: titleId, className: "diag-section-title", children: t("screens.diagnostics.sections.bench.title") }), _jsx("button", { type: "button", className: "diag-section-action diag-section-action-primary", onClick: onStartNewBench, "data-testid": "diag-bench-start-new", children: t("screens.diagnostics.sections.bench.startNew") })] }), _jsx("div", { className: "diag-section-body", children: entries.length === 0 ? (_jsx("p", { className: "diag-empty", children: t("screens.diagnostics.sections.bench.empty") })) : (_jsx("div", { className: "diag-bench-bars", role: "img", "aria-label": ariaLabel, "data-testid": "diag-bench-chart", children: entries.map((e) => {
                        const widthPct = Math.max(2, (e.tps / max) * 100);
                        return (_jsxs("div", { className: "diag-bench-bar-row", children: [_jsx("span", { className: "diag-bench-bar-name", children: e.displayName }), _jsx("span", { className: "diag-bench-bar-track", children: _jsx("span", { className: "diag-bench-bar-fill", style: { width: `${widthPct.toFixed(1)}%` } }) }), _jsxs("span", { className: "diag-bench-bar-num num", children: [e.tps.toFixed(1), " tok/s"] })] }, e.modelId));
                    }) })) })] }));
}
function WorkspaceSection({ ws, tier, history }) {
    const { t } = useTranslation();
    const titleId = "diag-section-workspace-title";
    return (_jsxs("section", { className: "diag-section", "aria-labelledby": titleId, "data-testid": "diag-section-workspace", "data-tier": tier, children: [_jsx("header", { className: "diag-section-header", children: _jsx("h3", { id: titleId, className: "diag-section-title", children: t("screens.diagnostics.sections.workspace.title") }) }), _jsxs("div", { className: "diag-section-body", children: [_jsx("h4", { className: "diag-section-subtitle", children: t("screens.diagnostics.sections.workspace.fingerprint") }), ws ? (_jsxs("dl", { className: "diag-ws-fingerprint", children: [_jsxs("div", { className: "diag-ws-row", children: [_jsx("dt", { children: "OS" }), _jsx("dd", { className: "num", children: ws.fingerprint.os })] }), _jsxs("div", { className: "diag-ws-row", children: [_jsx("dt", { children: "arch" }), _jsx("dd", { className: "num", children: ws.fingerprint.arch })] }), _jsxs("div", { className: "diag-ws-row", children: [_jsx("dt", { children: "GPU" }), _jsx("dd", { className: "num", children: ws.fingerprint.gpu_class })] }), _jsxs("div", { className: "diag-ws-row", children: [_jsx("dt", { children: "VRAM" }), _jsxs("dd", { className: "num", children: [ws.fingerprint.vram_bucket_mb, "MB"] })] }), _jsxs("div", { className: "diag-ws-row", children: [_jsx("dt", { children: "RAM" }), _jsxs("dd", { className: "num", children: [ws.fingerprint.ram_bucket_mb, "MB"] })] })] })) : (_jsx("p", { className: "diag-empty", children: t("screens.diagnostics.sections.scan.empty") })), _jsx("h4", { className: "diag-section-subtitle", children: t("screens.diagnostics.sections.workspace.history") }), history.length === 0 ? (_jsx("p", { className: "diag-empty", children: t("screens.diagnostics.sections.workspace.never") })) : (_jsxs("table", { className: "diag-ws-history", "data-testid": "diag-ws-history", children: [_jsx("thead", { children: _jsxs("tr", { children: [_jsx("th", { scope: "col", children: "\uB0A0\uC9DC" }), _jsx("th", { scope: "col", children: "tier" }), _jsx("th", { scope: "col", children: "\uCE90\uC2DC" })] }) }), _jsx("tbody", { children: history.map((h, i) => (_jsxs("tr", { children: [_jsx("td", { className: "num", children: h.date }), _jsx("td", { children: _jsx("span", { className: `diag-tier-chip diag-tier-chip-${h.tier}`, children: h.tier }) }), _jsx("td", { className: "num", children: h.invalidatedCaches })] }, `${h.date}-${i}`))) })] }))] })] }));
}
