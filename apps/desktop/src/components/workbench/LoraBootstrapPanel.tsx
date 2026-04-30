// LoraBootstrapPanel — Workbench Step 3 (LoRA)에서 실 모드 진입 가능 여부 + 부트스트랩 동의 흐름.
//
// 정책 (Phase 9'.b ADR-0043 + 2026-04-30 audit fix):
// - status === "ready" → 토글로 실 모드 활성. mock 모드는 항상 가능.
// - status === "missing" → "처음 한 번 약 5~10GB 다운로드 필요해요. 진행할래요?" 명시 동의 dialog.
// - 동의 후 lora_bootstrap_venv 호출 → BootstrapEvent stream으로 진행률 라이브 노출.
// - 부트스트랩 중에도 cancel 가능 (token_id 기반).
// - 첫 마운트 시 workbench_real_status 1회 호출 — 캐시는 backend가 함.

import { useCallback, useEffect, useState } from "react";

import {
  cancelLoraBootstrap,
  getWorkbenchRealStatus,
  loraBootstrapVenv,
  type BootstrapEvent,
  type WorkbenchRealStatus,
} from "../../ipc/workbench";

type BootstrapState =
  | { kind: "idle" }
  | { kind: "consenting" } // 동의 dialog 노출 중
  | {
      kind: "running";
      tokenId: string;
      stage: string;
      logs: string[];
    }
  | { kind: "done" }
  | { kind: "cancelled" }
  | { kind: "failed"; error: string };

export interface LoraBootstrapPanelProps {
  /** 실 모드 토글 콜백. WorkbenchConfig.use_real_lora 같은 필드를 caller가 보유. */
  onRealModeChange?: (enabled: boolean) => void;
  /** 외부 토글 상태 (controlled). undefined면 panel 자체 상태로 유지. */
  realModeEnabled?: boolean;
}

const MAX_LOG_LINES = 60;

