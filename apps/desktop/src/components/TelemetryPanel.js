import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// TelemetryPanel — Phase 7'.a. Settings의 GeneralPanel에 들어가는 opt-in 토글.
//
// 정책 (ADR-0027 §5, phase-7p-release-prep-reinforcement.md §5.2):
// - 기본 비활성. 사용자가 토글을 켜면 backend가 anonymous UUID 발급.
// - 프롬프트 / 모델 출력은 절대 전송하지 않아요. 익명 사용 통계만.
// - 실제 endpoint(GlitchTip self-hosted) 연결은 Phase 7'.b — 본 v1은 config만.
// - a11y: switch role + aria-checked + aria-label. 한국어 해요체.
// - design-system tokens만 — Settings의 fieldset 패턴 일관.
import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { getTelemetryConfig, setTelemetryEnabled, } from "../ipc/telemetry";
export function TelemetryPanel() {
    const { t } = useTranslation();
    const [config, setConfig] = useState(null);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState(null);
    useEffect(() => {
        let cancelled = false;
        void (async () => {
            try {
                const cfg = await getTelemetryConfig();
                if (!cancelled)
                    setConfig(cfg);
            }
            catch (e) {
                console.warn("getTelemetryConfig failed:", e);
                if (!cancelled)
                    setError("screens.settings.telemetry.errors.loadFailed");
            }
        })();
        return () => {
            cancelled = true;
        };
    }, []);
    const handleToggle = useCallback(async () => {
        if (busy || !config)
            return;
        setBusy(true);
        setError(null);
        const next = !config.enabled;
        // optimistic — 실패 시 revert.
        setConfig({ ...config, enabled: next });
        try {
            const updated = await setTelemetryEnabled(next);
            setConfig(updated);
        }
        catch (e) {
            console.warn("setTelemetryEnabled failed:", e);
            setConfig(config); // revert.
            setError("screens.settings.telemetry.errors.toggleFailed");
        }
        finally {
            setBusy(false);
        }
    }, [busy, config]);
    const errorText = useMemo(() => (error ? t(error) : null), [error, t]);
    const isOn = config?.enabled ?? false;
    return (_jsxs("fieldset", { className: "settings-fieldset", "data-testid": "telemetry-panel", children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.telemetry.title") }), _jsx("p", { className: "settings-hint", children: t("screens.settings.telemetry.description") }), _jsxs("div", { className: "settings-toggle-row", children: [_jsx("button", { type: "button", role: "switch", "aria-checked": isOn, "aria-label": t("screens.settings.telemetry.toggleLabel"), "aria-disabled": busy || !config, disabled: busy || !config, className: `settings-toggle${isOn ? " is-on" : ""}${busy ? " is-busy" : ""}`, onClick: () => void handleToggle(), "data-testid": "telemetry-toggle", children: _jsx("span", { className: "settings-toggle-track", "aria-hidden": true, children: _jsx("span", { className: "settings-toggle-thumb" }) }) }), _jsx("span", { "data-testid": "telemetry-status-label", children: isOn
                            ? t("screens.settings.telemetry.statusOn")
                            : t("screens.settings.telemetry.statusOff") })] }), config?.anon_id && isOn && (_jsxs("p", { className: "settings-readonly-text", "data-testid": "telemetry-anon-id-hint", children: [_jsxs("span", { style: { color: "var(--text-muted)" }, children: [t("screens.settings.telemetry.anonIdLabel"), ":", " "] }), _jsx("code", { className: "num", children: shortenUuid(config.anon_id) })] })), _jsx("p", { className: "settings-hint", children: t("screens.settings.telemetry.privacyNote") }), errorText && (_jsx("p", { className: "settings-error", role: "alert", children: errorText }))] }));
}
/** UUID v4를 첫 8자만 표시 — 사용자가 식별 자체로 사용하지 않도록. */
function shortenUuid(id) {
    if (id.length <= 8)
        return id;
    return `${id.slice(0, 8)}…`;
}
