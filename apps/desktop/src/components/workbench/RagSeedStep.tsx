// RagSeedStep — Phase 12'.b (ADR-0050) Stage 1: RAG 시드 진입점.
//
// 정책:
// - URL hash가 있을 때만 Workbench 페이지에 노출 (PromptTemplateStep 다음).
// - 의도별 권장:
//   - `ko-rag` → 한국어 자료에 KURE-v1 권장 메시지.
//   - `vision-image` / `vision-multimodal` → graceful "이미지 RAG는 v2에서 지원" 안내.
//   - 그 외 → 일반 안내.
// - path text input + ingest 시작 (폴더 picker는 v2+ — Tauri dialog plugin 미설치).
// - 자세한 관리는 Workspace > Knowledge 탭 deep link.
// - 외부 통신 0: ingest는 로컬 SQLite + ONNX 임베더.

import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";

import { useActiveWorkspace } from "../../contexts/ActiveWorkspaceContext";
import type { IntentId, ModelEntry } from "../../ipc/catalog";
import {
  isTerminalIngestEvent,
  startIngest,
  type IngestConfig,
  type IngestEvent,
} from "../../ipc/knowledge";

interface RagSeedStepProps {
  model: ModelEntry;
  intent: IntentId | null;
  /** 디폴트 store_path — Workspace 페이지가 사용하는 형식. */
  storePath?: string;
}

type RagStatus =
  | { kind: "idle" }
  | { kind: "running"; stage: string; percent: number | null }
  | { kind: "done"; chunks: number; files: number }
  | { kind: "failed"; message: string };

const VISION_INTENTS: ReadonlyArray<IntentId> = [
  "vision-image",
  "vision-multimodal",
];

