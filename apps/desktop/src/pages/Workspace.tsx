// Workspace — Phase 4.5'.b. Knowledge tab (ingest + search).
//
// 정책 (phase-5pb-4p5b-ipc-reinforcement.md, CLAUDE.md §4.1, §4.3):
// - 한국어 해요체. 디자인 토큰만.
// - a11y: 탭은 semantic <button>, role=tab + aria-selected. progressbar role/aria-valuenow.
// - Channel<IngestEvent> 기반 진행 상태 — terminal까지 stage 라벨 갱신.
// - workspace 단위 직렬화 — backend가 AlreadyIngesting 거부.
// - Stats display: 문서 N개 / 청크 N개. Knowledge IPC workspaceStats() 호출.
// - Search panel: top-k cosine, 결과 list + 빈 상태.

import {
  useCallback,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";

import { HelpButton } from "../components/HelpButton";
import { useActiveWorkspaceOptional } from "../contexts/ActiveWorkspaceContext";
import {
  cancelIngest,
  isTerminalIngestEvent,
  searchKnowledge,
  startIngest,
  workspaceStats,
  type IngestConfig,
  type IngestEvent,
  type IngestStage,
  type SearchHit,
  type StartIngestHandle,
  type WorkspaceStats,
} from "../ipc/knowledge";

import "./workspace.css";

// ── 상수 ─────────────────────────────────────────────────────────────

const TAB_KEYS = ["knowledge"] as const;
type TabKey = (typeof TAB_KEYS)[number];

const DEFAULT_K = 5;
const MAX_K = 20;

type IngestStatus = "idle" | "running" | "done" | "failed" | "cancelled";

interface IngestState {
  status: IngestStatus;
  /** 현재 stage — UI 라벨 키 (knowledge.stage.*). */
  stage: IngestStage | null;
  /** 0-100. */
  percent: number;
  /** 현재 처리 중 파일 (Reading) 또는 null. */
  currentPath: string | null;
  /** 진행 handle. cancel용. */
  handle: StartIngestHandle | null;
  /** ingest_id — handle.ingest_id 미러. */
  ingestId: string | null;
  /** terminal failed/cancelled의 한국어 메시지. */
  error: string | null;
  /** 완료 후 노출할 카운터. */
  filesProcessed: number;
  chunksCreated: number;
}

const INITIAL_INGEST: IngestState = {
  status: "idle",
  stage: null,
  percent: 0,
  currentPath: null,
  handle: null,
  ingestId: null,
  error: null,
  filesProcessed: 0,
  chunksCreated: 0,
};

type IngestAction =
  | { type: "START"; handle: StartIngestHandle }
  | { type: "EVENT"; event: IngestEvent }
  | { type: "START_FAILED"; message: string }
  | { type: "RESET" };

function ingestReducer(state: IngestState, action: IngestAction): IngestState {
  switch (action.type) {
    case "START":
      return {
        ...INITIAL_INGEST,
        status: "running",
        handle: action.handle,
        ingestId: action.handle.ingest_id,
        stage: "reading",
      };
    case "START_FAILED":
      return {
        ...INITIAL_INGEST,
        status: "failed",
        error: action.message,
      };
    case "EVENT":
      return applyIngestEvent(state, action.event);
    case "RESET":
      return INITIAL_INGEST;
  }
}

function applyIngestEvent(state: IngestState, event: IngestEvent): IngestState {
  switch (event.kind) {
    case "started":
      return {
        ...state,
        status: "running",
        stage: "reading",
        percent: 0,
        ingestId: event.ingest_id,
      };
    case "reading":
      return {
        ...state,
        stage: "reading",
        currentPath: event.current_path,
      };
    case "chunking":
    case "embedding":
    case "writing": {
      const total = event.total > 0 ? event.total : 1;
      const percent = Math.min(100, Math.round((event.processed / total) * 100));
      return {
        ...state,
        stage: event.kind,
        percent,
      };
    }
    case "done":
      return {
        ...state,
        status: "done",
        stage: "done",
        percent: 100,
        handle: null,
        filesProcessed: event.summary.files_processed,
        chunksCreated: event.summary.chunks_created,
      };
    case "failed":
      return {
        ...state,
        status: "failed",
        error: event.error,
        handle: null,
      };
    case "cancelled":
      return {
        ...state,
        status: "cancelled",
        handle: null,
      };
  }
}

// ── 컴포넌트 ─────────────────────────────────────────────────────────

export interface WorkspaceProps {
  /**
   * 외부에서 워크스페이스 식별자를 주입 — 테스트/스토리북용 override.
   * production은 ActiveWorkspaceContext에서 자동 주입.
   */
  workspaceId?: string;
  /** SQLite 파일 경로. 빈 string이면 backend가 in-memory (테스트용). */
  storePath?: string;
}

export function Workspace({ workspaceId, storePath = "" }: WorkspaceProps) {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<TabKey>("knowledge");
  // 우선순위: 명시 prop → context.active.id → null(로딩).
  // Provider가 없는 테스트 환경에선 기존 호환을 위해 prop이 없으면 null 처리되어 loading skeleton 노출.
  const ctx = useActiveWorkspaceOptional();
  const effectiveWorkspaceId = workspaceId ?? ctx?.active?.id ?? null;

  return (
    <div className="workspace-root" data-testid="workspace-page">
      <header className="workspace-topbar">
        <div>
          <div className="workspace-title-row">
            <h2 className="workspace-page-title">
              {t("screens.workspace.title")}
            </h2>
            <HelpButton
              sectionId="knowledge"
              hint={t("screens.help.workspace") ?? undefined}
              testId="workspace-help"
            />
          </div>
          <p className="workspace-page-subtitle">
            {t("screens.workspace.subtitle")}
          </p>
        </div>
      </header>

      <div role="tablist" aria-label={t("screens.workspace.title")} className="workspace-tabs">
        {TAB_KEYS.map((key) => {
          const selected = activeTab === key;
          return (
            <button
              key={key}
              type="button"
              role="tab"
              id={`workspace-tab-${key}`}
              aria-selected={selected}
              aria-controls={`workspace-panel-${key}`}
              tabIndex={selected ? 0 : -1}
              data-testid={`workspace-tab-${key}`}
              className="workspace-tab-trigger"
              onClick={() => setActiveTab(key)}
            >
              {t(`screens.workspace.tabs.${key}`)}
            </button>
          );
        })}
      </div>

      {activeTab === "knowledge" && (
        <div
          role="tabpanel"
          id="workspace-panel-knowledge"
          aria-labelledby="workspace-tab-knowledge"
        >
          {effectiveWorkspaceId ? (
            <KnowledgeTab
              workspaceId={effectiveWorkspaceId}
              storePath={storePath}
            />
          ) : (
            <p
              className="workspace-empty"
              data-testid="workspace-loading"
              aria-live="polite"
            >
              {t("screens.workspace.loading")}
            </p>
          )}
        </div>
      )}
    </div>
  );
}

// ── Knowledge Tab ────────────────────────────────────────────────────

interface KnowledgeTabProps {
  workspaceId: string;
  storePath: string;
}

function KnowledgeTab({ workspaceId, storePath }: KnowledgeTabProps) {
  const { t } = useTranslation();
  const [stats, setStats] = useState<WorkspaceStats | null>(null);
  const [statsError, setStatsError] = useState<string | null>(null);
  const [ingest, dispatch] = useReducer(ingestReducer, INITIAL_INGEST);
  const handleRef = useRef<StartIngestHandle | null>(null);

  // ingest config form state.
  const [path, setPath] = useState("");
  const [kind, setKind] = useState<"file" | "directory">("directory");

  // search panel state.
  const [query, setQuery] = useState("");
  const [k, setK] = useState<number>(DEFAULT_K);
  const [hits, setHits] = useState<SearchHit[] | null>(null);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);

  // handle ref — 언마운트 cleanup용.
  useEffect(() => {
    handleRef.current = ingest.handle;
  }, [ingest.handle]);

  useEffect(() => {
    return () => {
      const h = handleRef.current;
      if (h) {
        void h.cancel().catch(() => {
          /* idempotent */
        });
      }
    };
  }, []);

  // stats 로드 — workspaceId/storePath 변경 시 + ingest done 시.
  const refreshStats = useCallback(async () => {
    try {
      setStatsError(null);
      const s = await workspaceStats(workspaceId, storePath);
      setStats(s);
    } catch (e) {
      setStats(null);
      setStatsError(extractErrorMessage(e, t("screens.workspace.knowledge.errors.statsFailed")));
    }
  }, [workspaceId, storePath, t]);

  useEffect(() => {
    void refreshStats();
  }, [refreshStats]);

  useEffect(() => {
    if (ingest.status === "done") {
      void refreshStats();
    }
  }, [ingest.status, refreshStats]);

  const handleStart = useCallback(async () => {
    if (ingest.status === "running") return;
    if (!path.trim()) {
      dispatch({
        type: "START_FAILED",
        message: t("screens.workspace.knowledge.errors.missingPath"),
      });
      return;
    }
    const config: IngestConfig = {
      workspace_id: workspaceId,
      path: path.trim(),
      kind,
      store_path: storePath,
    };
    try {
      const handle = await startIngest(config, (ev) => {
        dispatch({ type: "EVENT", event: ev });
        if (isTerminalIngestEvent(ev)) {
          handleRef.current = null;
        }
      });
      dispatch({ type: "START", handle });
    } catch (e) {
      const msg = extractErrorMessage(e, t("screens.workspace.knowledge.errors.startFailed"));
      dispatch({ type: "START_FAILED", message: msg });
    }
  }, [ingest.status, kind, path, storePath, t, workspaceId]);

  const handleCancel = useCallback(async () => {
    if (ingest.handle) {
      try {
        await ingest.handle.cancel();
      } catch {
        // idempotent.
      }
    } else {
      try {
        await cancelIngest(workspaceId);
      } catch {
        // idempotent.
      }
    }
  }, [ingest.handle, workspaceId]);

  const handleSearch = useCallback(async () => {
    const q = query.trim();
    if (!q) {
      setHits([]);
      setSearchError(null);
      return;
    }
    setSearching(true);
    setSearchError(null);
    try {
      const results = await searchKnowledge(workspaceId, q, k, storePath);
      setHits(results);
    } catch (e) {
      setHits(null);
      setSearchError(extractErrorMessage(e, t("screens.workspace.knowledge.errors.searchFailed")));
    } finally {
      setSearching(false);
    }
  }, [k, query, storePath, t, workspaceId]);

  const stageLabel = useMemo(() => {
    if (ingest.status === "idle") return null;
    if (ingest.status === "done") return t("screens.workspace.knowledge.stage.done");
    if (ingest.status === "failed") return t("screens.workspace.knowledge.stage.failed");
    if (ingest.status === "cancelled") return t("screens.workspace.knowledge.stage.cancelled");
    if (ingest.stage) return t(`screens.workspace.knowledge.stage.${ingest.stage}`);
    return t("screens.workspace.knowledge.stage.reading");
  }, [ingest.stage, ingest.status, t]);

  const inputsDisabled = ingest.status === "running";

  return (
    <section
      className="workspace-section"
      role="region"
      aria-labelledby="workspace-knowledge-title"
    >
      <header className="workspace-section-header">
        <h3 id="workspace-knowledge-title" className="workspace-section-title">
          {t("screens.workspace.knowledge.title")}
        </h3>
        <p className="workspace-section-subtitle">
          {t("screens.workspace.knowledge.subtitle")}
        </p>
      </header>

      {statsError && (
        <p className="workspace-error" role="alert" data-testid="workspace-stats-error">
          {statsError}
        </p>
      )}

      <dl
        className="workspace-stats num"
        aria-label={t("screens.workspace.knowledge.statsAria")}
        data-testid="workspace-stats"
      >
        <div className="workspace-stat-item">
          <dt className="workspace-stat-label">
            {t("screens.workspace.knowledge.stats.documents")}
          </dt>
          <dd className="workspace-stat-value" data-testid="workspace-stat-documents">
            {stats ? stats.documents : 0}
          </dd>
        </div>
        <div className="workspace-stat-item">
          <dt className="workspace-stat-label">
            {t("screens.workspace.knowledge.stats.chunks")}
          </dt>
          <dd className="workspace-stat-value" data-testid="workspace-stat-chunks">
            {stats ? stats.chunks : 0}
          </dd>
        </div>
      </dl>

      {/* ── Ingest panel ─────────────────────────────────────────── */}
      <section
        className="workspace-panel"
        role="region"
        aria-labelledby="workspace-ingest-title"
        data-testid="workspace-ingest-panel"
      >
        <header className="workspace-section-header">
          <h4 id="workspace-ingest-title" className="workspace-section-title">
            {t("screens.workspace.knowledge.ingest.title")}
          </h4>
          <p className="workspace-section-subtitle">
            {t("screens.workspace.knowledge.ingest.subtitle")}
          </p>
        </header>

        <div className="workspace-form">
          <label className="workspace-field">
            <span className="workspace-field-label">
              {t("screens.workspace.knowledge.ingest.pathLabel")}
            </span>
            <input
              type="text"
              value={path}
              onChange={(e) => setPath(e.target.value)}
              disabled={inputsDisabled}
              className="workspace-input"
              placeholder={t("screens.workspace.knowledge.ingest.pathPlaceholder")}
              data-testid="workspace-ingest-path"
            />
            <span className="workspace-field-hint">
              {t("screens.workspace.knowledge.ingest.pathHint")}
            </span>
          </label>

          <fieldset
            className="workspace-radiogroup"
            role="radiogroup"
            aria-labelledby="workspace-ingest-kind-label"
          >
            <legend id="workspace-ingest-kind-label">
              {t("screens.workspace.knowledge.ingest.kindLabel")}
            </legend>
            {(["file", "directory"] as const).map((opt) => {
              const checked = kind === opt;
              return (
                <button
                  key={opt}
                  type="button"
                  role="radio"
                  aria-checked={checked}
                  className={`workspace-radio${checked ? " is-checked" : ""}`}
                  onClick={() => setKind(opt)}
                  disabled={inputsDisabled}
                  data-testid={`workspace-ingest-kind-${opt}`}
                >
                  {t(`screens.workspace.knowledge.ingest.kind.${opt}`)}
                </button>
              );
            })}
          </fieldset>
        </div>

        {ingest.status !== "idle" && (
          <div className="workspace-progress" data-testid="workspace-ingest-progress">
            <div className="workspace-progress-meta num">
              <span>
                <span className="workspace-stage-badge" data-testid="workspace-stage-badge">
                  {stageLabel}
                </span>
              </span>
              <span>{ingest.percent}%</span>
            </div>
            <div
              role="progressbar"
              aria-label={t("screens.workspace.knowledge.ingest.progressAria")}
              aria-valuenow={ingest.percent}
              aria-valuemin={0}
              aria-valuemax={100}
              className="workspace-progress-bar"
            >
              <div
                className="workspace-progress-fill"
                style={{ width: `${ingest.percent}%` }}
              />
            </div>
            {ingest.currentPath && (
              <p className="workspace-progress-message" aria-live="polite">
                {t("screens.workspace.knowledge.ingest.currentPath", {
                  path: ingest.currentPath,
                })}
              </p>
            )}
            {ingest.status === "done" && (
              <p className="workspace-progress-message" aria-live="polite" data-testid="workspace-ingest-summary">
                {t("screens.workspace.knowledge.ingest.summary", {
                  files: ingest.filesProcessed,
                  chunks: ingest.chunksCreated,
                })}
              </p>
            )}
          </div>
        )}

        {ingest.error && (
          <p className="workspace-error" role="alert" data-testid="workspace-ingest-error">
            {ingest.error}
          </p>
        )}

        <div className="workspace-actions">
          {ingest.status !== "running" && (
            <button
              type="button"
              className="workspace-button workspace-button-primary"
              onClick={handleStart}
              data-testid="workspace-ingest-start"
              disabled={!path.trim()}
            >
              {t("screens.workspace.knowledge.ingest.start")}
            </button>
          )}
          {ingest.status === "running" && (
            <button
              type="button"
              className="workspace-button workspace-button-secondary"
              onClick={handleCancel}
              data-testid="workspace-ingest-cancel"
            >
              {t("screens.workspace.knowledge.ingest.cancel")}
            </button>
          )}
          {(ingest.status === "done" ||
            ingest.status === "failed" ||
            ingest.status === "cancelled") && (
            <button
              type="button"
              className="workspace-button workspace-button-secondary"
              onClick={() => dispatch({ type: "RESET" })}
              data-testid="workspace-ingest-reset"
            >
              {t("screens.workspace.knowledge.ingest.reset")}
            </button>
          )}
        </div>
      </section>

      {/* ── Search panel ─────────────────────────────────────────── */}
      <section
        className="workspace-panel"
        role="region"
        aria-labelledby="workspace-search-title"
        data-testid="workspace-search-panel"
      >
        <header className="workspace-section-header">
          <h4 id="workspace-search-title" className="workspace-section-title">
            {t("screens.workspace.knowledge.search.title")}
          </h4>
          <p className="workspace-section-subtitle">
            {t("screens.workspace.knowledge.search.subtitle")}
          </p>
        </header>

        <div className="workspace-form">
          <label className="workspace-field">
            <span className="workspace-field-label">
              {t("screens.workspace.knowledge.search.queryLabel")}
            </span>
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              className="workspace-input"
              placeholder={t("screens.workspace.knowledge.search.queryPlaceholder")}
              data-testid="workspace-search-query"
            />
          </label>

          <label className="workspace-field">
            <span className="workspace-field-label">
              {t("screens.workspace.knowledge.search.kLabel")}
            </span>
            <input
              type="number"
              min={1}
              max={MAX_K}
              value={k}
              onChange={(e) =>
                setK(Math.min(MAX_K, Math.max(1, Number(e.target.value) || DEFAULT_K)))
              }
              className="workspace-input num"
              data-testid="workspace-search-k"
            />
            <span className="workspace-field-hint">
              {t("screens.workspace.knowledge.search.kHint", { max: MAX_K })}
            </span>
          </label>
        </div>

        <div className="workspace-actions">
          <button
            type="button"
            className="workspace-button workspace-button-primary"
            onClick={handleSearch}
            disabled={searching || !query.trim()}
            data-testid="workspace-search-submit"
          >
            {t("screens.workspace.knowledge.search.submit")}
          </button>
        </div>

        {searchError && (
          <p className="workspace-error" role="alert" data-testid="workspace-search-error">
            {searchError}
          </p>
        )}

        {hits !== null && hits.length === 0 && (
          <p className="workspace-empty" data-testid="workspace-search-empty">
            {t("screens.workspace.knowledge.search.empty")}
          </p>
        )}

        {hits !== null && hits.length > 0 && (
          <ul
            className="workspace-results"
            aria-label={t("screens.workspace.knowledge.search.title")}
            data-testid="workspace-search-results"
          >
            {hits.map((h) => (
              <li key={h.chunk_id} className="workspace-result-item" data-testid="workspace-search-hit">
                <div className="workspace-result-meta">
                  <span className="workspace-result-path">{h.document_path}</span>
                  <span className="workspace-result-score num">
                    {Math.round(h.score * 100)}%
                  </span>
                </div>
                <p className="workspace-result-content">{h.content}</p>
              </li>
            ))}
          </ul>
        )}
      </section>
    </section>
  );
}

// ── helpers ─────────────────────────────────────────────────────────

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

export default Workspace;
