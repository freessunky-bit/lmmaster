// 에러 경계 fallback — react-error-boundary v4. Phase 1A.4.a 보강 §3.
//
// 정책: per-step 경계 + resetKeys=[step]로 transition 시 자동 reset.
// 한국어 해요체 — "문제가 생겼어요" / "다시 시도".

import { useTranslation } from "react-i18next";
import type { FallbackProps } from "react-error-boundary";

export function StepErrorFallback({ error, resetErrorBoundary }: FallbackProps) {
  const { t } = useTranslation();
  const message =
    error instanceof Error ? error.message : String(error ?? "");

  return (
    <div role="alert" className="onb-error">
      <h2 className="onb-error-title">{t("onboarding.error.title")}</h2>
      <p className="onb-error-body">{t("onboarding.error.body")}</p>
      {message && (
        <pre className="onb-error-detail" aria-label="error detail">
          {message}
        </pre>
      )}
      <div className="onb-error-actions">
        <button
          type="button"
          className="onb-button onb-button-primary"
          onClick={resetErrorBoundary}
        >
          {t("onboarding.error.retry")}
        </button>
      </div>
    </div>
  );
}