export function RagSeedStep({ model, intent, storePath }: RagSeedStepProps) {
  const { t } = useTranslation();
  const { active } = useActiveWorkspace();
  const [path, setPath] = useState("");
  const [status, setStatus] = useState<RagStatus>({ kind: "idle" });

  const isVisionIntent = intent !== null && VISION_INTENTS.includes(intent);
  const workspaceId = active?.id ?? null;

  const handleStart = useCallback(async () => {
    if (!path.trim() || !workspaceId) return;
    const config: IngestConfig = {
      workspace_id: workspaceId,
      path: path.trim(),
      store_path:
        storePath ?? `${workspaceId}/knowledge.db`, // Workspace 페이지와 동일 결.
    };
    setStatus({ kind: "running", stage: "reading", percent: null });
    try {
      await startIngest(config, (ev: IngestEvent) => {
        applyEvent(ev, setStatus);
        if (isTerminalIngestEvent(ev) && ev.kind === "failed") {
          setStatus({ kind: "failed", message: ev.error });
        }
      });
    } catch (e) {
      const msg = extractMessage(
        e,
        t(
          "screens.workbench.ragSeed.startFailed",
          "자료 추가에 실패했어요. 잠시 뒤에 다시 시도해 주세요.",
        ),
      );
      setStatus({ kind: "failed", message: msg });
    }
  }, [path, workspaceId, storePath, t]);

  const goToWorkspace = useCallback(() => {
    if (typeof window !== "undefined") {
      window.location.hash = "#/workspace";
    }
  }, []);

  // Vision intent — graceful 안내 + Workspace 안내.
  if (isVisionIntent) {
    return (
      <section
        className="workbench-rag-seed-step"
        role="region"
        aria-labelledby="workbench-rag-seed-heading"
        data-testid="workbench-rag-seed-step"
      >
        <h3
          id="workbench-rag-seed-heading"
          className="workbench-rag-seed-title"
        >
          {t("screens.workbench.ragSeed.title", "② 자료 추가 (RAG) — 선택")}
        </h3>
        <p
          className="workbench-rag-seed-deferred"
          data-testid="workbench-rag-seed-vision-deferred"
        >
          {t(
            "screens.workbench.ragSeed.visionDeferred",
            "이미지 RAG는 다음 버전에서 지원할게요. 지금은 텍스트 자료(.txt / .md / .pdf)만 임베드 가능해요.",
          )}
        </p>
      </section>
    );
  }

  return (
    <section
      className="workbench-rag-seed-step"
      role="region"
      aria-labelledby="workbench-rag-seed-heading"
      data-testid="workbench-rag-seed-step"
    >
      <header className="workbench-rag-seed-header">
        <h3
          id="workbench-rag-seed-heading"
          className="workbench-rag-seed-title"
        >
          {t("screens.workbench.ragSeed.title", "② 자료 추가 (RAG) — 선택")}
        </h3>
        <p className="workbench-rag-seed-subtitle">
          {t(
            "screens.workbench.ragSeed.subtitle",
            "내 도메인 자료를 추가하면 모델이 내 맥락을 더 잘 이해해요. 프롬프트만으로 답이 부족할 때 켜 보세요.",
          )}
        </p>
      </header>

      {intent === "ko-rag" && (
        <p
          className="workbench-rag-seed-recommendation"
          data-testid="workbench-rag-seed-ko-rag-tip"
        >
          {t(
            "screens.workbench.ragSeed.koRagTip",
            "한국어 자료라면 KURE-v1 임베더가 가장 정확해요. Workspace > 임베딩 모델에서 활성화할 수 있어요.",
          )}
        </p>
      )}

      <div className="workbench-rag-seed-form">
        <label className="workbench-rag-seed-field">
          <span className="workbench-rag-seed-field-label">
            {t(
              "screens.workbench.ragSeed.pathLabel",
              "폴더 또는 파일 경로",
            )}
          </span>
          <input
            type="text"
            className="workbench-rag-seed-input"
            value={path}
            onChange={(e) => setPath(e.target.value)}
            placeholder={t(
              "screens.workbench.ragSeed.pathPlaceholder",
              "/path/to/notes 또는 D:\\문서\\사내자료",
            )}
            disabled={status.kind === "running"}
            data-testid="workbench-rag-seed-path"
          />
          <span className="workbench-rag-seed-field-hint">
            {t(
              "screens.workbench.ragSeed.pathHint",
              "텍스트 / Markdown / PDF 파일을 자동으로 청크 분할해 임베드해요.",
            )}
          </span>
        </label>
      </div>

      <div className="workbench-rag-seed-actions">
        <button
          type="button"
          className="workbench-rag-seed-action workbench-rag-seed-action-primary"
          onClick={handleStart}
          disabled={
            !path.trim() || !workspaceId || status.kind === "running"
          }
          data-testid="workbench-rag-seed-start"
        >
          {status.kind === "running"
            ? t(
                "screens.workbench.ragSeed.running",
                "추가하고 있어요…",
              )
            : t(
                "screens.workbench.ragSeed.start",
                "지금 추가해 볼게요",
              )}
        </button>
        <button
          type="button"
          className="workbench-rag-seed-action"
          onClick={goToWorkspace}
          data-testid="workbench-rag-seed-workspace-link"
        >
          {t(
            "screens.workbench.ragSeed.workspaceLink",
            "Workspace에서 더 자세히 관리하기",
          )}
        </button>
      </div>

      {!workspaceId && (
        <p
          className="workbench-rag-seed-empty"
          role="alert"
          data-testid="workbench-rag-seed-no-workspace"
        >
          {t(
            "screens.workbench.ragSeed.noWorkspace",
            "활성 Workspace가 없어요. 사이드바에서 먼저 만들어 주세요.",
          )}
        </p>
      )}

      {status.kind === "running" && (
        <p
          className="workbench-rag-seed-status"
          aria-live="polite"
          data-testid="workbench-rag-seed-progress"
        >
          {t(
            `screens.workbench.ragSeed.stage.${status.stage}`,
            status.stage,
          )}
          {status.percent !== null && ` ${status.percent}%`}
        </p>
      )}

      {status.kind === "done" && (
        <p
          className="workbench-rag-seed-status workbench-rag-seed-status-ok"
          role="status"
          data-testid="workbench-rag-seed-done"
        >
          {t("screens.workbench.ragSeed.done", {
            files: status.files,
            chunks: status.chunks,
            defaultValue: `완료 — 파일 ${status.files}개에서 청크 ${status.chunks}개를 만들었어요.`,
          })}
        </p>
      )}

      {status.kind === "failed" && (
        <p
          className="workbench-rag-seed-status workbench-rag-seed-status-error"
          role="alert"
          data-testid="workbench-rag-seed-error"
        >
          {status.message}
        </p>
      )}

      {model.context_guidance && (
        <p className="workbench-rag-seed-model-hint">
          {t("screens.workbench.ragSeed.modelHintLabel", "모델 안내")}: {model.context_guidance}
        </p>
      )}
    </section>
  );
}

function applyEvent(
  ev: IngestEvent,
  setStatus: (s: RagStatus) => void,
): void {
  switch (ev.kind) {
    case "started":
    case "reading":
      setStatus({ kind: "running", stage: "reading", percent: null });
      break;
    case "chunking":
      setStatus({
        kind: "running",
        stage: "chunking",
        percent:
          ev.total > 0 ? Math.round((ev.processed * 100) / ev.total) : null,
      });
      break;
    case "embedding":
      setStatus({
        kind: "running",
        stage: "embedding",
        percent:
          ev.total > 0 ? Math.round((ev.processed * 100) / ev.total) : null,
      });
      break;
    case "writing":
      setStatus({
        kind: "running",
        stage: "writing",
        percent:
          ev.total > 0 ? Math.round((ev.processed * 100) / ev.total) : null,
      });
      break;
    case "done":
      setStatus({
        kind: "done",
        files: ev.summary.files_processed,
        chunks: ev.summary.chunks_created,
      });
      break;
    case "failed":
      setStatus({ kind: "failed", message: ev.error });
      break;
    case "cancelled":
      setStatus({ kind: "idle" });
      break;
  }
}

function extractMessage(e: unknown, fallback: string): string {
  if (e && typeof e === "object" && "message" in e) {
    const m = (e as { message: unknown }).message;
    if (typeof m === "string" && m.length > 0) return m;
  }
  if (typeof e === "string" && e.length > 0) return e;
  return fallback;
}
