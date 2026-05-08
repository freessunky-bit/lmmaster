// ApiKeyIssueModal — 신규 발급 + 1회 reveal 모달.
//
// 정책 (ADR-0022 §10, ADR-0029, ADR-0066):
// - alias 필수.
// - "어디서 호출?" 라디오 (network_scope: localhost / lan / any) — Origin 입력 치환.
// - 허용 모델 multi-select (전체 sentinel + 설치된 모델 체크박스). glob은 고급 설정.
// - 고급 설정 collapse — Origin / 경로 / 만료 / 키별 필터 (ADR-0029).
// - 발급 후 평문 표시 단계: 8초 자동 mask + 5분 후 modal auto-close.
// - 클립보드 카피 버튼.
// - Esc / 배경 클릭으로 닫기 (단 reveal 단계는 명시 close 버튼만).

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  createApiKey,
  defaultNoOriginScope,
  SEED_PIPELINE_IDS,
  type ApiKeyScope,
  type CreatedKey,
  type NetworkScope,
  type SeedPipelineId,
} from "../../ipc/keys";
import { listLocalLlamaCppModels } from "../../ipc/chat";
import { listRuntimeModels } from "../../ipc/runtimes";
import {
  getGatewayAllowExternal,
  listLanAddresses,
} from "../../ipc/gateway-settings";
import { getGatewayStatus } from "../../ipc/gateway";

import { ApiKeyRevealStep } from "./ApiKeyRevealStep";

export interface ApiKeyIssueModalProps {
  onClose: () => void;
  onCreated: (key: CreatedKey) => void;
  /** Settings → 사내망 노출 진입 라우트 트리거 — App.tsx에서 주입. */
  onOpenLanSettings?: () => void;
}

