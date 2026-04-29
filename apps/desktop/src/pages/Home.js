import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// Home — MainShell의 메인 영역 본문. Phase 1' 추가 화면.
//
// 정책:
// - Tailscale 패턴: 게이트웨이 status pill + 추천 카드 그리드 + 자가스캔 mini summary.
// - Phase 2' 카탈로그 진입 전이라 추천 카드는 정적 시드 (Korean 1순위 EXAONE/HCX-SEED).
// - 자가스캔 결과는 props로 받음 — 향후 scanner crate IPC 연결 시 채움.
// - 디자인 토큰만 사용. 인라인 스타일 금지.
import { useTranslation } from "react-i18next";
import { StatusPill } from "@lmmaster/design-system/react";
import { SpotlightCard } from "../components/SpotlightCard";
const SEED_MODELS = [
    {
        id: "exaone:1.2b",
        display_name: "EXAONE 4.0 1.2B",
        reason: "한국어 + 영어 이중. 작은 PC에서도 잘 돌아요.",
        size: "약 800MB",
        badge: "한국어 1순위",
    },
    {
        id: "hyperclova-x-seed-text-instruct:8b",
        display_name: "HyperCLOVA-X SEED 8B",
        reason: "네이버 한국어 토크나이저로 응답이 빨라요.",
        size: "약 5GB",
        badge: "한국어",
    },
    {
        id: "qwen2.5:3b",
        display_name: "Qwen 2.5 3B",
        reason: "다국어 + 코드. 가벼운 사이즈에 안정적이에요.",
        size: "약 2GB",
        badge: "다국어",
    },
];
export function Home({ gw, scanSummary, onPickModel }) {
    const { t } = useTranslation();
    return (_jsxs("div", { className: "home-root", children: [_jsxs("section", { className: "home-hero", "aria-labelledby": "home-hero-title", children: [_jsx("h2", { id: "home-hero-title", className: "home-hero-title", children: t("home.hero.title") }), _jsx("p", { className: "home-hero-subtitle", children: t("home.hero.subtitle") }), _jsx(GatewayPillLarge, { gw: gw })] }), _jsxs("section", { className: "home-section", "aria-labelledby": "home-recommend-title", children: [_jsxs("header", { className: "home-section-header", children: [_jsx("h3", { id: "home-recommend-title", className: "home-section-title", children: t("home.recommend.title") }), _jsx("p", { className: "home-section-subtitle", children: t("home.recommend.subtitle") })] }), _jsx("div", { className: "home-card-grid", children: SEED_MODELS.map((m) => (_jsxs(SpotlightCard, { className: "home-model-card is-clickable", role: "button", tabIndex: 0, onClick: () => onPickModel?.(m.id), onKeyDown: (e) => {
                                if (e.key === "Enter" || e.key === " ") {
                                    e.preventDefault();
                                    onPickModel?.(m.id);
                                }
                            }, "aria-label": t("home.recommend.cardLabel", {
                                name: m.display_name,
                                reason: m.reason,
                            }), children: [_jsxs("header", { className: "home-model-card-header", children: [_jsx("span", { className: "home-model-card-title", children: m.display_name }), _jsx("span", { className: "home-model-card-badge", children: m.badge })] }), _jsx("p", { className: "home-model-card-reason", children: m.reason }), _jsx("p", { className: "home-model-card-size num", children: m.size })] }, m.id))) })] }), _jsxs("section", { className: "home-section", "aria-labelledby": "home-scan-title", children: [_jsx("header", { className: "home-section-header", children: _jsx("h3", { id: "home-scan-title", className: "home-section-title", children: t("home.scan.title") }) }), _jsx("div", { className: "home-scan-card", children: scanSummary ? (_jsx("p", { className: "home-scan-summary", children: scanSummary })) : (_jsx("p", { className: "home-scan-empty", children: t("home.scan.empty") })) })] })] }));
}
// ── 부속 ───────────────────────────────────────────────────────────
function GatewayPillLarge({ gw }) {
    const { t } = useTranslation();
    const banner = gw.status === "listening"
        ? t("home.gateway-ready", { port: gw.port ?? "?" })
        : gw.status === "failed"
            ? t("home.gateway-failed", { error: gw.error ?? "" })
            : gw.status === "stopping"
                ? t("home.gateway-stopping")
                : t("home.gateway-booting");
    return (_jsx(StatusPill, { status: mapStatus(gw.status), label: banner, detail: gw.port != null ? `:${gw.port}` : null, size: "lg" }));
}
function mapStatus(s) {
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
