// Runtimes — 어댑터 상태 + 모델 목록 페이지. Phase 4.c.
//
// 정책 (phase-4-screens-decision.md §1.1 runtimes, phase-4c-runtimes-decision.md):
// - 좌측 어댑터 카드 column (sm 320px) — Ollama / LM Studio.
//   각 카드는 <header(name + StatusPill + version + port)>
//             <body(model_count, last_ping_at)>
//             <footer(disabled stop / restart / 로그 보기)>.
// - 우측 main — 선택된 어댑터의 모델 VirtualList (24px row).
//   컬럼: name (mono, flex) | size (num) | digest 8자 prefix.
//   검색 input + 정렬 select (name / size).
//   빈 상태: "어떤 모델도 로드되지 않았어요" + 카탈로그 CTA.
// - start/stop/restart는 v1 disabled (외부 데몬이라 안전 위험 — §2.a 결정).
// - auto-refresh polling 거부 — 사용자 manual 새로고침으로 충분 (§2.c).
//
// a11y:
// - 어댑터 카드: <article role="region" aria-labelledby>.
// - virtual list row는 VirtualList 컴포넌트가 role="listitem" 처리.

import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { StatusPill, VirtualList } from "@lmmaster/design-system/react";
import type { PillStatus } from "@lmmaster/design-system/react";
import "@lmmaster/design-system/react/pill.css";
import "@lmmaster/design-system/react/virtual-list.css";

import {
  listRuntimeModels,
  listRuntimeStatuses,
  type RuntimeModelView,
  type RuntimeStatus,
} from "../ipc/runtimes";
import type { RuntimeKind } from "../ipc/catalog";

import "./runtimes.css";

/** 카탈로그 nav로 이동시키는 custom event — App.tsx의 listener가 받는다. */
const NAV_EVENT = "lmmaster:navigate";

type SortKey = "name" | "size";

interface RuntimeCardMeta {
  kind: RuntimeKind;
  display_name: string;
  port: number;
}

const RUNTIME_META: Record<string, RuntimeCardMeta> = {
  ollama: { kind: "ollama", display_name: "Ollama", port: 11434 },
  "lm-studio": { kind: "lm-studio", display_name: "LM Studio", port: 1234 },
};

