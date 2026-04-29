import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// Catalog — 모델 카탈로그 페이지.
//
// 정책 (phase-2pb-catalog-ui-decision.md):
// - 좌측 sidebar: 검색 + 6 카테고리 라디오.
// - 우측 main: 추천 4슬롯 strip + 필터 chips + 그리드.
// - 카드 클릭 → ModelDetailDrawer 슬라이드.
// - 데이터: getCatalog() 1회 + getRecommendation(category) (카테고리 변경 시).
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { getCatalog, getRecommendation, } from "../ipc/catalog";
import { onCatalogRefreshed } from "../ipc/catalog-refresh";
import { CustomModelsSection } from "../components/catalog/CustomModelsSection";
import { ModelCard } from "../components/catalog/ModelCard";
import { ModelDetailDrawer } from "../components/catalog/ModelDetailDrawer";
import { RecommendationStrip } from "../components/catalog/RecommendationStrip";
import { idOf, modelHasFlag } from "../components/catalog/format";
import { HelpButton } from "../components/HelpButton";
import "./catalog.css";
const CATEGORY_KEYS = [
    "all",
    "agent-general",
    "roleplay",
    "coding",
    "slm",
    "sound-stt",
    "sound-tts",
];
export function CatalogPage() {
    const { t } = useTranslation();
    const [entries, setEntries] = useState([]);
    const [recommendation, setRecommendation] = useState(null);
    const [recLoading, setRecLoading] = useState(false);
    const [category, setCategory] = useState("all");
    const [search, setSearch] = useState("");
    const [filters, setFilters] = useState(new Set());
    const [sort, setSort] = useState("score");
    const [selected, setSelected] = useState(null);
    const cardRefs = useRef(new Map());
    // 1) 카탈로그 로드 — 한 번만. Home에서 preselect한 모델 ID가 있으면 자동으로 drawer 열기.
    //    Phase 1' integration: catalog://refreshed event 시 entries 다시 fetch.
    useEffect(() => {
        let cancelled = false;
        let unlisten = null;
        const reload = (preselectId) => {
            getCatalog()
                .then((view) => {
                if (cancelled)
                    return;
                setEntries(view.entries);
                if (preselectId) {
                    try {
                        window.localStorage.removeItem("lmmaster.catalog.preselect");
                    }
                    catch {
                        /* ignore */
                    }
                    const target = view.entries.find((m) => m.id === preselectId);
                    if (target)
                        setSelected(target);
                }
            })
                .catch((e) => {
                console.warn("getCatalog failed:", e);
            });
        };
        let preselect = null;
        try {
            preselect = window.localStorage.getItem("lmmaster.catalog.preselect");
        }
        catch {
            /* localStorage unavailable */
        }
        reload(preselect ?? undefined);
        (async () => {
            try {
                unlisten = await onCatalogRefreshed(() => {
                    // 새 매니페스트 도착 — entries 재조회.
                    reload();
                });
            }
            catch (e) {
                console.warn("onCatalogRefreshed listen failed:", e);
            }
        })();
        return () => {
            cancelled = true;
            unlisten?.();
        };
    }, []);
    // 2) 추천 — 카테고리 바뀔 때만 (all → agent-general default).
    useEffect(() => {
        let cancelled = false;
        setRecLoading(true);
        const targetCat = category === "all" ? "agent-general" : category;
        getRecommendation(targetCat)
            .then((rec) => {
            if (!cancelled) {
                setRecommendation(rec);
                setRecLoading(false);
            }
        })
            .catch((e) => {
            console.warn("getRecommendation failed:", e);
            if (!cancelled) {
                setRecommendation(null);
                setRecLoading(false);
            }
        });
        return () => {
            cancelled = true;
        };
    }, [category]);
    const byId = useMemo(() => {
        const m = new Map();
        for (const e of entries)
            m.set(e.id, e);
        return m;
    }, [entries]);
    const recommendedIds = useMemo(() => {
        if (!recommendation)
            return new Set();
        return new Set([
            recommendation.best_choice,
            recommendation.balanced_choice,
            recommendation.lightweight_choice,
            recommendation.fallback_choice,
        ].filter((x) => !!x));
    }, [recommendation]);
    const visible = useMemo(() => {
        let list = entries;
        if (category !== "all") {
            list = list.filter((e) => e.category === category);
        }
        if (search.trim()) {
            const q = search.trim().toLowerCase();
            list = list.filter((e) => e.display_name.toLowerCase().includes(q) ||
                e.id.toLowerCase().includes(q));
        }
        for (const f of filters) {
            if (f === "recommendedOnly") {
                list = list.filter((e) => recommendedIds.has(e.id));
            }
            else {
                list = list.filter((e) => modelHasFlag(e, f));
            }
        }
        list = sortEntries(list, sort, recommendation);
        return list;
    }, [entries, category, search, filters, sort, recommendedIds, recommendation]);
    const handleSlotSelect = (modelId) => {
        const el = cardRefs.current.get(modelId);
        if (el) {
            el.scrollIntoView({ behavior: "smooth", block: "center" });
            el.classList.add("is-pulsed");
            window.setTimeout(() => el.classList.remove("is-pulsed"), 1200);
        }
    };
    const toggleFilter = (key) => {
        setFilters((prev) => {
            const next = new Set(prev);
            if (next.has(key))
                next.delete(key);
            else
                next.add(key);
            return next;
        });
    };
    return (_jsxs("div", { className: "catalog-root", children: [_jsxs("header", { className: "catalog-page-header", children: [_jsxs("div", { className: "catalog-title-row", children: [_jsx("h2", { className: "catalog-page-title", children: t("catalog.title") }), _jsx(HelpButton, { sectionId: "catalog", hint: t("screens.help.catalog") ?? undefined, testId: "catalog-help" })] }), _jsx("p", { className: "catalog-page-subtitle", children: t("catalog.subtitle") })] }), _jsxs("div", { className: "catalog-shell", children: [_jsxs("aside", { className: "catalog-sidebar", "aria-labelledby": "catalog-sidebar-heading", children: [_jsx("h3", { id: "catalog-sidebar-heading", className: "catalog-sidebar-heading", children: t("catalog.search.placeholder") }), _jsx("input", { type: "search", className: "catalog-search", placeholder: t("catalog.search.placeholder"), value: search, onChange: (e) => setSearch(e.target.value), "aria-label": t("catalog.search.placeholder") }), _jsx("nav", { className: "catalog-categories", "aria-label": t("catalog.title"), role: "radiogroup", children: CATEGORY_KEYS.map((key) => (_jsx("button", { type: "button", role: "radio", "aria-checked": category === key, className: `catalog-category${category === key ? " is-active" : ""}`, onClick: () => setCategory(key), children: t(`catalog.category.${key}`) }, key))) })] }), _jsxs("main", { className: "catalog-main", children: [_jsx(RecommendationStrip, { recommendation: recommendation, loading: recLoading, byId: byId, onSelect: handleSlotSelect }), _jsx(CustomModelsSection, { onSelect: (m) => setSelected(customModelToEntry(m)) }), _jsxs("div", { className: "catalog-toolbar", role: "toolbar", "aria-label": "Filters", children: [_jsx("div", { className: "catalog-filter-chips", children: ["tool", "vision", "structured", "recommendedOnly"].map((key) => (_jsx("button", { type: "button", className: `catalog-filter-chip${filters.has(key) ? " is-on" : ""}`, "aria-pressed": filters.has(key), onClick: () => toggleFilter(key), children: t(`catalog.filter.${key}`) }, key))) }), _jsxs("label", { className: "catalog-sort", children: [_jsx("span", { className: "catalog-sort-label", children: t("catalog.sort.label") }), _jsxs("select", { value: sort, onChange: (e) => setSort(e.target.value), children: [_jsx("option", { value: "score", children: t("catalog.sort.score") }), _jsx("option", { value: "korean", children: t("catalog.sort.korean") }), _jsx("option", { value: "size", children: t("catalog.sort.size") })] })] })] }), visible.length === 0 ? (_jsx("p", { className: "catalog-empty", children: entries.length === 0
                                    ? t("catalog.empty.category")
                                    : t("catalog.empty.noMatch") })) : (_jsx("div", { className: "catalog-grid", role: "list", children: visible.map((m) => (_jsx("div", { role: "listitem", ref: (el) => {
                                        if (el)
                                            cardRefs.current.set(m.id, el);
                                        else
                                            cardRefs.current.delete(m.id);
                                    }, children: _jsx(ModelCard, { model: m, recommendation: recommendation, onSelect: setSelected }) }, m.id))) }))] })] }), _jsx(ModelDetailDrawer, { model: selected, onClose: () => setSelected(null), onInstall: (modelId) => {
                    try {
                        window.localStorage.setItem("lmmaster.install.preselect", modelId);
                    }
                    catch {
                        /* ignore */
                    }
                    window.dispatchEvent(new CustomEvent("lmmaster:nav", { detail: "install" }));
                    setSelected(null);
                } })] }));
}
function sortEntries(list, sort, rec) {
    const copy = [...list];
    switch (sort) {
        case "korean":
            copy.sort((a, b) => (b.language_strength ?? 0) - (a.language_strength ?? 0) ||
                a.display_name.localeCompare(b.display_name, "ko"));
            return copy;
        case "size":
            copy.sort((a, b) => a.install_size_mb - b.install_size_mb ||
                a.display_name.localeCompare(b.display_name, "ko"));
            return copy;
        case "score":
        default: {
            const order = scoreOrder(rec);
            copy.sort((a, b) => (order.get(a.id) ?? 999) - (order.get(b.id) ?? 999) ||
                a.display_name.localeCompare(b.display_name, "ko"));
            return copy;
        }
    }
}
function scoreOrder(rec) {
    const m = new Map();
    if (!rec)
        return m;
    // best=0, balanced=1, lightweight=2, fallback=3, others later, excluded last.
    let i = 0;
    for (const id of [
        rec.best_choice,
        rec.balanced_choice,
        rec.lightweight_choice,
        rec.fallback_choice,
    ]) {
        if (id && !m.has(id)) {
            m.set(id, i++);
        }
    }
    // excluded id에 마지막 우선순위.
    for (const e of rec.excluded) {
        const id = idOf(e);
        if (!m.has(id))
            m.set(id, 999);
    }
    return m;
}
/**
 * Phase 8'.b.1 — CustomModel을 ModelDetailDrawer가 받는 ModelEntry로 변환.
 *
 * graceful fallback:
 * - quant_options는 quant_type 한 개로 합성. size는 0 (artifact_paths만 있어 unknown).
 * - use_case_examples / context_guidance는 빈 값 — drawer가 conditional 렌더링하므로 안전.
 * - language_strength 등 점수는 6 (중립) — 카드 별점 표시를 위해.
 * - verification은 community + custom-model badge로 충분.
 */
function customModelToEntry(m) {
    return {
        id: m.id,
        display_name: m.id,
        category: "agent-general",
        model_family: m.base_model,
        source: { type: "direct-url", url: "" },
        runner_compatibility: ["ollama"],
        quantization_options: [
            {
                label: m.quant_type,
                size_mb: 0,
                sha256: "",
            },
        ],
        min_vram_mb: null,
        rec_vram_mb: null,
        min_ram_mb: 0,
        rec_ram_mb: 0,
        install_size_mb: 0,
        context_guidance: undefined,
        language_strength: 6,
        roleplay_strength: 6,
        coding_strength: 6,
        tool_support: false,
        vision_support: false,
        structured_output_support: false,
        license: m.base_model,
        maturity: "experimental",
        portable_suitability: 6,
        on_device_suitability: 6,
        fine_tune_suitability: 9,
        verification: { tier: "community" },
        hf_meta: null,
        use_case_examples: [],
        notes: null,
        warnings: [],
    };
}
