import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// CustomModelsSection — Workbench가 등록한 사용자 정의 모델을 카탈로그에 노출.
//
// 정책 (Phase 8'.b.1, ADR-0038 multi-workspace 호환):
// - active workspace가 바뀔 때마다 list_custom_models 재호출 — Phase 8'.1 ActiveWorkspaceContext 사용.
//   v1 백엔드는 workspace_id 필터링 미지원이지만, ContextChange를 트리거 삼아 hot-refresh 흐름을 정착.
// - 카드 디자인은 catalog-card 토큰 재사용 + custom 전용 badge "내가 만든 모델".
// - 클릭 → onSelect(model) 콜백 → Catalog 페이지가 ModelDetailDrawer에 흘려보냄.
// - empty 상태: "Workbench에서 모델을 만들면 여기 표시돼요" + Workbench 진입 CTA.
// - 한국어 해요체 + design-system tokens.
// - a11y: section + role="list" + listitem, button focus-visible 토큰 적용 (CSS).
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listCustomModels } from "../../ipc/workbench";
import { useActiveWorkspaceOptional } from "../../contexts/ActiveWorkspaceContext";
const NAV_EVENT = "lmmaster:navigate";
/**
 * Workbench가 등록한 모델 섹션 — 추천 strip 아래 / 그리드 위.
 *
 * 데이터:
 * - 마운트 시 listCustomModels() 호출.
 * - active workspace.id 변경 시 refetch (Phase 8'.1 multi-workspace 흐름).
 * - 실패는 console.warn — UI는 빈 상태로.
 */
export function CustomModelsSection({ onSelect }) {
    const { t } = useTranslation();
    const ws = useActiveWorkspaceOptional();
    const activeId = ws?.active?.id ?? null;
    const [models, setModels] = useState([]);
    const [loading, setLoading] = useState(true);
    useEffect(() => {
        let cancelled = false;
        setLoading(true);
        listCustomModels()
            .then((list) => {
            if (cancelled)
                return;
            setModels(list);
            setLoading(false);
        })
            .catch((e) => {
            if (cancelled)
                return;
            console.warn("listCustomModels 실패:", e);
            setModels([]);
            setLoading(false);
        });
        return () => {
            cancelled = true;
        };
        // activeId 변경 시 refetch — backend가 workspace 필터링을 v1.x에 추가해도 호환.
    }, [activeId]);
    const goWorkbench = () => {
        window.dispatchEvent(new CustomEvent(NAV_EVENT, { detail: "workbench" }));
    };
    if (loading) {
        return (_jsxs("section", { className: "catalog-custom-section", "data-testid": "custom-models-section", children: [_jsx("header", { className: "catalog-custom-header", children: _jsx("h3", { className: "catalog-rec-title", children: t("screens.catalog.custom.title") }) }), _jsx("p", { className: "catalog-rec-loading", children: t("screens.catalog.custom.loading") })] }));
    }
    if (models.length === 0) {
        return (_jsxs("section", { className: "catalog-custom-section", "data-testid": "custom-models-section", children: [_jsx("header", { className: "catalog-custom-header", children: _jsx("h3", { className: "catalog-rec-title", children: t("screens.catalog.custom.title") }) }), _jsxs("div", { className: "catalog-custom-empty", "data-testid": "custom-models-empty", children: [_jsx("p", { className: "catalog-rec-slot-empty", children: t("screens.catalog.custom.empty") }), _jsx("button", { type: "button", className: "onb-button onb-button-ghost", onClick: goWorkbench, "data-testid": "custom-models-empty-cta", children: t("screens.catalog.custom.openWorkbench") })] })] }));
    }
    return (_jsxs("section", { className: "catalog-custom-section", "data-testid": "custom-models-section", children: [_jsxs("header", { className: "catalog-custom-header", children: [_jsx("h3", { className: "catalog-rec-title", children: t("screens.catalog.custom.title") }), _jsx("p", { className: "catalog-page-subtitle", children: t("screens.catalog.custom.subtitle") })] }), _jsx("div", { className: "catalog-custom-grid", role: "list", children: models.map((m) => (_jsxs("button", { type: "button", role: "listitem", className: "catalog-custom-card", onClick: () => onSelect?.(m), "data-testid": "custom-model-card", children: [_jsx("span", { className: "catalog-custom-badge", "data-testid": "custom-model-badge", children: t("screens.catalog.custom.badge") }), _jsx("span", { className: "catalog-custom-name", children: m.id }), _jsx("span", { className: "catalog-custom-meta", children: t("screens.catalog.custom.basedOn", { base: m.base_model }) }), _jsx("span", { className: "catalog-custom-meta", children: t("screens.catalog.custom.quant", { quant: m.quant_type }) }), m.eval_total > 0 && (_jsx("span", { className: "catalog-custom-meta num", children: t("screens.catalog.custom.evalSummary", {
                                passed: m.eval_passed,
                                total: m.eval_total,
                            }) }))] }, m.id))) })] }));
}
