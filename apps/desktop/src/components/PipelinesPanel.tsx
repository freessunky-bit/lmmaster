// PipelinesPanel — 게이트웨이 필터(Pipeline) 토글 + 감사 로그 노출. Phase 6'.c.
//
// 정책 (ADR-0025, phase-6p-updater-pipelines-decision.md §6):
// - 3종 v1 시드 토글 (pii-redact / token-quota / observability) — backend가 화이트리스트 검증.
// - 감사 로그는 backend ring buffer cap 200. 시간 역순(최신부터) 표시.
// - 한국어 해요체. design-system tokens만 사용.
// - a11y: switch role + aria-checked, log은 role="log" aria-live="polite", 키보드 친화.
// - 토글 실패 시 optimistic UI revert + 한국어 에러 메시지. 라벨/설명은 i18n 키 우선.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  clearAuditLog,
  getAuditLog,
  listPipelines,
  setPipelineEnabled,
  type AuditEntry,
  type PipelineDescriptor,
} from "../ipc/pipelines";

import "./pipelinesPanel.css";

/** 알려진 pipeline id — i18n 키 분기에 사용. */
type KnownPipelineId = "pii-redact" | "token-quota" | "observability";

const KNOWN_IDS: KnownPipelineId[] = [
  "pii-redact",
  "token-quota",
  "observability",
];

function isKnownId(id: string): id is KnownPipelineId {
  return (KNOWN_IDS as string[]).includes(id);
}

/** 알려진 pipeline id에 매핑되는 i18n 서브키 ("piiRedact" 등) — JSON 키 친화 형태. */
function i18nIdKey(id: KnownPipelineId): "piiRedact" | "tokenQuota" | "observability" {
  switch (id) {
    case "pii-redact":
      return "piiRedact";
    case "token-quota":
      return "tokenQuota";
    case "observability":
      return "observability";
  }
}

/** action variant — 색상 토큰 분기에 사용. */
type AuditAction = "passed" | "modified" | "blocked" | "other";

function classifyAction(action: string): AuditAction {
  switch (action) {
    case "passed":
    case "modified":
    case "blocked":
      return action;
    default:
      return "other";
  }
}

/** action별 i18n 키 — fallback 한국어 라벨은 i18n 누락 시에만 노출. */
function actionLabelKey(action: AuditAction): string {
  switch (action) {
    case "passed":
      return "screens.settings.pipelines.audit.actionPassed";
    case "modified":
      return "screens.settings.pipelines.audit.actionModified";
    case "blocked":
      return "screens.settings.pipelines.audit.actionBlocked";
    case "other":
      return "screens.settings.pipelines.audit.actionPassed";
  }
}

