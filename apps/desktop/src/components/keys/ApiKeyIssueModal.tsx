// ApiKeyIssueModal — 신규 발급 + 1회 reveal 모달.
//
// 정책 (ADR-0022 §10):
// - alias 필수.
// - allowed_origins 1개 이상.
// - 발급 후 평문 표시 단계: 8초 자동 mask + 5분 후 modal auto-close.
// - 클립보드 카피 버튼.
// - Esc / 배경 클릭으로 닫기 (단 reveal 단계는 명시 close 버튼만).

import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  createApiKey,
  defaultWebScope,
  type ApiKeyScope,
  type CreatedKey,
} from "../../ipc/keys";

export interface ApiKeyIssueModalProps {
  onClose: () => void;
  onCreated: (key: CreatedKey) => void;
}

const AUTOMASK_SECONDS = 8;
const AUTOCLOSE_SECONDS = 300;

export function ApiKeyIssueModal({ onClose, onCreated }: ApiKeyIssueModalProps) {
  const { t } = useTranslation();
  const [alias, setAlias] = useState("");
  const [origins, setOrigins] = useState<string[]>([""]);
  const [models, setModels] = useState("*");
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

  const setOrigin = (idx: number, value: string) => {
    setOrigins((prev) => prev.map((o, i) => (i === idx ? value : o)));
  };

  const addOrigin = () => setOrigins((prev) => [...prev, ""]);
  const removeOrigin = (idx: number) =>
    setOrigins((prev) => prev.filter((_, i) => i !== idx));

  const handleSubmit = useCallback(async () => {
    setError(null);
    if (alias.trim().length === 0) {
      setError(t("keys.errors.emptyAlias"));
      return;
    }
    const cleanedOrigins = origins.map((o) => o.trim()).filter((o) => o.length > 0);
    if (cleanedOrigins.length === 0) {
      setError(t("keys.errors.emptyOrigin"));
      return;
    }
    const cleanedModels = models
      .split(",")
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
    const scope: ApiKeyScope = {
      ...defaultWebScope(cleanedOrigins[0]!),
      models: cleanedModels.length > 0 ? cleanedModels : ["*"],
      allowed_origins: cleanedOrigins,
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
  }, [alias, origins, models, t, onCreated]);

  if (isReveal && created) {
    return (
      <RevealStep
        plaintext={created.plaintext_once}
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
          <label className="keys-field">
            <span className="keys-field-label">{t("keys.modal.aliasLabel")}</span>
            <input
              ref={aliasRef}
              type="text"
              className="keys-input"
              placeholder={t("keys.modal.aliasPlaceholder")}
              value={alias}
              onChange={(e) => setAlias(e.target.value)}
            />
          </label>
          <fieldset className="keys-field">
            <legend className="keys-field-label">
              {t("keys.modal.originLabel")}
            </legend>
            <p className="keys-field-hint">{t("keys.modal.originHint")}</p>
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

// ── Reveal step ────────────────────────────────────────────────────────

interface RevealStepProps {
  plaintext: string;
  onClose: () => void;
  closeRef: React.RefObject<HTMLButtonElement>;
}

function RevealStep({ plaintext, onClose, closeRef }: RevealStepProps) {
  const { t } = useTranslation();
  const [maskedAt, setMaskedAt] = useState<number | null>(null);
  const [copied, setCopied] = useState(false);
  const [secondsLeft, setSecondsLeft] = useState(AUTOMASK_SECONDS);

  // 8초 카운트다운 + auto-mask.
  useEffect(() => {
    if (maskedAt !== null) return;
    if (secondsLeft <= 0) {
      setMaskedAt(Date.now());
      return;
    }
    const id = window.setTimeout(() => setSecondsLeft(secondsLeft - 1), 1000);
    return () => window.clearTimeout(id);
  }, [secondsLeft, maskedAt]);

  // 5분 auto-close.
  useEffect(() => {
    const id = window.setTimeout(onClose, AUTOCLOSE_SECONDS * 1000);
    return () => window.clearTimeout(id);
  }, [onClose]);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(plaintext);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch (e) {
      console.warn("clipboard write failed:", e);
    }
  }, [plaintext]);

  const masked = maskedAt !== null;
  const display = masked
    ? plaintext.slice(0, 11) + "·".repeat(Math.max(0, plaintext.length - 11))
    : plaintext;

  return (
    <div className="keys-modal-backdrop" role="presentation">
      <div
        className="keys-modal keys-reveal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="keys-reveal-title"
      >
        <header className="keys-modal-header">
          <h3 id="keys-reveal-title" className="keys-modal-title">
            {t("keys.modal.revealTitle")}
          </h3>
        </header>
        <div className="keys-modal-body">
          <p className="keys-reveal-body">{t("keys.modal.revealBody")}</p>
          <div className="keys-reveal-key num" data-testid="keys-reveal-key">
            {display}
          </div>
          {!masked && (
            <p className="keys-reveal-countdown" aria-live="polite">
              {t("keys.modal.revealAutomask", { seconds: secondsLeft })}
            </p>
          )}
        </div>
        <footer className="keys-modal-footer">
          <button
            type="button"
            className="keys-button-secondary"
            onClick={handleCopy}
            disabled={masked}
          >
            {copied ? t("keys.modal.revealCopied") : t("keys.modal.revealCopy")}
          </button>
          <button
            ref={closeRef}
            type="button"
            className="keys-button-primary"
            onClick={onClose}
          >
            {t("keys.modal.revealClose")}
          </button>
        </footer>
      </div>
    </div>
  );
}
