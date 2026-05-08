// Phase 8'.c.4 (ADR-0066) — 게이트웨이 사내망 노출 토글 + LAN URL 자동 표시 UI.
//
// 정책:
// - 토글: role="switch" + aria-checked. ON → 게이트웨이 0.0.0.0 바인딩 (재시작 후 적용).
// - LAN IP 자동 감지 (RFC 1918 private 범위만). 다중 NIC 모두 노출.
// - 변경 시 "재시작 후 적용" 안내 (자동 hot-restart는 v1.x).
// - 회사 PC 보안 경고 카피 — 사용자가 무심코 켜는 시나리오 사전 차단.

import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  getGatewayAllowExternal,
  listLanAddresses,
  setGatewayAllowExternal,
} from "../ipc/gateway-settings";
import { getGatewayStatus } from "../ipc/gateway";

type LoadState =
  | { kind: "loading" }
  | { kind: "ready"; allow: boolean; lanIps: string[]; port: number | null }
  | { kind: "error"; message: string };

export function GatewayLanPanel() {
  const { t } = useTranslation();
  const [state, setState] = useState<LoadState>({ kind: "loading" });
  const [pendingRestart, setPendingRestart] = useState(false);
  const [savedAllow, setSavedAllow] = useState<boolean | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [allow, lanIps, gw] = await Promise.all([
        getGatewayAllowExternal(),
        listLanAddresses(),
        getGatewayStatus(),
      ]);
      setState({ kind: "ready", allow, lanIps, port: gw.port });
      setSavedAllow(allow);
    } catch (e) {
      setState({
        kind: "error",
        message: e instanceof Error ? e.message : String(e),
      });
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handleToggle = useCallback(async () => {
    if (state.kind !== "ready") return;
    const next = !state.allow;
    try {
      await setGatewayAllowExternal(next);
      setState({ ...state, allow: next });
      // savedAllow와 비교해 변경된 상태면 재시작 안내 노출.
      if (savedAllow !== null && savedAllow !== next) {
        setPendingRestart(true);
      } else {
        setPendingRestart(false);
      }
    } catch (e) {
      setState({
        kind: "error",
        message: e instanceof Error ? e.message : String(e),
      });
    }
  }, [state, savedAllow]);

  const handleCopy = useCallback(async (url: string) => {
    try {
      await navigator.clipboard.writeText(url);
    } catch (e) {
      console.warn("clipboard write failed:", e);
    }
  }, []);

  if (state.kind === "loading") {
    return (
      <fieldset className="settings-fieldset" data-testid="settings-gateway-lan">
        <legend className="settings-legend">
          {t("screens.settings.advanced.lanGateway.legend")}
        </legend>
        <p className="settings-hint">{t("common.loading")}</p>
      </fieldset>
    );
  }

  if (state.kind === "error") {
    return (
      <fieldset className="settings-fieldset" data-testid="settings-gateway-lan">
        <legend className="settings-legend">
          {t("screens.settings.advanced.lanGateway.legend")}
        </legend>
        <p
          className="settings-error"
          role="alert"
          data-testid="settings-gateway-lan-error"
        >
          {state.message}
        </p>
      </fieldset>
    );
  }

  const { allow, lanIps, port } = state;

  return (
    <fieldset className="settings-fieldset" data-testid="settings-gateway-lan">
      <legend className="settings-legend">
        {t("screens.settings.advanced.lanGateway.legend")}
      </legend>
      <p className="settings-hint">
        {t("screens.settings.advanced.lanGateway.hint")}
      </p>

      <div className="settings-toggle-row">
        <button
          type="button"
          role="switch"
          aria-checked={allow}
          aria-label={t("screens.settings.advanced.lanGateway.toggleLabel")}
          className={`settings-toggle${allow ? " is-on" : ""}`}
          onClick={handleToggle}
          data-testid="settings-gateway-lan-toggle"
        >
          <span className="settings-toggle-track" aria-hidden>
            <span className="settings-toggle-thumb" />
          </span>
        </button>
        <span>{t("screens.settings.advanced.lanGateway.toggleLabel")}</span>
      </div>

      <p
        className="settings-warning"
        role="note"
        data-testid="settings-gateway-lan-warning"
      >
        {t("screens.settings.advanced.lanGateway.companyPcWarning")}
      </p>

      {pendingRestart && (
        <p
          className="settings-hint settings-pending-restart"
          role="status"
          data-testid="settings-gateway-lan-pending-restart"
        >
          {t("screens.settings.advanced.lanGateway.pendingRestart")}
        </p>
      )}

      {allow && (
        <div
          className="settings-lan-urls"
          data-testid="settings-gateway-lan-urls"
        >
          <p className="settings-field-label">
            {t("screens.settings.advanced.lanGateway.urlsLabel")}
          </p>
          {lanIps.length === 0 ? (
            <p
              className="settings-hint"
              data-testid="settings-gateway-lan-empty"
            >
              {t("screens.settings.advanced.lanGateway.urlsEmpty")}
            </p>
          ) : (
            <ul className="settings-lan-url-list">
              {lanIps.map((ip) => {
                const url =
                  port != null
                    ? `http://${ip}:${port}`
                    : `http://${ip}:<포트>`;
                return (
                  <li
                    key={ip}
                    className="settings-lan-url-row"
                    data-testid={`settings-gateway-lan-url-${ip}`}
                  >
                    <code className="num">{url}</code>
                    {port != null && (
                      <button
                        type="button"
                        className="settings-btn-secondary"
                        onClick={() => handleCopy(url)}
                        aria-label={t(
                          "screens.settings.advanced.lanGateway.copyUrl",
                          { url },
                        )}
                      >
                        {t("screens.settings.advanced.lanGateway.copy")}
                      </button>
                    )}
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      )}
    </fieldset>
  );
}
