// Catalog — 모델 카탈로그 페이지.
//
// 정책 (phase-2pb-catalog-ui-decision.md):
// - 좌측 sidebar: 검색 + 6 카테고리 라디오.
// - 우측 main: 추천 4슬롯 strip + 필터 chips + 그리드.
// - 카드 클릭 → ModelDetailDrawer 슬라이드.
// - 데이터: getCatalog() 1회 + getRecommendation(category) (카테고리 변경 시).

import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  getCatalog,
  getRecommendation,
  type ModelCategory,
  type ModelEntry,
  type Recommendation,
} from "../ipc/catalog";
import { onCatalogRefreshed } from "../ipc/catalog-refresh";
import type { CustomModel } from "../ipc/workbench";

import { CustomModelsSection } from "../components/catalog/CustomModelsSection";
import { ModelCard } from "../components/catalog/ModelCard";
import { ModelDetailDrawer } from "../components/catalog/ModelDetailDrawer";
import { RecommendationStrip } from "../components/catalog/RecommendationStrip";
import { idOf, modelHasFlag } from "../components/catalog/format";
import { HelpButton } from "../components/HelpButton";

import "./catalog.css";

const CATEGORY_KEYS: (ModelCategory | "all")[] = [
  "all",
  "agent-general",
  "roleplay",
  "coding",
  "slm",
  "sound-stt",
  "sound-tts",
];

type FilterKey = "tool" | "vision" | "structured" | "recommendedOnly";
type SortKey = "score" | "korean" | "size";