export function LoraBootstrapPanel({
  onRealModeChange,
  realModeEnabled,
}: LoraBootstrapPanelProps) {
  const [status, setStatus] = useState<WorkbenchRealStatus | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [bootstrap, setBootstrap] = useState<BootstrapState>({ kind: "idle" });

  // 첫 마운트 시 status 1회 — 부트스트랩 후엔 자동 갱신.
  const refreshStatus = useCallback(async () => {
    try {
      const s = await getWorkbenchRealStatus();
      setStatus(s);
      setStatusError(null);
    } catch (e) {
      setStatusError((e as { message?: string }).message ?? String(e));
    }
  }, []);

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);

  const handleStartBootstrap = useCallback(async () => {
    setBootstrap({
      kind: "running",
      tokenId: "",
      stage: "준비하고 있어요",
      logs: [],
    });
    try {
      const tokenId = await loraBootstrapVenv({
        onEvent: (event: BootstrapEvent) => {
          setBootstrap((prev) => mergeBootstrapEvent(prev, event));
        },
      });
      // backend가 token_id를 즉시 반환 — running 상태에 채워넣음.
      setBootstrap((prev) =>
        prev.kind === "running" ? { ...prev, tokenId } : prev,
      );
    } catch (e) {
      const msg = (e as { message?: string }).message ?? String(e);
      setBootstrap({ kind: "failed", error: msg });
    }
  }, []);

  const handleCancelBootstrap = useCallback(async () => {
    if (bootstrap.kind !== "running") return;
    if (!bootstrap.tokenId) return;
    try {
      await cancelLoraBootstrap(bootstrap.tokenId);
    } catch (e) {
      console.warn("cancelLoraBootstrap failed:", e);
    }
  }, [bootstrap]);

  // 부트스트랩이 done이면 status 자동 갱신 — UI가 "ready" 상태로 자연 전환.
  useEffect(() => {
    if (bootstrap.kind === "done") {
      void refreshStatus();
    }
  }, [bootstrap.kind, refreshStatus]);

  if (statusError) {
    return (
      <div className="lora-bootstrap-panel is-error" role="alert">
        <p>실 모드 상태를 가져오지 못했어요: {statusError}</p>
        <button type="button" className="workspace-button" onClick={refreshStatus}>
          다시 확인
        </button>
      </div>
    );
  }

  if (!status) {
    return (
      <div className="lora-bootstrap-panel" aria-busy="true">
        <p className="lora-bootstrap-text">실 모드 사용 가능 여부를 확인하고 있어요…</p>
      </div>
    );
  }

  // 부트스트랩 진행 중 — 다른 것 가리고 진행률만.
  if (bootstrap.kind === "running") {
    return (
      <div
        className="lora-bootstrap-panel is-running"
        role="status"
        aria-live="polite"
        data-testid="lora-bootstrap-running"
      >
        <p className="lora-bootstrap-text">
          <strong>{bootstrap.stage}</strong>
        </p>
        {bootstrap.logs.length > 0 && (
          <pre className="lora-bootstrap-log" aria-label="설치 로그">
            {bootstrap.logs.slice(-12).join("\n")}
          </pre>
        )}
        <button
          type="button"
          className="workspace-button workspace-button-secondary"
          onClick={handleCancelBootstrap}
        >
          그만할게요
        </button>
      </div>
    );
  }

  if (bootstrap.kind === "failed") {
    return (
      <div className="lora-bootstrap-panel is-error" role="alert">
        <p>실 모드 부트스트랩이 실패했어요: {bootstrap.error}</p>
        <div className="workspace-actions">
          <button
            type="button"
            className="workspace-button workspace-button-primary"
            onClick={handleStartBootstrap}
          >
            다시 시도할게요
          </button>
          <button
            type="button"
            className="workspace-button"
            onClick={() => setBootstrap({ kind: "idle" })}
          >
            mock 모드로 진행할게요
          </button>
        </div>
      </div>
    );
  }

  if (bootstrap.kind === "cancelled") {
    return (
      <div className="lora-bootstrap-panel is-warn" role="status">
        <p>부트스트랩을 취소했어요. mock 모드로 진행할 수 있어요.</p>
        <button
          type="button"
          className="workspace-button"
          onClick={() => setBootstrap({ kind: "idle" })}
        >
          알겠어요
        </button>
      </div>
    );
  }

  // 동의 dialog
  if (bootstrap.kind === "consenting") {
    return (
      <div
        className="lora-bootstrap-panel is-consent"
        role="dialog"
        aria-modal="false"
        aria-labelledby="lora-consent-title"
        data-testid="lora-bootstrap-consent"
      >
        <h4 id="lora-consent-title">실 LoRA 모드를 켜려면 한 번만 설치할게요</h4>
        <ul className="lora-bootstrap-list">
          <li>Python 가상환경 + LLaMA-Factory + PyTorch — 약 <strong>5~10GB</strong> 다운로드</li>
          <li>30분쯤 걸려요. 네트워크와 디스크가 여유로울 때 권장해요.</li>
          <li>받은 파일은 <code>{status.trainer_venv_dir}</code> 에 저장돼요.</li>
          <li>받지 않으면 mock 모드 (모의 학습)로만 진행돼요.</li>
        </ul>
        <div className="workspace-actions">
          <button
            type="button"
            className="workspace-button workspace-button-primary"
            onClick={handleStartBootstrap}
          >
            네, 받을게요
          </button>
          <button
            type="button"
            className="workspace-button"
            onClick={() => setBootstrap({ kind: "idle" })}
          >
            나중에 할래요
          </button>
        </div>
      </div>
    );
  }

  // bootstrap === "idle" or "done" — status 노출.
  const venvReady = status.trainer_venv_ready;
  return (
    <div
      className={`lora-bootstrap-panel${venvReady ? " is-ready" : ""}`}
      data-testid="lora-bootstrap-status"
    >
      <div className="lora-bootstrap-row">
        <span
          className={`lora-bootstrap-pill${venvReady ? " is-on" : ""}`}
          aria-label={venvReady ? "실 모드 사용 가능" : "실 모드 미설치"}
        >
          {venvReady ? "실 모드 사용 가능" : "실 모드 미설치"}
        </span>
        <span className="lora-bootstrap-meta">
          {venvReady
            ? "LLaMA-Factory venv가 준비돼 있어요. 토글로 켤 수 있어요."
            : "지금은 mock 모드로만 동작해요. 실 학습이 필요하면 한 번 설치해 주세요."}
        </span>
      </div>

      {venvReady ? (
        <button
          type="button"
          role="switch"
          aria-checked={!!realModeEnabled}
          className={`workbench-toggle${realModeEnabled ? " is-on" : ""}`}
          onClick={() => onRealModeChange?.(!realModeEnabled)}
          data-testid="lora-real-mode-toggle"
        >
          <span className="workbench-toggle-track" aria-hidden>
            <span className="workbench-toggle-thumb" />
          </span>
          <span className="workbench-toggle-text">
            {realModeEnabled ? "실 모드 켜짐" : "실 모드 꺼짐 (mock)"}
          </span>
        </button>
      ) : (
        <button
          type="button"
          className="workspace-button workspace-button-primary"
          onClick={() => setBootstrap({ kind: "consenting" })}
          data-testid="lora-bootstrap-start"
        >
          실 모드 받기 시작
        </button>
      )}
    </div>
  );
}

function mergeBootstrapEvent(
  prev: BootstrapState,
  event: BootstrapEvent,
): BootstrapState {
  if (prev.kind !== "running") return prev;
  switch (event.kind) {
    case "probing":
      return { ...prev, stage: "Python 후보를 찾고 있어요" };
    case "python-ready":
      return {
        ...prev,
        stage: `Python ${event.version} 발견 — venv를 만들 준비 중이에요`,
      };
    case "creating-venv":
      return { ...prev, stage: "가상환경을 만들고 있어요" };
    case "installing-deps":
      return {
        ...prev,
        stage: `의존성을 설치하고 있어요 (${event.phase})`,
      };
    case "log": {
      const next = [...prev.logs, event.line];
      // 최근 60줄만 보관 — 메모리 보호.
      if (next.length > MAX_LOG_LINES) next.splice(0, next.length - MAX_LOG_LINES);
      return { ...prev, logs: next };
    }
    case "done":
      return { kind: "done" };
    case "failed":
      return { kind: "failed", error: event.error };
  }
}