export function ApiKeyIssueModal({
  onClose,
  onCreated,
  onOpenLanSettings,
}: ApiKeyIssueModalProps) {
  const { t } = useTranslation();
  const [alias, setAlias] = useState("");
  // Phase 8'.c.4 — "어디서 호출?" 라디오. Origin URL 직접 입력 치환.
  const [networkScope, setNetworkScope] = useState<NetworkScope>("localhost");
  const [allowExternal, setAllowExternal] = useState<boolean>(false);
  // Phase 8'.c.4 — 모델 multi-select. selectAll = true → ["*"] sentinel.
  const [selectAllModels, setSelectAllModels] = useState(true);
  const [selectedModels, setSelectedModels] = useState<Set<string>>(new Set());
  const [installedModels, setInstalledModels] = useState<string[]>([]);
  const [loadingModels, setLoadingModels] = useState(true);
  // Phase 8'.c.4 — RevealStep으로 전달할 게이트웨이 포트 + LAN IPs.
  const [gatewayPort, setGatewayPort] = useState<number | null>(null);
  const [lanIps, setLanIps] = useState<string[]>([]);
  // 고급 설정 collapse — default 접힘.
  const [showAdvanced, setShowAdvanced] = useState(false);
  // 고급 설정 안의 기존 필드.
  const [origins, setOrigins] = useState<string[]>([""]);
  const [useGlobalPipelines, setUseGlobalPipelines] = useState(true);
  const [keyEnabledPipelines, setKeyEnabledPipelines] = useState<
    Record<SeedPipelineId, boolean>
  >({
    "pii-redact": true,
    "token-quota": true,
    observability: true,
    "prompt-sanitize": true,
  });
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [created, setCreated] = useState<CreatedKey | null>(null);
  const aliasRef = useRef<HTMLInputElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);

  const isReveal = created !== null;

  useEffect(() => {
    if (isReveal) {
      closeRef.current?.focus();
    } else {
      aliasRef.current?.focus();
    }
  }, [isReveal]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !isReveal) {
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [isReveal, onClose]);

  // 설치된 모델 + 게이트웨이 상태 + LAN IPs fetch — 실패한 어댑터는 graceful (빈 vec).
  useEffect(() => {
    let cancelled = false;
    Promise.all([
      listRuntimeModels("ollama").catch(() => []),
      listRuntimeModels("lm-studio").catch(() => []),
      listLocalLlamaCppModels().catch(() => []),
      getGatewayAllowExternal().catch(() => false),
      getGatewayStatus().catch(() => ({ port: null })),
      listLanAddresses().catch(() => [] as string[]),
    ])
      .then(([ollama, lmStudio, llamaCpp, allowExt, gw, lan]) => {
        if (cancelled) return;
        const ids = new Set<string>();
        for (const m of ollama) ids.add(m.id);
        for (const m of lmStudio) ids.add(m.id);
        for (const id of llamaCpp) ids.add(id);
        setInstalledModels(Array.from(ids).sort());
        setAllowExternal(allowExt);
        setGatewayPort(gw.port ?? null);
        setLanIps(lan);
        setLoadingModels(false);
      })
      .catch(() => {
        if (cancelled) return;
        setInstalledModels([]);
        setLoadingModels(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const setOrigin = (idx: number, value: string) => {
    setOrigins((prev) => prev.map((o, i) => (i === idx ? value : o)));
  };

  const addOrigin = () => setOrigins((prev) => [...prev, ""]);
  const removeOrigin = (idx: number) =>
    setOrigins((prev) => prev.filter((_, i) => i !== idx));

  const toggleModel = useCallback((id: string) => {
    setSelectedModels((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const computedModels = useMemo<string[]>(() => {
    if (selectAllModels) return ["*"];
    return Array.from(selectedModels).sort();
  }, [selectAllModels, selectedModels]);

  const handleSubmit = useCallback(async () => {
    setError(null);
    if (alias.trim().length === 0) {
      setError(t("keys.errors.emptyAlias"));
      return;
    }
    if (computedModels.length === 0) {
      setError(t("keys.errors.emptyModels"));
      return;
    }
    // Phase 8'.c.4 — origin은 optional. 고급 설정에서 입력 시에만 enforce.
    const cleanedOrigins = origins.map((o) => o.trim()).filter((o) => o.length > 0);
    const computedEnabledPipelines: string[] | null = useGlobalPipelines
      ? null
      : SEED_PIPELINE_IDS.filter((id) => keyEnabledPipelines[id]);
    const scope: ApiKeyScope = {
      ...defaultNoOriginScope(networkScope),
      models: computedModels,
      allowed_origins: cleanedOrigins,
      enabled_pipelines: computedEnabledPipelines,
    };
    setSubmitting(true);
    try {
      const issued = await createApiKey({ alias: alias.trim(), scope });
      setCreated(issued);
      onCreated(issued);
    } catch (e) {
      console.warn("createApiKey failed:", e);
      setError(t("keys.errors.createFailed"));
    } finally {
      setSubmitting(false);
    }
  }, [
    alias,
    networkScope,
    computedModels,
    origins,
    useGlobalPipelines,
    keyEnabledPipelines,
    t,
    onCreated,
  ]);

  if (isReveal && created) {
    // Phase 8'.c.4 — modelExample은 multi-select에서 선택된 모델 첫 번째 (없으면 첫 설치 모델, 그것도 없으면 placeholder).
    const modelExample =
      computedModels[0] && computedModels[0] !== "*"
        ? computedModels[0]
        : (installedModels[0] ?? "");
    return (
      <ApiKeyRevealStep
        plaintext={created.plaintext_once}
        keyPrefix={created.key_prefix}
        networkScope={networkScope}
        gatewayPort={gatewayPort}
        lanIps={lanIps}
        modelExample={modelExample}
        onClose={onClose}
        closeRef={closeRef}
      />
    );
  }

  return (
    <div className="keys-modal-backdrop" role="presentation" onClick={onClose}>
      <div
        className="keys-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="keys-modal-title"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="keys-modal-header">
          <h3 id="keys-modal-title" className="keys-modal-title">
            {t("keys.modal.createTitle")}
          </h3>
        </header>
        <div className="keys-modal-body">
          {/* 별칭 */}
          <label className="keys-field">
            <span className="keys-field-label">{t("keys.modal.aliasLabel")}</span>
            <input
              ref={aliasRef}
              type="text"
              className="keys-input"
              placeholder={t("keys.modal.aliasPlaceholder")}
              value={alias}
              onChange={(e) => setAlias(e.target.value)}
              data-testid="keys-modal-alias"
            />
          </label>

          {/* 어디서 호출? — Phase 8'.c.4 라디오 */}
          <fieldset className="keys-field" data-testid="keys-network-scope-fieldset">
            <legend className="keys-field-label">
              {t("keys.modal.network.legend")}
            </legend>
            <div role="radiogroup" className="keys-radio-group">
              <NetworkScopeRadio
                scope="localhost"
                checked={networkScope === "localhost"}
                onChange={() => setNetworkScope("localhost")}
                title={t("keys.modal.network.localhost.title")}
                hint={t("keys.modal.network.localhost.hint")}
              />
              <NetworkScopeRadio
                scope="lan"
                checked={networkScope === "lan"}
                onChange={() => setNetworkScope("lan")}
                title={t("keys.modal.network.lan.title")}
                hint={t("keys.modal.network.lan.hint")}
                extra={
                  networkScope === "lan" && !allowExternal && onOpenLanSettings ? (
                    <button
                      type="button"
                      className="keys-inline-link"
                      onClick={onOpenLanSettings}
                      data-testid="keys-network-scope-open-lan-settings"
                    >
                      {t("keys.modal.network.lan.openSettings")}
                    </button>
                  ) : null
                }
              />
              <NetworkScopeRadio
                scope="any"
                checked={networkScope === "any"}
                onChange={() => setNetworkScope("any")}
                title={t("keys.modal.network.any.title")}
                hint={t("keys.modal.network.any.hint")}
              />
            </div>
            {networkScope === "any" && (
              <p
                className="keys-field-warning"
                role="note"
                data-testid="keys-network-scope-any-warning"
              >
                {t("keys.modal.network.any.warning")}
              </p>
            )}
          </fieldset>

          {/* 모델 multi-select */}
          <fieldset className="keys-field" data-testid="keys-models-fieldset">
            <legend className="keys-field-label">
              {t("keys.modal.modelsLabel")}
            </legend>
            <label className="keys-checkbox-row">
              <input
                type="checkbox"
                checked={selectAllModels}
                onChange={(e) => setSelectAllModels(e.target.checked)}
                data-testid="keys-models-select-all"
              />
              <span>{t("keys.modal.models.selectAll")}</span>
            </label>
            <p className="keys-field-hint">
              {t("keys.modal.models.selectAllHint")}
            </p>
            {!selectAllModels && (
              <div
                className="keys-models-list"
                data-testid="keys-models-list"
              >
                {loadingModels && (
                  <p className="keys-field-hint">
                    {t("keys.modal.models.loading")}
                  </p>
                )}
                {!loadingModels && installedModels.length === 0 && (
                  <p
                    className="keys-field-hint"
                    data-testid="keys-models-empty"
                  >
                    {t("keys.modal.models.empty")}
                  </p>
                )}
                {installedModels.map((id) => (
                  <label
                    key={id}
                    className="keys-checkbox-row"
                    data-testid={`keys-models-cb-${id}`}
                  >
                    <input
                      type="checkbox"
                      checked={selectedModels.has(id)}
                      onChange={() => toggleModel(id)}
                    />
                    <span>{id}</span>
                  </label>
                ))}
              </div>
            )}
          </fieldset>

          {/* 고급 설정 collapse */}
          <fieldset className="keys-field keys-advanced">
            <button
              type="button"
              className="keys-advanced-toggle"
              onClick={() => setShowAdvanced((v) => !v)}
              aria-expanded={showAdvanced}
              data-testid="keys-advanced-toggle"
            >
              <span aria-hidden>{showAdvanced ? "▾" : "▸"}</span>
              <span>{t("keys.modal.advanced.legend")}</span>
            </button>
            {showAdvanced && (
              <div
                className="keys-advanced-body"
                data-testid="keys-advanced-body"
              >
                {/* Origin (이제 optional) */}
                <fieldset className="keys-field">
                  <legend className="keys-field-label">
                    {t("keys.modal.originLabel")}
                  </legend>
                  <p className="keys-field-hint">
                    {t("keys.modal.advanced.originHint")}
                  </p>
                  <div className="keys-origin-list">
                    {origins.map((o, i) => (
                      <div key={i} className="keys-origin-row">
                        <input
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

                {/* 키별 필터 — ADR-0029 */}
                <fieldset
                  className="keys-field"
                  data-testid="keys-pipelines-fieldset"
                >
                  <legend className="keys-field-label">
                    {t("keys.modal.pipelines.legend")}
                  </legend>
                  <label className="keys-checkbox-row">
                    <input
                      type="checkbox"
                      checked={useGlobalPipelines}
                      onChange={(e) => setUseGlobalPipelines(e.target.checked)}
                      data-testid="keys-pipelines-use-global"
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
                        <label
                          key={id}
                          className="keys-checkbox-row"
                          data-testid={`keys-pipelines-cb-${id}`}
                        >
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
                  {!useGlobalPipelines &&
                    SEED_PIPELINE_IDS.every((id) => !keyEnabledPipelines[id]) && (
                      <p
                        className="keys-field-warning"
                        role="alert"
                        data-testid="keys-pipelines-warn-empty"
                      >
                        {t("keys.modal.pipelines.warningEmpty")}
                      </p>
                    )}
                </fieldset>
              </div>
            )}
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
            {t("keys.modal.cancel")}
          </button>
          <button
            type="button"
            className="keys-button-primary"
            onClick={handleSubmit}
            disabled={submitting}
          >
            {t("keys.modal.submit")}
          </button>
        </footer>
      </div>
    </div>
  );
}

// ── Network scope radio ───────────────────────────────────────────────

interface NetworkScopeRadioProps {
  scope: NetworkScope;
  checked: boolean;
  onChange: () => void;
  title: string;
  hint: string;
  extra?: React.ReactNode;
}

function NetworkScopeRadio({
  scope,
  checked,
  onChange,
  title,
  hint,
  extra,
}: NetworkScopeRadioProps) {
  return (
    <label
      className="keys-radio-card"
      data-testid={`keys-network-scope-radio-${scope}`}
    >
      <input
        type="radio"
        name="keys-network-scope"
        value={scope}
        checked={checked}
        onChange={onChange}
      />
      <div className="keys-radio-card-body">
        <span className="keys-radio-card-title">{title}</span>
        <span className="keys-radio-card-hint">{hint}</span>
        {extra}
      </div>
    </label>
  );
}

// ── Reveal step은 ./ApiKeyRevealStep.tsx로 분리 (Phase 8'.c.4 / ADR-0066). ──
