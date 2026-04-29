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

import {
  matchSection,
  parseSections,
  renderMarkdown,
  type MarkdownSection,
} from "../components/_render-markdown";

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
] as const;

type SectionId = (typeof SECTION_IDS)[number];

/** 섹션 → 페이지 이동 매핑. CTA "이 기능 사용해 보기"가 dispatch하는 nav 키. */
const SECTION_NAV_MAP: Partial<Record<SectionId, string>> = {
  catalog: "catalog",
  workbench: "workbench",
  knowledge: "workspace",
  "api-keys": "keys",
  portable: "settings",
  diagnostics: "diagnostics",
};

/** 검색 키워드 — i18n 명시 cheat (한국어 잘 매칭되도록). */
const SECTION_KEYWORDS: Record<SectionId, string[]> = {
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
function readSectionFromUrl(): SectionId {
  if (typeof globalThis.window === "undefined") return SECTION_IDS[0];
  try {
    const params = new URLSearchParams(globalThis.window.location?.search ?? "");
    const fromQuery = params.get("section");
    if (fromQuery && (SECTION_IDS as readonly string[]).includes(fromQuery)) {
      return fromQuery as SectionId;
    }
    const hash = globalThis.window.location?.hash ?? "";
    const fromHash = hash.replace(/^#/, "");
    if (fromHash && (SECTION_IDS as readonly string[]).includes(fromHash)) {
      return fromHash as SectionId;
    }
  } catch {
    /* noop */
  }
  return SECTION_IDS[0];
}

interface GuideEvent {
  /** 외부에서 deep link로 진입할 때 dispatch하는 custom event detail. */
  section?: SectionId;
}

const GUIDE_OPEN_EVENT = "lmmaster:guide:open";

export interface GuideProps {
  /** 테스트/외부 호출용 — props로 초기 섹션 강제. */
  initialSection?: SectionId;
}

export function Guide({ initialSection }: GuideProps = {}) {
  const { t, i18n } = useTranslation();
  const lang = (i18n.resolvedLanguage ?? "ko").startsWith("en") ? "en" : "ko";

  // 마크다운 본문은 언어별 — 한 번 파싱해 캐시.
  const sections = useMemo<MarkdownSection[]>(
    () => parseSections(lang === "en" ? guideEn : guideKo),
    [lang],
  );

  const [activeId, setActiveId] = useState<SectionId>(
    () => initialSection ?? readSectionFromUrl(),
  );
  const [query, setQuery] = useState("");

  // 외부 dispatch — HelpButton 등이 사용.
  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent<GuideEvent>).detail;
      if (detail?.section && (SECTION_IDS as readonly string[]).includes(detail.section)) {
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
      const id = s.id as SectionId;
      const keywords = SECTION_KEYWORDS[id] ?? [];
      return matchSection(s, keywords, query);
    });
  }, [sections, query]);

  const activeSection = useMemo(
    () => sections.find((s) => s.id === activeId) ?? sections[0] ?? null,
    [sections, activeId],
  );

  const activeHtml = useMemo(
    () => (activeSection ? renderMarkdown(activeSection.body) : ""),
    [activeSection],
  );

  const handleTry = useCallback(() => {
    if (!activeSection) return;
    const navKey = SECTION_NAV_MAP[activeSection.id as SectionId];
    if (!navKey) return;
    try {
      globalThis.window?.dispatchEvent(
        new CustomEvent("lmmaster:navigate", { detail: navKey }),
      );
    } catch {
      /* noop — environments without CustomEvent */
    }
  }, [activeSection]);

  return (
    <div className="guide-root" data-testid="guide-page">
      <header className="guide-topbar">
        <div>
          <h2 className="guide-page-title">{t("screens.guide.title")}</h2>
          <p className="guide-page-subtitle">
            {t("screens.guide.subtitle")}
          </p>
        </div>
      </header>

      <div className="guide-shell">
        <aside
          className="guide-sidebar"
          aria-labelledby="guide-sidebar-heading"
        >
          <h3 id="guide-sidebar-heading" className="guide-sidebar-heading">
            {t("screens.guide.sectionsHeading")}
          </h3>
          <input
            type="search"
            className="guide-search"
            placeholder={t("screens.guide.searchPlaceholder") ?? undefined}
            aria-label={t("screens.guide.searchPlaceholder") ?? undefined}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            data-testid="guide-search"
          />
          {filteredSections.length === 0 ? (
            <p className="guide-empty" data-testid="guide-no-results">
              {t("screens.guide.noResults")}
            </p>
          ) : (
            <nav
              className="guide-section-nav"
              aria-label={t("screens.guide.sectionsHeading") ?? undefined}
            >
              {filteredSections.map((s) => {
                const id = s.id as SectionId;
                return (
                  <button
                    key={s.id}
                    type="button"
                    className={`guide-section-item${
                      s.id === activeId ? " is-active" : ""
                    }`}
                    aria-current={s.id === activeId ? "page" : undefined}
                    onClick={() => setActiveId(id)}
                    data-testid={`guide-section-${s.id}`}
                  >
                    {t(`screens.guide.sections.${s.id}`, s.title)}
                  </button>
                );
              })}
            </nav>
          )}
        </aside>

        <main
          className="guide-main"
          aria-labelledby="guide-active-title"
          data-testid="guide-main"
        >
          {activeSection ? (
            <article className="guide-article">
              <h3
                id="guide-active-title"
                className="guide-article-title"
                data-testid={`guide-active-${activeSection.id}`}
              >
                {t(
                  `screens.guide.sections.${activeSection.id}`,
                  activeSection.title,
                )}
              </h3>
              <div
                className="guide-article-body"
                // 본 마크다운은 우리가 만든 파일이라 신뢰. user-input 아님.
                dangerouslySetInnerHTML={{ __html: activeHtml }}
              />
              {SECTION_NAV_MAP[activeSection.id as SectionId] && (
                <div className="guide-article-cta">
                  <button
                    type="button"
                    className="guide-cta-button"
                    onClick={handleTry}
                    data-testid="guide-cta-try"
                  >
                    {t("screens.guide.tryAction")}
                  </button>
                </div>
              )}
            </article>
          ) : (
            <p
              className="guide-empty"
              data-testid="guide-no-section"
              role="status"
            >
              {t("screens.guide.noResults")}
            </p>
          )}
        </main>
      </div>
    </div>
  );
}
