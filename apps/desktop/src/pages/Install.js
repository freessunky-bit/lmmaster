import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// Install — 런타임 설치 메인 화면 (마법사와 분리된 진입점). Phase 4.b.
//
// 정책 (phase-4-screens-decision.md §1.1 install + phase-4b-install-screen-decision.md):
// - 카드 그리드 (Ollama + LM Studio 2 카드).
// - 카드 상태별 액션: not-installed → "받을게요", running/installed → "재설치", 공통 "자세히" / "폴더 열기".
// - 카드 클릭 시 우측 drawer로 manifest detail.
// - 설치 진행 중일 때 하단 진행 패널 (InstallProgress compact 모드) — 접힘 가능.
// - 둘 다 설치되어 있으면 빈 상태 + 카탈로그 이동 CTA.
// - 디자인 토큰만 사용. 인라인 스타일 금지.
import { useCallback, useEffect, useMemo, useRef, useState, } from "react";
import { useTranslation } from "react-i18next";
import { StatusPill } from "@lmmaster/design-system/react";
import { InstallProgress } from "../components/InstallProgress";
import { detectEnvironment } from "../ipc/environment";
import { cancelInstall, installApp } from "../ipc/install";
import { isTerminal, } from "../ipc/install-events";
import "./install.css";
const RUNTIME_DEFS = [
    {
        id: "ollama",
        nameKey: "screens.install.cards.ollama.name",
        reasonKey: "screens.install.cards.ollama.reason",
        licenseKey: "screens.install.cards.ollama.license",
        installSize: "약 800MB",
        homepage: "https://ollama.com/",
    },
    {
        id: "lm-studio",
        nameKey: "screens.install.cards.lmStudio.name",
        reasonKey: "screens.install.cards.lmStudio.reason",
        licenseKey: "screens.install.cards.lmStudio.license",
        installSize: "공식 사이트 안내",
        homepage: "https://lmstudio.ai/",
    },
];
// ── 페이지 ────────────────────────────────────────────────────────
export function InstallPage({ onNavigate, }) {
    const { t } = useTranslation();
    const [env, setEnv] = useState(null);
    const [envError, setEnvError] = useState(null);
    const [active, setActive] = useState(null);
    const [selected, setSelected] = useState(null);
    // 환경 감지 — 마운트 시 1회.
    useEffect(() => {
        let cancelled = false;
        detectEnvironment()
            .then((report) => {
            if (!cancelled)
                setEnv(report);
        })
            .catch((e) => {
            if (cancelled)
                return;
            const message = e.message ?? String(e);
            setEnvError(message);
        });
        return () => {
            cancelled = true;
        };
    }, []);
    // 런타임 → status 매핑.
    const status = useMemo(() => statusMapFrom(env?.runtimes ?? []), [env]);
    const allReady = useMemo(() => {
        return (isReadyOrRunning(status.ollama) && isReadyOrRunning(status["lm-studio"]));
    }, [status]);
    const handleInstall = useCallback(async (id) => {
        // 이미 설치 진행 중이면 무시 — IPC 측 already-installing도 보호하지만 UI에서 즉시 차단.
        if (active)
            return;
        const initial = {
            id,
            data: { log: [] },
        };
        setActive(initial);
        try {
            await installApp(id, {
                onEvent: (event) => {
                    setActive((prev) => {
                        if (!prev || prev.id !== id)
                            return prev;
                        return { id, data: applyEvent(prev.data, event) };
                    });
                },
            });
            // 설치 종료 후 environment 재감지 — status 갱신.
            try {
                const report = await detectEnvironment();
                setEnv(report);
            }
            catch (e) {
                console.warn("detectEnvironment after install failed:", e);
            }
        }
        catch (e) {
            const apiErr = e;
            console.warn("installApp failed:", apiErr);
            // 사용자에게는 진행 패널의 마지막 이벤트가 failed로 노출됨.
            // already-installing이면 panel 정리.
            if (apiErr.kind === "already-installing") {
                // active 그대로 유지 — 진행 중인 것이 있다는 뜻.
                return;
            }
        }
        finally {
            // 패널은 사용자가 명시적으로 닫을 수 있도록 잠시 유지 — 다음 install 클릭 시 자동 교체.
            // 종료 이벤트(finished/failed/cancelled) 받으면 active.data.latest로 표시되고,
            // 새 install 호출 시 setActive로 교체된다.
        }
    }, [active]);
    const handleCancel = useCallback(async () => {
        if (!active)
            return;
        try {
            await cancelInstall(active.id);
        }
        catch (e) {
            console.warn("cancelInstall failed:", e);
        }
    }, [active]);
    const handleDismissPanel = useCallback(() => {
        setActive(null);
    }, []);
    return (_jsxs("div", { className: "install-root", children: [_jsxs("div", { className: "install-topbar", children: [_jsxs("div", { className: "install-topbar-row", children: [_jsx("h2", { className: "install-title", children: t("screens.install.title") }), _jsx(AggregateStatusPill, { status: status })] }), _jsx("p", { className: "install-subtitle", children: t("screens.install.subtitle") })] }), envError && (_jsx("div", { className: "install-empty", role: "alert", children: _jsx("h3", { className: "install-empty-title", children: envError }) })), _jsx("section", { className: "install-card-grid", "aria-label": t("screens.install.title"), children: RUNTIME_DEFS.map((def) => (_jsx(RuntimeCard, { def: def, status: status[def.id], onInstall: () => handleInstall(def.id), onSelect: () => setSelected(def), isInstalling: active?.id === def.id && !panelDismissable(active) }, def.id))) }), active && (_jsx(ProgressPanel, { active: active, onCancel: handleCancel, onDismiss: handleDismissPanel })), !active && allReady && (_jsx(EmptyState, { onNavigate: onNavigate })), selected && (_jsx(ManifestDrawer, { def: selected, onClose: () => setSelected(null) }))] }));
}
// ── 합산 StatusPill ────────────────────────────────────────────
function AggregateStatusPill({ status, }) {
    const { t } = useTranslation();
    const gw = aggregateGatewayLikeStatus(status);
    const label = (() => {
        switch (gw) {
            case "listening":
                return t("gateway.status.listening");
            case "booting":
                return t("gateway.status.booting");
            case "failed":
                return t("gateway.status.failed");
            case "stopping":
                return t("status.standby");
            default:
                return t("status.standby");
        }
    })();
    return _jsx(StatusPill, { status: gw, label: label, size: "md" });
}
function aggregateGatewayLikeStatus(status) {
    // 런타임 합산 → "running 1+개면 listening, installed 1+개면 stopping(준비됐어요),
    // 모두 unknown이면 booting, 모두 not-installed면 idle".
    const values = Object.values(status);
    if (values.some((s) => s === "running"))
        return "listening";
    if (values.some((s) => s === "installed"))
        return "stopping";
    if (values.every((s) => s === "unknown"))
        return "booting";
    return "idle";
}
// ── RuntimeCard ────────────────────────────────────────────────
function RuntimeCard({ def, status, onInstall, onSelect, isInstalling, }) {
    const { t } = useTranslation();
    const isReady = status === "running" || status === "installed";
    const pillState = pillStateFor(status, isInstalling);
    const announcement = isReady
        ? t("screens.install.alreadyReady")
        : t("screens.install.notInstalled");
    // 카드는 region — 액션은 footer 버튼으로만 트리거 (nested interactive 회피).
    // 비-버튼 영역 click도 onSelect 트리거 — 마우스 편의 (키보드는 footer 버튼이 책임).
    const titleId = `install-card-title-${def.id}`;
    const handleAreaClick = (e) => {
        if (e.target.closest("button"))
            return;
        onSelect();
    };
    return (_jsxs("article", { className: "install-card", "data-runtime": def.id, "data-status": status, onClick: handleAreaClick, "aria-labelledby": titleId, children: [_jsxs("div", { className: "install-card-header", children: [_jsx("h3", { id: titleId, className: "install-card-name", children: t(def.nameKey) }), _jsx("span", { className: "install-card-license", children: t(def.licenseKey) })] }), _jsxs("div", { className: "install-card-status-row", children: [_jsx(StatusPill, { status: pillState.status, label: pillState.label(t), size: "sm" }), _jsx("span", { className: "install-card-status-text", children: announcement })] }), _jsx("p", { className: "install-card-reason", children: t(def.reasonKey) }), _jsxs("div", { className: "install-card-footer", children: [_jsx("button", { type: "button", className: "install-action is-primary", onClick: onInstall, disabled: isInstalling, children: isReady
                            ? t("screens.install.actions.reinstall")
                            : t("screens.install.actions.install") }), _jsx("button", { type: "button", className: "install-action", onClick: onSelect, children: t("screens.install.actions.details") })] })] }));
}
function pillStateFor(status, isInstalling) {
    if (isInstalling) {
        return {
            status: "booting",
            label: (t) => t("status.installing"),
        };
    }
    switch (status) {
        case "running":
            return { status: "listening", label: (t) => t("gateway.status.listening") };
        case "installed":
            return { status: "stopping", label: (t) => t("status.standby") };
        case "not-installed":
            return { status: "idle", label: (t) => t("status.standby") };
        default:
            return { status: "idle", label: (t) => t("status.standby") };
    }
}
// ── 진행 패널 ──────────────────────────────────────────────────
function ProgressPanel({ active, onCancel, onDismiss, }) {
    const { t } = useTranslation();
    const isFinished = isTerminalData(active.data);
    return (_jsxs("section", { className: "install-progress-panel", "aria-labelledby": "install-progress-panel-title", "aria-live": "polite", children: [_jsxs("div", { className: "install-topbar-row", children: [_jsx("h3", { id: "install-progress-panel-title", className: "install-progress-panel-title", children: nameForId(active.id, t) }), isFinished && (_jsx("button", { type: "button", className: "install-action", onClick: onDismiss, children: t("screens.install.drawer.close") }))] }), _jsx(InstallProgress, { compact: true, title: nameForId(active.id, t), data: active.data, onCancel: isFinished ? undefined : onCancel })] }));
}
// ── 빈 상태 ────────────────────────────────────────────────────
function EmptyState({ onNavigate, }) {
    const { t } = useTranslation();
    return (_jsxs("section", { className: "install-empty", role: "status", children: [_jsx("h3", { className: "install-empty-title", children: t("screens.install.empty.title") }), _jsx("p", { className: "install-empty-body", children: t("screens.install.empty.body") }), _jsx("button", { type: "button", className: "install-action is-primary install-empty-cta", onClick: () => onNavigate?.("catalog"), children: t("screens.install.empty.cta") })] }));
}
// ── Drawer (manifest detail) ──────────────────────────────────
function ManifestDrawer({ def, onClose, }) {
    const { t } = useTranslation();
    const closeBtnRef = useRef(null);
    useEffect(() => {
        const onKey = (e) => {
            if (e.key === "Escape")
                onClose();
        };
        window.addEventListener("keydown", onKey);
        closeBtnRef.current?.focus();
        return () => window.removeEventListener("keydown", onKey);
    }, [onClose]);
    return (_jsx("div", { className: "install-drawer-backdrop", role: "presentation", onClick: onClose, children: _jsxs("div", { className: "install-drawer", role: "dialog", "aria-modal": "true", "aria-labelledby": "install-drawer-title", onClick: (e) => e.stopPropagation(), children: [_jsxs("div", { className: "install-drawer-header", children: [_jsx("h3", { id: "install-drawer-title", className: "install-drawer-title", children: t(def.nameKey) }), _jsx("button", { ref: closeBtnRef, type: "button", className: "install-drawer-close", onClick: onClose, "aria-label": t("screens.install.drawer.close"), children: "\u00D7" })] }), _jsxs("div", { className: "install-drawer-body", children: [_jsxs("section", { children: [_jsx("h4", { className: "install-drawer-section-title", children: t("screens.install.drawer.licenseFull") }), _jsx("p", { className: "install-drawer-text", children: t(def.licenseKey) })] }), _jsxs("section", { children: [_jsx("h4", { className: "install-drawer-section-title", children: t("screens.install.drawer.installSize") }), _jsx("p", { className: "install-drawer-text num", children: def.installSize })] }), _jsxs("section", { children: [_jsx("h4", { className: "install-drawer-section-title", children: t("screens.install.drawer.homepage") }), _jsx("p", { className: "install-drawer-text", children: _jsx("a", { className: "install-drawer-link", href: def.homepage, target: "_blank", rel: "noreferrer", children: def.homepage }) })] }), _jsx("section", { children: _jsx("p", { className: "install-drawer-text", children: t(def.reasonKey) }) })] })] }) }));
}
// ── 헬퍼 ────────────────────────────────────────────────────────
function statusMapFrom(runtimes) {
    const find = (k) => {
        const r = runtimes.find((x) => x.runtime === k);
        if (!r)
            return "unknown";
        if (r.status === "running" ||
            r.status === "installed" ||
            r.status === "not-installed") {
            return r.status;
        }
        return "unknown";
    };
    return {
        ollama: find("ollama"),
        "lm-studio": find("lm-studio"),
    };
}
function isReadyOrRunning(s) {
    return s === "running" || s === "installed";
}
function applyEvent(prev, event) {
    const log = [...prev.log, event].slice(-10);
    let progress = prev.progress;
    let retryAttempt = prev.retryAttempt;
    if (event.kind === "download") {
        if (event.download.kind === "progress") {
            progress = {
                downloaded: event.download.downloaded,
                total: event.download.total,
                speed_bps: event.download.speed_bps,
            };
        }
        else if (event.download.kind === "retrying") {
            retryAttempt = event.download.attempt;
        }
    }
    return { latest: event, progress, log, retryAttempt };
}
function isTerminalData(data) {
    return data.latest != null && isTerminal(data.latest);
}
function panelDismissable(active) {
    if (!active)
        return false;
    return isTerminalData(active.data);
}
function nameForId(id, t) {
    if (id === "ollama")
        return t("screens.install.cards.ollama.name");
    return t("screens.install.cards.lmStudio.name");
}
