// Phase 12'.a — Minimal Markdown renderer (EulaGate에서 추출).
//
// 정책:
// - 우리가 작성한 markdown 파일에만 사용. user-input 처리 X.
// - escape는 안전 first — entities 5종 변환.
// - 지원: # / ## / ### / - / **bold** / `code` / 빈 줄 (단락 구분) / --- (구분선).
// - 외부 의존성 없음 (react-markdown 도입 X).
//
// EulaGate가 본 모듈을 사용하므로, 변경 시 EulaGate 테스트도 함께 통과해야 해요.
const MD_ESCAPE = {
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    '"': "&quot;",
    "'": "&#39;",
};
export function escapeHtml(s) {
    return s.replace(/[&<>"']/g, (c) => MD_ESCAPE[c] ?? c);
}
export function renderInline(s) {
    let out = escapeHtml(s);
    out = out.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
    out = out.replace(/`([^`]+)`/g, "<code>$1</code>");
    return out;
}
/** "# 제목" 라인에서 제목 추출. 없으면 첫 비빈 줄 일부. */
function extractTitle(md) {
    for (const line of md.split(/\r?\n/)) {
        if (line.startsWith("# "))
            return line.slice(2).trim();
        if (line.startsWith("## "))
            return line.slice(3).trim();
    }
    return md.trim().slice(0, 40) || "section";
}
/** slug — 기본 ASCII 알파넘 + dash. 한국어는 그대로 보존 (URL hash로 사용 가능). */
function slugifyAscii(s) {
    return s
        .toLowerCase()
        .replace(/[\s_]+/g, "-")
        .replace(/[^a-z0-9가-힣\-]/g, "")
        .replace(/-+/g, "-")
        .replace(/^-|-$/g, "");
}
/**
 * markdown 본문을 섹션 단위로 분리. 각 섹션의 첫 `# 제목`이 id 기반.
 *
 * 섹션 ID는 본문 안의 explicit 마커로 지정해요. 마커 형식:
 *   `<!-- section: id -->`
 * 마커가 있으면 우선, 없으면 제목 slugify.
 */
export function parseSections(md) {
    const blocks = md.split(/\n---\n/);
    const out = [];
    for (const block of blocks) {
        const trimmed = block.trim();
        if (trimmed.length === 0)
            continue;
        const idMatch = trimmed.match(/<!--\s*section:\s*([a-z0-9\-_]+)\s*-->/i);
        const title = extractTitle(trimmed);
        const id = idMatch?.[1] ?? (slugifyAscii(title) || `section-${out.length + 1}`);
        // 본문에서 마커 라인 제거.
        const body = trimmed
            .replace(/<!--\s*section:\s*[a-z0-9\-_]+\s*-->\s*\n?/i, "")
            .trim();
        const searchText = body
            .replace(/[#*`_\->\[\]\(\)]+/g, " ")
            .replace(/\s+/g, " ")
            .toLowerCase();
        out.push({ id, title, body, searchText });
    }
    return out;
}
/** 우리가 작성한 markdown만 처리 — user input 아님. */
export function renderMarkdown(md) {
    const lines = md.split(/\r?\n/);
    const out = [];
    let inList = false;
    const closeList = () => {
        if (inList) {
            out.push("</ul>");
            inList = false;
        }
    };
    for (const line of lines) {
        if (line.startsWith("# ")) {
            closeList();
            out.push(`<h1>${renderInline(line.slice(2))}</h1>`);
        }
        else if (line.startsWith("## ")) {
            closeList();
            out.push(`<h2>${renderInline(line.slice(3))}</h2>`);
        }
        else if (line.startsWith("### ")) {
            closeList();
            out.push(`<h3>${renderInline(line.slice(4))}</h3>`);
        }
        else if (line.startsWith("- ")) {
            if (!inList) {
                out.push("<ul>");
                inList = true;
            }
            out.push(`<li>${renderInline(line.slice(2))}</li>`);
        }
        else if (line.trim() === "") {
            closeList();
        }
        else if (line.trim() === "---") {
            // 섹션 구분선은 외부 splitter가 처리. 여기선 무시.
            closeList();
        }
        else {
            closeList();
            out.push(`<p>${renderInline(line)}</p>`);
        }
    }
    closeList();
    return out.join("\n");
}
// ── 한국어 jamo cheat 검색 — Command Palette 패턴 재활용 ────────────────
/**
 * 단순 substring 매칭 — query를 lowercase로 각 섹션의 title/searchText/keywords에 contains 검사.
 * 한글 jamo 분해는 안 함 (Command Palette 정책 동일). 키워드 명시 시 cho-only 검색 자연.
 */
export function matchSection(section, keywords, query) {
    if (!query.trim())
        return true;
    const q = query.toLowerCase().trim();
    const haystack = [section.title, section.searchText, ...keywords]
        .join(" ")
        .toLowerCase();
    return haystack.includes(q);
}
