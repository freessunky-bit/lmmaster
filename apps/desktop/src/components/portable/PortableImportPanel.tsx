// PortableImportPanel — Phase 11' (ADR-0039).
//
// 정책:
// - "워크스페이스 가져올게요" 버튼 → 파일 경로 입력 → 검증(verify_archive) → preview ("어떤 PC, 언제, NN GB").
// - 패스프레이즈 입력 (preview.has_keys=true 시).
// - conflict_policy 라디오 (skip / overwrite / rename).
// - "가져올게요" 버튼 → 진행률 stream.
// - 완료 후 repair_tier 안내 (green / yellow / red).
// - a11y: dialog role + aria-modal + Esc / 배경 클릭 닫기 (옵션 단계만).
// - 한국어 카피 해요체.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  cancelWorkspaceImport,
  isTerminalImportEvent,
  startWorkspaceImport,
  verifyWorkspaceArchive,
  type ArchivePreview,
  type ConflictPolicy,
  type ImportEvent,
  type ImportOptions,
  type RepairTier,
} from "../../ipc/portable";

import "./portable.css";

type Phase =
  | "idle"
  | "preview"
  | "options"
  | "running"
  | "done"
  | "failed";

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = bytes;
  let idx = 0;
  while (v >= 1024 && idx < units.length - 1) {
    v /= 1024;
    idx += 1;
  }
  return `${idx === 0 ? Math.round(v) : v.toFixed(1)} ${units[idx]}`;
}

interface DoneInfo {
  manifestSummary: string;
  repairTier: RepairTier;
}

