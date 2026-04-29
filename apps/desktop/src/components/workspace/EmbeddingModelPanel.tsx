// EmbeddingModelPanel — Phase 9'.a (ADR-0042).
//
// 정책 (CLAUDE.md §4.1, §4.3):
// - 한국어 해요체. 디자인 토큰만.
// - a11y: 카드는 role=group + aria-labelledby. 활성 표시는 role=radiogroup + aria-checked.
// - 다운로드 진행률은 progressbar role + aria-valuenow.
// - HelpButton과 일관 — 외부 통신 안내 (HuggingFace 호출) 명시.
// - 외부 통신 0 원칙 예외: HuggingFace `huggingface.co` 만 화이트리스트, 사용자 명시 클릭으로만 호출.

import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  cancelEmbeddingDownload,
  isTerminalEmbeddingDownloadEvent,
  listEmbeddingModels,
  setActiveEmbeddingModel,
  startEmbeddingDownload,
  type DownloadEmbeddingHandle,
  type EmbeddingDownloadEvent,
  type EmbeddingModelInfo,
} from "../../ipc/knowledge";

interface DownloadStatus {
  status: "idle" | "running" | "done" | "failed" | "cancelled";
  /** 0 ~ 100. */
  percent: number;
  /** 한국어 해요체 메시지 (실패 / 취소 / 진행 중 파일명). */
  message: string | null;
  handle: DownloadEmbeddingHandle | null;
}

const INITIAL_DOWNLOAD: DownloadStatus = {
  status: "idle",
  percent: 0,
  message: null,
  handle: null,
};

export interface EmbeddingModelPanelProps {
  /** 테스트 hook — 자동 polling 빈도 (ms). 기본 5000. */
  refreshIntervalMs?: number;
}

