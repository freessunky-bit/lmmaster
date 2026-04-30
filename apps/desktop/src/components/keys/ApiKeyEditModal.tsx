// ApiKeyEditModal — Phase 13'.c. 기존 키의 scope 전체 편집.
//
// 정책:
// - alias / key_prefix는 read-only.
// - models / endpoints / origins / expires_at / pipelines override 편집 가능.
// - 평문 재발급 없이 필터만 갱신 (backend update_api_key_scope).
// - 빈 endpoints + 빈 models 콤보는 backend가 거부 → 사용자에게 한국어 에러 노출.
// - Esc / 배경 클릭 닫기 + auto-focus.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  SEED_PIPELINE_IDS,
  updateApiKeyScope,
  type ApiKeyScope,
  type ApiKeyView,
  type SeedPipelineId,
} from "../../ipc/keys";

export interface ApiKeyEditModalProps {
  apiKey: ApiKeyView;
  onClose: () => void;
  onSaved: () => void;
}

export function ApiKeyEditModal({ apiKey, onClose, onSaved }: ApiKeyEditModalProps) {
  const { t } = useTranslation();
  const [origins, setOrigins] = useState<string[]>(
    apiKey.scope.allowed_origins.length > 0 ? apiKey.scope.allowed_origins : [""],
  );
  const [models, setModels] = useState(apiKey.scope.models.join(", "));
  const [endpoints, setEndpoints] = useState(apiKey.scope.endpoints.join(", "));
  const [expiresAt, setExpiresAt] = useState(apiKey.scope.expires_at ?? "");

  const [useGlobalPipelines, setUseGlobalPipelines] = useState(
    apiKey.scope.enabled_pipelines == null,
  );
  const initialPipelines = useMemo(() => {
    const set = new Set(apiKey.scope.enabled_pipelines ?? []);
    return SEED_PIPELINE_IDS.reduce(
      (acc, id) => {
        acc[id] = set.has(id);
        return acc;
      },
      {} as Record<SeedPipelineId, boolean>,
    );
  }, [apiKey.scope.enabled_pipelines]);
  const [keyEnabledPipelines, setKeyEnabledPipelines] =
    useState<Record<SeedPipelineId, boolean>>(initialPipelines);

  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const firstFieldRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    firstFieldRef.current?.focus();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const setOrigin = (idx: number, value: string) => {
    setOrigins((prev) => prev.map((o, i) => (i === idx ? value : o)));
  };
  const addOrigin = () => setOrigins((prev) => [...prev, ""]);
  const removeOrigin = (idx: number) =>
    setOrigins((prev) => prev.filter((_, i) => i !== idx));

  const handleSubmit = useCallback(async () => {
    setError(null);
    const cleanedOrigins = origins.map((o) => o.trim()).filter((o) => o.length > 0);
    const cleanedModels = models
      .split(",")
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
    const cleanedEndpoints = endpoints
      .split(",")
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
    if (cleanedModels.length === 0 && cleanedEndpoints.length === 0) {
      setError(t("keys.errors.emptyScope"));
      return;
    }
    const computedEnabledPipelines: string[] | null = useGlobalPipelines
      ? null
      : SEED_PIPELINE_IDS.filter((id) => keyEnabledPipelines[id]);
    const trimmedExpiry = expiresAt.trim();
    const newScope: ApiKeyScope = {
      models: cleanedModels.length > 0 ? cleanedModels : ["*"],
      endpoints: cleanedEndpoints.length > 0 ? cleanedEndpoints : ["/v1/*"],
      allowed_origins: cleanedOrigins,
      expires_at: trimmedExpiry.length > 0 ? trimmedExpiry : null,
      project_id: apiKey.scope.project_id ?? null,
      rate_limit: apiKey.scope.rate_limit ?? null,
      enabled_pipelines: computedEnabledPipelines,
    };
    setSubmitting(true);
    try {
      await updateApiKeyScope({ id: apiKey.id, scope: newScope });
      onSaved();
      onClose();
    } catch (e) {
      console.warn("updateApiKeyScope failed:", e);
      const errorObj = e as { kind?: string };
      if (errorObj.kind === "empty-scope") {
        setError(t("keys.errors.emptyScope"));
      } else {
        setError(t("keys.errors.updateFailed"));
      }
    } finally {
      setSubmitting(false);
    }
  }, [
    apiKey.id,
    apiKey.scope.project_id,
    apiKey.scope.rate_limit,
    origins,
    models,
    endpoints,
    expiresAt,
    useGlobalPipelines,
    keyEnabledPipelines,
    onClose,
    onSaved,
    t,
  ]);

  return (
    <div className="keys-modal-backdrop" role="presentation" onClick={onClose}>
      <div
        className="keys-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="keys-edit-modal-title"
        data-testid="keys-edit-modal"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="keys-modal-header">
          <h3 id="keys-edit-modal-title" className="keys-modal-title">
            {t("keys.editModal.title")}
          </h3>
          <p className="keys-modal-subtitle">{t("keys.editModal.subtitle")}</p>
        </header>
        <div className="keys-modal-body">
          <div className="keys-field is-readonly">
            <span className="keys-field-label">{t("keys.editModal.aliasLabel")}</span>
            <span className="keys-field-readonly">{apiKey.alias}</span>
          </div>
          <div className="keys-field is-readonly">
            <span className="keys-field-label">{t("keys.editModal.prefixLabel")}</span>
            <span className="keys-field-readonly num">{apiKey.key_prefix}</span>
          </div>

          <fieldset className="keys-field">
            <legend className="keys-field-label">{t("keys.modal.originLabel")}</legend>
            <p className="keys-field-hint">{t("keys.modal.originHint")}</p>
            <div className="keys-origin-list">
              {origins.map((o, i) => (
                <div key={i} className="keys-origin-row">
                  <input
                    ref={i === 0 ? firstFieldRef : undefined}
                    type="text"
                    className="keys-input"
                    placeholder={t("keys.modal.originPlaceholder")}
                    value={o}
                    onChange={(e) => setOrigin(i, e.target.value)}
                  />
                  {origins.length > 1 && (
                    <button
                      type="button"
                      className="keys-button-secondary"
                      onClick={() => removeOrigin(i)}
                      aria-label={t("keys.modal.removeOrigin")}
                    >
                      ×
                    </button>
                  )}
                </div>
              ))}
              <button
                type="button"
                className="keys-button-secondary"
                onClick={addOrigin}
              >
                {t("keys.modal.addOrigin")}
              </button>
            </div>
          </fieldset>

          <label className="keys-field">
            <span className="keys-field-label">{t("keys.modal.modelsLabel")}</span>
            <p className="keys-field-hint">{t("keys.modal.modelsHint")}</p>
            <input
              type="text"
              className="keys-input"
              value={models}
              onChange={(e) => setModels(e.target.value)}
            />
          </label>

          <label className="keys-field">
            <span className="keys-field-label">
              {t("keys.editModal.endpointsLabel")}
            </span>
            <p className="keys-field-hint">{t("keys.editModal.endpointsHint")}</p>
            <input
              type="text"
              className="keys-input"
              value={endpoints}
              onChange={(e) => setEndpoints(e.target.value)}
            />
          </label>

          <label className="keys-field">
            <span className="keys-field-label">{t("keys.editModal.expiresLabel")}</span>
            <input
              type="text"
              className="keys-input"
              placeholder={t("keys.editModal.expiresPlaceholder")}
              value={expiresAt}
              onChange={(e) => setExpiresAt(e.target.value)}
            />
          </label>

          <fieldset className="keys-field" data-testid="keys-edit-pipelines-fieldset">
            <legend className="keys-field-label">
              {t("keys.modal.pipelines.legend")}
            </legend>
            <label className="keys-checkbox-row">
              <input
                type="checkbox"
                checked={useGlobalPipelines}
                onChange={(e) => setUseGlobalPipelines(e.target.checked)}
              />
              <span>{t("keys.modal.pipelines.useGlobal")}</span>
            </label>
            <p className="keys-field-hint">
              {t("keys.modal.pipelines.useGlobalHint")}
            </p>
            <div
              className={`keys-pipelines-grid${useGlobalPipelines ? " is-disabled" : ""}`}
              aria-disabled={useGlobalPipelines}
            >
              {SEED_PIPELINE_IDS.map((id) => {
                const i18nIdKey =
                  id === "pii-redact"
                    ? "piiRedact"
                    : id === "token-quota"
                      ? "tokenQuota"
                      : id === "prompt-sanitize"
                        ? "promptSanitize"
                        : "observability";
                return (
                  <label key={id} className="keys-checkbox-row">
                    <input
                      type="checkbox"
                      disabled={useGlobalPipelines}
                      checked={keyEnabledPipelines[id]}
                      onChange={(e) =>
                        setKeyEnabledPipelines((prev) => ({
                          ...prev,
                          [id]: e.target.checked,
                        }))
                      }
                    />
                    <span>{t(`keys.modal.pipelines.ids.${i18nIdKey}`)}</span>
                  </label>
                );
              })}
            </div>
          </fieldset>

          {error && (
            <p className="keys-error" role="alert">
              {error}
            </p>
          )}
        </div>
        <footer className="keys-modal-footer">
          <button
            type="button"
            className="keys-button-secondary"
            onClick={onClose}
            disabled={submitting}
          >
            {t("keys.editModal.cancel")}
          </button>
          <button
            type="button"
            className="keys-button-primary"
            onClick={handleSubmit}
            disabled={submitting}
          >
            {t("keys.editModal.save")}
          </button>
        </footer>
      </div>
    </div>
  );
}
