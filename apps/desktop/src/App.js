import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { StatusPill } from "@lmmaster/design-system/react";
import { CommandPalette } from "./components/command-palette/CommandPalette";
import { EulaGate } from "./components/EulaGate";
import { CommandPaletteProvider, useCommandRegistration, } from "./components/command-palette/context";
import { useCommandPaletteHotkey } from "./hooks/useCommandPaletteHotkey";
import { ShortcutsModal, useShortcutsHotkey, } from "./components/ShortcutsModal";
import { TourWelcomeToast } from "./components/TourWelcomeToast";
import { getGatewayStatus, onGatewayFailed, onGatewayReady, } from "./ipc/gateway";
import { getLastScan, onScanSummary } from "./ipc/scanner";
import { OnboardingApp } from "./onboarding/OnboardingApp";
import { isOnboardingCompleted, markCompleted, resetOnboarding, } from "./onboarding/persistence";
import { ApiKeysPanel } from "./components/keys/ApiKeysPanel";
import { WorkspaceRepairBanner } from "./components/workspace/WorkspaceRepairBanner";
import { WorkspaceSwitcher } from "./components/WorkspaceSwitcher";
import { ActiveWorkspaceProvider } from "./contexts/ActiveWorkspaceContext";
import { CatalogPage } from "./pages/Catalog";
import { Diagnostics } from "./pages/Diagnostics";
import { Guide } from "./pages/Guide";
import { Home } from "./pages/Home";
import { InstallPage } from "./pages/Install";
import { Projects } from "./pages/Projects";
import { RuntimesPage } from "./pages/Runtimes";
import { Settings } from "./pages/Settings";
import { Workbench } from "./pages/Workbench";
import { Workspace } from "./pages/Workspace";
import "./components/workspace/workspace.css";
import "./pages/home.css";
const NAV_KEYS = [
    "home",
    "catalog",
    "install",
    "runtimes",
    "workspace",
    "projects",
    "keys",
    "workbench",
    "diagnostics",
    "guide",
    "settings",
];
export default function App() {
    const [completed, setCompleted] = useState(isOnboardingCompleted);
    // 첫 onboarding 직후를 표시 — TourWelcomeToast trigger.
    // 페이지 새로고침 시 리셋되지만 toast는 localStorage `lmmaster.tour.shown` 1회 영속이라 안전.
    const [justCompleted, setJustCompleted] = useState(false);
    const handleComplete = useCallback(() => {
        markCompleted();
        setCompleted(true);
        setJustCompleted(true);
    }, []);
    // 마법사 다시 보기 — settings/팔레트에서 호출.
    const reopenWizard = useCallback(() => {
        resetOnboarding();
        setCompleted(false);
        setJustCompleted(false);
    }, []);
    // EULA 게이트 — onboarding / main 진입 전 1차 차단. version-bound.
    // patch 갱신 시 같은 키 유지(자동 동의), minor/major 갱신 시 새 키 → 재동의.
    const handleEulaAccept = useCallback(() => {
        // 영속은 EulaGate 내부에서 처리 — 여기선 후속 처리(분석/로그) 자리.
    }, []);
    return (_jsxs(CommandPaletteProvider, { children: [_jsx(PaletteHotkey, {}), _jsx(CommandPalette, {}), _jsx(EulaGate, { eulaVersion: "1.0.0", onAccept: handleEulaAccept, children: _jsx(ActiveWorkspaceProvider, { children: !completed ? (_jsx(OnboardingApp, { onComplete: handleComplete })) : (_jsx(MainShell, { onReopenWizard: reopenWizard, tourTrigger: justCompleted })) }) })] }));
}
/** Provider 안에서 hotkey 등록. */
function PaletteHotkey() {
    useCommandPaletteHotkey();
    return null;
}
function MainShell({ onReopenWizard, tourTrigger, }) {
    const { t } = useTranslation();
    const [gw, setGw] = useState({
        port: null,
        status: "booting",
        error: null,
    });
    const [scan, setScan] = useState(null);
    const [activeNav, setActiveNav] = useState("home");
    const [shortcutsOpen, setShortcutsOpen] = useState(false);
    // F1 / Shift+? — ShortcutsModal toggle. Ctrl+1~9 — NAV 이동.
    useShortcutsHotkey({
        open: shortcutsOpen,
        setOpen: setShortcutsOpen,
        onNav: (navKey) => {
            if (NAV_KEYS.includes(navKey)) {
                setActiveNav(navKey);
            }
        },
    });
    useEffect(() => {
        let cancelled = false;
        let unlistenReady = null;
        let unlistenFailed = null;
        let unlistenScan = null;
        (async () => {
            // Race fix: register listeners FIRST so we don't miss a rapid emit.
            // Backend's gateway::run can emit `gateway://ready` within milliseconds of
            // app start; if we awaited get_gateway_status first, the snapshot might
            // still read "booting" and we'd miss the event arriving in between.
            unlistenReady = await onGatewayReady((port) => {
                setGw((prev) => ({ ...prev, port, status: "listening", error: null }));
            });
            unlistenFailed = await onGatewayFailed((error) => {
                setGw((prev) => ({ ...prev, status: "failed", error }));
            });
            // Snapshot AFTER listener registration — covers the case where gateway
            // is already listening before the React app mounted.
            try {
                const snap = await getGatewayStatus();
                if (!cancelled) {
                    setGw((prev) => 
                    // Don't downgrade if listener already received a "listening" event.
                    prev.status === "listening" ? prev : snap);
                }
            }
            catch (e) {
                if (!cancelled)
                    console.warn("get_gateway_status failed:", e);
            }
            // 자가 점검 — 캐시된 결과 먼저, 이후 scan:summary event로 자동 갱신.
            try {
                const cached = await getLastScan();
                if (!cancelled && cached)
                    setScan(cached);
            }
            catch (e) {
                if (!cancelled)
                    console.warn("get_last_scan failed:", e);
            }
            unlistenScan = await onScanSummary((s) => setScan(s));
        })();
        return () => {
            cancelled = true;
            unlistenReady?.();
            unlistenFailed?.();
            unlistenScan?.();
        };
    }, []);
    // 페이지 간 cross-navigation custom event 리스너 (Diagnostics → Catalog 등).
    useEffect(() => {
        const handler = (e) => {
            const detail = e.detail;
            if (typeof detail === "string" &&
                NAV_KEYS.includes(detail)) {
                setActiveNav(detail);
            }
        };
        window.addEventListener("lmmaster:navigate", handler);
        return () => window.removeEventListener("lmmaster:navigate", handler);
    }, []);
    // MainShell 시드 명령 — 팔레트에 등록.
    const commands = useMemo(() => [
        {
            id: "nav.home",
            group: "navigation",
            label: t("palette.cmd.nav.home"),
            keywords: ["home", "ㅎ", "메인"],
            perform: () => {
                // 홈 라우팅은 Phase 4 — 지금은 noop.
            },
        },
        {
            id: "nav.diagnostics",
            group: "navigation",
            label: t("palette.cmd.nav.diagnostics"),
            keywords: ["diagnostics", "ㅈㄷ", "로그"],
            perform: () => {
                // 진단 라우팅도 Phase 4.
            },
        },
        {
            id: "system.gateway.copyUrl",
            group: "system",
            label: t("palette.cmd.system.gateway.copyUrl"),
            keywords: ["copy", "url", "port", "복사"],
            isAvailable: () => gw.status === "listening" && gw.port != null,
            perform: async () => {
                if (gw.port == null)
                    return;
                const url = `http://127.0.0.1:${gw.port}`;
                try {
                    await navigator.clipboard.writeText(url);
                }
                catch {
                    console.warn("clipboard write failed for", url);
                }
            },
        },
        {
            id: "system.wizard.reopen",
            group: "system",
            label: t("palette.cmd.system.wizard.reopen"),
            keywords: ["onboarding", "wizard", "ㅁㅂㅅ"],
            perform: onReopenWizard,
        },
    ], [gw.port, gw.status, onReopenWizard, t]);
    useCommandRegistration(commands);
    // 게이트웨이 banner는 Home 컴포넌트가 자체 렌더 — App.tsx 레벨에선 추가 가공 불필요.
    return (_jsxs("div", { className: "app-shell", children: [_jsxs("aside", { className: "sidebar", children: [_jsx("div", { className: "brand", children: "LMmaster" }), _jsx(WorkspaceSwitcher, {}), _jsx("nav", { children: NAV_KEYS.map((key) => (_jsx("button", { type: "button", className: `nav-item${activeNav === key ? " active" : ""}`, onClick: () => setActiveNav(key), children: t(`nav.${key}`) }, key))) }), _jsx(StatusPill, { status: mapGatewayStatus(gw.status), label: t(`gateway.status.${gw.status}`), detail: gw.port != null ? `:${gw.port}` : null, size: "sm", className: "sidebar-pill", ariaLabel: gw.error ?? undefined })] }), _jsxs("main", { className: "content", children: [_jsx("header", { className: "topbar", children: _jsx("div", { className: "title", children: t(`nav.${activeNav}`) }) }), _jsx(WorkspaceRepairBanner, {}), activeNav === "catalog" ? (_jsx(CatalogPage, {})) : activeNav === "keys" ? (_jsx(ApiKeysPanel, {})) : activeNav === "install" ? (_jsx(InstallPage, {})) : activeNav === "runtimes" ? (_jsx(RuntimesPage, {})) : activeNav === "workspace" ? (_jsx(Workspace, {})) : activeNav === "projects" ? (_jsx(Projects, {})) : activeNav === "workbench" ? (_jsx(Workbench, {})) : activeNav === "diagnostics" ? (_jsx(Diagnostics, {})) : activeNav === "guide" ? (_jsx(Guide, {})) : activeNav === "settings" ? (_jsx(Settings, {})) : (_jsx(Home, { gw: gw, scanSummary: scan?.summary_korean, onPickModel: (modelId) => {
                            try {
                                window.localStorage.setItem("lmmaster.catalog.preselect", modelId);
                            }
                            catch {
                                /* ignore — catalog will just open without preselection */
                            }
                            setActiveNav("catalog");
                        } }))] }), _jsx(ShortcutsModal, { open: shortcutsOpen, onClose: () => setShortcutsOpen(false) }), _jsx(TourWelcomeToast, { trigger: tourTrigger })] }));
}
function mapGatewayStatus(s) {
    switch (s) {
        case "listening":
            return "listening";
        case "failed":
            return "failed";
        case "stopping":
            return "stopping";
        case "booting":
            return "booting";
        default:
            return "idle";
    }
}
