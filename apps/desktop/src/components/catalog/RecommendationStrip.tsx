// RecommendationStrip — 추천 4슬롯 가로 스트립.
//
// 정책 (phase-2pb-catalog-ui-decision.md §4):
// - Best는 강조 보더(--primary) + glow.
// - Balanced/Lightweight/Fallback은 secondary 보더 동급.
// - Best=null이면 빈 상태 메시지.
// - 슬롯 클릭 → onSelect(modelId)로 Catalog가 그리드 highlight + scroll.

import { useTranslation } from "react-i18next";

import type { ModelEntry, Recommendation } from "../../ipc/catalog";

export interface RecommendationStripProps {
  recommendation: Recommendation | null;
  loading: boolean;
  /** id → ModelEntry 매핑 (display_name 표시용). */
  byId: Map<string, ModelEntry>;
  onSelect: (modelId: string) => void;
}

type SlotKey = "best" | "balanced" | "lightweight" | "fallback";

const SLOT_ORDER: SlotKey[] = ["best", "balanced", "lightweight", "fallback"];

export function RecommendationStrip({
  recommendation,
  loading,
  byId,
  onSelect,
}: RecommendationStripProps) {
  const { t } = useTranslation();

  if (loading) {
    return (
      <div className="catalog-rec-strip" aria-busy="true">
        <p className="catalog-rec-loading">{t("recommendation.loading")}</p>
      </div>
    );
  }

  if (!recommendation) {
    return null;
  }

  const slots: Record<SlotKey, string | null> = {
    best: recommendation.best_choice,
    balanced: recommendation.balanced_choice,
    lightweight: recommendation.lightweight_choice,
    fallback: recommendation.fallback_choice,
  };

  return (
    <section
      className="catalog-rec-strip"
      aria-labelledby="catalog-rec-title"
    >
      <h3 id="catalog-rec-title" className="catalog-rec-title">
        {t("home.recommend.title")}
      </h3>
      <div className="catalog-rec-grid" role="list">
        {SLOT_ORDER.map((key) => (
          <RecommendationSlot
            key={key}
            slot={key}
            modelId={slots[key]}
            entry={slots[key] ? byId.get(slots[key]!) : undefined}
            onSelect={onSelect}
            isBestEmpty={key === "best" && slots.best == null}
          />
        ))}
      </div>
    </section>
  );
}

interface SlotProps {
  slot: SlotKey;
  modelId: string | null;
  entry: ModelEntry | undefined;
  onSelect: (modelId: string) => void;
  isBestEmpty: boolean;
}

function RecommendationSlot({
  slot,
  modelId,
  entry,
  onSelect,
  isBestEmpty,
}: SlotProps) {
  const { t } = useTranslation();

  if (isBestEmpty) {
    return (
      <div className="catalog-rec-slot is-best is-empty" role="listitem">
        <span className="catalog-rec-slot-label">
          {t(`recommendation.${slot}.label`)}
        </span>
        <p className="catalog-rec-slot-empty">
          {t("recommendation.empty.best")}
        </p>
      </div>
    );
  }

  if (!modelId || !entry) {
    return (
      <div className={`catalog-rec-slot is-${slot} is-empty`} role="listitem">
        <span className="catalog-rec-slot-label">
          {t(`recommendation.${slot}.label`)}
        </span>
        <p className="catalog-rec-slot-empty">{t("recommendation.empty.slot")}</p>
      </div>
    );
  }

  return (
    <button
      type="button"
      className={`catalog-rec-slot is-${slot}`}
      role="listitem"
      onClick={() => onSelect(modelId)}
    >
      <span className="catalog-rec-slot-label">
        {t(`recommendation.${slot}.label`)}
      </span>
      <span className="catalog-rec-slot-name">{entry.display_name}</span>
    </button>
  );
}
