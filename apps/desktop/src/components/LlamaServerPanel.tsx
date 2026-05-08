// Phase 13'.h.2.e.1 — LlamaCpp binary path 등록 UI.
//
// 정책:
// - file picker → backend `set_llama_server_path(path_token)` IPC.
// - 저장된 path는 backend가 settings.json에 raw로 보관 + 다음 시작 시 env 주입.
// - i18n ko 우선 — 한국어 카피 §4.1 톤(해요체).

import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  clearLlamaServerPath,
  getLlamaServerPath,
  setLlamaServerPath,
} from "../ipc/llama-server-settings";
import { pickFile } from "../ipc/path-tokens";

type Status =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "saved"; path: string }
  | { kind: "error"; message: string };

export function LlamaServerPanel() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<Status>({ kind: "loading" });

  const refresh = useCallback(async () => {
    try {
      const path = await getLlamaServerPath();
      setStatus(path ? { kind: "saved", path } : { kind: "idle" });
    } catch (e) {
      setStatus({
        kind: "error",
        message: e instanceof Error ? e.message : String(e),
      });
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handlePick = useCallback(async () => {
    try {
      // Windows: .exe 우선. macOS/Linux는 확장자 없는 binary도 허용.
      const result = await pickFile([
        { name: "llama-server", extensions: ["exe", ""] },
      ]);
      if (!result) return;
      await setLlamaServerPath(result.token);
      await refresh();
    } catch (e) {
      const message = parseError(e);
      setStatus({ kind: "error", message });
    }
  }, [refresh]);

  const handleClear = useCallback(async () => {
    try {
      await clearLlamaServerPath();
      await refresh();
    } catch (e) {
      const message = parseError(e);
      setStatus({ kind: "error", message });
    }
  }, [refresh]);

  return (
    <fieldset
      className="settings-fieldset"
      data-testid="settings-llama-server"
    >
      <legend className="settings-legend">
        {t(
          "screens.settings.advanced.llamaServer",
          "LlamaCpp 서버 경로",
        )}
      </legend>
      <p className="settings-hint">
        {t(
          "screens.settings.advanced.llamaServer.hint",
          "비전 모델 채팅에 필요해요. llama.cpp의 llama-server 실행 파일을 골라주세요.",
        )}
      </p>

      {status.kind === "saved" && (
        <p
          className="settings-readonly-text"
          data-testid="settings-llama-server-path"
        >
          <code className="num">{status.path}</code>
        </p>
      )}

      {status.kind === "idle" && (
        <p
          className="settings-hint"
          data-testid="settings-llama-server-empty"
        >
          {t(
            "screens.settings.advanced.llamaServer.notSet",
            "아직 등록하지 않았어요.",
          )}
        </p>
      )}

      {status.kind === "error" && (
        <p
          className="settings-error"
          role="alert"
          data-testid="settings-llama-server-error"
        >
          {status.message}
        </p>
      )}

      <div className="settings-actions">
        <button
          type="button"
          className="settings-btn-primary"
          onClick={handlePick}
          disabled={status.kind === "loading"}
          data-testid="settings-llama-server-pick"
        >
          {t(
            "screens.settings.advanced.llamaServer.pick",
            "실행 파일 선택할게요",
          )}
        </button>
        {status.kind === "saved" && (
          <button
            type="button"
            className="settings-btn-secondary"
            onClick={handleClear}
            data-testid="settings-llama-server-clear"
          >
            {t(
              "screens.settings.advanced.llamaServer.clear",
              "초기화할게요",
            )}
          </button>
        )}
      </div>
    </fieldset>
  );
}

function parseError(e: unknown): string {
  if (e && typeof e === "object" && "kind" in e) {
    const kind = (e as { kind?: string }).kind;
    const message = (e as { message?: string }).message;
    switch (kind) {
      case "invalid-token":
        return "파일 선택 토큰이 만료됐어요. 다시 선택해 주세요.";
      case "validation":
        return `검증 실패: ${message ?? "확인할 수 없어요"}`;
      case "save":
        return `저장 실패: ${message ?? "확인할 수 없어요"}`;
      default:
        return message ?? `알 수 없는 오류 (${kind})`;
    }
  }
  return e instanceof Error ? e.message : String(e);
}
