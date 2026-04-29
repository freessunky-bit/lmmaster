// Step 1 — 언어 선택 (ko/en). Phase 1A.4.a.
//
// 동작:
// - 라디오 그룹으로 ko/en 선택. 선택 즉시 i18n.changeLanguage + 머신 SET_LANG.
// - "계속할게요" 클릭 → NEXT.
// - 선택은 미리 채워져 있음 (initial context.lang = 'ko').

import { useTranslation } from "react-i18next";

import { useOnboardingLang, useOnboardingSend } from "../context";

const OPTIONS = [
  { id: "ko", labelKey: "onboarding.language.option.ko" },
  { id: "en", labelKey: "onboarding.language.option.en" },
] as const;

export function Step1Language() {
  const { t, i18n } = useTranslation();
  const lang = useOnboardingLang();
  const send = useOnboardingSend();

  const choose = (next: "ko" | "en") => {
    if (next !== lang) {
      void i18n.changeLanguage(next);
      send({ type: "SET_LANG", lang: next });
    }
  };

  return (
    <section className="onb-step" aria-labelledby="onb-step1-title">
      <header className="onb-step-header">
        <h1 id="onb-step1-title" className="onb-step-title">
          {t("onboarding.language.title")}
        </h1>
        <p className="onb-step-subtitle">
          {t("onboarding.language.subtitle")}
        </p>
      </header>

      <div
        className="onb-radio-group"
        role="radiogroup"
        aria-label={t("onboarding.language.title") ?? undefined}
      >
        {OPTIONS.map((opt) => {
          const active = lang === opt.id;
          return (
            <button
              key={opt.id}
              type="button"
              role="radio"
              aria-checked={active}
              className={`onb-radio${active ? " is-active" : ""}`}
              onClick={() => choose(opt.id)}
            >
              <span className="onb-radio-dot" aria-hidden />
              <span className="onb-radio-label">{t(opt.labelKey)}</span>
            </button>
          );
        })}
      </div>

      <div className="onb-step-actions">
        <button
          type="button"
          className="onb-button onb-button-primary"
          onClick={() => send({ type: "NEXT" })}
        >
          {t("onboarding.actions.next")}
        </button>
      </div>
    </section>
  );
}