export function EmbeddingModelPanel({
  refreshIntervalMs = 5000,
}: EmbeddingModelPanelProps) {
  const { t } = useTranslation();
  const [models, setModels] = useState<EmbeddingModelInfo[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [downloads, setDownloads] = useState<Record<string, DownloadStatus>>({});
  const [activatingKind, setActivatingKind] = useState<string | null>(null);
  const [activateError, setActivateError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setLoadError(null);
      const list = await listEmbeddingModels();
      setModels(list);
    } catch (e) {
      setModels(null);
      setLoadError(
        extractErrorMessage(
          e,
          t("screens.workspace.embeddingModels.errors.loadFailed"),
        ),
      );
    }
  }, [t]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // refreshIntervalMs마다 자동 polling — 활성 다운로드 중에도 카드 상태 동기화.
  useEffect(() => {
    if (refreshIntervalMs <= 0) return;
    const id = setInterval(() => {
      void refresh();
    }, refreshIntervalMs);
    return () => clearInterval(id);
  }, [refresh, refreshIntervalMs]);

  const updateDownload = useCallback(
    (kind: string, partial: Partial<DownloadStatus>) => {
      setDownloads((prev) => {
        const current = prev[kind] ?? INITIAL_DOWNLOAD;
        return { ...prev, [kind]: { ...current, ...partial } };
      });
    },
    [],
  );

  const handleStart = useCallback(
    async (kind: string) => {
      updateDownload(kind, {
        status: "running",
        percent: 0,
        message: t("screens.workspace.embeddingModels.progress.starting"),
      });
      try {
        const handle = await startEmbeddingDownload(kind, (ev) => {
          applyDownloadEvent(ev, updateDownload, t);
          if (isTerminalEmbeddingDownloadEvent(ev)) {
            void refresh();
          }
        });
        updateDownload(kind, { handle });
      } catch (e) {
        updateDownload(kind, {
          status: "failed",
          message: extractErrorMessage(
            e,
            t("screens.workspace.embeddingModels.errors.downloadFailed"),
          ),
          handle: null,
        });
      }
    },
    [refresh, t, updateDownload],
  );

  const handleCancel = useCallback(
    async (kind: string) => {
      const current = downloads[kind];
      if (current?.handle) {
        try {
          await current.handle.cancel();
        } catch {
          // idempotent.
        }
      } else {
        try {
          await cancelEmbeddingDownload(kind);
        } catch {
          // idempotent.
        }
      }
    },
    [downloads],
  );

  const handleActivate = useCallback(
    async (kind: string) => {
      setActivatingKind(kind);
      setActivateError(null);
      try {
        await setActiveEmbeddingModel(kind);
        await refresh();
      } catch (e) {
        setActivateError(
          extractErrorMessage(
            e,
            t("screens.workspace.embeddingModels.errors.activateFailed"),
          ),
        );
      } finally {
        setActivatingKind(null);
      }
    },
    [refresh, t],
  );

  const sortedModels = useMemo(
    () =>
      models
        ? [...models].sort(
            (a, b) => b.korean_score - a.korean_score || a.dim - b.dim,
          )
        : null,
    [models],
  );

  return (
    <section
      className="workspace-panel"
      role="region"
      aria-labelledby="workspace-embed-title"
      data-testid="workspace-embed-panel"
    >
      <header className="workspace-section-header">
        <h4
          id="workspace-embed-title"
          className="workspace-section-title"
        >
          {t("screens.workspace.embeddingModels.title")}
        </h4>
        <p className="workspace-section-subtitle">
          {t("screens.workspace.embeddingModels.subtitle")}
        </p>
      </header>

      {loadError && (
        <p
          className="workspace-error"
          role="alert"
          data-testid="workspace-embed-load-error"
        >
          {loadError}
        </p>
      )}

      {activateError && (
        <p
          className="workspace-error"
          role="alert"
          data-testid="workspace-embed-activate-error"
        >
          {activateError}
        </p>
      )}

      {sortedModels === null && !loadError && (
        <p className="workspace-empty" aria-live="polite">
          {t("screens.workspace.embeddingModels.loading")}
        </p>
      )}

      {sortedModels && sortedModels.length === 0 && (
        <p
          className="workspace-empty"
          data-testid="workspace-embed-empty"
        >
          {t("screens.workspace.embeddingModels.empty")}
        </p>
      )}

      {sortedModels && sortedModels.length > 0 && (
        <ul
          className="workspace-results"
          role="radiogroup"
          aria-label={t("screens.workspace.embeddingModels.title")}
          data-testid="workspace-embed-list"
        >
          {sortedModels.map((m) => {
            const dl = downloads[m.kind] ?? INITIAL_DOWNLOAD;
            return (
              <li
                key={m.kind}
                className="workspace-result-item"
                role="radio"
                aria-checked={m.active}
                data-testid={`workspace-embed-card-${m.kind}`}
              >
                <div className="workspace-result-meta">
                  <span className="workspace-result-path">
                    {t(
                      `screens.workspace.embeddingModels.cards.${m.kind}.name`,
                    )}
                  </span>
                  <span className="workspace-result-score num">
                    {m.dim}d · {m.approx_size_mb}MB
                  </span>
                </div>
                <p className="workspace-result-content">
                  {t(
                    `screens.workspace.embeddingModels.cards.${m.kind}.description`,
                  )}
                </p>
                <p
                  className="workspace-field-hint"
                  data-testid={`workspace-embed-korean-${m.kind}`}
                >
                  {t("screens.workspace.embeddingModels.koreanScore", {
                    percent: Math.round(m.korean_score * 100),
                  })}
                </p>

                {dl.status === "running" && (
                  <div
                    className="workspace-progress"
                    data-testid={`workspace-embed-progress-${m.kind}`}
                  >
                    <div className="workspace-progress-meta num">
                      <span>
                        {dl.message ??
                          t(
                            "screens.workspace.embeddingModels.progress.downloading",
                          )}
                      </span>
                      <span>{dl.percent}%</span>
                    </div>
                    <div
                      role="progressbar"
                      aria-label={t(
                        "screens.workspace.embeddingModels.progress.aria",
                      )}
                      aria-valuenow={dl.percent}
                      aria-valuemin={0}
                      aria-valuemax={100}
                      className="workspace-progress-bar"
                    >
                      <div
                        className="workspace-progress-fill"
                        style={{ width: `${dl.percent}%` }}
                      />
                    </div>
                  </div>
                )}

                {dl.status === "failed" && dl.message && (
                  <p
                    className="workspace-error"
                    role="alert"
                    data-testid={`workspace-embed-error-${m.kind}`}
                  >
                    {dl.message}
                  </p>
                )}

                {dl.status === "cancelled" && (
                  <p
                    className="workspace-progress-message"
                    data-testid={`workspace-embed-cancelled-${m.kind}`}
                  >
                    {t(
                      "screens.workspace.embeddingModels.progress.cancelled",
                    )}
                  </p>
                )}

                <div className="workspace-actions">
                  {!m.downloaded && dl.status !== "running" && (
                    <button
                      type="button"
                      className="workspace-button workspace-button-primary"
                      onClick={() => handleStart(m.kind)}
                      data-testid={`workspace-embed-download-${m.kind}`}
                    >
                      {t("screens.workspace.embeddingModels.actions.download")}
                    </button>
                  )}
                  {dl.status === "running" && (
                    <button
                      type="button"
                      className="workspace-button workspace-button-secondary"
                      onClick={() => handleCancel(m.kind)}
                      data-testid={`workspace-embed-cancel-${m.kind}`}
                    >
                      {t("screens.workspace.embeddingModels.actions.cancel")}
                    </button>
                  )}
                  {m.downloaded && !m.active && (
                    <button
                      type="button"
                      className="workspace-button workspace-button-primary"
                      onClick={() => handleActivate(m.kind)}
                      disabled={activatingKind === m.kind}
                      data-testid={`workspace-embed-activate-${m.kind}`}
                    >
                      {t("screens.workspace.embeddingModels.actions.activate")}
                    </button>
                  )}
                  {m.active && (
                    <span
                      className="workspace-stage-badge"
                      data-testid={`workspace-embed-active-${m.kind}`}
                    >
                      {t(
                        "screens.workspace.embeddingModels.actions.activeBadge",
                      )}
                    </span>
                  )}
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

function applyDownloadEvent(
  ev: EmbeddingDownloadEvent,
  update: (kind: string, p: Partial<DownloadStatus>) => void,
  t: (key: string, opts?: Record<string, unknown>) => string,
): void {
  switch (ev.kind) {
    case "started":
      update(ev.model_kind, {
        status: "running",
        percent: 0,
        message: t("screens.workspace.embeddingModels.progress.fileStarting", {
          file: ev.file,
        }),
      });
      return;
    case "progress": {
      const total = ev.total ?? 0;
      const percent =
        total > 0 ? Math.min(100, Math.round((ev.downloaded / total) * 100)) : 0;
      update(ev.model_kind, {
        status: "running",
        percent,
        message: t("screens.workspace.embeddingModels.progress.fileDownloading", {
          file: ev.file,
        }),
      });
      return;
    }
    case "verifying":
      update(ev.model_kind, {
        message: t("screens.workspace.embeddingModels.progress.verifying"),
      });
      return;
    case "done":
      update(ev.model_kind, {
        status: "done",
        percent: 100,
        message: t("screens.workspace.embeddingModels.progress.done"),
        handle: null,
      });
      return;
    case "cancelled":
      update(ev.model_kind, {
        status: "cancelled",
        message: t("screens.workspace.embeddingModels.progress.cancelled"),
        handle: null,
      });
      return;
    case "failed":
      update(ev.model_kind, {
        status: "failed",
        message: ev.error,
        handle: null,
      });
      return;
  }
}

function extractErrorMessage(err: unknown, fallback: string): string {
  if (err && typeof err === "object") {
    const e = err as Record<string, unknown>;
    if (typeof e.message === "string" && e.message.length > 0) return e.message;
    if (typeof e.kind === "string") return `${e.kind}`;
  }
  if (err instanceof Error) return err.message || fallback;
  if (typeof err === "string") return err;
  return fallback;
}

export default EmbeddingModelPanel;