export function RuntimesPage() {
  const { t } = useTranslation();
  const [statuses, setStatuses] = useState<RuntimeStatus[]>([]);
  const [statusesLoaded, setStatusesLoaded] = useState(false);
  const [selectedKind, setSelectedKind] = useState<RuntimeKind | null>(null);
  const [models, setModels] = useState<RuntimeModelView[]>([]);
  const [modelsLoading, setModelsLoading] = useState(false);
  const [modelsErrored, setModelsErrored] = useState(false);
  const [search, setSearch] = useState("");
  const [sort, setSort] = useState<SortKey>("name");

  // 1) 어댑터 합산 status — 한 번만 로드.
  useEffect(() => {
    let cancelled = false;
    listRuntimeStatuses()
      .then((rows) => {
        if (cancelled) return;
        setStatuses(rows);
        setStatusesLoaded(true);
        // 첫 카드 자동 선택 — 사용자가 빈 우측 main을 보지 않도록.
        if (rows.length > 0 && selectedKind == null) {
          setSelectedKind(rows[0]!.kind);
        }
      })
      .catch((e) => {
        if (cancelled) return;
        console.warn("listRuntimeStatuses failed:", e);
        setStatusesLoaded(true);
      });
    return () => {
      cancelled = true;
    };
    // selectedKind는 첫 진입 시 한 번만 결정 — 의존성 배열에서 제외 (eslint-disable는 미설정이라 주석으로 표기).
  }, [selectedKind]);

  // 2) 선택된 어댑터의 모델 — 선택 변경 시마다 fetch.
  useEffect(() => {
    if (selectedKind == null) {
      setModels([]);
      return;
    }
    let cancelled = false;
    setModelsLoading(true);
    setModelsErrored(false);
    listRuntimeModels(selectedKind)
      .then((rows) => {
        if (cancelled) return;
        setModels(rows);
        setModelsLoading(false);
      })
      .catch((e) => {
        if (cancelled) return;
        console.warn("listRuntimeModels failed:", e);
        setModels([]);
        setModelsLoading(false);
        setModelsErrored(true);
      });
    return () => {
      cancelled = true;
    };
  }, [selectedKind]);

  const summary = useMemo(() => {
    const total = statuses.length;
    const running = statuses.filter((s) => s.running).length;
    return { total, running };
  }, [statuses]);

  const visibleModels = useMemo(() => {
    let list = models;
    const q = search.trim().toLowerCase();
    if (q.length > 0) {
      list = list.filter((m) => m.id.toLowerCase().includes(q));
    }
    const copy = [...list];
    if (sort === "size") {
      copy.sort(
        (a, b) => b.size_bytes - a.size_bytes || a.id.localeCompare(b.id, "ko"),
      );
    } else {
      copy.sort((a, b) => a.id.localeCompare(b.id, "ko"));
    }
    return copy;
  }, [models, search, sort]);

  const onNavCatalog = useCallback(() => {
    if (typeof window !== "undefined") {
      window.dispatchEvent(new CustomEvent(NAV_EVENT, { detail: "catalog" }));
    }
  }, []);

  const selectedDisplayName = selectedKind
    ? RUNTIME_META[selectedKind]?.display_name ?? selectedKind
    : "";

  return (
    <div className="runtimes-root">
      <header className="runtimes-page-header">
        <div className="runtimes-page-header-text">
          <h2 className="runtimes-page-title">{t("screens.runtimes.title")}</h2>
          <p className="runtimes-page-subtitle">
            {t("screens.runtimes.subtitle")}
          </p>
        </div>
        <div
          className="runtimes-summary"
          role="status"
          aria-live="polite"
          aria-label={t("screens.runtimes.summary.runningCount", {
            running: summary.running,
            total: summary.total,
          })}
        >
          <span className="num">{summary.running}</span>
          <span className="runtimes-summary-sep">/</span>
          <span className="num">{summary.total}</span>
          <span className="runtimes-summary-label">
            {t("screens.runtimes.summary.runningCount", {
              running: summary.running,
              total: summary.total,
            })
              .split(/\s+/)
              .pop()}
          </span>
        </div>
      </header>

      <div className="runtimes-shell">
        <aside
          className="runtimes-sidebar"
          aria-label={t("screens.runtimes.title")}
        >
          {!statusesLoaded ? (
            <p className="runtimes-empty">…</p>
          ) : statuses.length === 0 ? (
            <p className="runtimes-empty">…</p>
          ) : (
            statuses.map((s) => {
              const meta = RUNTIME_META[s.kind];
              const isActive = s.kind === selectedKind;
              const cardId = `runtime-card-${s.kind}`;
              return (
                <article
                  key={s.kind}
                  role="region"
                  aria-labelledby={`${cardId}-title`}
                  className={`runtimes-card${isActive ? " is-active" : ""}`}
                  onClick={() => setSelectedKind(s.kind)}
                  data-testid={`runtime-card-${s.kind}`}
                >
                  <header className="runtimes-card-header">
                    <div className="runtimes-card-titlebar">
                      <h3
                        id={`${cardId}-title`}
                        className="runtimes-card-title"
                      >
                        {meta?.display_name ?? s.kind}
                      </h3>
                      <StatusPill
                        status={statusToPill(s)}
                        label={pillLabel(t, s)}
                        detail={
                          s.latency_ms != null ? `${s.latency_ms}ms` : null
                        }
                        size="sm"
                      />
                    </div>
                    <div className="runtimes-card-meta">
                      {s.installed && s.version && (
                        <span className="runtimes-card-version num">
                          v{s.version}
                        </span>
                      )}
                      {meta && (
                        <span className="runtimes-card-port num">
                          :{meta.port}
                        </span>
                      )}
                    </div>
                  </header>
                  <div className="runtimes-card-body">
                    {!s.installed ? (
                      <p className="runtimes-card-line runtimes-card-line-warn">
                        {t("screens.runtimes.card.notInstalled")}
                      </p>
                    ) : (
                      <>
                        <p className="runtimes-card-line">
                          {t("screens.runtimes.card.modelCount", {
                            count: s.model_count,
                          })}
                        </p>
                        {s.last_ping_at && (
                          <p className="runtimes-card-line runtimes-card-line-muted">
                            {t("screens.runtimes.card.lastPing", {
                              seconds: secondsAgo(s.last_ping_at),
                            })}
                          </p>
                        )}
                      </>
                    )}
                  </div>
                  <footer
                    className="runtimes-card-footer"
                    aria-label={t("screens.runtimes.card.actions.disabledHint")}
                  >
                    <button
                      type="button"
                      className="runtimes-card-action"
                      disabled
                      title={t("screens.runtimes.card.actions.disabledHint")}
                    >
                      {t("screens.runtimes.card.actions.stop")}
                    </button>
                    <button
                      type="button"
                      className="runtimes-card-action"
                      disabled
                      title={t("screens.runtimes.card.actions.disabledHint")}
                    >
                      {t("screens.runtimes.card.actions.restart")}
                    </button>
                    <button
                      type="button"
                      className="runtimes-card-action"
                      disabled
                      title={t("screens.runtimes.card.actions.disabledHint")}
                    >
                      {t("screens.runtimes.card.actions.logs")}
                    </button>
                  </footer>
                </article>
              );
            })
          )}
        </aside>

        <main
          className="runtimes-main"
          aria-label={selectedDisplayName}
        >
          <div className="runtimes-toolbar" role="toolbar">
            <input
              type="search"
              className="runtimes-search"
              placeholder={t("screens.runtimes.models.search")}
              aria-label={t("screens.runtimes.models.search")}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
            <label className="runtimes-sort">
              <span className="runtimes-sort-label">
                {t("screens.runtimes.models.sort.name")}
              </span>
              <select
                value={sort}
                onChange={(e) => setSort(e.target.value as SortKey)}
                aria-label={t("screens.runtimes.models.sort.name")}
              >
                <option value="name">
                  {t("screens.runtimes.models.sort.name")}
                </option>
                <option value="size">
                  {t("screens.runtimes.models.sort.size")}
                </option>
              </select>
            </label>
          </div>

          <div className="runtimes-table-header" role="presentation">
            <span className="runtimes-col-name">
              {t("screens.runtimes.models.column.name")}
            </span>
            <span className="runtimes-col-size">
              {t("screens.runtimes.models.column.size")}
            </span>
            <span className="runtimes-col-digest">
              {t("screens.runtimes.models.column.digest")}
            </span>
          </div>

          {modelsLoading ? (
            <div className="runtimes-table-loading">…</div>
          ) : visibleModels.length === 0 ? (
            <div className="runtimes-vlist-empty-wrap">
              <EmptyState
                title={t("screens.runtimes.models.empty.title")}
                body={t("screens.runtimes.models.empty.body")}
                cta={t("screens.runtimes.models.empty.cta")}
                onCta={onNavCatalog}
              />
            </div>
          ) : (
            <VirtualList<RuntimeModelView>
              items={visibleModels}
              rowHeight={24}
              keyOf={(m) => `${m.runtime_kind}:${m.id}`}
              ariaLabel={selectedDisplayName}
              renderRow={(m) => (
                <div className="runtimes-row">
                  <span className="runtimes-cell-name mono">{m.id}</span>
                  <span className="runtimes-cell-size num">
                    {formatSize(m.size_bytes)}
                  </span>
                  <span className="runtimes-cell-digest mono">
                    {(m.digest || "").slice(0, 8)}
                  </span>
                </div>
              )}
              className="runtimes-vlist"
              height="100%"
            />
          )}
        </main>
      </div>
    </div>
  );
}

