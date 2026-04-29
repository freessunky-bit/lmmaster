import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// ModelCard — 카탈로그 그리드의 카드 한 장.
//
// 정책 (phase-2pb-catalog-ui-decision.md):
// - 3행 정보 우선순위: (1) display_name + 배지, (2) 카테고리 + 한국어 별점 + 사용처, (3) 메트릭 + compat hint.
// - excluded 모델은 dim + reason chip — 클릭 가능하지만 install CTA 비활성.
// - 카드 자체는 SpotlightCard 위에 얹어 hover spotlight.
// - Drawer 열기는 onSelect 콜백 — Catalog가 owner.
import { useTranslation } from "react-i18next";
import { SpotlightCard } from "../SpotlightCard";
import { compatOf, formatSize, idOf, languageStars, } from "./format";
export function ModelCard({ model, recommendation, onSelect }) {
    const { t } = useTranslation();
    const excluded = findExcluded(model, recommendation);
    const compat = compatOf(model, recommendation);
    const isExcluded = !!excluded;
    return (_jsxs(SpotlightCard, { className: `catalog-card${isExcluded ? " is-excluded" : ""}`, role: "button", tabIndex: 0, onClick: () => onSelect(model), onKeyDown: (e) => {
            if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onSelect(model);
            }
        }, "aria-disabled": isExcluded || undefined, children: [_jsx("header", { className: "catalog-card-header", children: _jsxs("div", { className: "catalog-card-badges", children: [_jsx("span", { className: `catalog-card-chip catalog-card-chip-${model.verification.tier}`, "data-testid": "verification-chip", children: t(`model.verification.${model.verification.tier}`) }), _jsx("span", { className: `catalog-card-chip catalog-card-chip-${model.maturity}`, "data-testid": "maturity-chip", children: t(`model.maturity.${model.maturity}`) })] }) }), _jsx("h4", { className: "catalog-card-title", children: model.display_name }), _jsxs("div", { className: "catalog-card-meta", children: [_jsx("span", { className: "catalog-card-category", children: t(`catalog.category.${model.category}`) }), _jsx("span", { className: "catalog-card-stars", "aria-label": `${t("model.metric.korean", "한국어 강도")}: ${model.language_strength ?? 0}/10`, children: languageStars(model.language_strength) })] }), model.use_case_examples.length > 0 && (_jsx("p", { className: "catalog-card-usecase", children: model.use_case_examples[0] })), _jsxs("dl", { className: "catalog-card-metrics", children: [_jsxs("div", { className: "catalog-card-metric", children: [_jsx("dt", { children: t("model.metric.vram") }), _jsx("dd", { className: "num", children: model.rec_vram_mb == null
                                    ? t("model.metric.noVram")
                                    : formatSize(model.rec_vram_mb) })] }), _jsxs("div", { className: "catalog-card-metric", children: [_jsx("dt", { children: t("model.metric.ram") }), _jsx("dd", { className: "num", children: formatSize(model.rec_ram_mb) })] }), _jsxs("div", { className: "catalog-card-metric", children: [_jsx("dt", { children: t("model.metric.size") }), _jsx("dd", { className: "num", children: formatSize(model.install_size_mb) })] })] }), _jsx("div", { className: "catalog-card-footer", children: excluded ? (_jsx("span", { className: "catalog-card-compat is-unfit", "data-testid": "exclude-chip", children: excludeText(t, excluded) })) : (_jsx("span", { className: `catalog-card-compat is-${compat}`, "data-testid": "compat-chip", children: t(`model.compat.${compat}`) })) })] }));
}
function findExcluded(model, rec) {
    if (!rec)
        return undefined;
    return rec.excluded.find((e) => idOf(e) === model.id);
}
function excludeText(t, reason) {
    switch (reason.kind) {
        case "insufficient-vram":
            return t("model.exclude.insufficientVram", {
                need: formatSize(reason.need_mb),
                have: formatSize(reason.have_mb),
            });
        case "insufficient-ram":
            return t("model.exclude.insufficientRam", {
                need: formatSize(reason.need_mb),
                have: formatSize(reason.have_mb),
            });
        case "deprecated":
            return t("model.exclude.deprecated");
        case "incompatible-runtime":
            return t("model.exclude.incompatible");
    }
}
