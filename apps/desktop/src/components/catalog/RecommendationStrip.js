import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// RecommendationStrip — 추천 4슬롯 가로 스트립.
//
// 정책 (phase-2pb-catalog-ui-decision.md §4):
// - Best는 강조 보더(--primary) + glow.
// - Balanced/Lightweight/Fallback은 secondary 보더 동급.
// - Best=null이면 빈 상태 메시지.
// - 슬롯 클릭 → onSelect(modelId)로 Catalog가 그리드 highlight + scroll.
import { useTranslation } from "react-i18next";
const SLOT_ORDER = ["best", "balanced", "lightweight", "fallback"];
export function RecommendationStrip({ recommendation, loading, byId, onSelect, }) {
    const { t } = useTranslation();
    if (loading) {
        return (_jsx("div", { className: "catalog-rec-strip", "aria-busy": "true", children: _jsx("p", { className: "catalog-rec-loading", children: t("recommendation.loading") }) }));
    }
    if (!recommendation) {
        return null;
    }
    const slots = {
        best: recommendation.best_choice,
        balanced: recommendation.balanced_choice,
        lightweight: recommendation.lightweight_choice,
        fallback: recommendation.fallback_choice,
    };
    return (_jsxs("section", { className: "catalog-rec-strip", "aria-labelledby": "catalog-rec-title", children: [_jsx("h3", { id: "catalog-rec-title", className: "catalog-rec-title", children: t("home.recommend.title") }), _jsx("div", { className: "catalog-rec-grid", role: "list", children: SLOT_ORDER.map((key) => (_jsx(RecommendationSlot, { slot: key, modelId: slots[key], entry: slots[key] ? byId.get(slots[key]) : undefined, onSelect: onSelect, isBestEmpty: key === "best" && slots.best == null }, key))) })] }));
}
function RecommendationSlot({ slot, modelId, entry, onSelect, isBestEmpty, }) {
    const { t } = useTranslation();
    if (isBestEmpty) {
        return (_jsxs("div", { className: "catalog-rec-slot is-best is-empty", role: "listitem", children: [_jsx("span", { className: "catalog-rec-slot-label", children: t(`recommendation.${slot}.label`) }), _jsx("p", { className: "catalog-rec-slot-empty", children: t("recommendation.empty.best") })] }));
    }
    if (!modelId || !entry) {
        return (_jsxs("div", { className: `catalog-rec-slot is-${slot} is-empty`, role: "listitem", children: [_jsx("span", { className: "catalog-rec-slot-label", children: t(`recommendation.${slot}.label`) }), _jsx("p", { className: "catalog-rec-slot-empty", children: t("recommendation.empty.slot") })] }));
    }
    return (_jsxs("button", { type: "button", className: `catalog-rec-slot is-${slot}`, role: "listitem", onClick: () => onSelect(modelId), children: [_jsx("span", { className: "catalog-rec-slot-label", children: t(`recommendation.${slot}.label`) }), _jsx("span", { className: "catalog-rec-slot-name", children: entry.display_name })] }));
}
