import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// InstallProgress — 재사용 가능한 설치 진행률 패널.
// Phase 1' 추출: Step3Install의 InstallRunningPanel을 props 기반으로 일반화.
//
// 정책:
// - 250ms debounce로 speed/ETA jitter 최소화.
// - reduced-motion은 design-system tokens.css가 처리.
// - 한국어 phase 라벨은 i18n key로 매핑 (caller에서 t() 적용).
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
export function InstallProgress({ title, data, onCancel, compact = false, }) {
    const { t } = useTranslation();
    const phase = phaseOf(data.latest);
    const phaseText = phaseLabel(phase, t);
    if (compact) {
        return (_jsxs("div", { className: "onb-install-compact", "aria-labelledby": "install-progress-title", "aria-live": "polite", children: [_jsxs("div", { className: "onb-install-compact-row", children: [_jsx("span", { id: "install-progress-title", className: "onb-install-compact-title", children: t("onboarding.install.running.title", { name: title }) }), _jsxs("span", { className: "onb-install-compact-phase onb-install-phase", "data-phase": phase, children: [phaseText, data.retryAttempt != null && (_jsxs("span", { className: "onb-install-retry", children: [" ", t("onboarding.install.retrySuffix", {
                                            attempt: data.retryAttempt,
                                        })] }))] }), onCancel && (_jsx("button", { type: "button", className: "onb-button onb-button-secondary onb-install-compact-cancel", onClick: onCancel, children: t("onboarding.install.cancel") }))] }), _jsx(ProgressBar, { progress: data.progress, compact: true })] }));
    }
    return (_jsxs("div", { className: "onb-step", "aria-labelledby": "install-progress-title", children: [_jsxs("header", { className: "onb-step-header", children: [_jsx("h1", { id: "install-progress-title", className: "onb-step-title", children: t("onboarding.install.running.title", { name: title }) }), _jsxs("p", { className: "onb-step-subtitle onb-install-phase", "data-phase": phase, "aria-live": "polite", children: [phaseText, data.retryAttempt != null && (_jsxs("span", { className: "onb-install-retry", children: [" ", t("onboarding.install.retrySuffix", { attempt: data.retryAttempt })] }))] })] }), _jsx(ProgressBar, { progress: data.progress }), _jsxs("details", { className: "onb-install-log", children: [_jsx("summary", { children: t("onboarding.install.detailsLabel") }), _jsxs("ul", { children: [(!data.log || data.log.length === 0) && (_jsx("li", { children: t("onboarding.install.noLogYet") })), data.log?.map((e, i) => (_jsx("li", { className: "num", children: describeEvent(e) }, i)))] })] }), onCancel && (_jsx("div", { className: "onb-step-actions", children: _jsx("button", { type: "button", className: "onb-button onb-button-secondary", onClick: onCancel, children: t("onboarding.install.cancel") }) }))] }));
}
// ── ProgressBar (debounced) ────────────────────────────────────────
function ProgressBar({ progress, compact = false, }) {
    const { t } = useTranslation();
    const [smoothed, setSmoothed] = useState(progress);
    const lastUpdateRef = useRef(0);
    useEffect(() => {
        const now = Date.now();
        if (now - lastUpdateRef.current < 250 && progress) {
            const handle = setTimeout(() => {
                setSmoothed(progress);
                lastUpdateRef.current = Date.now();
            }, 250 - (now - lastUpdateRef.current));
            return () => clearTimeout(handle);
        }
        setSmoothed(progress);
        lastUpdateRef.current = now;
        return undefined;
    }, [progress]);
    const downloaded = smoothed?.downloaded ?? 0;
    const total = smoothed?.total ?? null;
    const speed = smoothed?.speed_bps ?? 0;
    const ratio = total != null && total > 0 ? Math.min(1, downloaded / total) : null;
    const etaSec = total != null && total > 0 && speed > 0
        ? Math.max(0, Math.round((total - downloaded) / speed))
        : null;
    const etaText = (() => {
        if (etaSec == null)
            return t("onboarding.install.etaPending");
        if (etaSec >= 60) {
            return t("onboarding.install.etaMinutes", {
                minutes: Math.floor(etaSec / 60),
                seconds: etaSec % 60,
            });
        }
        return t("onboarding.install.etaSeconds", { seconds: etaSec });
    })();
    return (_jsxs("div", { className: `onb-install-progress${compact ? " is-compact" : ""}`, children: [_jsx("progress", { className: "onb-install-bar", value: ratio == null ? undefined : ratio, max: ratio == null ? undefined : 1, "aria-label": t("onboarding.install.progressAria") ?? undefined }), _jsxs("div", { className: "onb-install-meta num", children: [_jsx("span", { children: ratio != null ? `${Math.round(ratio * 100)}%` : "—" }), _jsx("span", { children: speed > 0 ? formatSpeed(speed) : t("onboarding.install.speedPending") }), _jsx("span", { children: etaText })] })] }));
}
// ── 헬퍼 ───────────────────────────────────────────────────────────
export function phaseOf(ev) {
    if (!ev)
        return "starting";
    switch (ev.kind) {
        case "started":
            return "starting";
        case "download":
            return "download";
        case "extract":
            return "extract";
        case "post-check":
            return "post-check";
        case "finished":
        case "failed":
        case "cancelled":
            return "finished";
    }
}
export function phaseLabel(phase, t) {
    switch (phase) {
        case "starting":
            return t("onboarding.install.phase.starting");
        case "download":
            return t("onboarding.install.phase.download");
        case "extract":
            return t("onboarding.install.phase.extract");
        case "post-check":
            return t("onboarding.install.phase.postCheck");
        case "finished":
            return t("onboarding.install.phase.finished");
    }
}
export function formatSpeed(bps) {
    if (bps >= 1024 * 1024)
        return `${(bps / (1024 * 1024)).toFixed(1)} MB/s`;
    if (bps >= 1024)
        return `${(bps / 1024).toFixed(0)} KB/s`;
    return `${bps} B/s`;
}
export function describeEvent(ev) {
    switch (ev.kind) {
        case "started":
            return `[start] ${ev.id} · ${ev.method}`;
        case "download": {
            const inner = ev.download;
            switch (inner.kind) {
                case "started":
                    return `[download.start] ${inner.url} (resume_from=${inner.resume_from})`;
                case "progress":
                    return `[download.progress] ${inner.downloaded}/${inner.total ?? "?"} · ${inner.speed_bps}B/s`;
                case "verified":
                    return `[download.verified] sha256=${inner.sha256_hex.slice(0, 12)}…`;
                case "finished":
                    return `[download.finished] ${inner.bytes}B`;
                case "retrying":
                    return `[download.retrying] attempt=${inner.attempt} delay=${inner.delay_ms}ms reason=${inner.reason}`;
            }
            break;
        }
        case "extract":
            return `[extract.${ev.phase}] entries=${ev.entries} bytes=${ev.total_bytes}`;
        case "post-check":
            return `[post-check] ${ev.status}`;
        case "finished":
            return `[finished] ${ev.outcome.kind}`;
        case "failed":
            return `[failed] ${ev.code}: ${ev.message}`;
        case "cancelled":
            return `[cancelled]`;
    }
    return "";
}
