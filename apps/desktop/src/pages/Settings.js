import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// Settings — Phase 4.g + Phase 6'.b 자동 갱신 섹션.
//
// 정책 (phase-4-screens-decision.md §1.1 settings, ADR-0026 §2):
// - 좌측 카테고리 nav (sm 240px) — 일반 / 워크스페이스 / 카탈로그 / 고급 4 카테고리.
// - 우측 form 패널 — 선택된 카테고리.
// - 일반: 언어, 테마(dark-only 자물쇠), 자가스캔 주기, 음성 입출력(disabled), 자동 갱신 (Phase 6'.b).
// - 워크스페이스: 경로, "다른 폴더로 옮기기"(disabled v1.1), "워크스페이스 정리".
// - 카탈로그: registry URL(read-only), 마지막 갱신, "지금 갱신"(disabled placeholder).
// - 고급: Gemini opt-in(disabled), SQLCipher env hint, 진단 로그 export(disabled), 빌드 정보.
// - form은 fieldset + legend. 토글은 role="switch" + aria-checked.
// - 자동 갱신: interval 1h~24h 강제 (ADR-0026 §2). 단발 "지금 확인" + Outdated 시 ToastUpdate 인라인.
import { useCallback, useEffect, useMemo, useState, } from "react";
import { useTranslation } from "react-i18next";
import { checkWorkspaceRepair, getWorkspaceFingerprint, } from "../ipc/workspace";
import { getEncryptDbHint, getNotifyOnPhase5, getScanInterval, setNotifyOnPhase5 as writeNotifyOnPhase5, setScanInterval as writeScanInterval, } from "../ipc/settings";
import { checkForUpdate, getAutoUpdateStatus, startAutoUpdatePoller, stopAutoUpdatePoller, } from "../ipc/updater";
import { ToastUpdate } from "../components/ToastUpdate";
import { PipelinesPanel } from "../components/PipelinesPanel";
import { TelemetryPanel } from "../components/TelemetryPanel";
import { CatalogRefreshPanel } from "../components/CatalogRefreshPanel";
import { WorkbenchArtifactPanel } from "../components/WorkbenchArtifactPanel";
import { HelpButton } from "../components/HelpButton";
import { PortableExportPanel } from "../components/portable/PortableExportPanel";
import { PortableImportPanel } from "../components/portable/PortableImportPanel";
import "./settings.css";
const CATEGORIES = [
    "general",
    "workspace",
    "portable",
    "catalog",
    "advanced",
];
/** registry URL은 외부 통신 0 정책상 표시만. */
const REGISTRY_URL = "lmmaster://registry/local";
/** 빌드 시점 mock — 실제는 Tauri info()에서 가져옴. v1.1에 wire-up. */
const APP_VERSION = "0.1.0";
const BUILD_COMMIT = "dev";
/** 자동 갱신 — GitHub repo (외부 통신 0 정책 예외, ADR-0026 §1). */
const UPDATE_REPO = "anthropics/lmmaster";
/** ADR-0026 §2: 1h~24h 허용 범위. UI는 4개 단계로 압축. */
const INTERVAL_OPTIONS = [
    { secs: 3600, key: "1h" },
    { secs: 6 * 3600, key: "6h" },
    { secs: 12 * 3600, key: "12h" },
    { secs: 24 * 3600, key: "24h" },
];
const DEFAULT_INTERVAL_SECS = 6 * 3600;
export function Settings() {
    const { t, i18n } = useTranslation();
    const [active, setActive] = useState("general");
    const [lastChecked] = useState(() => formatDate(new Date()));
    return (_jsxs("div", { className: "settings-root", children: [_jsx("header", { className: "settings-topbar", children: _jsxs("div", { className: "settings-topbar-titles", children: [_jsx("h2", { className: "settings-page-title", children: t("screens.settings.title") }), _jsxs("p", { className: "settings-page-subtitle", children: [_jsx("span", { className: "settings-version-label", children: t("screens.settings.versionLabel") }), _jsx("span", { className: "settings-version-num num", children: APP_VERSION }), _jsx("span", { className: "settings-version-sep", "aria-hidden": true, children: "\u00B7" }), _jsx("span", { children: t("screens.settings.lastChecked", { when: lastChecked }) })] })] }) }), _jsxs("div", { className: "settings-shell", children: [_jsxs("aside", { className: "settings-sidebar", "aria-labelledby": "settings-sidebar-heading", children: [_jsx("h3", { id: "settings-sidebar-heading", className: "settings-sidebar-heading", children: t("screens.settings.title") }), _jsx("div", { className: "settings-categories", role: "radiogroup", "aria-label": t("screens.settings.title"), children: CATEGORIES.map((key) => (_jsx("button", { type: "button", role: "radio", "aria-checked": active === key, className: `settings-category${active === key ? " is-active" : ""}`, onClick: () => setActive(key), "data-testid": `settings-category-${key}`, children: t(`screens.settings.categories.${key}`) }, key))) })] }), _jsxs("main", { className: "settings-main", children: [active === "general" && (_jsx(GeneralPanel, { currentLang: i18n.resolvedLanguage ?? "ko", onChangeLanguage: (lng) => i18n.changeLanguage(lng) })), active === "workspace" && _jsx(WorkspacePanel, {}), active === "portable" && _jsx(PortablePanel, {}), active === "catalog" && _jsx(CatalogPanel, {}), active === "advanced" && (_jsx(AdvancedPanel, { version: APP_VERSION, commit: BUILD_COMMIT }))] })] })] }));
}
function GeneralPanel({ currentLang, onChangeLanguage }) {
    const { t } = useTranslation();
    const [scanMin, setScanMin] = useState(60);
    const [notifyOn, setNotifyOn] = useState(false);
    // 첫 마운트 시 localStorage 로드.
    useEffect(() => {
        setScanMin(getScanInterval());
        setNotifyOn(getNotifyOnPhase5());
    }, []);
    const handleLangChange = useCallback((lng) => {
        onChangeLanguage(lng);
    }, [onChangeLanguage]);
    const handleScanChange = useCallback((next) => {
        setScanMin(next);
        writeScanInterval(next);
    }, []);
    return (_jsxs("form", { className: "settings-form", onSubmit: (e) => e.preventDefault(), "aria-labelledby": "settings-form-general", children: [_jsx("h3", { id: "settings-form-general", className: "visually-hidden", children: t("screens.settings.categories.general") }), _jsxs("fieldset", { className: "settings-fieldset", children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.general.language") }), _jsxs("div", { className: "settings-radio-row", role: "radiogroup", children: [_jsx(SettingRadio, { name: "language", value: "ko", checked: currentLang === "ko", onChange: () => handleLangChange("ko"), label: t("screens.settings.general.language.ko") }), _jsx(SettingRadio, { name: "language", value: "en", checked: currentLang === "en", onChange: () => handleLangChange("en"), label: t("screens.settings.general.language.en") })] })] }), _jsxs("fieldset", { className: "settings-fieldset", children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.general.theme") }), _jsxs("div", { className: "settings-radio-row", role: "radiogroup", children: [_jsx(SettingRadio, { name: "theme", value: "dark", checked: true, label: t("screens.settings.general.theme.dark") }), _jsx(SettingRadio, { name: "theme", value: "light", checked: false, disabled: true, label: t("screens.settings.general.theme.light"), iconRight: _jsx(LockIcon, {}) })] })] }), _jsxs("fieldset", { className: "settings-fieldset", children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.general.scanInterval") }), _jsxs("div", { className: "settings-radio-row", role: "radiogroup", children: [_jsx(SettingRadio, { name: "scan_interval", value: "0", checked: scanMin === 0, onChange: () => handleScanChange(0), label: t("screens.settings.general.scanInterval.off") }), _jsx(SettingRadio, { name: "scan_interval", value: "15", checked: scanMin === 15, onChange: () => handleScanChange(15), label: t("screens.settings.general.scanInterval.15m") }), _jsx(SettingRadio, { name: "scan_interval", value: "60", checked: scanMin === 60, onChange: () => handleScanChange(60), label: t("screens.settings.general.scanInterval.60m") })] })] }), _jsxs("fieldset", { className: "settings-fieldset", children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.general.voice") }), _jsxs("div", { className: "settings-toggle-row", children: [_jsx(ToggleSwitch, { checked: notifyOn, onChange: () => {
                                    const next = !notifyOn;
                                    setNotifyOn(next);
                                    writeNotifyOnPhase5(next);
                                }, disabled: true, ariaLabel: t("screens.settings.general.voice") }), _jsx("span", { className: "settings-coming-soon", children: t("screens.settings.general.voice.comingSoon") })] })] }), _jsx(AutoUpdatePanel, {}), _jsx(PipelinesPanel, {}), _jsx(TelemetryPanel, {})] }));
}
function AutoUpdatePanel() {
    const { t } = useTranslation();
    const [status, setStatus] = useState(null);
    const [intervalSecs, setIntervalSecs] = useState(DEFAULT_INTERVAL_SECS);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState(null);
    const [info, setInfo] = useState(null);
    const [outdated, setOutdated] = useState(null);
    // 첫 마운트 시 상태 로드.
    useEffect(() => {
        let cancelled = false;
        getAutoUpdateStatus()
            .then((s) => {
            if (cancelled)
                return;
            setStatus(s);
            if (s.interval_secs && s.interval_secs > 0) {
                setIntervalSecs(s.interval_secs);
            }
        })
            .catch((e) => {
            console.warn("getAutoUpdateStatus failed:", e);
            if (!cancelled)
                setError("screens.settings.autoUpdate.errorStatus");
        });
        return () => {
            cancelled = true;
        };
    }, []);
    const handleEnable = useCallback(async (nextSecs) => {
        setBusy(true);
        setError(null);
        setInfo(null);
        try {
            await startAutoUpdatePoller(UPDATE_REPO, APP_VERSION, nextSecs, (ev) => {
                if (ev.kind === "outdated") {
                    setOutdated({
                        release: ev.latest,
                        currentVersion: ev.current_version,
                    });
                }
            });
            const s = await getAutoUpdateStatus();
            setStatus(s);
            setIntervalSecs(nextSecs);
        }
        catch (e) {
            console.warn("startAutoUpdatePoller failed:", e);
            setError("screens.settings.autoUpdate.errorStart");
        }
        finally {
            setBusy(false);
        }
    }, []);
    const handleDisable = useCallback(async () => {
        setBusy(true);
        setError(null);
        setInfo(null);
        try {
            await stopAutoUpdatePoller();
            const s = await getAutoUpdateStatus();
            setStatus(s);
        }
        catch (e) {
            console.warn("stopAutoUpdatePoller failed:", e);
            setError("screens.settings.autoUpdate.errorStop");
        }
        finally {
            setBusy(false);
        }
    }, []);
    const handleToggle = useCallback(() => {
        const isActive = status?.active ?? false;
        if (isActive) {
            void handleDisable();
        }
        else {
            void handleEnable(intervalSecs);
        }
    }, [status?.active, intervalSecs, handleEnable, handleDisable]);
    const handleIntervalChange = useCallback((next) => {
        setIntervalSecs(next);
        // 활성 상태에서 interval 변경 시 재시작.
        if (status?.active) {
            void (async () => {
                setBusy(true);
                try {
                    await stopAutoUpdatePoller();
                    await startAutoUpdatePoller(UPDATE_REPO, APP_VERSION, next, (ev) => {
                        if (ev.kind === "outdated") {
                            setOutdated({
                                release: ev.latest,
                                currentVersion: ev.current_version,
                            });
                        }
                    });
                    const s = await getAutoUpdateStatus();
                    setStatus(s);
                }
                catch (e) {
                    console.warn("interval change failed:", e);
                    setError("screens.settings.autoUpdate.errorStart");
                }
                finally {
                    setBusy(false);
                }
            })();
        }
    }, [status?.active]);
    const handleCheckNow = useCallback(async () => {
        setBusy(true);
        setError(null);
        setInfo(null);
        try {
            await checkForUpdate(UPDATE_REPO, APP_VERSION, (ev) => {
                if (ev.kind === "outdated") {
                    setOutdated({
                        release: ev.latest,
                        currentVersion: ev.current_version,
                    });
                }
                else if (ev.kind === "up-to-date") {
                    setInfo("screens.settings.autoUpdate.upToDate");
                }
                else if (ev.kind === "failed") {
                    setError(`screens.settings.autoUpdate.errorCheck::${ev.error}`);
                }
            });
        }
        catch (e) {
            console.warn("checkForUpdate failed:", e);
            setError("screens.settings.autoUpdate.errorCheck");
        }
        finally {
            setBusy(false);
        }
    }, []);
    const isActive = status?.active ?? false;
    const lastChecked = status?.last_check_iso ?? null;
    const errorText = useMemo(() => {
        if (!error)
            return null;
        const idx = error.indexOf("::");
        if (idx > 0) {
            const key = error.slice(0, idx);
            const detail = error.slice(idx + 2);
            return `${t(key)} (${detail})`;
        }
        return t(error);
    }, [error, t]);
    return (_jsxs("fieldset", { className: "settings-fieldset", children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.autoUpdate.title") }), _jsxs("div", { className: "settings-toggle-row", children: [_jsx(ToggleSwitch, { checked: isActive, onChange: handleToggle, disabled: busy, ariaLabel: t("screens.settings.autoUpdate.toggleLabel") }), _jsx("span", { children: isActive
                            ? t("screens.settings.autoUpdate.toggleOn")
                            : t("screens.settings.autoUpdate.toggleOff") })] }), _jsxs("fieldset", { className: "settings-fieldset", style: { borderStyle: "dashed" }, disabled: !isActive, children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.autoUpdate.intervalLabel") }), _jsx("div", { className: "settings-radio-row", role: "radiogroup", children: INTERVAL_OPTIONS.map((opt) => (_jsx(SettingRadio, { name: "auto_update_interval", value: String(opt.secs), checked: intervalSecs === opt.secs, onChange: isActive ? () => handleIntervalChange(opt.secs) : undefined, disabled: !isActive || busy, label: t(`screens.settings.autoUpdate.interval.${opt.key}`) }, opt.key))) })] }), _jsxs("p", { className: "settings-readonly-text", children: [_jsxs("span", { style: { color: "var(--text-muted)" }, children: [t("screens.settings.autoUpdate.repoLabel"), ":", " "] }), _jsx("code", { className: "num", children: UPDATE_REPO })] }), _jsx("p", { className: "settings-hint", children: lastChecked
                    ? t("screens.settings.autoUpdate.lastChecked", { when: lastChecked })
                    : t("screens.settings.autoUpdate.neverChecked") }), _jsx("button", { type: "button", className: "settings-btn-primary", onClick: handleCheckNow, disabled: busy, "data-testid": "settings-autoupdate-check-now", children: busy
                    ? t("screens.settings.autoUpdate.checking")
                    : t("screens.settings.autoUpdate.checkNow") }), info && (_jsx("p", { className: "settings-success", role: "status", "aria-live": "polite", children: t(info) })), errorText && (_jsx("p", { className: "settings-error", role: "alert", children: errorText })), outdated && (_jsx(ToastUpdate, { release: outdated.release, currentVersion: outdated.currentVersion, onSkip: () => setOutdated(null), onDismiss: () => setOutdated(null) }))] }));
}
// ── 워크스페이스 ─────────────────────────────────────────────────────
function WorkspacePanel() {
    const { t } = useTranslation();
    const [status, setStatus] = useState(null);
    const [repairResult, setRepairResult] = useState(null);
    const [repairing, setRepairing] = useState(false);
    const [error, setError] = useState(null);
    useEffect(() => {
        let cancelled = false;
        getWorkspaceFingerprint()
            .then((s) => {
            if (!cancelled)
                setStatus(s);
        })
            .catch((e) => {
            console.warn("getWorkspaceFingerprint failed:", e);
            if (!cancelled)
                setError("screens.settings.workspace.errorLoad");
        });
        return () => {
            cancelled = true;
        };
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);
    const handleRepair = useCallback(async () => {
        setRepairing(true);
        setError(null);
        setRepairResult(null);
        try {
            const r = await checkWorkspaceRepair();
            const tier = r.tier;
            // i18n key + opts JSON 패턴 — 렌더 시점에 t()로 변환.
            setRepairResult(JSON.stringify({
                key: "screens.settings.workspace.repairDone",
                opts: { tier, caches: r.invalidated_caches.length },
            }));
        }
        catch (e) {
            console.warn("checkWorkspaceRepair failed:", e);
            setError("screens.settings.workspace.errorRepair");
        }
        finally {
            setRepairing(false);
        }
        // t는 의도적으로 deps 제외 — useTranslation 객체가 매 렌더 새 ref라.
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, []);
    // repairResult를 렌더 시점에 t()로 변환.
    const repairResultText = useMemo(() => {
        if (!repairResult)
            return null;
        try {
            const parsed = JSON.parse(repairResult);
            return t(parsed.key, parsed.opts);
        }
        catch {
            return repairResult;
        }
    }, [repairResult, t]);
    return (_jsxs("form", { className: "settings-form", onSubmit: (e) => e.preventDefault(), "aria-labelledby": "settings-form-workspace", children: [_jsx("h3", { id: "settings-form-workspace", className: "visually-hidden", children: t("screens.settings.categories.workspace") }), _jsxs("fieldset", { className: "settings-fieldset", children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.workspace.path") }), _jsx("p", { className: "settings-readonly-text", children: _jsx("code", { className: "num", children: status?.workspace_root ?? "…" }) }), _jsxs("button", { type: "button", className: "settings-btn-secondary", disabled: true, "aria-label": t("screens.settings.workspace.relocate"), children: [_jsx("span", { children: t("screens.settings.workspace.relocate") }), _jsx("span", { className: "settings-coming-soon", children: t("screens.settings.workspace.relocate.comingSoon") })] })] }), _jsxs("fieldset", { className: "settings-fieldset", children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.workspace.repair") }), _jsx("button", { type: "button", className: "settings-btn-primary", onClick: handleRepair, disabled: repairing, "data-testid": "settings-workspace-repair-btn", children: repairing
                            ? t("screens.settings.workspace.repairing")
                            : t("screens.settings.workspace.repairButton") }), repairResultText && (_jsx("p", { className: "settings-success", role: "status", "aria-live": "polite", children: repairResultText })), error && (_jsx("p", { className: "settings-error", role: "alert", children: t(error) }))] })] }));
}
// ── 포터블 이동 (Phase 11') ──────────────────────────────────────────
function PortablePanel() {
    const { t } = useTranslation();
    return (_jsxs("form", { className: "settings-form", onSubmit: (e) => e.preventDefault(), "aria-labelledby": "settings-form-portable", children: [_jsx("h3", { id: "settings-form-portable", className: "visually-hidden", children: t("screens.settings.categories.portable") }), _jsx("div", { className: "settings-portable-help-row", children: _jsx(HelpButton, { sectionId: "portable", hint: t("screens.help.portable") ?? undefined, testId: "settings-portable-help" }) }), _jsx(PortableExportPanel, {}), _jsx(PortableImportPanel, {})] }));
}
// ── 카탈로그 ─────────────────────────────────────────────────────────
function CatalogPanel() {
    const { t } = useTranslation();
    const lastUpdated = useMemo(() => formatDate(new Date()), 
    // mock — v1은 카탈로그 최신 매니페스트 시각 placeholder.
    []);
    return (_jsxs("form", { className: "settings-form", onSubmit: (e) => e.preventDefault(), "aria-labelledby": "settings-form-catalog", children: [_jsx("h3", { id: "settings-form-catalog", className: "visually-hidden", children: t("screens.settings.categories.catalog") }), _jsxs("fieldset", { className: "settings-fieldset", children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.catalog.registryUrl") }), _jsx("input", { type: "text", className: "settings-input num", value: REGISTRY_URL, readOnly: true, "aria-readonly": true, "data-testid": "settings-registry-url" }), _jsx("p", { className: "settings-hint", children: t("screens.settings.catalog.registryHint") })] }), _jsxs("fieldset", { className: "settings-fieldset", children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.catalog.lastUpdated") }), _jsx("p", { className: "settings-readonly-text num", children: lastUpdated })] }), _jsx(CatalogRefreshPanel, {})] }));
}
function AdvancedPanel({ version, commit }) {
    const { t } = useTranslation();
    const sqlcipher = getEncryptDbHint();
    return (_jsxs("form", { className: "settings-form", onSubmit: (e) => e.preventDefault(), "aria-labelledby": "settings-form-advanced", children: [_jsx("h3", { id: "settings-form-advanced", className: "visually-hidden", children: t("screens.settings.categories.advanced") }), _jsxs("fieldset", { className: "settings-fieldset", children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.advanced.gemini") }), _jsxs("div", { className: "settings-toggle-row", children: [_jsx(ToggleSwitch, { checked: false, onChange: () => { }, disabled: true, ariaLabel: t("screens.settings.advanced.gemini") }), _jsx("span", { className: "settings-coming-soon", children: t("screens.settings.advanced.gemini.comingSoon") })] })] }), _jsxs("fieldset", { className: "settings-fieldset", children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.advanced.sqlcipher") }), _jsx("p", { className: "settings-readonly-text", children: _jsxs("code", { className: "num", children: ["LMMASTER_ENCRYPT_DB=", sqlcipher ? "1" : "0"] }) }), _jsx("p", { className: "settings-hint", children: t("screens.settings.advanced.sqlcipher.envHint") })] }), _jsxs("fieldset", { className: "settings-fieldset", children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.advanced.exportLogs") }), _jsxs("button", { type: "button", className: "settings-btn-secondary", disabled: true, "aria-label": t("screens.settings.advanced.exportLogs"), children: [_jsx("span", { children: t("screens.settings.advanced.exportLogs") }), _jsx("span", { className: "settings-coming-soon", children: t("screens.settings.advanced.exportLogs.comingSoon") })] })] }), _jsx(WorkbenchArtifactPanel, {}), _jsxs("fieldset", { className: "settings-fieldset", children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.advanced.buildInfo") }), _jsxs("dl", { className: "settings-build-info", children: [_jsxs("div", { className: "settings-build-info-row", children: [_jsx("dt", { children: t("screens.settings.versionLabel") }), _jsx("dd", { className: "num", children: version })] }), _jsxs("div", { className: "settings-build-info-row", children: [_jsx("dt", { children: "commit" }), _jsx("dd", { className: "num", children: commit })] })] })] })] }));
}
function SettingRadio({ name, value, checked, onChange, disabled, label, iconRight, }) {
    return (_jsxs("label", { className: `settings-radio${disabled ? " is-disabled" : ""}${checked ? " is-checked" : ""}`, children: [_jsx("input", { type: "radio", name: name, value: value, checked: checked, disabled: disabled, 
                // onChange 없으면 readOnly — React 경고 회피.
                readOnly: !onChange, onChange: onChange ?? (() => { }) }), _jsx("span", { className: "settings-radio-label", children: label }), iconRight && (_jsx("span", { className: "settings-radio-icon", "aria-hidden": true, children: iconRight }))] }));
}
function ToggleSwitch({ checked, onChange, disabled, ariaLabel, }) {
    return (_jsx("button", { type: "button", role: "switch", "aria-checked": checked, "aria-label": ariaLabel, "aria-disabled": disabled, disabled: disabled, className: `settings-toggle${checked ? " is-on" : ""}${disabled ? " is-disabled" : ""}`, onClick: onChange, children: _jsx("span", { className: "settings-toggle-track", "aria-hidden": true, children: _jsx("span", { className: "settings-toggle-thumb" }) }) }));
}
function LockIcon() {
    return (_jsxs("svg", { width: "14", height: "14", viewBox: "0 0 14 14", fill: "none", children: [_jsx("rect", { x: "2.5", y: "6", width: "9", height: "6", rx: "1.2", stroke: "currentColor", strokeWidth: "1.2" }), _jsx("path", { d: "M4.5 6V4.2C4.5 2.85 5.62 1.75 7 1.75C8.38 1.75 9.5 2.85 9.5 4.2V6", stroke: "currentColor", strokeWidth: "1.2", strokeLinecap: "round" })] }));
}
function formatDate(d) {
    // YYYY-MM-DD — UI 단계 단순. v1.x 한국어 상대시각.
    const yyyy = d.getFullYear();
    const mm = String(d.getMonth() + 1).padStart(2, "0");
    const dd = String(d.getDate()).padStart(2, "0");
    return `${yyyy}-${mm}-${dd}`;
}
