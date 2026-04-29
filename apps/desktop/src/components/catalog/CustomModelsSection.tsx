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

import type { CustomModel } from "../../ipc/workbench";
import { listCustomModels } from "../../ipc/workbench";
import { useActiveWorkspaceOptional } from "../../contexts/ActiveWorkspaceContext";

const NAV_EVENT = "lmmaster:navigate";

export interface CustomModelsSectionProps {
  /**
   * 카드 클릭 시 호출 — Catalog 페이지가 ModelDetailDrawer에 흘려보냄.
   * `null` 전달 시 호출자는 ModelEntry placeholder를 만들어야 함.
   */
  onSelect?(model: CustomModel): void;
}

/**
 * Workbench가 등록한 모델 섹션 — 추천 strip 아래 / 그리드 위.
 *
 * 데이터:
 * - 마운트 시 listCustomModels() 호출.
 * - active workspace.id 변경 시 refetch (Phase 8'.1 multi-workspace 흐름).
 * - 실패는 console.warn — UI는 빈 상태로.
 */
export function CustomModelsSection({ onSelect }: CustomModelsSectionProps) {
  const { t } = useTranslation();
  const ws = useActiveWorkspaceOptional();
  const activeId = ws?.active?.id ?? null;
  const [models, setModels] = useState<CustomModel[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    listCustomModels()
      .then((list) => {
        if (cancelled) return;
        setModels(list);
        setLoading(false);
      })
      .catch((e) => {
        if (cancelled) return;
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
    window.dispatchEvent(
      new CustomEvent(NAV_EVENT, { detail: "workbench" }),
    );
  };

  if (loading) {
    return (
      <section className="catalog-custom-section" data-testid="custom-models-section">
        <header className="catalog-custom-header">
          <h3 className="catalog-rec-title">
            {t("screens.catalog.custom.title")}
          </h3>
        </header>
        <p className="catalog-rec-loading">
          {t("screens.catalog.custom.loading")}
        </p>
      </section>
    );
  }

  if (models.length === 0) {
    return (
      <section className="catalog-custom-section" data-testid="custom-models-section">
        <header className="catalog-custom-header">
          <h3 className="catalog-rec-title">
            {t("screens.catalog.custom.title")}
          </h3>
        </header>
        <div className="catalog-custom-empty" data-testid="custom-models-empty">
          <p className="catalog-rec-slot-empty">
            {t("screens.catalog.custom.empty")}
          </p>
          <button
            type="button"
            className="onb-button onb-button-ghost"
            onClick={goWorkbench}
            data-testid="custom-models-empty-cta"
          >
            {t("screens.catalog.custom.openWorkbench")}
          </button>
        </div>
      </section>
    );
  }

  return (
    <section className="catalog-custom-section" data-testid="custom-models-section">
      <header className="catalog-custom-header">
        <h3 className="catalog-rec-title">
          {t("screens.catalog.custom.title")}
        </h3>
        <p className="catalog-page-subtitle">
          {t("screens.catalog.custom.subtitle")}
        </p>
      </header>
      <div className="catalog-custom-grid" role="list">
        {models.map((m) => (
          <button
            key={m.id}
            type="button"
            role="listitem"
            className="catalog-custom-card"
            onClick={() => onSelect?.(m)}
            data-testid="custom-model-card"
          >
            <span className="catalog-custom-badge" data-testid="custom-model-badge">
              {t("screens.catalog.custom.badge")}
            </span>
            <span className="catalog-custom-name">{m.id}</span>
            <span className="catalog-custom-meta">
              {t("screens.catalog.custom.basedOn", { base: m.base_model })}
            </span>
            <span className="catalog-custom-meta">
              {t("screens.catalog.custom.quant", { quant: m.quant_type })}
            </span>
            {m.eval_total > 0 && (
              <span className="catalog-custom-meta num">
                {t("screens.catalog.custom.evalSummary", {
                  passed: m.eval_passed,
                  total: m.eval_total,
                })}
              </span>
            )}
          </button>
        ))}
      </div>
    </section>
  );
}
