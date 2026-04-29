// ApiKeysPanel — Settings 화면의 키 목록 + 발급 + 회수.
//
// 정책 (ADR-0022 §5, §10):
// - prefix만 노출. 평문은 발급 시 1회만 modal에서 노출.
// - revoke는 confirm 후 idempotent.
// - 빈 상태 + 회수된 키는 dim 표시.

import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  listApiKeys,
  revokeApiKey,
  type ApiKeyView,
  type CreatedKey,
} from "../../ipc/keys";

import { HelpButton } from "../HelpButton";

import { ApiKeyIssueModal } from "./ApiKeyIssueModal";

import "./keys.css";

export function ApiKeysPanel() {
  const { t } = useTranslation();
  const [keys, setKeys] = useState<ApiKeyView[]>([]);
  const [showModal, setShowModal] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const list = await listApiKeys();
      setKeys(list);
    } catch (e) {
      console.warn("listApiKeys failed:", e);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handleRevoke = useCallback(
    async (id: string) => {
      const ok = window.confirm(t("keys.actions.revokeConfirm"));
      if (!ok) return;
      try {
        await revokeApiKey(id);
        await refresh();
      } catch (e) {
        console.warn("revokeApiKey failed:", e);
        setError(t("keys.errors.revokeFailed"));
      }
    },
    [refresh, t],
  );

  const handleCreated = useCallback(
    (_k: CreatedKey) => {
      // refresh는 modal close 후. created 키도 목록에 표시.
      refresh();
    },
    [refresh],
  );

  return (
    <section className="keys-panel" aria-labelledby="keys-panel-title">
      <header className="keys-panel-header">
        <div>
          <div className="keys-title-row">
            <h2 id="keys-panel-title" className="keys-panel-title">
              {t("keys.title")}
            </h2>
            <HelpButton
              sectionId="api-keys"
              hint={t("screens.help.apiKeys") ?? undefined}
              testId="keys-help"
            />
          </div>
          <p className="keys-panel-subtitle">{t("keys.subtitle")}</p>
        </div>
        <button
          type="button"
          className="keys-button-primary"
          onClick={() => setShowModal(true)}
        >
          {t("keys.create")}
        </button>
      </header>

      {error && (
        <p className="keys-error" role="alert">
          {error}
        </p>
      )}

      {keys.length === 0 ? (
        <div className="keys-empty">
          <h3 className="keys-empty-title">{t("keys.empty.title")}</h3>
          <p className="keys-empty-body">{t("keys.empty.body")}</p>
        </div>
      ) : (
        <table className="keys-table" data-testid="keys-table">
          <thead>
            <tr>
              <th>{t("keys.table.alias")}</th>
              <th>{t("keys.table.prefix")}</th>
              <th>{t("keys.table.scope")}</th>
              <th>{t("keys.table.created")}</th>
              <th>{t("keys.table.lastUsed")}</th>
              <th>{t("keys.table.status")}</th>
              <th aria-label="actions" />
            </tr>
          </thead>
          <tbody>
            {keys.map((k) => (
              <tr
                key={k.id}
                className={k.revoked_at ? "keys-row is-revoked" : "keys-row"}
              >
                <td>{k.alias}</td>
                <td className="num">{k.key_prefix}</td>
                <td className="keys-scope-cell">
                  {k.scope.allowed_origins.join(", ") || "—"}
                </td>
                <td>{formatDate(k.created_at)}</td>
                <td>
                  {k.last_used_at ? formatDate(k.last_used_at) : t("keys.neverUsed")}
                </td>
                <td>
                  <span
                    className={`keys-status keys-status-${
                      k.revoked_at ? "revoked" : "active"
                    }`}
                  >
                    {k.revoked_at
                      ? t("keys.status.revoked")
                      : t("keys.status.active")}
                  </span>
                </td>
                <td>
                  {!k.revoked_at && (
                    <button
                      type="button"
                      className="keys-button-secondary"
                      onClick={() => handleRevoke(k.id)}
                    >
                      {t("keys.actions.revoke")}
                    </button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {showModal && (
        <ApiKeyIssueModal
          onClose={() => setShowModal(false)}
          onCreated={handleCreated}
        />
      )}
    </section>
  );
}

function formatDate(iso: string): string {
  // ISO 그대로 표시 — UI 단계 단순화. v1.x에 한국어 상대시각 (방금 / N분 전).
  return iso.slice(0, 10);
}
