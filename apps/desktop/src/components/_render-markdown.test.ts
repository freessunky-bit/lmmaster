// _render-markdown — Phase 12'.a 단위 테스트.

import { describe, expect, it } from "vitest";

import {
  escapeHtml,
  matchSection,
  parseSections,
  renderInline,
  renderMarkdown,
} from "./_render-markdown";

describe("escapeHtml", () => {
  it("HTML 엔티티 5종을 escape해요", () => {
    expect(escapeHtml("<a>")).toBe("&lt;a&gt;");
    expect(escapeHtml('"hello"')).toBe("&quot;hello&quot;");
    expect(escapeHtml("a & b")).toBe("a &amp; b");
    expect(escapeHtml("'single'")).toBe("&#39;single&#39;");
  });

  it("일반 텍스트는 그대로", () => {
    expect(escapeHtml("hello world")).toBe("hello world");
    expect(escapeHtml("한국어")).toBe("한국어");
  });
});

describe("renderInline", () => {
  it("**bold** → strong, `code` → code", () => {
    expect(renderInline("plain **bold** text")).toContain(
      "<strong>bold</strong>",
    );
    expect(renderInline("use `npm install` here")).toContain(
      "<code>npm install</code>",
    );
  });

  it("inline 처리 후에도 escape는 유지", () => {
    expect(renderInline("**a<b>**")).toContain("<strong>a&lt;b&gt;</strong>");
  });
});

describe("renderMarkdown", () => {
  it("# 제목 → h1", () => {
    const html = renderMarkdown("# 시작하기");
    expect(html).toContain("<h1>시작하기</h1>");
  });

  it("## 부제목 → h2", () => {
    const html = renderMarkdown("## 첫 단계");
    expect(html).toContain("<h2>첫 단계</h2>");
  });

  it("- 리스트 → ul/li", () => {
    const html = renderMarkdown("- 첫째\n- 둘째");
    expect(html).toContain("<ul>");
    expect(html).toContain("<li>첫째</li>");
    expect(html).toContain("<li>둘째</li>");
    expect(html).toContain("</ul>");
  });

  it("일반 텍스트 → p", () => {
    const html = renderMarkdown("이건 본문이에요");
    expect(html).toContain("<p>이건 본문이에요</p>");
  });

  it("--- 라인은 무시 (구분자)", () => {
    const html = renderMarkdown("a\n---\nb");
    expect(html).toContain("<p>a</p>");
    expect(html).toContain("<p>b</p>");
    expect(html).not.toContain("---");
  });
});

describe("parseSections", () => {
  const sample = [
    "<!-- section: getting-started -->",
    "# 시작하기",
    "",
    "안녕하세요",
    "",
    "---",
    "",
    "<!-- section: catalog -->",
    "# 카탈로그",
    "",
    "추천 strip이에요",
  ].join("\n");

  it("--- 라인으로 섹션을 나누고 마커 id를 우선 사용", () => {
    const sections = parseSections(sample);
    expect(sections.length).toBe(2);
    expect(sections[0]?.id).toBe("getting-started");
    expect(sections[1]?.id).toBe("catalog");
  });

  it("title 추출", () => {
    const sections = parseSections(sample);
    expect(sections[0]?.title).toBe("시작하기");
    expect(sections[1]?.title).toBe("카탈로그");
  });

  it("body에서 마커 라인 제거", () => {
    const sections = parseSections(sample);
    expect(sections[0]?.body).not.toContain("<!-- section:");
    expect(sections[0]?.body).toContain("# 시작하기");
    expect(sections[0]?.body).toContain("안녕하세요");
  });

  it("searchText 생성 (lowercase, 마크다운 문자 제거)", () => {
    const sections = parseSections(sample);
    expect(sections[0]?.searchText).toContain("시작하기");
    expect(sections[0]?.searchText).toContain("안녕하세요");
    expect(sections[0]?.searchText).not.toContain("#");
  });

  it("빈 입력 → 빈 배열", () => {
    expect(parseSections("")).toEqual([]);
  });
});

describe("matchSection", () => {
  const section = {
    id: "catalog",
    title: "모델 카탈로그",
    body: "추천 strip 본문",
    searchText: "추천 strip 본문",
  };

  it("빈 query → 모두 매칭", () => {
    expect(matchSection(section, [], "")).toBe(true);
    expect(matchSection(section, [], "  ")).toBe(true);
  });

  it("title substring 매칭", () => {
    expect(matchSection(section, [], "카탈로그")).toBe(true);
    expect(matchSection(section, [], "모델")).toBe(true);
  });

  it("body searchText 매칭", () => {
    expect(matchSection(section, [], "추천")).toBe(true);
    expect(matchSection(section, [], "strip")).toBe(true);
  });

  it("keyword 매칭 (jamo cheat 포함)", () => {
    expect(matchSection(section, ["ㅁㄷ"], "ㅁㄷ")).toBe(true);
    expect(matchSection(section, ["model"], "model")).toBe(true);
  });

  it("매치 없음 → false", () => {
    expect(matchSection(section, [], "워크벤치")).toBe(false);
  });

  it("case-insensitive", () => {
    expect(matchSection({ ...section, title: "Catalog" }, [], "CAT")).toBe(true);
  });
});
