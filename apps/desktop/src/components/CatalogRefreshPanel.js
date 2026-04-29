import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// CatalogRefreshPanel — Phase 1' integration. Settings → 카탈로그 카테고리에 mount.
//
// 정책 (CLAUDE.md §4.1, §4.3):
// - 한국어 해요체. "지금 갱신할게요" / "갱신 중이에요…" / "갱신 완료" 등.
// - <button> + role 명시. focus-visible ring은 토큰.
// - 로컬 상태만 보유 — IPC 호출 결과를 LastRefresh로 표시.
// - 자동 갱신은 백엔드 cron이 처리, 본 컴포넌트는 status + manual trigger만.
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getLastCatalogRefresh, onCatalogRefreshed, refreshCatalogNow, } from "../ipc/catalog-refresh";
export function CatalogRefreshPanel() {
    const { t } = useTranslation();
    const [last, setLast] = useState(null);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState(null);
    // 첫 마운트 — 마지막 결과 로드 + listener 등록.
    useEffect(() => {
        let cancelled = false;
        let unlisten = null;
        (async () => {
            try {
                const cached = await getLastCatalogRefresh();
                if (!cancelled)
                    setLast(cached);
            }
            catch (e) {
                if (!cancelled)
                    console.warn("getLastCatalogRefresh failed:", e);
            }
            try {
                unlisten = await onCatalogRefreshed((p) => {
                    if (!cancelled)
                        setLast(p);
                });
            }
            catch (e) {
                if (!cancelled)
                    console.warn("onCatalogRefreshed listen failed:", e);
            }
        })();
        return () => {
            cancelled = true;
            unlisten?.();
        };
    }, []);
    const handleRefresh = useCallback(async () => {
        setBusy(true);
        setError(null);
        try {
            const r = await refreshCatalogNow();
            setLast(r);
        }
        catch (e) {
            console.warn("refreshCatalogNow failed:", e);
            setError("screens.settings.catalogRefresh.errorRefresh");
        }
        finally {
            setBusy(false);
        }
    }, []);
    return (_jsxs("fieldset", { className: "settings-fieldset", "data-testid": "catalog-refresh-panel", children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.catalogRefresh.title") }), _jsx("p", { className: "settings-hint", children: t("screens.settings.catalogRefresh.intervalHint") }), _jsx("p", { className: "settings-readonly-text", "data-testid": "catalog-refresh-last", children: last
                    ? t("screens.settings.catalogRefresh.lastRefresh", {
                        when: formatTimestamp(last.at_ms),
                        fetched: last.fetched_count,
                        failed: last.failed_count,
                    })
                    : t("screens.settings.catalogRefresh.never") }), last && last.outcome !== "ok" && (_jsx("p", { className: "settings-warning", role: "status", "aria-live": "polite", children: last.outcome === "partial"
                    ? t("screens.settings.catalogRefresh.partial")
                    : t("screens.settings.catalogRefresh.failed") })), _jsx("button", { type: "button", className: "settings-btn-primary", onClick: handleRefresh, disabled: busy, "data-testid": "catalog-refresh-now-btn", children: busy
                    ? t("screens.settings.catalogRefresh.refreshing")
                    : t("screens.settings.catalogRefresh.refreshNow") }), error && (_jsx("p", { className: "settings-error", role: "alert", children: t(error) }))] }));
}
/** UNIX epoch ms → YYYY-MM-DD HH:mm 형식. v1.x에 한국어 상대시각 유지. */
function formatTimestamp(ms) {
    if (!ms)
        return "-";
    const d = new Date(ms);
    const pad = (n) => String(n).padStart(2, "0");
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}
