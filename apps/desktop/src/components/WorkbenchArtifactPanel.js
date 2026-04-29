import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// WorkbenchArtifactPanel — Phase 8'.0.c (ADR-0037).
//
// 정책:
// - Settings 고급 탭에 들어가는 "워크벤치 임시 파일" 패널.
// - 현재 사용량(개수 + 크기) + retention 정책 노출 + "지금 정리할게요" 버튼.
// - 빈 상태("저장된 결과물 없어요"), 로딩, 에러 모두 한국어 해요체.
// - a11y: fieldset + legend, aria-live="polite"로 결과 알림.
import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { cleanupArtifactsNow, getArtifactStats, } from "../ipc/workbench";
/** byte → 사람이 읽기 좋은 string. KB/MB/GB. */
function formatBytes(bytes) {
    if (!Number.isFinite(bytes) || bytes <= 0)
        return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    let v = bytes;
    let idx = 0;
    while (v >= 1024 && idx < units.length - 1) {
        v /= 1024;
        idx += 1;
    }
    // 정수면 소수점 X, 그 외는 1자리.
    const formatted = idx === 0 ? `${Math.round(v)}` : v.toFixed(1);
    return `${formatted} ${units[idx]}`;
}
export function WorkbenchArtifactPanel() {
    const { t } = useTranslation();
    const [stats, setStats] = useState(null);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState(null);
    const [info, setInfo] = useState(null);
    const refresh = useCallback(async () => {
        try {
            const s = await getArtifactStats();
            setStats(s);
        }
        catch (e) {
            console.warn("getArtifactStats failed:", e);
            setError("screens.settings.workbench.artifacts.loadFailed");
        }
    }, []);
    useEffect(() => {
        let cancelled = false;
        void (async () => {
            if (cancelled)
                return;
            await refresh();
        })();
        return () => {
            cancelled = true;
        };
    }, [refresh]);
    const handleCleanup = useCallback(async () => {
        if (busy)
            return;
        setBusy(true);
        setError(null);
        setInfo(null);
        try {
            const report = await cleanupArtifactsNow();
            setInfo(JSON.stringify({
                key: "screens.settings.workbench.artifacts.cleanupDone",
                opts: {
                    removed: report.removed_count,
                    freed: formatBytes(report.freed_bytes),
                },
            }));
            await refresh();
        }
        catch (e) {
            console.warn("cleanupArtifactsNow failed:", e);
            setError("screens.settings.workbench.artifacts.cleanupFailed");
        }
        finally {
            setBusy(false);
        }
    }, [busy, refresh]);
    const policyHint = useMemo(() => {
        if (!stats)
            return "";
        return t("screens.settings.workbench.artifacts.policyHint", {
            days: stats.policy.max_age_days,
            cap: formatBytes(stats.policy.max_total_size_bytes),
        });
    }, [stats, t]);
    const infoText = useMemo(() => {
        if (!info)
            return null;
        try {
            const parsed = JSON.parse(info);
            return t(parsed.key, parsed.opts);
        }
        catch {
            return info;
        }
    }, [info, t]);
    return (_jsxs("fieldset", { className: "settings-fieldset", "data-testid": "workbench-artifact-panel", children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.workbench.artifacts.title") }), _jsx("p", { className: "settings-hint", children: t("screens.settings.workbench.artifacts.description") }), !stats && !error && (_jsx("p", { className: "settings-hint", "aria-live": "polite", children: t("screens.settings.workbench.artifacts.loading") })), stats && (_jsxs("dl", { className: "settings-build-info", "aria-live": "polite", children: [_jsxs("div", { className: "settings-build-info-row", children: [_jsx("dt", { children: t("screens.settings.workbench.artifacts.runCount", {
                                    count: stats.run_count,
                                }) }), _jsx("dd", { className: "num", children: stats.run_count })] }), _jsxs("div", { className: "settings-build-info-row", children: [_jsx("dt", { children: t("screens.settings.workbench.artifacts.totalBytes", {
                                    size: formatBytes(stats.total_bytes),
                                }) }), _jsx("dd", { className: "num", children: formatBytes(stats.total_bytes) })] })] })), _jsx("p", { className: "settings-hint", children: policyHint }), _jsx("button", { type: "button", className: "settings-btn-primary", onClick: () => void handleCleanup(), disabled: busy, "data-testid": "workbench-artifact-cleanup-btn", children: busy
                    ? t("screens.settings.workbench.artifacts.cleaning")
                    : t("screens.settings.workbench.artifacts.cleanupNow") }), infoText && (_jsx("p", { className: "settings-success", role: "status", "aria-live": "polite", "data-testid": "workbench-artifact-info", children: infoText })), error && (_jsx("p", { className: "settings-error", role: "alert", "data-testid": "workbench-artifact-error", children: t(error) }))] }));
}
