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
  installLlamaCppRuntime,
  setLlamaServerPath,
  type LlamaInstallEvent,
} from "../ipc/llama-server-settings";
import { pickFile } from "../ipc/path-tokens";

type Status =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "saved"; path: string }
  | { kind: "error"; message: string };

type AutoSetupState =
  | { kind: "idle" }
  | { kind: "running"; status: string; percent: number | null }
  | { kind: "done" }
  | { kind: "failed"; message: string };

export function LlamaServerPanel() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<Status>({ kind: "loading" });
  const [autoState, setAutoState] = useState<AutoSetupState>({ kind: "idle" });

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

  const handleAutoSetup = useCallback(async () => {
    setAutoState({
      kind: "running",
      status: "준비하고 있어요…",
      percent: null,
    });
    try {
      await installLlamaCppRuntime((event: LlamaInstallEvent) => {
        if (event.kind === "status") {
          setAutoState({
            kind: "running",
            status: event.status,
            percent: null,
          });
        } else if (event.kind === "progress") {
          const percent =
            event.total_bytes > 0
              ? Math.round((event.completed_bytes / event.total_bytes) * 100)
              : null;
          const mb = (event.completed_bytes / 1_048_576).toFixed(0);
          const totalMb = (event.total_bytes / 1_048_576).toFixed(0);
          const speedMbps = (event.speed_bps / 1_048_576).toFixed(1);
          setAutoState({
            kind: "running",
            status:
              event.total_bytes > 0
                ? `다운로드 중 ${mb}MB / ${totalMb}MB (${speedMbps} MB/s)`
                : `다운로드 중 ${mb}MB`,
            percent,
          });
        } else if (event.kind === "completed") {
          setAutoState({ kind: "done" });
        } else if (event.kind === "failed") {
          setAutoState({ kind: "failed", message: event.message });
        }
      });
      setAutoState({ kind: "done" });
      await refresh();
    } catch (e) {
      const message = parseError(e);
      setAutoState({ kind: "failed", message });
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

      {autoState.kind === "running" && (
        <div
          className="settings-readonly-text"
          data-testid="settings-llama-server-auto-progress"
          aria-live="polite"
        >
          <p>{autoState.status}</p>
          {autoState.percent !== null && (
            <div
              role="progressbar"
              aria-valuenow={autoState.percent}
              aria-valuemin={0}
              aria-valuemax={100}
              style={{
                height: 6,
                background: "var(--surface-3)",
                borderRadius: 3,
                marginTop: 8,
              }}
            >
              <div
                style={{
                  width: `${autoState.percent}%`,
                  height: "100%",
                  background: "var(--primary)",
                  borderRadius: 3,
                  transition: "width 200ms",
                }}
              />
            </div>
          )}
        </div>
      )}

      {autoState.kind === "failed" && (
        <p
          className="settings-error"
          role="alert"
          data-testid="settings-llama-server-auto-error"
        >
          자동 셋업 실패: {autoState.message}
        </p>
      )}

      <div className="settings-actions">
        <button
          type="button"
          className="settings-btn-primary"
          onClick={handleAutoSetup}
          disabled={autoState.kind === "running"}
          data-testid="settings-llama-server-auto"
        >
          {t(
            "screens.settings.advanced.llamaServer.autoSetup",
            "자동 셋업할게요",
          )}
        </button>
        <button
          type="button"
          className="settings-btn-secondary"
          onClick={handlePick}
          disabled={
            status.kind === "loading" || autoState.kind === "running"
          }
          data-testid="settings-llama-server-pick"
        >
          {t(
            "screens.settings.advanced.llamaServer.pick",
            "실행 파일 직접 선택",
          )}
        </button>
        {status.kind === "saved" && (
          <button
            type="button"
            className="settings-btn-secondary"
            onClick={handleClear}
            disabled={autoState.kind === "running"}
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
