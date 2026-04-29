import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// Guide — Phase 12'.a. NAV "가이드" 페이지.
//
// 정책 (phase-8p-9p-10p-residual-plan.md §1.9):
// - 8 섹션 (시작하기 / 카탈로그 / 워크벤치 / 자료 인덱싱 / API 키 / 포터블 / 진단 / FAQ).
// - 좌측 sidebar — 섹션 목록 + 검색. 검색은 substring + jamo cheat (Command Palette 패턴).
// - 우측 본문 — 마크다운 렌더 + "이 기능 사용해 보기" CTA (lmmaster:navigate dispatch).
// - deep link: ?section=workbench URL hash로 진입 — HelpButton에서 사용.
// - i18n: ko / en 마크다운 동시 갱신 (guide-{ko,en}-v1.md).
// - markdown renderer는 _render-markdown.ts에서 공유 (EulaGate와 함께).
import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { matchSection, parseSections, renderMarkdown, } from "../components/_render-markdown";
import guideKo from "../i18n/guide-ko-v1.md?raw";
import guideEn from "../i18n/guide-en-v1.md?raw";
import "./guide.css";
/** 8 섹션 — 마크다운 마커와 일치해야 해요 (`<!-- section: id -->`). */
const SECTION_IDS = [
    "getting-started",
    "catalog",
    "workbench",
    "knowledge",
    "api-keys",
    "portable",
    "diagnostics",
    "faq",
];
/** 섹션 → 페이지 이동 매핑. CTA "이 기능 사용해 보기"가 dispatch하는 nav 키. */
const SECTION_NAV_MAP = {
    catalog: "catalog",
    workbench: "workbench",
    knowledge: "workspace",
    "api-keys": "keys",
    portable: "settings",
    diagnostics: "diagnostics",
};
/** 검색 키워드 — i18n 명시 cheat (한국어 잘 매칭되도록). */
const SECTION_KEYWORDS = {
    "getting-started": ["시작", "마법사", "wizard", "onboarding", "ㅅㅈ"],
    catalog: ["카탈로그", "추천", "모델", "ㅁㄷ", "model"],
    workbench: ["워크벤치", "양자화", "lora", "training", "fine-tune", "ㅇㅈㅎ"],
    knowledge: ["RAG", "지식", "자료", "검색", "ingest", "ㅈㄹ"],
    "api-keys": ["API", "키", "외부", "웹앱", "external", "ㅋ"],
    portable: ["포터블", "이동", "내보내기", "가져오기", "export", "import"],
    diagnostics: ["진단", "자가 점검", "갱신", "업데이트", "ㅈㄷ"],
    faq: ["FAQ", "자주", "묻는", "단축키", "ㅈㄷㄴ"],
};
/** URL ?section=...에서 초기 섹션 추출. invalid면 첫 섹션. */
function readSectionFromUrl() {
    if (typeof globalThis.window === "undefined")
        return SECTION_IDS[0];
    try {
        const params = new URLSearchParams(globalThis.window.location?.search ?? "");
        const fromQuery = params.get("section");
        if (fromQuery && SECTION_IDS.includes(fromQuery)) {
            return fromQuery;
        }
        const hash = globalThis.window.location?.hash ?? "";
        const fromHash = hash.replace(/^#/, "");
        if (fromHash && SECTION_IDS.includes(fromHash)) {
            return fromHash;
        }
    }
    catch {
        /* noop */
    }
    return SECTION_IDS[0];
}
const GUIDE_OPEN_EVENT = "lmmaster:guide:open";
export function Guide({ initialSection } = {}) {
    const { t, i18n } = useTranslation();
    const lang = (i18n.resolvedLanguage ?? "ko").startsWith("en") ? "en" : "ko";
    // 마크다운 본문은 언어별 — 한 번 파싱해 캐시.
    const sections = useMemo(() => parseSections(lang === "en" ? guideEn : guideKo), [lang]);
    const [activeId, setActiveId] = useState(() => initialSection ?? readSectionFromUrl());
    const [query, setQuery] = useState("");
    // 외부 dispatch — HelpButton 등이 사용.
    useEffect(() => {
        const handler = (e) => {
            const detail = e.detail;
            if (detail?.section && SECTION_IDS.includes(detail.section)) {
                setActiveId(detail.section);
            }
        };
        globalThis.window?.addEventListener(GUIDE_OPEN_EVENT, handler);
        return () => {
            globalThis.window?.removeEventListener(GUIDE_OPEN_EVENT, handler);
        };
    }, []);
    // initialSection prop 변화 시 동기화 (테스트 안정성).
    useEffect(() => {
        if (initialSection) {
            setActiveId(initialSection);
        }
    }, [initialSection]);
    const filteredSections = useMemo(() => {
        return sections.filter((s) => {
            const id = s.id;
            const keywords = SECTION_KEYWORDS[id] ?? [];
            return matchSection(s, keywords, query);
        });
    }, [sections, query]);
    const activeSection = useMemo(() => sections.find((s) => s.id === activeId) ?? sections[0] ?? null, [sections, activeId]);
    const activeHtml = useMemo(() => (activeSection ? renderMarkdown(activeSection.body) : ""), [activeSection]);
    const handleTry = useCallback(() => {
        if (!activeSection)
            return;
        const navKey = SECTION_NAV_MAP[activeSection.id];
        if (!navKey)
            return;
        try {
            globalThis.window?.dispatchEvent(new CustomEvent("lmmaster:navigate", { detail: navKey }));
        }
        catch {
            /* noop — environments without CustomEvent */
        }
    }, [activeSection]);
    return (_jsxs("div", { className: "guide-root", "data-testid": "guide-page", children: [_jsx("header", { className: "guide-topbar", children: _jsxs("div", { children: [_jsx("h2", { className: "guide-page-title", children: t("screens.guide.title") }), _jsx("p", { className: "guide-page-subtitle", children: t("screens.guide.subtitle") })] }) }), _jsxs("div", { className: "guide-shell", children: [_jsxs("aside", { className: "guide-sidebar", "aria-labelledby": "guide-sidebar-heading", children: [_jsx("h3", { id: "guide-sidebar-heading", className: "guide-sidebar-heading", children: t("screens.guide.sectionsHeading") }), _jsx("input", { type: "search", className: "guide-search", placeholder: t("screens.guide.searchPlaceholder") ?? undefined, "aria-label": t("screens.guide.searchPlaceholder") ?? undefined, value: query, onChange: (e) => setQuery(e.target.value), "data-testid": "guide-search" }), filteredSections.length === 0 ? (_jsx("p", { className: "guide-empty", "data-testid": "guide-no-results", children: t("screens.guide.noResults") })) : (_jsx("nav", { className: "guide-section-nav", "aria-label": t("screens.guide.sectionsHeading") ?? undefined, children: filteredSections.map((s) => {
                                    const id = s.id;
                                    return (_jsx("button", { type: "button", className: `guide-section-item${s.id === activeId ? " is-active" : ""}`, "aria-current": s.id === activeId ? "page" : undefined, onClick: () => setActiveId(id), "data-testid": `guide-section-${s.id}`, children: t(`screens.guide.sections.${s.id}`, s.title) }, s.id));
                                }) }))] }), _jsx("main", { className: "guide-main", "aria-labelledby": "guide-active-title", "data-testid": "guide-main", children: activeSection ? (_jsxs("article", { className: "guide-article", children: [_jsx("h3", { id: "guide-active-title", className: "guide-article-title", "data-testid": `guide-active-${activeSection.id}`, children: t(`screens.guide.sections.${activeSection.id}`, activeSection.title) }), _jsx("div", { className: "guide-article-body", 
                                    // 본 마크다운은 우리가 만든 파일이라 신뢰. user-input 아님.
                                    dangerouslySetInnerHTML: { __html: activeHtml } }), SECTION_NAV_MAP[activeSection.id] && (_jsx("div", { className: "guide-article-cta", children: _jsx("button", { type: "button", className: "guide-cta-button", onClick: handleTry, "data-testid": "guide-cta-try", children: t("screens.guide.tryAction") }) }))] })) : (_jsx("p", { className: "guide-empty", "data-testid": "guide-no-section", role: "status", children: t("screens.guide.noResults") })) })] })] }));
}
