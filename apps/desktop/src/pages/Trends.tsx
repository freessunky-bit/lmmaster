// Trends — Phase 22' v2.0 prototype + Phase 23' Dataset 통합 (2026-05-07).
//
// 정책:
// - 4B+ 모델 게이트 (Gemma 3 4B / Nemotron 3 Nano 4B / Nemotron 30B A3B / EXAONE 3.5 7.8B / HCX-SEED 8B 중 1개+ 설치).
// - 미충족 시: 메뉴 disabled UI + "카탈로그에서 4B+ 모델 설치" CTA.
// - 충족 시: AI 트렌드 카드 그리드 (paper/blog/news/video/sns/github 6 카테고리) + 데이터셋 섹션.
// - 본 prototype은 *placeholder 카피 + mockup 카드*. 실 trends-bundle fetch는 Phase 22'.c (v2.0 진입 시점).
// - 데이터셋 섹션은 Personas-Korea + NSFW 한국어 RP 후보 안내 (Phase 23' 진입 후 실 카드).
//
// 디자인:
// - 카드 그리드 + lucide 아이콘 + 디자인 토큰만.
// - a11y: focus-visible + role="list" / "listitem" + 한국어 aria-label.
// - 해요체 톤 (CLAUDE.md §4.1).

import {
  AlertTriangle,
  BookOpen,
  Database,
  GitBranch,
  Newspaper,
  Sparkles,
  Users,
  Video,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { getCatalog, type ModelEntry } from "../ipc/catalog";
import { listDatasets, type DatasetEntry } from "../ipc/datasets";
import { DatasetImportDrawer } from "../components/datasets/DatasetImportDrawer";
import { InstalledDatasetsSection } from "../components/datasets/InstalledDatasetsSection";
import {
  SUMMARY_KIND_LABEL_KO,
  summarizeTrends,
  type SummaryInput,
  type SummaryKind,
  type TrendsSummary,
} from "../ipc/trends";
import trendsBundleData from "../../../../manifests/apps/trends-bundle.json";

import "./trends.css";

/** 4B+ 모델 게이트 — 본 ID 중 *1개 이상* 설치 시 트렌드 메뉴 활성. */
const TREND_MODEL_GATE: readonly string[] = [
  "gemma-3-4b",
  "nemotron-3-nano-4b",
  "nemotron-3-nano-30b-a3b",
  "exaone-3.5-7.8b-instruct",
  "exaone-4.0-32b-instruct",
  "hcx-seed-8b",
];

interface MockTrendCard {
  kind: "paper" | "blog" | "news" | "video" | "sns" | "github";
  titleKey: string;
  hintKey: string;
  source: string;
}

/** Phase 22'.c — trends-bundle.json items type. SummaryInput + 큐레이션 메타. */
type TrendBundleItem = SummaryInput & {
  attribution?: string;
  published_at?: string;
  tags?: string[];
  score?: number;
};

/** Phase 22' B 안 — placeholder 카드 6 카테고리. 실 데이터는 trends-bundle fetch (v2.0). */
const MOCK_CARDS: readonly MockTrendCard[] = [
  {
    kind: "paper",
    titleKey: "trends.cards.paper.title",
    hintKey: "trends.cards.paper.hint",
    source: "HuggingFace Daily Papers + arXiv cs.LG/CL",
  },
  {
    kind: "blog",
    titleKey: "trends.cards.blog.title",
    hintKey: "trends.cards.blog.hint",
    source: "OpenAI / Anthropic / Google DeepMind / NVIDIA blogs",
  },
  {
    kind: "news",
    titleKey: "trends.cards.news.title",
    hintKey: "trends.cards.news.hint",
    source: "TechCrunch AI / The Verge / VentureBeat / THE AI / AI타임스",
  },
  {
    kind: "video",
    titleKey: "trends.cards.video.title",
    hintKey: "trends.cards.video.hint",
    source: "3Blue1Brown / Yannic Kilcher / Two Minute Papers / Sebastian Raschka",
  },
  {
    kind: "sns",
    titleKey: "trends.cards.sns.title",
    hintKey: "trends.cards.sns.hint",
    source: "Bluesky public.api + Mastodon RSS + 본인 블로그 (Karpathy / Lilian Weng / Simon Willison)",
  },
  {
    kind: "github",
    titleKey: "trends.cards.github.title",
    hintKey: "trends.cards.github.hint",
    source: "GitHub Trending (Apache-2 / MIT) — LangChain / vLLM / 한국 OSS",
  },
];

/** Phase 23'.c 시드 4 entries (datasets-bundle.json 기반 — Vite static import). */

const KIND_ICON: Record<MockTrendCard["kind"], typeof BookOpen> = {
  paper: BookOpen,
  blog: Sparkles,
  news: Newspaper,
  video: Video,
  sns: Users,
  github: GitBranch,
};

export function Trends({ onNavigate }: { onNavigate?: (target: "catalog") => void }) {
  const { t } = useTranslation();
  const [entries, setEntries] = useState<ModelEntry[]>([]);
  const [datasets, setDatasets] = useState<DatasetEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedDataset, setSelectedDataset] = useState<DatasetEntry | null>(
    null,
  );
  const [installedRefresh, setInstalledRefresh] = useState(0);
  const [trendsSummary, setTrendsSummary] = useState<TrendsSummary | null>(null);
  const [summarizing, setSummarizing] = useState(false);
  const [summaryError, setSummaryError] = useState<string | null>(null);

  // Phase 22'.c — bundleItems의 attribution / published_at / tags / score는
  // SummaryInput에 없는 큐레이션 메타. type 확장으로 type-safe 접근.
  const bundleItems = (trendsBundleData.items ?? []) as TrendBundleItem[];

  const handleSummarize = useCallback(async () => {
    if (bundleItems.length === 0) return;
    setSummarizing(true);
    setSummaryError(null);
    try {
      const result = await summarizeTrends(bundleItems, false);
      setTrendsSummary(result);
    } catch (e) {
      setSummaryError(String(e));
    } finally {
      setSummarizing(false);
    }
  }, [bundleItems]);

  useEffect(() => {
    let cancelled = false;
    Promise.all([getCatalog(), listDatasets()])
      .then(([cat, ds]) => {
        if (!cancelled) {
          setEntries(cat.entries);
          setDatasets(ds);
          setLoading(false);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setEntries([]);
          setDatasets([]);
          setLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // 게이트 — 설치 여부는 v2.0.c에서 실 IPC. 현재 prototype은 *카탈로그 등록 여부*로 근사.
  const installedGateModels = useMemo(
    () => entries.filter((e) => TREND_MODEL_GATE.includes(e.id)),
    [entries],
  );
  const gatePass = installedGateModels.length > 0;

  if (loading) {
    return (
      <div className="trends-page" data-testid="trends-loading">
        <p>{t("trends.loading", "트렌드 데이터를 준비하고 있어요…")}</p>
      </div>
    );
  }

  return (
    <div className="trends-page" data-testid="trends-page">
      <header className="trends-hero">
        <h1 className="trends-title">
          <Sparkles size={20} aria-hidden="true" />
          <span>{t("trends.title", "AI 트렌드 리포트")}</span>
          <span className="trends-tier-badge" aria-label="Prototype">
            {t("trends.prototype", "Prototype")}
          </span>
        </h1>
        <p className="trends-subtitle">
          {t(
            "trends.subtitle",
            "최신 AI 모델 출시 + 회사 동향 + 학술 + 거물 SNS 인용을 한 곳에서 한국어로 봐요. 큐레이션 데이터셋이 매주 도착해요 (Phase 22' B 안).",
          )}
        </p>
      </header>

      {!gatePass && (
        <section
          className="trends-gate"
          role="status"
          aria-live="polite"
          data-testid="trends-gate-disabled"
        >
          <AlertTriangle size={18} aria-hidden="true" />
          <div>
            <h2 className="trends-gate-title">
              {t("trends.gate.title", "4B+ 모델이 필요해요")}
            </h2>
            <p>
              {t(
                "trends.gate.body",
                "트렌드 메뉴는 한국어 요약 정렬을 위해 4B+ 모델 1개 이상 설치가 필요해요. EXAONE 3.5 7.8B / Nemotron 3 Nano 4B / Gemma 3 4B / HCX-SEED 8B 중 하나를 카탈로그에서 받아 주세요.",
              )}
            </p>
            <button
              type="button"
              className="trends-gate-cta"
              onClick={() => onNavigate?.("catalog")}
              data-testid="trends-gate-catalog-cta"
            >
              {t("trends.gate.cta", "카탈로그로 갈게요")}
            </button>
          </div>
        </section>
      )}

      {/* Phase 22'.c — 시드 카드 그리드. v2.0 진입 시 큐레이터 GHA cron이 자동 갱신. */}
      {bundleItems.length > 0 && (
        <section
          className="trends-section"
          aria-labelledby="trends-bundle-heading"
          data-testid="trends-bundle-section"
        >
          <h2 id="trends-bundle-heading" className="trends-section-heading">
            <Sparkles size={18} aria-hidden="true" />
            <span>{t("trends.bundle.heading", "이번 주 큐레이션")}</span>
            <span className="trends-tier-badge" aria-label="Preview">
              {t("trends.bundle.previewBadge", "프리뷰")}
            </span>
            <span className="trends-section-count" aria-live="polite">
              {t("trends.bundle.itemCount", "{{count}}건", {
                count: bundleItems.length,
              })}
            </span>
          </h2>
          {trendsBundleData.curator_note_ko && (
            <p className="trends-section-meta trends-curator-note">
              {trendsBundleData.curator_note_ko}
            </p>
          )}
          <p className="trends-section-meta trends-preview-note">
            {t(
              "trends.bundle.previewNote",
              "본 콘텐츠는 v2.0 진입 전 시드예요. 실 큐레이션 GHA cron + jsdelivr propagate는 v2.0에서 자동 동작해요.",
            )}
          </p>
          <ul className="trends-grid" role="list">
            {bundleItems.map((item) => {
              const Icon =
                KIND_ICON[item.kind as MockTrendCard["kind"]] ?? BookOpen;
              return (
                <li
                  key={item.id}
                  className={`trends-card trends-card-${item.kind}`}
                  role="listitem"
                  data-testid={`trends-bundle-item-${item.id}`}
                >
                  <div className="trends-card-head">
                    <Icon size={16} aria-hidden="true" />
                    <span className="trends-card-kind">
                      {t(`trends.kind.${item.kind}`, item.kind)}
                    </span>
                  </div>
                  <h3 className="trends-card-title">{item.title}</h3>
                  <p className="trends-card-hint">{item.summary_ko}</p>
                  <p className="trends-card-meta">
                    <span className="trends-card-meta-item">{item.source}</span>
                    {item.published_at && (
                      <>
                        <span className="trends-card-meta-sep">·</span>
                        <span className="trends-card-meta-item">
                          {t("trends.bundle.publishedAt", "발행")}:{" "}
                          {item.published_at}
                        </span>
                      </>
                    )}
                  </p>
                  {item.attribution && (
                    <p className="trends-card-source">
                      <span className="trends-card-source-label">
                        {t("trends.bundle.attribution", "출처")}:
                      </span>{" "}
                      {item.attribution}
                    </p>
                  )}
                  {item.source_url && (
                    <p className="trends-card-source-url">
                      <code>{item.source_url}</code>
                    </p>
                  )}
                </li>
              );
            })}
          </ul>
        </section>
      )}

      {/* Phase 22'.c v2.0 — 6 카테고리 mock cards 섹션은 v2.0 큐레이션으로 교체 예정. */}
      {/* 사용자 요청 (2026-05-09): 진짜 데이터로 오해 방지 위해 v2.0 진입 전까지 숨김. */}

      <section
        className="trends-section"
        aria-labelledby="trends-datasets-heading"
        data-testid="trends-datasets-section"
      >
        <h2 id="trends-datasets-heading" className="trends-section-heading">
          <Database size={18} aria-hidden="true" />
          <span>{t("trends.datasets.heading", "데이터셋 카탈로그")}</span>
          <span className="trends-tier-badge" aria-label="Phase 23'">
            Phase 23'
          </span>
        </h2>
        <p className="trends-section-meta">
          {t(
            "trends.datasets.meta",
            "한국어 페르소나 / RP fine-tune 시드 / 학습 데이터셋. ADR-0061 + ADR-0062 진입 후 실 다운로드 IPC와 RAG 시드 1-click 자동 import가 합류해요.",
          )}
        </p>
        {datasets.length === 0 ? (
          <p className="trends-empty">
            {t(
              "trends.datasets.empty",
              "데이터셋 카탈로그가 비어있어요. v0.3.0에서 registry-fetcher 자동 갱신이 합류하면 더 많은 시드가 도착해요.",
            )}
          </p>
        ) : (
          <ul className="trends-grid" role="list">
            {datasets.map((ds) => {
              const isNsfw = ds.content_warning === "rp-explicit";
              const repo =
                ds.source.repo ?? ds.source.url ?? ds.source.path ?? ds.id;
              const sizeMb = ds.size_mb ?? 0;
              const sizeLabel =
                sizeMb >= 1024
                  ? `${(sizeMb / 1024).toFixed(1)}GB`
                  : `${sizeMb}MB`;
              return (
                <li
                  key={ds.id}
                  className={`trends-card trends-card-dataset trends-card-status-available${isNsfw ? " trends-card-nsfw" : ""}`}
                  role="listitem"
                  data-testid={`dataset-card-${ds.id}`}
                >
                  <div className="trends-card-head">
                    <Database size={16} aria-hidden="true" />
                    <span className="trends-card-kind">
                      {t(`trends.datasets.category.${ds.category}`, ds.category)}
                    </span>
                    {isNsfw && (
                      <span
                        className="trends-card-chip trends-card-chip-nsfw"
                        aria-label="NSFW"
                      >
                        {t("catalog.adultContent.chip", "성인")}
                      </span>
                    )}
                    {!ds.commercial && (
                      <span
                        className="trends-card-chip trends-card-chip-noncommercial"
                        aria-label="비상업"
                      >
                        {t("catalog.commercial.chip", "비상업")}
                      </span>
                    )}
                  </div>
                  <h3 className="trends-card-title">{ds.display_name}</h3>
                  <p className="trends-card-hint">
                    {ds.curator_note_ko ?? ""}
                  </p>
                  <p className="trends-card-meta">
                    <span className="trends-card-meta-item">
                      {t("trends.datasets.licenseLabel", "라이선스")}: {ds.license}
                    </span>
                    <span className="trends-card-meta-sep">·</span>
                    <span className="trends-card-meta-item">
                      {t("trends.datasets.sizeLabel", "크기")}: {sizeLabel}
                    </span>
                    {ds.row_count !== undefined && (
                      <>
                        <span className="trends-card-meta-sep">·</span>
                        <span className="trends-card-meta-item">
                          {t("trends.datasets.rowCountLabel", "행")}:{" "}
                          {ds.row_count.toLocaleString()}
                        </span>
                      </>
                    )}
                    <span className="trends-card-meta-sep">·</span>
                    <span className="trends-card-meta-item">
                      {t("trends.datasets.languagesLabel", "언어")}:{" "}
                      {ds.languages.join(" / ")}
                    </span>
                  </p>
                  <p className="trends-card-source">
                    <span className="trends-card-source-label">HF</span>{" "}
                    <code>{repo}</code>
                  </p>
                  <button
                    type="button"
                    className="trends-card-action"
                    onClick={() => setSelectedDataset(ds)}
                    aria-label={t(
                      "trends.datasets.importAria",
                      "{{name}} 가져오기",
                      { name: ds.display_name },
                    )}
                  >
                    {t("trends.datasets.importCta", "이 데이터셋 가져올게요")}
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </section>

      <InstalledDatasetsSection refreshSignal={installedRefresh} />

      <section
        className="trends-section"
        aria-labelledby="trends-summary-heading"
        data-testid="trends-summary-section"
      >
        <h2 id="trends-summary-heading" className="trends-section-heading">
          <Sparkles size={18} aria-hidden="true" />
          <span>{t("trends.summary.heading", "내 모델로 요약하기")}</span>
        </h2>
        <p className="trends-section-meta">
          {t(
            "trends.summary.meta",
            "현재 trends-bundle 큐레이션 데이터를 내 PC의 4B+ 모델로 한국어 요약해요. 모델 미설치 시에는 placeholder 응답이 보여요. 같은 데이터에 대해 한 번 요약하면 30일 캐시돼요.",
          )}
        </p>
        <p className="trends-section-meta">
          {t(
            "trends.summary.bundleCount",
            "현재 bundle 항목: {{count}}건",
            { count: bundleItems.length },
          )}
        </p>
        <button
          type="button"
          className="trends-card-action"
          onClick={() => void handleSummarize()}
          disabled={summarizing || bundleItems.length === 0}
          data-testid="trends-summarize-cta"
        >
          {summarizing
            ? t("trends.summary.runningCta", "요약하고 있어요…")
            : bundleItems.length === 0
              ? t(
                  "trends.summary.emptyCta",
                  "큐레이션 데이터 도착 후 가능해요",
                )
              : t("trends.summary.cta", "지금 요약할게요")}
        </button>

        {summaryError && (
          <p
            className="trends-card-hint"
            style={{ color: "var(--danger, var(--text-muted))" }}
          >
            {t(
              "trends.summary.error",
              "요약에 실패했어요: {{error}}",
              { error: summaryError },
            )}
          </p>
        )}

        {trendsSummary && (
          <div data-testid="trends-summary-result">
            <p className="trends-card-hint">
              {t(
                "trends.summary.modelInfo",
                "요약 모델: {{model}}",
                { model: trendsSummary.model_kind },
              )}
            </p>
            <ul className="trends-grid" role="list">
              {(Object.entries(trendsSummary.sections) as Array<
                [SummaryKind, string]
              >).map(([kind, text]) => (
                <li
                  key={kind}
                  className={`trends-card trends-card-${kind}`}
                  role="listitem"
                >
                  <div className="trends-card-head">
                    <span className="trends-card-kind">
                      {SUMMARY_KIND_LABEL_KO[kind]}
                    </span>
                  </div>
                  <p className="trends-card-hint">{text}</p>
                </li>
              ))}
            </ul>
          </div>
        )}
      </section>

      <DatasetImportDrawer
        dataset={selectedDataset}
        onClose={() => setSelectedDataset(null)}
        onCompleted={() => setInstalledRefresh((n) => n + 1)}
      />

      <footer className="trends-footnote">
        <p>
          {t(
            "trends.footnote",
            "본 prototype은 v2.0 진입 전 placeholder예요. 실 큐레이션 데이터는 별도 repo lmmaster-trends-bundle (또는 v1.x 본 repo prototype)에서 매주 push되어 jsdelivr propagate 후 사용자 PC에 도착해요. 자세한 흐름은 docs/adr/0060-trend-report.md 참고.",
          )}
        </p>
      </footer>
    </div>
  );
}
