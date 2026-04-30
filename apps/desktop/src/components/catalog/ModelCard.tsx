// ModelCard — 카탈로그 그리드의 카드 한 장.
//
// 정책 (phase-2pb-catalog-ui-decision.md):
// - 3행 정보 우선순위: (1) display_name + 배지, (2) 카테고리 + 한국어 별점 + 사용처, (3) 메트릭 + compat hint.
// - excluded 모델은 dim + reason chip — 클릭 가능하지만 install CTA 비활성.
// - 카드 자체는 SpotlightCard 위에 얹어 hover spotlight.
// - Drawer 열기는 onSelect 콜백 — Catalog가 owner.

import { useTranslation } from "react-i18next";

import type {
  ExclusionReason,
  IntentId,
  ModelEntry,
  Recommendation,
} from "../../ipc/catalog";
import { buildWorkbenchHash } from "../workbench/hash";
import { SpotlightCard } from "../SpotlightCard";

import {
  compatOf,
  formatSize,
  idOf,
  languageStars,
} from "./format";

export interface ModelCardProps {
  model: ModelEntry;
  recommendation: Recommendation | null;
  onSelect: (model: ModelEntry) => void;
  /**
   * 사용자가 선택한 의도 — Phase 11'.b (ADR-0048).
   * `null`이면 도메인 점수 바 미렌더 (기존 표시 유지). `Some`이면 해당 점수만 노출.
   */
  intent?: IntentId | null;
}

export function ModelCard({
  model,
  recommendation,
  onSelect,
  intent = null,
}: ModelCardProps) {
  const { t } = useTranslation();
  const excluded = findExcluded(model, recommendation);
  const compat = compatOf(model, recommendation);
  const isExcluded = !!excluded;
  const domainScore = pickDomainScore(model, intent);

  return (
    <SpotlightCard
      className={`catalog-card${isExcluded ? " is-excluded" : ""}`}
      role="button"
      tabIndex={0}
      onClick={() => onSelect(model)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect(model);
        }
      }}
      aria-disabled={isExcluded || undefined}
    >
      <header className="catalog-card-header">
        <div className="catalog-card-badges">
          <span
            className={`catalog-card-chip catalog-card-chip-${model.verification.tier}`}
            data-testid="verification-chip"
          >
            {t(`model.verification.${model.verification.tier}`)}
          </span>
          <span
            className={`catalog-card-chip catalog-card-chip-${model.maturity}`}
            data-testid="maturity-chip"
          >
            {t(`model.maturity.${model.maturity}`)}
          </span>
          {model.content_warning === "rp-explicit" && (
            <span
              className="catalog-card-chip catalog-card-chip-adult"
              data-testid="adult-chip"
              title={t(
                "catalog.adultContent.chipTitle",
                "성인 콘텐츠 — 필터로 노출 중",
              )}
            >
              {t("catalog.adultContent.chip", "🔞 성인")}
            </span>
          )}
          {model.commercial === false && (
            <span
              className="catalog-card-chip catalog-card-chip-noncommercial"
              data-testid="noncommercial-chip"
              title={t(
                "catalog.commercial.chipTitle",
                "비상업 라이선스 — 상업 사용 거부",
              )}
            >
              {t("catalog.commercial.chip", "⚠ 비상업")}
            </span>
          )}
        </div>
      </header>

      <h4 className="catalog-card-title">{model.display_name}</h4>

      <div className="catalog-card-meta">
        <span className="catalog-card-category">
          {t(`catalog.category.${model.category}`)}
        </span>
        <span
          className="catalog-card-stars"
          aria-label={`${t("model.metric.korean", "한국어 강도")}: ${
            model.language_strength ?? 0
          }/10`}
        >
          {languageStars(model.language_strength)}
        </span>
      </div>

      {model.use_case_examples.length > 0 && (
        <p className="catalog-card-usecase">{model.use_case_examples[0]}</p>
      )}

      {domainScore != null && intent && (
        <div
          className="catalog-card-domain-score"
          data-testid={`domain-score-${intent}`}
          aria-label={t("catalog.intent.scoreAria", {
            intent: t(`catalog.intent.${intent}`, intent),
            score: domainScore.toFixed(1),
            defaultValue: `${intent} 점수: ${domainScore.toFixed(1)} / 100`,
          })}
        >
          <span className="catalog-card-domain-score-label">
            {t(`catalog.intent.${intent}`, intent)}
          </span>
          <span className="catalog-card-domain-score-track" aria-hidden>
            <span
              className="catalog-card-domain-score-fill"
              style={{ width: `${Math.min(100, Math.max(0, domainScore))}%` }}
            />
          </span>
          <span className="catalog-card-domain-score-value num">
            {domainScore.toFixed(1)}
          </span>
        </div>
      )}

      <dl className="catalog-card-metrics">
        <div className="catalog-card-metric">
          <dt>{t("model.metric.vram")}</dt>
          <dd className="num">
            {model.rec_vram_mb == null
              ? t("model.metric.noVram")
              : formatSize(model.rec_vram_mb)}
          </dd>
        </div>
        <div className="catalog-card-metric">
          <dt>{t("model.metric.ram")}</dt>
          <dd className="num">{formatSize(model.rec_ram_mb)}</dd>
        </div>
        <div className="catalog-card-metric">
          <dt>{t("model.metric.size")}</dt>
          <dd className="num">{formatSize(model.install_size_mb)}</dd>
        </div>
      </dl>

      <div className="catalog-card-footer">
        {excluded ? (
          <span className="catalog-card-compat is-unfit" data-testid="exclude-chip">
            {excludeText(t, excluded)}
          </span>
        ) : (
          <span
            className={`catalog-card-compat is-${compat}`}
            data-testid="compat-chip"
          >
            {t(`model.compat.${compat}`)}
          </span>
        )}
        {!isExcluded && (
          <button
            type="button"
            className="catalog-card-start-btn"
            data-testid={`catalog-card-start-${model.id}`}
            onClick={(e) => {
              e.stopPropagation();
              window.location.hash = buildWorkbenchHash(model.id, intent);
            }}
            title={t(
              "catalog.card.startWorkbenchTitle",
              "Workbench에서 이 모델로 시작해 볼래요? 의도와 모델 컨텍스트가 함께 전달돼요.",
            )}
          >
            {t("catalog.card.startWorkbench", "이 모델로 시작 →")}
          </button>
        )}
      </div>
    </SpotlightCard>
  );
}

function findExcluded(
  model: ModelEntry,
  rec: Recommendation | null,
): ExclusionReason | undefined {
  if (!rec) return undefined;
  return rec.excluded.find((e) => idOf(e) === model.id);
}

/**
 * `intent`가 선택되어 있고 model이 그 점수를 보유하면 0..100 숫자 반환.
 * 미선택 또는 미보유 시 null — 기존 표시 유지.
 */
function pickDomainScore(
  model: ModelEntry,
  intent: IntentId | null,
): number | null {
  if (!intent) return null;
  const scores = model.domain_scores;
  if (!scores) return null;
  const v = scores[intent];
  return typeof v === "number" ? v : null;
}

type TFn = ReturnType<typeof useTranslation>["t"];

function excludeText(t: TFn, reason: ExclusionReason): string {
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