export function CatalogPage() {
  const { t } = useTranslation();
  const [entries, setEntries] = useState<ModelEntry[]>([]);
  const [recommendation, setRecommendation] = useState<Recommendation | null>(null);
  const [recLoading, setRecLoading] = useState(false);
  const [category, setCategory] = useState<ModelCategory | "all">("all");
  const [search, setSearch] = useState("");
  const [filters, setFilters] = useState<Set<FilterKey>>(new Set());
  const [sort, setSort] = useState<SortKey>("score");
  const [selected, setSelected] = useState<ModelEntry | null>(null);

  const cardRefs = useRef(new Map<string, HTMLDivElement>());

  // 1) 카탈로그 로드 — 한 번만. Home에서 preselect한 모델 ID가 있으면 자동으로 drawer 열기.
  //    Phase 1' integration: catalog://refreshed event 시 entries 다시 fetch.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    const reload = (preselectId?: string) => {
      getCatalog()
        .then((view) => {
          if (cancelled) return;
          setEntries(view.entries);
          if (preselectId) {
            try {
              window.localStorage.removeItem("lmmaster.catalog.preselect");
            } catch {
              /* ignore */
            }
            const target = view.entries.find((m) => m.id === preselectId);
            if (target) setSelected(target);
          }
        })
        .catch((e) => {
          console.warn("getCatalog failed:", e);
        });
    };

    let preselect: string | null = null;
    try {
      preselect = window.localStorage.getItem("lmmaster.catalog.preselect");
    } catch {
      /* localStorage unavailable */
    }
    reload(preselect ?? undefined);

    (async () => {
      try {
        unlisten = await onCatalogRefreshed(() => {
          // 새 매니페스트 도착 — entries 재조회.
          reload();
        });
      } catch (e) {
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
    const targetCat: ModelCategory =
      category === "all" ? "agent-general" : category;
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
    const m = new Map<string, ModelEntry>();
    for (const e of entries) m.set(e.id, e);
    return m;
  }, [entries]);

  const recommendedIds = useMemo(() => {
    if (!recommendation) return new Set<string>();
    return new Set(
      [
        recommendation.best_choice,
        recommendation.balanced_choice,
        recommendation.lightweight_choice,
        recommendation.fallback_choice,
      ].filter((x): x is string => !!x),
    );
  }, [recommendation]);

  const visible = useMemo(() => {
    let list = entries;
    if (category !== "all") {
      list = list.filter((e) => e.category === category);
    }
    if (search.trim()) {
      const q = search.trim().toLowerCase();
      list = list.filter(
        (e) =>
          e.display_name.toLowerCase().includes(q) ||
          e.id.toLowerCase().includes(q),
      );
    }
    for (const f of filters) {
      if (f === "recommendedOnly") {
        list = list.filter((e) => recommendedIds.has(e.id));
      } else {
        list = list.filter((e) => modelHasFlag(e, f));
      }
    }
    list = sortEntries(list, sort, recommendation);
    return list;
  }, [entries, category, search, filters, sort, recommendedIds, recommendation]);

  const handleSlotSelect = (modelId: string) => {
    const el = cardRefs.current.get(modelId);
    if (el) {
      el.scrollIntoView({ behavior: "smooth", block: "center" });
      el.classList.add("is-pulsed");
      window.setTimeout(() => el.classList.remove("is-pulsed"), 1200);
    }
  };

  const toggleFilter = (key: FilterKey) => {
    setFilters((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  return (
    <div className="catalog-root">
      <header className="catalog-page-header">
        <div className="catalog-title-row">
          <h2 className="catalog-page-title">{t("catalog.title")}</h2>
          <HelpButton
            sectionId="catalog"
            hint={t("screens.help.catalog") ?? undefined}
            testId="catalog-help"
          />
        </div>
        <p className="catalog-page-subtitle">{t("catalog.subtitle")}</p>
      </header>

      <div className="catalog-shell">
        <aside className="catalog-sidebar" aria-labelledby="catalog-sidebar-heading">
          <h3 id="catalog-sidebar-heading" className="catalog-sidebar-heading">
            {t("catalog.search.placeholder")}
          </h3>
          <input
            type="search"
            className="catalog-search"
            placeholder={t("catalog.search.placeholder")}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            aria-label={t("catalog.search.placeholder")}
          />
          <nav
            className="catalog-categories"
            aria-label={t("catalog.title")}
            role="radiogroup"
          >
            {CATEGORY_KEYS.map((key) => (
              <button
                key={key}
                type="button"
                role="radio"
                aria-checked={category === key}
                className={`catalog-category${category === key ? " is-active" : ""}`}
                onClick={() => setCategory(key)}
              >
                {t(`catalog.category.${key}`)}
              </button>
            ))}
          </nav>
        </aside>

        <main className="catalog-main">
          <RecommendationStrip
            recommendation={recommendation}
            loading={recLoading}
            byId={byId}
            onSelect={handleSlotSelect}
          />

          <CustomModelsSection onSelect={(m) => setSelected(customModelToEntry(m))} />

          <div className="catalog-toolbar" role="toolbar" aria-label="Filters">
            <div className="catalog-filter-chips">
              {(["tool", "vision", "structured", "recommendedOnly"] as FilterKey[]).map(
                (key) => (
                  <button
                    key={key}
                    type="button"
                    className={`catalog-filter-chip${filters.has(key) ? " is-on" : ""}`}
                    aria-pressed={filters.has(key)}
                    onClick={() => toggleFilter(key)}
                  >
                    {t(`catalog.filter.${key}`)}
                  </button>
                ),
              )}
            </div>
            <label className="catalog-sort">
              <span className="catalog-sort-label">{t("catalog.sort.label")}</span>
              <select
                value={sort}
                onChange={(e) => setSort(e.target.value as SortKey)}
              >
                <option value="score">{t("catalog.sort.score")}</option>
                <option value="korean">{t("catalog.sort.korean")}</option>
                <option value="size">{t("catalog.sort.size")}</option>
              </select>
            </label>
          </div>

          {visible.length === 0 ? (
            <p className="catalog-empty">
              {entries.length === 0
                ? t("catalog.empty.category")
                : t("catalog.empty.noMatch")}
            </p>
          ) : (
            <div className="catalog-grid" role="list">
              {visible.map((m) => (
                <div
                  key={m.id}
                  role="listitem"
                  ref={(el) => {
                    if (el) cardRefs.current.set(m.id, el);
                    else cardRefs.current.delete(m.id);
                  }}
                >
                  <ModelCard
                    model={m}
                    recommendation={recommendation}
                    onSelect={setSelected}
                  />
                </div>
              ))}
            </div>
          )}
        </main>
      </div>

      <ModelDetailDrawer
        model={selected}
        onClose={() => setSelected(null)}
        onInstall={(modelId) => {
          try {
            window.localStorage.setItem("lmmaster.install.preselect", modelId);
          } catch {
            /* ignore */
          }
          window.dispatchEvent(new CustomEvent("lmmaster:nav", { detail: "install" }));
          setSelected(null);
        }}
      />
    </div>
  );
}

function sortEntries(
  list: ModelEntry[],
  sort: SortKey,
  rec: Recommendation | null,
): ModelEntry[] {
  const copy = [...list];
  switch (sort) {
    case "korean":
      copy.sort(
        (a, b) =>
          (b.language_strength ?? 0) - (a.language_strength ?? 0) ||
          a.display_name.localeCompare(b.display_name, "ko"),
      );
      return copy;
    case "size":
      copy.sort(
        (a, b) =>
          a.install_size_mb - b.install_size_mb ||
          a.display_name.localeCompare(b.display_name, "ko"),
      );
      return copy;
    case "score":
    default: {
      const order = scoreOrder(rec);
      copy.sort(
        (a, b) =>
          (order.get(a.id) ?? 999) - (order.get(b.id) ?? 999) ||
          a.display_name.localeCompare(b.display_name, "ko"),
      );
      return copy;
    }
  }
}

function scoreOrder(rec: Recommendation | null): Map<string, number> {
  const m = new Map<string, number>();
  if (!rec) return m;
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
    if (!m.has(id)) m.set(id, 999);
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
function customModelToEntry(m: CustomModel): ModelEntry {
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
