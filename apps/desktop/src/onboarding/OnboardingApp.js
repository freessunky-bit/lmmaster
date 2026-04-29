import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// 마법사 root — Steps + AnimatePresence + per-step ErrorBoundary.
//
// 정책 (Phase 1A.4.a 보강 §2, §3, §6):
// - Ark UI Steps headless. step 인덱스는 머신 value로부터 derive — xstate가 진실원.
// - per-step <ErrorBoundary resetKeys={[step]}> — Step 2 실패가 Step 1을 죽이지 않게.
// - <MotionConfig reducedMotion="user">로 prefers-reduced-motion 자동 반영.
// - done(final) 도달 → onComplete 호출 (caller가 markCompleted + 전환).
import { useEffect, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Steps } from "@ark-ui/react/steps";
import { AnimatePresence, MotionConfig, motion } from "framer-motion";
import { ErrorBoundary } from "react-error-boundary";
import { useCommandRegistration } from "../components/command-palette/context";
import { OnboardingProvider, useOnboardingDone, useOnboardingLang, useOnboardingSend, useOnboardingStep, } from "./context";
import { StepErrorFallback } from "./StepErrorFallback";
import { Step1Language } from "./steps/Step1Language";
import { Step2Scan } from "./steps/Step2Scan";
import { Step3Install } from "./steps/Step3Install";
import { Step4Done } from "./steps/Step4Done";
import "./onboarding.css";
const STEP_KEYS = ["language", "scan", "install", "done"];
const STEP_INDEX = {
    language: 0,
    scan: 1,
    install: 2,
    done: 3,
};
export function OnboardingApp({ onComplete }) {
    return (_jsx(OnboardingProvider, { children: _jsx(OnboardingShell, { onComplete: onComplete }) }));
}
function OnboardingShell({ onComplete }) {
    const { t, i18n } = useTranslation();
    const lang = useOnboardingLang();
    const step = useOnboardingStep();
    const isDone = useOnboardingDone();
    const send = useOnboardingSend();
    // 머신 lang ↔ i18n 동기화 — hydrate 직후/외부 변경 대응.
    useEffect(() => {
        if (i18n.resolvedLanguage !== lang) {
            void i18n.changeLanguage(lang);
        }
    }, [lang, i18n]);
    // 마법사 시드 명령 — 팔레트에 등록.
    const wizardCommands = useMemo(() => [
        {
            id: "wizard.lang.ko",
            group: "wizard",
            label: t("palette.cmd.wizard.lang.ko"),
            keywords: ["language", "korean", "ko", "ㅎㄱ", "한국어"],
            perform: () => {
                void i18n.changeLanguage("ko");
                send({ type: "SET_LANG", lang: "ko" });
            },
        },
        {
            id: "wizard.lang.en",
            group: "wizard",
            label: t("palette.cmd.wizard.lang.en"),
            keywords: ["english", "en", "영어"],
            perform: () => {
                void i18n.changeLanguage("en");
                send({ type: "SET_LANG", lang: "en" });
            },
        },
        {
            id: "wizard.scan.retry",
            group: "wizard",
            label: t("palette.cmd.wizard.scan.retry"),
            keywords: ["scan", "environment", "ㅎㄱㅈㄱ", "재점검"],
            isAvailable: () => step === "scan",
            perform: () => {
                send({ type: "RETRY" });
            },
        },
        {
            id: "wizard.restart",
            group: "wizard",
            label: t("palette.cmd.wizard.restart"),
            keywords: ["restart", "reset", "처음", "다시"],
            perform: () => {
                // language로 BACK 시리즈 — 가능한 단계까지.
                send({ type: "BACK" });
                send({ type: "BACK" });
                send({ type: "BACK" });
            },
        },
    ], [i18n, send, step, t]);
    useCommandRegistration(wizardCommands);
    const stepIndex = STEP_INDEX[step];
    const stepItems = useMemo(() => STEP_KEYS.map((key) => ({
        value: key,
        title: t(`onboarding.steps.${key}`),
    })), [t]);
    return (_jsxs(MotionConfig, { reducedMotion: "user", children: [_jsx("div", { className: "onb-root surface-aurora", role: "main", "aria-label": t("onboarding.aria.root") ?? undefined, children: _jsx("div", { className: "onb-card", children: _jsxs(Steps.Root, { count: STEP_KEYS.length, step: stepIndex, 
                        // step 변경은 xstate가 책임 — Ark의 onStepChange는 사용 안 함.
                        linear: true, children: [_jsx(Steps.List, { className: "onb-stepper", children: stepItems.map((item, index) => (_jsxs(Steps.Item, { index: index, className: "onb-stepper-item", children: [_jsxs(Steps.Trigger, { type: "button", disabled: true, className: "onb-stepper-trigger", "aria-current": index === stepIndex ? "step" : undefined, children: [_jsx(Steps.Indicator, { className: "onb-stepper-dot", children: index + 1 }), _jsx("span", { className: "onb-stepper-title", children: item.title })] }), index < stepItems.length - 1 && (_jsx(Steps.Separator, { className: "onb-stepper-sep" }))] }, item.value))) }), _jsx("div", { className: "onb-content", "aria-live": "polite", children: _jsx(AnimatePresence, { mode: "wait", initial: false, children: _jsx(motion.div, { className: "onb-content-inner", initial: { opacity: 0, x: 24 }, animate: { opacity: 1, x: 0 }, exit: { opacity: 0, x: -24 }, transition: { duration: 0.2, ease: [0.16, 1, 0.3, 1] }, children: _jsx(ErrorBoundary, { FallbackComponent: StepErrorFallback, resetKeys: [step], children: _jsx(CurrentStep, { step: step, onFinish: onComplete }) }) }, step) }) })] }) }) }), _jsx(DoneObserver, { isDone: isDone, onComplete: onComplete })] }));
}
function CurrentStep({ step, onFinish, }) {
    switch (step) {
        case "language":
            return _jsx(Step1Language, {});
        case "scan":
            return _jsx(Step2Scan, {});
        case "install":
            return _jsx(Step3Install, {});
        case "done":
            return _jsx(Step4Done, { onFinish: onFinish });
    }
}
/**
 * 머신이 final에 도달했지만 사용자가 CTA를 누르지 않은 경우 — 노출만 하고 자동 호출 안 함.
 * (Step4Done의 명시적 클릭이 onComplete를 호출). 이 컴포넌트는 향후 텔레메트리/로그 hook 자리.
 */
function DoneObserver({ isDone, onComplete: _onComplete, }) {
    useEffect(() => {
        if (isDone) {
            // 디버그 로그만 — 실제 전환은 Step4Done의 사용자 액션에서.
            console.debug("[onboarding] machine reached final state");
        }
    }, [isDone]);
    return null;
}