interface EmptyStateProps {
  title: string;
  body: string;
  cta: string;
  onCta: () => void;
}

function EmptyState({ title, body, cta, onCta }: EmptyStateProps) {
  return (
    <div className="runtimes-empty-state" role="status" aria-live="polite">
      <p className="runtimes-empty-title">{title}</p>
      <p className="runtimes-empty-body">{body}</p>
      <button
        type="button"
        className="runtimes-empty-cta"
        onClick={onCta}
      >
        {cta}
      </button>
    </div>
  );
}

function statusToPill(s: RuntimeStatus): PillStatus {
  if (!s.installed) return "idle";
  if (s.running) return "listening";
  return "failed";
}

function pillLabel(
  t: (k: string, opts?: Record<string, unknown>) => string,
  s: RuntimeStatus,
): string {
  if (!s.installed) return t("screens.runtimes.card.notInstalled");
  if (s.running) return t("gateway.status.listening");
  return t("gateway.status.failed");
}

function formatSize(bytes: number): string {
  if (bytes <= 0) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let n = bytes;
  let i = 0;
  while (n >= 1024 && i < units.length - 1) {
    n /= 1024;
    i += 1;
  }
  // 1 decimal for MB+, 0 for KB/B.
  if (i <= 1) return `${Math.round(n)} ${units[i]}`;
  return `${n.toFixed(1)} ${units[i]}`;
}

function secondsAgo(rfc3339: string): number {
  const t = Date.parse(rfc3339);
  if (Number.isNaN(t)) return 0;
  const diff = Math.max(0, Math.floor((Date.now() - t) / 1000));
  return diff;
}
