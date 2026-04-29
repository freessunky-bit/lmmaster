// PortableExportPanel — Phase 11' (ADR-0039).
//
// 정책:
// - "이 워크스페이스 내보낼게요" 버튼 → 옵션 dialog (모델 포함 / 키 포함 + 패스프레이즈) → 진행률.
// - 옵션 default: include_models=false, include_keys=false (사용자 명시 opt-in).
// - 진행률은 ExportEvent stream → progress bar + 현재 파일명 + cancel 버튼.
// - 완료 시 toast + sha256 + 사이즈 + target 경로.
// - a11y: dialog role + aria-modal + Esc / 배경 클릭 닫기 (옵션 단계만, 진행 중에는 cancel만).
// - 한국어 카피 해요체.

import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  cancelWorkspaceExport,
  isTerminalExportEvent,
  startWorkspaceExport,
  type ExportEvent,
  type ExportOptions,
} from "../../ipc/portable";

import "./portable.css";

type Phase = "idle" | "options" | "running" | "done" | "failed";

/** byte → 한국어 친화적 사이즈 표기. */
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
  sha256: string;
  archiveSizeBytes: number;
  targetPath: string;
}

export function PortableExportPanel() {
  const { t } = useTranslation();
  const [phase, setPhase] = useState<Phase>("idle");
  const [includeModels, setIncludeModels] = useState(false);
  const [includeKeys, setIncludeKeys] = useState(false);
  const [passphrase, setPassphrase] = useState("");
  const [targetPath, setTargetPath] = useState("");
  const [progress, setProgress] = useState({ processed: 0, total: 0 });
  const [currentPath, setCurrentPath] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState<DoneInfo | null>(null);
  const exportIdRef = useRef<string | null>(null);
  const dialogCloseRef = useRef<HTMLButtonElement>(null);
  const targetInputRef = useRef<HTMLInputElement>(null);

  const isOpen = phase === "options" || phase === "running";

  // 옵션 dialog 진입 시 첫 input에 포커스.
  useEffect(() => {
    if (phase === "options") {
      targetInputRef.current?.focus();
    } else if (phase === "running") {
      dialogCloseRef.current?.focus();
    }
  }, [phase]);

  // Esc 키 닫기 (옵션 단계만).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && phase === "options") {
        setPhase("idle");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [phase]);

  const reset = useCallback(() => {
    setPhase("idle");
    setIncludeModels(false);
    setIncludeKeys(false);
    setPassphrase("");
    setTargetPath("");
    setProgress({ processed: 0, total: 0 });
    setCurrentPath("");
    setError(null);
    setDone(null);
    exportIdRef.current = null;
  }, []);

  const handleStart = useCallback(async () => {
    setError(null);
    if (targetPath.trim().length === 0) {
      setError("screens.settings.portable.export.errors.emptyTarget");
      return;
    }
    if (includeKeys && passphrase.trim().length === 0) {
      setError("screens.settings.portable.export.errors.emptyPassphrase");
      return;
    }
    const opts: ExportOptions = {
      include_models: includeModels,
      include_keys: includeKeys,
      key_passphrase: includeKeys ? passphrase : null,
      target_path: targetPath.trim(),
    };
    setPhase("running");
    setProgress({ processed: 0, total: 0 });
    try {
      const res = await startWorkspaceExport(opts, (ev: ExportEvent) => {
        if (ev.kind === "counting") {
          setProgress({ processed: 0, total: Number(ev.total_files) });
        } else if (ev.kind === "compressing") {
          setProgress({
            processed: Number(ev.processed),
            total: Number(ev.total),
          });
          setCurrentPath(ev.current_path);
        } else if (ev.kind === "done") {
          setDone({
            sha256: ev.sha256,
            archiveSizeBytes: Number(ev.archive_size_bytes),
            targetPath: ev.target_path,
          });
        } else if (ev.kind === "failed") {
          setError(`screens.settings.portable.export.errors.runner::${ev.error}`);
        }
        if (isTerminalExportEvent(ev)) {
          // 별도 처리 — invoke의 await에서 추가 응답이 옴.
        }
      });
      exportIdRef.current = res.export_id;
      setDone({
        sha256: res.summary.sha256,
        archiveSizeBytes: Number(res.summary.archive_size_bytes),
        targetPath: opts.target_path,
      });
      setPhase("done");
    } catch (e) {
      console.warn("startWorkspaceExport failed:", e);
      const msg =
        e && typeof e === "object" && "message" in e
          ? `screens.settings.portable.export.errors.runner::${(e as { message: string }).message}`
          : "screens.settings.portable.export.errors.start";
      setError(msg);
      setPhase("failed");
    }
  }, [includeKeys, includeModels, passphrase, targetPath]);

  const handleCancel = useCallback(async () => {
    if (!exportIdRef.current) {
      setPhase("idle");
      return;
    }
    try {
      await cancelWorkspaceExport(exportIdRef.current);
    } catch (e) {
      console.warn("cancelWorkspaceExport failed:", e);
    }
  }, []);

  const errorText = (() => {
    if (!error) return null;
    const idx = error.indexOf("::");
    if (idx > 0) {
      const key = error.slice(0, idx);
      const detail = error.slice(idx + 2);
      return `${t(key)} (${detail})`;
    }
    return t(error);
  })();

  const percent =
    progress.total > 0
      ? Math.min(100, Math.round((progress.processed / progress.total) * 100))
      : 0;

  return (
    <fieldset className="settings-fieldset" data-testid="portable-export-panel">
      <legend className="settings-legend">
        {t("screens.settings.portable.export.title")}
      </legend>
      <p className="settings-hint">
        {t("screens.settings.portable.export.subtitle")}
      </p>

      <button
        type="button"
        className="settings-btn-primary"
        onClick={() => setPhase("options")}
        disabled={isOpen}
        data-testid="portable-export-start-btn"
      >
        {t("screens.settings.portable.export.start")}
      </button>

      {phase === "done" && done && (
        <div
          className="portable-result-card"
          role="status"
          aria-live="polite"
          data-testid="portable-export-done"
        >
          <p className="portable-result-headline">
            {t("screens.settings.portable.export.done")}
          </p>
          <dl className="portable-result-meta">
            <div className="portable-result-row">
              <dt>{t("screens.settings.portable.export.targetPath")}</dt>
              <dd className="num">{done.targetPath}</dd>
            </div>
            <div className="portable-result-row">
              <dt>{t("screens.settings.portable.export.archiveSize")}</dt>
              <dd className="num">{formatBytes(done.archiveSizeBytes)}</dd>
            </div>
            <div className="portable-result-row">
              <dt>sha256</dt>
              <dd className="num portable-sha">{done.sha256}</dd>
            </div>
          </dl>
          <button
            type="button"
            className="settings-btn-secondary"
            onClick={reset}
            data-testid="portable-export-reset-btn"
          >
            {t("screens.settings.portable.export.again")}
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
          onClick={() => phase === "options" && setPhase("idle")}
        >
          <div
            className="portable-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="portable-export-modal-title"
            onClick={(e) => e.stopPropagation()}
            data-testid="portable-export-modal"
          >
            <header className="portable-modal-header">
              <h3
                id="portable-export-modal-title"
                className="portable-modal-title"
              >
                {phase === "options"
                  ? t("screens.settings.portable.export.dialogTitle")
                  : t("screens.settings.portable.export.runningTitle")}
              </h3>
            </header>

            {phase === "options" && (
              <div className="portable-modal-body">
                <label className="portable-input-field">
                  <span>
                    {t("screens.settings.portable.export.targetPath")}
                  </span>
                  <input
                    ref={targetInputRef}
                    type="text"
                    className="settings-input num"
                    value={targetPath}
                    onChange={(e) => setTargetPath(e.target.value)}
                    placeholder="C:\\Users\\me\\Desktop\\workspace.zip"
                    data-testid="portable-export-target-input"
                  />
                  <small className="settings-hint">
                    {t("screens.settings.portable.export.targetHint")}
                  </small>
                </label>

                <label className="portable-checkbox-row">
                  <input
                    type="checkbox"
                    checked={includeModels}
                    onChange={(e) => setIncludeModels(e.target.checked)}
                    data-testid="portable-export-include-models"
                  />
                  <span>
                    <strong>
                      {t("screens.settings.portable.export.includeModels")}
                    </strong>
                    <small className="settings-hint">
                      {t("screens.settings.portable.export.includeModelsHint")}
                    </small>
                  </span>
                </label>

                <label className="portable-checkbox-row">
                  <input
                    type="checkbox"
                    checked={includeKeys}
                    onChange={(e) => {
                      setIncludeKeys(e.target.checked);
                      if (!e.target.checked) setPassphrase("");
                    }}
                    data-testid="portable-export-include-keys"
                  />
                  <span>
                    <strong>
                      {t("screens.settings.portable.export.includeKeys")}
                    </strong>
                    <small className="settings-hint">
                      {t("screens.settings.portable.export.includeKeysHint")}
                    </small>
                  </span>
                </label>

                {includeKeys && (
                  <label className="portable-input-field">
                    <span>
                      {t("screens.settings.portable.export.passphraseLabel")}
                    </span>
                    <input
                      type="password"
                      className="settings-input"
                      value={passphrase}
                      onChange={(e) => setPassphrase(e.target.value)}
                      autoComplete="new-password"
                      data-testid="portable-export-passphrase"
                    />
                    <small className="settings-hint">
                      {t("screens.settings.portable.export.passphraseHint")}
                    </small>
                  </label>
                )}

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
                    data-testid="portable-export-dialog-close"
                  >
                    {t("screens.settings.portable.export.cancel")}
                  </button>
                  <button
                    type="button"
                    className="settings-btn-primary"
                    onClick={handleStart}
                    data-testid="portable-export-confirm-btn"
                  >
                    {t("screens.settings.portable.export.confirm")}
                  </button>
                </div>
              </div>
            )}

            {phase === "running" && (
              <div className="portable-modal-body">
                <p className="portable-running-message">
                  {t("screens.settings.portable.export.running")}
                </p>
                <div
                  className="portable-progress"
                  role="progressbar"
                  aria-valuenow={percent}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  data-testid="portable-export-progress"
                >
                  <div
                    className="portable-progress-fill"
                    style={{ width: `${percent}%` }}
                  />
                </div>
                <p className="portable-progress-meta num">
                  {progress.processed} / {progress.total} ·{" "}
                  {percent}%
                </p>
                {currentPath && (
                  <p
                    className="portable-progress-current num"
                    data-testid="portable-export-current"
                  >
                    {currentPath}
                  </p>
                )}
                <div className="portable-modal-footer">
                  <button
                    ref={dialogCloseRef}
                    type="button"
                    className="settings-btn-secondary"
                    onClick={handleCancel}
                    data-testid="portable-export-cancel-btn"
                  >
                    {t("screens.settings.portable.export.cancel")}
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