export function PortableImportPanel() {
  const { t } = useTranslation();
  const [phase, setPhase] = useState<Phase>("idle");
  const [sourcePath, setSourcePath] = useState("");
  const [preview, setPreview] = useState<ArchivePreview | null>(null);
  const [verifying, setVerifying] = useState(false);
  const [passphrase, setPassphrase] = useState("");
  const [conflictPolicy, setConflictPolicy] =
    useState<ConflictPolicy>("rename");
  const [progress, setProgress] = useState({ processed: 0, total: 0 });
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState<DoneInfo | null>(null);
  const importIdRef = useRef<string | null>(null);
  const sourceInputRef = useRef<HTMLInputElement>(null);
  const cancelBtnRef = useRef<HTMLButtonElement>(null);

  const isOpen =
    phase === "preview" ||
    phase === "options" ||
    phase === "running";

  useEffect(() => {
    if (phase === "preview") {
      sourceInputRef.current?.focus();
    } else if (phase === "running") {
      cancelBtnRef.current?.focus();
    }
  }, [phase]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (
        e.key === "Escape" &&
        (phase === "preview" || phase === "options")
      ) {
        setPhase("idle");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [phase]);

  const reset = useCallback(() => {
    setPhase("idle");
    setSourcePath("");
    setPreview(null);
    setVerifying(false);
    setPassphrase("");
    setConflictPolicy("rename");
    setProgress({ processed: 0, total: 0 });
    setError(null);
    setDone(null);
    importIdRef.current = null;
  }, []);

  const handleVerify = useCallback(async () => {
    setError(null);
    if (sourcePath.trim().length === 0) {
      setError("screens.settings.portable.import.errors.emptySource");
      return;
    }
    setVerifying(true);
    try {
      const p = await verifyWorkspaceArchive(sourcePath.trim());
      setPreview(p);
      setPhase("options");
    } catch (e) {
      console.warn("verifyWorkspaceArchive failed:", e);
      const msg =
        e && typeof e === "object" && "message" in e
          ? `screens.settings.portable.import.errors.verify::${(e as { message: string }).message}`
          : "screens.settings.portable.import.errors.verify";
      setError(msg);
    } finally {
      setVerifying(false);
    }
  }, [sourcePath]);

  const handleStart = useCallback(async () => {
    if (!preview) return;
    setError(null);
    if (preview.has_keys && passphrase.trim().length === 0) {
      setError("screens.settings.portable.import.errors.emptyPassphrase");
      return;
    }
    const opts: ImportOptions = {
      source_path: sourcePath.trim(),
      target_workspace_root: null,
      key_passphrase: preview.has_keys ? passphrase : null,
      conflict_policy: conflictPolicy,
      expected_sha256: null,
    };
    setPhase("running");
    setProgress({ processed: 0, total: preview.entries_count });
    try {
      const res = await startWorkspaceImport(opts, (ev: ImportEvent) => {
        if (ev.kind === "extracting") {
          setProgress({
            processed: Number(ev.processed),
            total: Number(ev.total),
          });
        } else if (ev.kind === "done") {
          setDone({
            manifestSummary: ev.manifest_summary,
            repairTier: ev.repair_tier,
          });
        } else if (ev.kind === "failed") {
          setError(`screens.settings.portable.import.errors.runner::${ev.error}`);
        }
        if (isTerminalImportEvent(ev)) {
          // 별도 처리 — invoke의 await에서 추가 응답이 옴.
        }
      });
      importIdRef.current = res.import_id;
      setDone({
        manifestSummary: res.summary.manifest_summary,
        repairTier: res.summary.repair_tier,
      });
      setPhase("done");
    } catch (e) {
      console.warn("startWorkspaceImport failed:", e);
      // Phase R-D (ADR-0056) — kind-based i18n switch.
      // PathDenied → 전용 i18n 키 (한국어/영어 토글 시 자동 전환).
      // 기타 → thiserror Display(이미 한국어)을 runner:: prefix로 raw 노출.
      let msg: string;
      if (
        e &&
        typeof e === "object" &&
        "kind" in e &&
        (e as { kind: string }).kind === "path-denied"
      ) {
        msg = "errors.path-denied";
      } else if (e && typeof e === "object" && "message" in e) {
        msg = `screens.settings.portable.import.errors.runner::${(e as { message: string }).message}`;
      } else {
        msg = "screens.settings.portable.import.errors.start";
      }
      setError(msg);
      setPhase("failed");
    }
  }, [preview, sourcePath, passphrase, conflictPolicy]);

  const handleCancel = useCallback(async () => {
    if (!importIdRef.current) {
      setPhase("idle");
      return;
    }
    try {
      await cancelWorkspaceImport(importIdRef.current);
    } catch (e) {
      console.warn("cancelWorkspaceImport failed:", e);
    }
  }, []);

  const errorText = useMemo(() => {
    if (!error) return null;
    const idx = error.indexOf("::");
    if (idx > 0) {
      const key = error.slice(0, idx);
      const detail = error.slice(idx + 2);
      return `${t(key)} (${detail})`;
    }
    return t(error);
  }, [error, t]);

  const percent =
    progress.total > 0
      ? Math.min(100, Math.round((progress.processed / progress.total) * 100))
      : 0;

  return (
    <fieldset className="settings-fieldset" data-testid="portable-import-panel">
      <legend className="settings-legend">
        {t("screens.settings.portable.import.title")}
      </legend>
      <p className="settings-hint">
        {t("screens.settings.portable.import.subtitle")}
      </p>

      <button
        type="button"
        className="settings-btn-primary"
        onClick={() => setPhase("preview")}
        disabled={isOpen}
        data-testid="portable-import-start-btn"
      >
        {t("screens.settings.portable.import.start")}
      </button>

      {phase === "done" && done && (
        <div
          className="portable-result-card"
          role="status"
          aria-live="polite"
          data-testid="portable-import-done"
        >
          <p className="portable-result-headline">
            {t("screens.settings.portable.import.done")}
          </p>
          <p
            className={`portable-tier portable-tier-${done.repairTier}`}
            data-testid="portable-import-tier"
          >
            {t(`screens.settings.portable.import.tier.${done.repairTier}`)}
          </p>
          <p className="portable-result-summary num">{done.manifestSummary}</p>
          <button
            type="button"
            className="settings-btn-secondary"
            onClick={reset}
            data-testid="portable-import-reset-btn"
          >
            {t("screens.settings.portable.import.again")}
          </button>
        </div>
      )}

      {phase === "failed" && errorText && (
        <p className="settings-error" role="alert">
          {errorText}
        </p>
      )}

      {isOpen && (
        <div
          className="portable-modal-backdrop"
          role="presentation"
          onClick={() =>
            (phase === "preview" || phase === "options") && setPhase("idle")
          }
        >
          <div
            className="portable-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="portable-import-modal-title"
            onClick={(e) => e.stopPropagation()}
            data-testid="portable-import-modal"
          >
            <header className="portable-modal-header">
              <h3
                id="portable-import-modal-title"
                className="portable-modal-title"
              >
                {phase === "preview"
                  ? t("screens.settings.portable.import.dialogTitle")
                  : phase === "options"
                  ? t("screens.settings.portable.import.optionsTitle")
                  : t("screens.settings.portable.import.runningTitle")}
              </h3>
            </header>

            {phase === "preview" && (
              <div className="portable-modal-body">
                <label className="portable-input-field">
                  <span>
                    {t("screens.settings.portable.import.selectFile")}
                  </span>
                  <input
                    ref={sourceInputRef}
                    type="text"
                    className="settings-input num"
                    value={sourcePath}
                    onChange={(e) => setSourcePath(e.target.value)}
                    placeholder="C:\\Users\\me\\Desktop\\workspace.zip"
                    data-testid="portable-import-source-input"
                  />
                  <small className="settings-hint">
                    {t("screens.settings.portable.import.selectFileHint")}
                  </small>
                </label>

                {errorText && (
                  <p className="settings-error" role="alert">
                    {errorText}
                  </p>
                )}

                <div className="portable-modal-footer">
                  <button
                    type="button"
                    className="settings-btn-secondary"
                    onClick={() => setPhase("idle")}
                    data-testid="portable-import-dialog-close"
                  >
                    {t("screens.settings.portable.import.cancel")}
                  </button>
                  <button
                    type="button"
                    className="settings-btn-primary"
                    onClick={handleVerify}
                    disabled={verifying}
                    data-testid="portable-import-verify-btn"
                  >
                    {verifying
                      ? t("screens.settings.portable.import.verifying")
                      : t("screens.settings.portable.import.verify")}
                  </button>
                </div>
              </div>
            )}

            {phase === "options" && preview && (
              <div className="portable-modal-body">
                <section
                  className="portable-preview-card"
                  data-testid="portable-import-preview"
                >
                  <h4 className="portable-preview-title">
                    {t("screens.settings.portable.import.preview.title")}
                  </h4>
                  <p className="portable-preview-summary num">
                    {preview.manifest_summary}
                  </p>
                  <dl className="portable-result-meta">
                    <div className="portable-result-row">
                      <dt>
                        {t("screens.settings.portable.import.preview.size")}
                      </dt>
                      <dd className="num">
                        {formatBytes(preview.size_bytes)}
                      </dd>
                    </div>
                    <div className="portable-result-row">
                      <dt>
                        {t(
                          "screens.settings.portable.import.preview.entries",
                        )}
                      </dt>
                      <dd className="num">{preview.entries_count}</dd>
                    </div>
                    <div className="portable-result-row">
                      <dt>
                        {t(
                          "screens.settings.portable.import.preview.contents",
                        )}
                      </dt>
                      <dd>
                        {preview.has_models
                          ? t(
                              "screens.settings.portable.import.preview.hasModels",
                            )
                          : t(
                              "screens.settings.portable.import.preview.metaOnly",
                            )}
                        {preview.has_keys
                          ? ` · ${t("screens.settings.portable.import.preview.hasKeys")}`
                          : ""}
                      </dd>
                    </div>
                  </dl>
                </section>

                {preview.has_keys && (
                  <label className="portable-input-field">
                    <span>
                      {t("screens.settings.portable.import.passphrase")}
                    </span>
                    <input
                      type="password"
                      className="settings-input"
                      value={passphrase}
                      onChange={(e) => setPassphrase(e.target.value)}
                      autoComplete="current-password"
                      data-testid="portable-import-passphrase"
                    />
                  </label>
                )}

                <fieldset
                  className="settings-fieldset"
                  style={{ borderStyle: "dashed" }}
                >
                  <legend className="settings-legend">
                    {t(
                      "screens.settings.portable.import.conflictPolicy.label",
                    )}
                  </legend>
                  <div className="settings-radio-row" role="radiogroup">
                    {(
                      ["skip", "overwrite", "rename"] as ConflictPolicy[]
                    ).map((p) => (
                      <label
                        key={p}
                        className={`settings-radio${
                          conflictPolicy === p ? " is-checked" : ""
                        }`}
                      >
                        <input
                          type="radio"
                          name="conflict_policy"
                          value={p}
                          checked={conflictPolicy === p}
                          onChange={() => setConflictPolicy(p)}
                          data-testid={`portable-import-policy-${p}`}
                        />
                        <span className="settings-radio-label">
                          {t(
                            `screens.settings.portable.import.conflictPolicy.${p}`,
                          )}
                        </span>
                      </label>
                    ))}
                  </div>
                </fieldset>

                {errorText && (
                  <p className="settings-error" role="alert">
                    {errorText}
                  </p>
                )}

                <div className="portable-modal-footer">
                  <button
                    type="button"
                    className="settings-btn-secondary"
                    onClick={() => setPhase("idle")}
                    data-testid="portable-import-options-close"
                  >
                    {t("screens.settings.portable.import.cancel")}
                  </button>
                  <button
                    type="button"
                    className="settings-btn-primary"
                    onClick={handleStart}
                    data-testid="portable-import-confirm-btn"
                  >
                    {t("screens.settings.portable.import.confirm")}
                  </button>
                </div>
              </div>
            )}

            {phase === "running" && (
              <div className="portable-modal-body">
                <p className="portable-running-message">
                  {t("screens.settings.portable.import.running")}
                </p>
                <div
                  className="portable-progress"
                  role="progressbar"
                  aria-valuenow={percent}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  data-testid="portable-import-progress"
                >
                  <div
                    className="portable-progress-fill"
                    style={{ width: `${percent}%` }}
                  />
                </div>
                <p className="portable-progress-meta num">
                  {progress.processed} / {progress.total} · {percent}%
                </p>
                <div className="portable-modal-footer">
                  <button
                    ref={cancelBtnRef}
                    type="button"
                    className="settings-btn-secondary"
                    onClick={handleCancel}
                    data-testid="portable-import-cancel-btn"
                  >
                    {t("screens.settings.portable.import.cancel")}
                  </button>
                </div>
              </div>
            )}
          </div>
        </div>
      )}
    </fieldset>
  );
}
