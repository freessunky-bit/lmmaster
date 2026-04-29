// Step 4 — 완료. Phase 1A.4.a.
//
// 머신은 이미 final 상태에 도달 — 본 컴포넌트는 사용자에게 완료 메시지 + CTA 노출.
// CTA 클릭 시 OnboardingApp이 onComplete 콜백을 호출 → markCompleted → MainShell 전환.

import { useTranslation } from "react-i18next";

export function Step4Done({ onFinish }: { onFinish: () => void }) {
  const { t } = useTranslation();

  return (
    <section className="onb-step onb-step-done" aria-labelledby="onb-step4-title">
      <div className="onb-done-mark" aria-hidden>
        ✓
      </div>
      <header className="onb-step-header">
        <h1 id="onb-step4-title" className="onb-step-title">
          {t("onboarding.done.title")}
        </h1>
        <p className="onb-step-subtitle">{t("onboarding.done.subtitle")}</p>
      </header>

      <div className="onb-step-actions">
        <button
          type="button"
          className="onb-button onb-button-primary onb-button-wide"
          onClick={onFinish}
          autoFocus
        >
          {t("onboarding.done.cta")}
        </button>
      </div>
    </section>
  );
}
