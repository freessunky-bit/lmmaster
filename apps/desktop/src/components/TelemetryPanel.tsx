// TelemetryPanel — Phase 7'.a. Settings의 GeneralPanel에 들어가는 opt-in 토글.
//
// 정책 (ADR-0027 §5, phase-7p-release-prep-reinforcement.md §5.2):
// - 기본 비활성. 사용자가 토글을 켜면 backend가 anonymous UUID 발급.
// - 프롬프트 / 모델 출력은 절대 전송하지 않아요. 익명 사용 통계만.
// - 실제 endpoint(GlitchTip self-hosted) 연결은 Phase 7'.b — 본 v1은 config만.
// - a11y: switch role + aria-checked + aria-label. 한국어 해요체.
// - design-system tokens만 — Settings의 fieldset 패턴 일관.

import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  getTelemetryConfig,
  setTelemetryEnabled,
  type TelemetryConfig,
} from "../ipc/telemetry";

export function TelemetryPanel() {
  const { t } = useTranslation();
  const [config, setConfig] = useState<TelemetryConfig | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const cfg = await getTelemetryConfig();
        if (!cancelled) setConfig(cfg);
      } catch (e) {
        console.warn("getTelemetryConfig failed:", e);
        if (!cancelled) setError("screens.settings.telemetry.errors.loadFailed");
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const handleToggle = useCallback(async () => {
    if (busy || !config) return;
    setBusy(true);
    setError(null);
    const next = !config.enabled;
    // optimistic — 실패 시 revert.
    setConfig({ ...config, enabled: next });
    try {
      const updated = await setTelemetryEnabled(next);
      setConfig(updated);
    } catch (e) {
      console.warn("setTelemetryEnabled failed:", e);
      setConfig(config); // revert.
      setError("screens.settings.telemetry.errors.toggleFailed");
    } finally {
      setBusy(false);
    }
  }, [busy, config]);

  const errorText = useMemo(() => (error ? t(error) : null), [error, t]);
  const isOn = config?.enabled ?? false;

  return (
    <fieldset className="settings-fieldset" data-testid="telemetry-panel">
      <legend className="settings-legend">
        {t("screens.settings.telemetry.title")}
      </legend>

      <p className="settings-hint">
        {t("screens.settings.telemetry.description")}
      </p>

      <div className="settings-toggle-row">
        <button
          type="button"
          role="switch"
          aria-checked={isOn}
          aria-label={t("screens.settings.telemetry.toggleLabel")}
          aria-disabled={busy || !config}
          disabled={busy || !config}
          className={`settings-toggle${isOn ? " is-on" : ""}${busy ? " is-busy" : ""}`}
          onClick={() => void handleToggle()}
          data-testid="telemetry-toggle"
        >
          <span className="settings-toggle-track" aria-hidden>
            <span className="settings-toggle-thumb" />
          </span>
        </button>
        <span data-testid="telemetry-status-label">
          {isOn
            ? t("screens.settings.telemetry.statusOn")
            : t("screens.settings.telemetry.statusOff")}
        </span>
      </div>

      {config?.anon_id && isOn && (
        <p className="settings-readonly-text" data-testid="telemetry-anon-id-hint">
          <span style={{ color: "var(--text-muted)" }}>
            {t("screens.settings.telemetry.anonIdLabel")}:{" "}
          </span>
          <code className="num">{shortenUuid(config.anon_id)}</code>
        </p>
      )}

      <p className="settings-hint">
        {t("screens.settings.telemetry.privacyNote")}
      </p>

      {errorText && (
        <p className="settings-error" role="alert">
          {errorText}
        </p>
      )}
    </fieldset>
  );
}

/** UUID v4를 첫 8자만 표시 — 사용자가 식별 자체로 사용하지 않도록. */
function shortenUuid(id: string): string {
  if (id.length <= 8) return id;
  return `${id.slice(0, 8)}…`;
}
