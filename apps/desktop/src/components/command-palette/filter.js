// Command Palette 필터 + 그룹화. Phase 1A.4.e §B4.
/**
 * 단순 substring 매칭 — label + keywords[]를 합쳐 lowercased 후 contains 검사.
 * 한글 jamo 분해는 v1에선 안 함. keywords에 수동 cheat (예: "ㅅㅊ")를 두면 cho-only 검색 자연스럽게 동작.
 */
export function matchesQuery(cmd, query) {
    if (!query.trim())
        return true;
    const q = query.toLowerCase().trim();
    const haystack = [cmd.label, ...(cmd.keywords ?? [])].join(" ").toLowerCase();
    return haystack.includes(q);
}
/** 그룹별 정렬 + 안정화. group 순서 = wizard → navigation → system → diagnostics. */
const GROUP_ORDER = [
    "wizard",
    "navigation",
    "system",
    "diagnostics",
];
export function groupCommands(commands) {
    const map = new Map();
    for (const cmd of commands) {
        const list = map.get(cmd.group) ?? [];
        list.push(cmd);
        map.set(cmd.group, list);
    }
    // 사전 등록 순서를 그룹 내에선 보존.
    return GROUP_ORDER.filter((g) => map.has(g)).map((g) => [g, map.get(g)]);
}