export function PipelinesPanel() {
  const { t } = useTranslation();
  const [pipelines, setPipelines] = useState<PipelineDescriptor[]>([]);
  const [auditEntries, setAuditEntries] = useState<AuditEntry[]>([]);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /** 마운트 후 첫 로드 완료 여부 — empty 상태 vs loading 구분. */
  const initialLoadDoneRef = useRef(false);

  // ── 첫 로드 ────────────────────────────────────────────────────────

  const refreshPipelines = useCallback(async () => {
    try {
      const list = await listPipelines();
      setPipelines(list);
    } catch (e) {
      console.warn("listPipelines failed:", e);
      setError("screens.settings.pipelines.errors.refreshFailed");
    }
  }, []);

  const refreshAudit = useCallback(async () => {
    setRefreshing(true);
    try {
      const entries = await getAuditLog(50);
      setAuditEntries(entries);
    } catch (e) {
      console.warn("getAuditLog failed:", e);
      setError("screens.settings.pipelines.errors.refreshFailed");
    } finally {
      setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      await Promise.all([refreshPipelines(), refreshAudit()]);
      if (!cancelled) {
        initialLoadDoneRef.current = true;
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [refreshPipelines, refreshAudit]);

  // ── Toggle 핸들러 — optimistic + revert ────────────────────────────

  const handleToggle = useCallback(
    async (id: string, currentEnabled: boolean) => {
      if (busyId !== null) return;
      setBusyId(id);
      setError(null);
      const next = !currentEnabled;
      // optimistic.
      setPipelines((prev) =>
        prev.map((p) => (p.id === id ? { ...p, enabled: next } : p)),
      );
      try {
        await setPipelineEnabled(id, next);
      } catch (e) {
        console.warn("setPipelineEnabled failed:", id, e);
        // revert.
        setPipelines((prev) =>
          prev.map((p) => (p.id === id ? { ...p, enabled: currentEnabled } : p)),
        );
        // backend 에러 kind에 따라 메시지 분기.
        if (
          typeof e === "object" &&
          e !== null &&
          "kind" in e &&
          (e as { kind?: string }).kind === "unknown-pipeline"
        ) {
          setError("screens.settings.pipelines.errors.unknownPipeline");
        } else {
          setError("screens.settings.pipelines.errors.toggleFailed");
        }
      } finally {
        setBusyId(null);
      }
    },
    [busyId],
  );

  const handleClear = useCallback(async () => {
    if (clearing) return;
    setClearing(true);
    setError(null);
    try {
      await clearAuditLog();
      setAuditEntries([]);
    } catch (e) {
      console.warn("clearAuditLog failed:", e);
      setError("screens.settings.pipelines.errors.clearFailed");
    } finally {
      setClearing(false);
    }
  }, [clearing]);

  const handleRefresh = useCallback(() => {
    void refreshAudit();
  }, [refreshAudit]);

  // ── i18n 라벨 helpers ──────────────────────────────────────────────

  const errorText = useMemo(() => (error ? t(error) : null), [error, t]);

  const localizedRows = useMemo(
    () =>
      pipelines.map((p) => {
        const known = isKnownId(p.id) ? i18nIdKey(p.id) : null;
        const nameKey = known
          ? `screens.settings.pipelines.pipelines.${known}.name`
          : null;
        const descKey = known
          ? `screens.settings.pipelines.pipelines.${known}.desc`
          : null;
        return {
          ...p,
          displayName: nameKey ? t(nameKey) : p.display_name_ko,
          description: descKey ? t(descKey) : p.description_ko,
        };
      }),
    [pipelines, t],
  );

  return (
    <fieldset className="settings-fieldset" data-testid="pipelines-panel">
      <legend className="settings-legend">
        {t("screens.settings.pipelines.title")}
      </legend>

      <p className="settings-hint pipelines-description">
        {t("screens.settings.pipelines.description")}
      </p>

      {/* ── 토글 목록 ─────────────────────────────────────────────── */}
      <ul
        className="pipelines-list"
        aria-label={t("screens.settings.pipelines.title")}
      >
        {localizedRows.map((p) => {
          const switchLabel = `${t(
            "screens.settings.pipelines.header.enabled",
          )}: ${p.displayName}`;
          return (
            <li
              key={p.id}
              className="pipelines-row"
              data-testid={`pipelines-row-${p.id}`}
            >
              <div className="pipelines-row-meta">
                <span className="pipelines-row-name">{p.displayName}</span>
                <span className="pipelines-row-desc">{p.description}</span>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={p.enabled}
                aria-label={switchLabel}
                disabled={busyId !== null && busyId !== p.id}
                className={`pipelines-toggle${p.enabled ? " is-on" : ""}${
                  busyId === p.id ? " is-busy" : ""
                }`}
                onClick={() => void handleToggle(p.id, p.enabled)}
                data-testid={`pipelines-toggle-${p.id}`}
              >
                <span className="pipelines-toggle-track" aria-hidden>
                  <span className="pipelines-toggle-thumb" />
                </span>
              </button>
            </li>
          );
        })}
      </ul>

      {/* ── 감사 로그 ─────────────────────────────────────────────── */}
      {/* role="log" + aria-live는 wrapper에. 내부 ol은 list role을 유지 — axe가 li parent로
          role=list 또는 ol/ul을 요구해요 (role="log"로 덮으면 list semantics 손상). */}
      <section
        className="pipelines-audit"
        role="log"
        aria-live="polite"
        aria-labelledby="pipelines-audit-title"
      >
        <header className="pipelines-audit-header">
          <h4 id="pipelines-audit-title" className="pipelines-audit-title">
            {t("screens.settings.pipelines.audit.title")}
          </h4>
          <div className="pipelines-audit-actions">
            <button
              type="button"
              className="settings-btn-secondary pipelines-btn-compact"
              onClick={handleRefresh}
              disabled={refreshing || clearing}
              data-testid="pipelines-audit-refresh"
            >
              {t("screens.settings.pipelines.audit.refresh")}
            </button>
            <button
              type="button"
              className="settings-btn-secondary pipelines-btn-compact"
              onClick={() => void handleClear()}
              disabled={clearing || refreshing || auditEntries.length === 0}
              data-testid="pipelines-audit-clear"
            >
              {t("screens.settings.pipelines.audit.clear")}
            </button>
          </div>
        </header>

        {auditEntries.length === 0 ? (
          <p
            className="pipelines-audit-empty"
            data-testid="pipelines-audit-empty"
          >
            {t("screens.settings.pipelines.audit.empty")}
          </p>
        ) : (
          <ol
            className="pipelines-audit-list"
            aria-label={t("screens.settings.pipelines.audit.title")}
            data-testid="pipelines-audit-list"
          >
            {auditEntries.map((entry, i) => {
              const action = classifyAction(entry.action);
              const known = isKnownId(entry.pipeline_id)
                ? i18nIdKey(entry.pipeline_id)
                : null;
              const nameLabel = known
                ? t(`screens.settings.pipelines.pipelines.${known}.name`)
                : entry.pipeline_id;
              return (
                <li
                  key={`${entry.timestamp_iso}-${i}`}
                  className="pipelines-audit-entry"
                  data-testid={`pipelines-audit-entry-${i}`}
                >
                  <span
                    className="pipelines-audit-pipeline"
                    data-testid={`pipelines-audit-entry-${i}-pipeline`}
                  >
                    {nameLabel}
                  </span>
                  <span
                    className={`pipelines-audit-action is-${action}`}
                    data-testid={`pipelines-audit-entry-${i}-action`}
                  >
                    {t(actionLabelKey(action))}
                  </span>
                  <time
                    className="pipelines-audit-timestamp num"
                    dateTime={entry.timestamp_iso}
                  >
                    {`${t(
                      "screens.settings.pipelines.audit.timestampPrefix",
                    )} ${entry.timestamp_iso}`}
                  </time>
                  {entry.details && (
                    <span
                      className="pipelines-audit-details"
                      data-testid={`pipelines-audit-entry-${i}-details`}
                    >
                      {`${t(
                        "screens.settings.pipelines.audit.detailsLabel",
                      )}: ${truncate(entry.details, 160)}`}
                    </span>
                  )}
                </li>
              );
            })}
          </ol>
        )}
      </section>

      {errorText && (
        <p className="settings-error" role="alert">
          {errorText}
        </p>
      )}
    </fieldset>
  );
}

/** 긴 details를 깔끔하게 자릅니다. UTF-16 length 기준 — 한국어/영어 동일 결. */
function truncate(s: string, max: number): string {
  if (s.length <= max) return s;
  return `${s.slice(0, max)}…`;
}
