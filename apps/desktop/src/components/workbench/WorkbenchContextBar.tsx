// WorkbenchContextBar — Phase 12'.a (ADR-0050) Catalog → Workbench 컨텍스트 표시.
//
// 정책:
// - URL hash가 있을 때만 Workbench 페이지 상단에 노출 — 기존 사용자(hash 없는 진입)는 0 영향.
// - 의도 + 모델 컨텍스트를 사용자에게 명시 + "변경" 버튼으로 Catalog 복귀.
// - a11y region role + aria-labelledby.

import { useTranslation } from "react-i18next";

import type { IntentId } from "../../ipc/catalog";

interface WorkbenchContextBarProps {
  modelDisplayName: string | null;
  intent: IntentId | null;
  /** "의도 변경" 클릭 — 보통 Catalog로 라우팅. */
  onChangeIntent?: () => void;
  /** "모델 변경" 클릭 — 보통 Catalog로 라우팅. */
  onChangeModel?: () => void;
}

const INTENT_LABELS_KO: Record<IntentId, string> = {
  "vision-image": "이미지 분석",
  "vision-multimodal": "이미지+텍스트 멀티모달",
  "translation-ko-en": "한↔영 번역",
  "translation-multi": "다국어 번역",
  "coding-general": "코딩",
  "coding-fim": "코드 자동완성 (FIM)",
  "agent-tool-use": "에이전트 / 도구 사용",
  "roleplay-narrative": "롤플레이 / 서사",
  "ko-conversation": "한국어 대화",
  "ko-rag": "한국어 RAG",
  "voice-stt": "음성 인식",
};

export function WorkbenchContextBar({
  modelDisplayName,
  intent,
  onChangeIntent,
  onChangeModel,
}: WorkbenchContextBarProps) {
  const { t } = useTranslation();
  const intentLabel = intent
    ? t(`catalog.intent.${intent}`, INTENT_LABELS_KO[intent])
    : null;

  return (
    <section
      className="workbench-context-bar"
      role="region"
      aria-labelledby="workbench-context-heading"
      data-testid="workbench-context-bar"
    >
      <h3
        id="workbench-context-heading"
        className="workbench-context-heading"
      >
        {t("screens.workbench.context.heading", "지금 작업 중인 컨텍스트")}
      </h3>
      <div className="workbench-context-row">
        {intentLabel && (
          <div
            className="workbench-context-chip"
            data-testid="workbench-context-intent"
          >
            <span className="workbench-context-chip-label">
              {t("screens.workbench.context.intentLabel", "의도")}
            </span>
            <span className="workbench-context-chip-value">{intentLabel}</span>
            {onChangeIntent && (
              <button
                type="button"
                className="workbench-context-change"
                onClick={onChangeIntent}
                data-testid="workbench-context-change-intent"
              >
                {t("screens.workbench.context.change", "변경")}
              </button>
            )}
          </div>
        )}
        {modelDisplayName && (
          <div
            className="workbench-context-chip"
            data-testid="workbench-context-model"
          >
            <span className="workbench-context-chip-label">
              {t("screens.workbench.context.modelLabel", "모델")}
            </span>
            <span className="workbench-context-chip-value num">
              {modelDisplayName}
            </span>
            {onChangeModel && (
              <button
                type="button"
                className="workbench-context-change"
                onClick={onChangeModel}
                data-testid="workbench-context-change-model"
              >
                {t("screens.workbench.context.change", "변경")}
              </button>
            )}
          </div>
        )}
      </div>
    </section>
  );
}
