import { Fragment as _Fragment, jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// EulaGate — Phase 7'.a. 첫 실행 EULA 동의 게이트.
//
// 정책 (ADR-0027 §4, phase-7p-release-prep-reinforcement.md §4):
// - 사용자가 동의하기 전에는 onboarding / main 화면에 진입할 수 없어요.
// - 동의는 version-bound localStorage 키(`lmmaster.eula.accepted.<version>`)에 영속.
// - patch 버전 갱신은 자동 동의(상위 호출자 책임), minor/major 갱신은 새 동의 필요.
// - 다크 패턴 회피: "동의할게요" 버튼은 사용자가 본문을 끝까지 스크롤하기 전엔 비활성.
// - a11y: role="dialog" + aria-modal=true + aria-labelledby. focus는 close 버튼 자동.
// - 한국어 / English 토글 — i18n.resolvedLanguage default. Esc 닫기 X (accept만 가능).
// - prefers-reduced-motion 존중 — 토큰만 사용.
import { useCallback, useEffect, useMemo, useRef, useState, } from "react";
import { useTranslation } from "react-i18next";
import eulaKo from "../i18n/eula-ko-v1.md?raw";
import eulaEn from "../i18n/eula-en-v1.md?raw";
import { renderMarkdown as sharedRenderMarkdown } from "./_render-markdown";
import "./eulaGate.css";
const LOCAL_STORAGE_PREFIX = "lmmaster.eula.accepted.";
/** 본문 스크롤이 끝까지 도달했는지 판정하는 여유 (px). */
const SCROLL_END_THRESHOLD = 16;
/** localStorage 키 — version-bound. */
function storageKey(version) {
    return `${LOCAL_STORAGE_PREFIX}${version}`;
}
/** 이미 동의한 버전인지 판정. SSR / storage 접근 실패 시 false. */
function isAlreadyAccepted(version) {
    try {
        return globalThis.localStorage?.getItem(storageKey(version)) === "true";
    }
    catch {
        return false;
    }
}
/** 동의 영속 — 실패하면 silent. 다음 실행 시 dialog가 다시 보여드릴 거예요. */
function persistAcceptance(version) {
    try {
        globalThis.localStorage?.setItem(storageKey(version), "true");
    }
    catch {
        /* noop */
    }
}
export function EulaGate({ eulaVersion, onAccept, children }) {
    const { t, i18n } = useTranslation();
    const [accepted, setAccepted] = useState(() => isAlreadyAccepted(eulaVersion));
    const [locale, setLocale] = useState(() => {
        const lng = i18n.resolvedLanguage ?? "ko";
        return lng.startsWith("en") ? "en" : "ko";
    });
    const [scrolledToEnd, setScrolledToEnd] = useState(false);
    const [confirmingDecline, setConfirmingDecline] = useState(false);
    const closeBtnRef = useRef(null);
    const bodyRef = useRef(null);
    const dialogRef = useRef(null);
    // 첫 마운트 시 close 버튼 focus — accept 모달 진입 시.
    useEffect(() => {
        if (!accepted && closeBtnRef.current) {
            closeBtnRef.current.focus();
        }
    }, [accepted]);
    // 언어 토글 시 스크롤 상태 reset — 새 본문을 다시 끝까지 읽도록.
    useEffect(() => {
        setScrolledToEnd(false);
        const el = bodyRef.current;
        if (el) {
            // jsdom 등 일부 환경에서는 scrollTop이 read-only일 수 있어요 — best-effort.
            try {
                el.scrollTop = 0;
            }
            catch {
                /* noop */
            }
        }
    }, [locale]);
    const handleScroll = useCallback(() => {
        const el = bodyRef.current;
        if (!el)
            return;
        const remaining = el.scrollHeight - el.scrollTop - el.clientHeight;
        // scrollHeight가 0이면 layout이 아직 측정 안 된 상태 — 무시.
        if (el.scrollHeight > 0 && remaining <= SCROLL_END_THRESHOLD) {
            setScrolledToEnd(true);
        }
    }, []);
    // 본문이 (실제로 layout이 측정된 후) 스크롤이 필요 없을 정도로 짧으면 즉시 활성.
    // jsdom처럼 scrollHeight=0인 환경에서는 자동 활성하지 않아요 — 명시적 scroll 시뮬레이션 필요.
    useEffect(() => {
        const el = bodyRef.current;
        if (!el)
            return;
        // 다음 frame에 측정 — 첫 렌더 직후 scrollHeight가 0인 케이스를 우회.
        const id = globalThis.requestAnimationFrame?.(() => {
            if (el.scrollHeight > 0 &&
                el.scrollHeight - el.clientHeight <= SCROLL_END_THRESHOLD) {
                setScrolledToEnd(true);
            }
        });
        return () => {
            if (id !== undefined) {
                globalThis.cancelAnimationFrame?.(id);
            }
        };
    }, [locale, accepted]);
    const handleAccept = useCallback(() => {
        if (!scrolledToEnd)
            return;
        persistAcceptance(eulaVersion);
        setAccepted(true);
        onAccept();
    }, [eulaVersion, onAccept, scrolledToEnd]);
    const handleDeclineRequest = useCallback(() => {
        setConfirmingDecline(true);
    }, []);
    const handleDeclineCancel = useCallback(() => {
        setConfirmingDecline(false);
    }, []);
    const handleDeclineExit = useCallback(() => {
        // 거절 = 앱 종료 의도. 브라우저 / Tauri 둘 다에서 best-effort.
        try {
            globalThis.window?.close();
        }
        catch {
            /* noop */
        }
    }, []);
    const markdown = locale === "ko" ? eulaKo : eulaEn;
    const html = useMemo(() => sharedRenderMarkdown(markdown), [markdown]);
    // Focus trap — Tab이 dialog 밖으로 나가지 못하도록.
    const handleKeyDown = useCallback((e) => {
        if (e.key !== "Tab")
            return;
        const root = dialogRef.current;
        if (!root)
            return;
        const focusable = root.querySelectorAll('button, [href], input, [tabindex]:not([tabindex="-1"])');
        if (focusable.length === 0)
            return;
        const first = focusable.item(0);
        const last = focusable.item(focusable.length - 1);
        if (!first || !last)
            return;
        const active = document.activeElement;
        if (e.shiftKey && active === first) {
            e.preventDefault();
            last.focus();
        }
        else if (!e.shiftKey && active === last) {
            e.preventDefault();
            first.focus();
        }
    }, []);
    if (accepted) {
        return _jsx(_Fragment, { children: children });
    }
    return (_jsxs("div", { className: "eula-gate-overlay", role: "dialog", "aria-modal": "true", "aria-labelledby": "eula-title", "data-testid": "eula-gate-dialog", onKeyDown: handleKeyDown, ref: dialogRef, children: [_jsxs("div", { className: "eula-gate-card", children: [_jsxs("header", { className: "eula-gate-header", children: [_jsx("h1", { id: "eula-title", className: "eula-gate-title", children: t("screens.eula.title") }), _jsxs("div", { className: "eula-gate-langs", role: "radiogroup", "aria-label": "locale", children: [_jsx("button", { type: "button", role: "radio", "aria-checked": locale === "ko", className: `eula-gate-lang${locale === "ko" ? " is-active" : ""}`, onClick: () => setLocale("ko"), "data-testid": "eula-lang-ko", ref: closeBtnRef, children: t("screens.eula.lang.ko") }), _jsx("button", { type: "button", role: "radio", "aria-checked": locale === "en", className: `eula-gate-lang${locale === "en" ? " is-active" : ""}`, onClick: () => setLocale("en"), "data-testid": "eula-lang-en", children: t("screens.eula.lang.en") })] })] }), _jsx("p", { className: "eula-gate-version", children: t("screens.eula.version", { version: eulaVersion }) }), _jsx("div", { className: "eula-gate-body", ref: bodyRef, onScroll: handleScroll, tabIndex: 0, "aria-label": t("screens.eula.title"), "data-testid": "eula-body", 
                        // 본 마크다운은 우리가 만든 파일이라 신뢰. user-input 아님.
                        dangerouslySetInnerHTML: { __html: html } }), !scrolledToEnd && (_jsx("p", { className: "eula-gate-scroll-hint", role: "status", "data-testid": "eula-scroll-hint", children: t("screens.eula.scrollToAccept") })), _jsxs("footer", { className: "eula-gate-footer", children: [_jsx("button", { type: "button", className: "eula-gate-decline", onClick: handleDeclineRequest, "data-testid": "eula-decline", children: t("screens.eula.decline") }), _jsx("button", { type: "button", className: "eula-gate-accept", onClick: handleAccept, disabled: !scrolledToEnd, "data-testid": "eula-accept", children: t("screens.eula.accept") })] })] }), confirmingDecline && (_jsx("div", { className: "eula-gate-confirm", role: "dialog", "aria-modal": "true", "aria-labelledby": "eula-decline-title", "data-testid": "eula-decline-confirm", children: _jsxs("div", { className: "eula-gate-confirm-card", children: [_jsx("h2", { id: "eula-decline-title", className: "eula-gate-confirm-title", children: t("screens.eula.decline.confirmTitle") }), _jsx("p", { className: "eula-gate-confirm-body", children: t("screens.eula.decline.confirmBody") }), _jsxs("div", { className: "eula-gate-confirm-actions", children: [_jsx("button", { type: "button", className: "eula-gate-decline", onClick: handleDeclineCancel, "data-testid": "eula-decline-cancel", children: t("screens.eula.decline.cancel") }), _jsx("button", { type: "button", className: "eula-gate-accept", onClick: handleDeclineExit, "data-testid": "eula-decline-exit", children: t("screens.eula.decline.exit") })] })] }) }))] }));
}
// ───────────────────────────────────────────────────────────────────
// Markdown renderer — Phase 12'.a에서 _render-markdown.ts로 추출.
// 본 모듈은 후방 호환을 위해 shared renderer를 re-export해요.
// ───────────────────────────────────────────────────────────────────
export const renderMarkdown = sharedRenderMarkdown;
