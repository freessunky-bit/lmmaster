// Phase 8'.c.4 (ADR-0066) Q1 helper — 모델 내보내기 도우미 UI.
//
// 정책:
// - "모델 폴더 열기" 버튼 — OS 파일 탐색기로 GGUF 파일 위치를 한 번에 노출.
// - 클라우드 GPU 내보내기 가이드 링크 — `docs/guides-ko/cloud-export-guide.md`.
// - 사용자 시나리오: LMmaster로 한국어 카탈로그 + 벤치 결정 → 클라우드 GPU에 모델 복사 → 5G 베타 테스트.

import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { getModelsDir, openModelsDir } from "../ipc/models-dir";

type LoadState =
  | { kind: "loading" }
  | { kind: "ready"; path: string }
  | { kind: "error"; message: string };

export function ModelsExportPanel() {
  const { t } = useTranslation();
  const [state, setState] = useState<LoadState>({ kind: "loading" });
  const [openError, setOpenError] = useState<string | null>(null);

  useEffect(() => {
    getModelsDir()
      .then((path) => setState({ kind: "ready", path }))
      .catch((e) =>
        setState({
          kind: "error",
          message: e instanceof Error ? e.message : String(e),
        }),
      );
  }, []);

  const handleOpen = useCallback(async () => {
    setOpenError(null);
    try {
      await openModelsDir();
    } catch (e) {
      setOpenError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  const handleCopyPath = useCallback(async () => {
    if (state.kind !== "ready") return;
    try {
      await navigator.clipboard.writeText(state.path);
    } catch (e) {
      console.warn("clipboard write failed:", e);
    }
  }, [state]);

  return (
    <fieldset
      className="settings-fieldset"
      data-testid="settings-models-export"
    >
      <legend className="settings-legend">
        {t("screens.settings.advanced.modelsExport.legend")}
      </legend>
      <p className="settings-hint">
        {t("screens.settings.advanced.modelsExport.hint")}
      </p>

      {state.kind === "ready" && (
        <p
          className="settings-readonly-text"
          data-testid="settings-models-export-path"
        >
          <code className="num">{state.path}</code>
        </p>
      )}

      {state.kind === "error" && (
        <p
          className="settings-error"
          role="alert"
          data-testid="settings-models-export-error"
        >
          {state.message}
        </p>
      )}

      {openError && (
        <p
          className="settings-error"
          role="alert"
          data-testid="settings-models-export-open-error"
        >
          {openError}
        </p>
      )}

      <div className="settings-actions">
        <button
          type="button"
          className="settings-btn-primary"
          onClick={handleOpen}
          disabled={state.kind !== "ready"}
          data-testid="settings-models-export-open"
        >
          {t("screens.settings.advanced.modelsExport.openFolder")}
        </button>
        <button
          type="button"
          className="settings-btn-secondary"
          onClick={handleCopyPath}
          disabled={state.kind !== "ready"}
          data-testid="settings-models-export-copy"
        >
          {t("screens.settings.advanced.modelsExport.copyPath")}
        </button>
      </div>

      <p className="settings-hint settings-models-export-guide">
        {t("screens.settings.advanced.modelsExport.guideHint")}{" "}
        <code className="num">docs/guides-ko/cloud-export-guide.md</code>
      </p>
    </fieldset>
  );
}
