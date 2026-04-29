// CatalogRefreshPanel — Phase 1' integration. Settings → 카탈로그 카테고리에 mount.
//
// 정책 (CLAUDE.md §4.1, §4.3):
// - 한국어 해요체. "지금 갱신할게요" / "갱신 중이에요…" / "갱신 완료" 등.
// - <button> + role 명시. focus-visible ring은 토큰.
// - 로컬 상태만 보유 — IPC 호출 결과를 LastRefresh로 표시.
// - 자동 갱신은 백엔드 cron이 처리, 본 컴포넌트는 status + manual trigger만.

import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  getLastCatalogRefresh,
  onCatalogRefreshed,
  refreshCatalogNow,
  type LastRefresh,
} from "../ipc/catalog-refresh";

export function CatalogRefreshPanel() {
  const { t } = useTranslation();
  const [last, setLast] = useState<LastRefresh | null>(null);
  const [busy, setBusy] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  // 첫 마운트 — 마지막 결과 로드 + listener 등록.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    (async () => {
      try {
        const cached = await getLastCatalogRefresh();
        if (!cancelled) setLast(cached);
      } catch (e) {
        if (!cancelled)
          console.warn("getLastCatalogRefresh failed:", e);
      }
      try {
        unlisten = await onCatalogRefreshed((p) => {
          if (!cancelled) setLast(p);
        });
      } catch (e) {
        if (!cancelled)
          console.warn("onCatalogRefreshed listen failed:", e);
      }
    })();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const handleRefresh = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const r = await refreshCatalogNow();
      setLast(r);
    } catch (e) {
      console.warn("refreshCatalogNow failed:", e);
      setError("screens.settings.catalogRefresh.errorRefresh");
    } finally {
      setBusy(false);
    }
  }, []);

  return (
    <fieldset
      className="settings-fieldset"
      data-testid="catalog-refresh-panel"
    >
      <legend className="settings-legend">
        {t("screens.settings.catalogRefresh.title")}
      </legend>

      <p className="settings-hint">
        {t("screens.settings.catalogRefresh.intervalHint")}
      </p>

      <p
        className="settings-readonly-text"
        data-testid="catalog-refresh-last"
      >
        {last
          ? t("screens.settings.catalogRefresh.lastRefresh", {
              when: formatTimestamp(last.at_ms),
              fetched: last.fetched_count,
              failed: last.failed_count,
            })
          : t("screens.settings.catalogRefresh.never")}
      </p>

      {last && last.outcome !== "ok" && (
        <p className="settings-warning" role="status" aria-live="polite">
          {last.outcome === "partial"
            ? t("screens.settings.catalogRefresh.partial")
            : t("screens.settings.catalogRefresh.failed")}
        </p>
      )}

      <button
        type="button"
        className="settings-btn-primary"
        onClick={handleRefresh}
        disabled={busy}
        data-testid="catalog-refresh-now-btn"
      >
        {busy
          ? t("screens.settings.catalogRefresh.refreshing")
          : t("screens.settings.catalogRefresh.refreshNow")}
      </button>

      {error && (
        <p className="settings-error" role="alert">
          {t(error)}
        </p>
      )}
    </fieldset>
  );
}

/** UNIX epoch ms → YYYY-MM-DD HH:mm 형식. v1.x에 한국어 상대시각 유지. */
function formatTimestamp(ms: number): string {
  if (!ms) return "-";
  const d = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}
