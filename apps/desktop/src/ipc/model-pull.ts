// model-pull IPC — Ollama `/api/pull` streaming wrapper.
//
// 정책 (phase-install-bench-bugfix-decision §2.2):
// - tauri::ipc::Channel<ModelPullEvent> per-call (broadcast 회피).
// - cancel은 별도 command — 같은 model_id로 idempotent.
// - LM Studio는 unsupported-runtime — frontend가 외부 링크 안내로 대체.

import { invoke, Channel } from "@tauri-apps/api/core";

import type { RuntimeKind } from "./catalog";

export type ModelPullEvent =
  | { kind: "status"; status: string }
  | {
      kind: "progress";
      completed_bytes: number;
      total_bytes: number;
      speed_bps: number;
      eta_secs: number | null;
    }
  | { kind: "completed" }
  | { kind: "cancelled" }
  | { kind: "failed"; message: string };

export type PullOutcomeIpc =
  | { kind: "completed" }
  | { kind: "cancelled" }
  | { kind: "failed"; message: string };

export type ModelPullApiError =
  | { kind: "already-pulling"; model_id: string }
  | { kind: "unsupported-runtime"; runtime: string }
  | { kind: "unreachable"; message: string }
  | { kind: "internal"; message: string };

/** 모델 풀 시작. Channel<ModelPullEvent>로 이벤트 stream. */
export async function startModelPull(args: {
  modelId: string;
  runtimeKind: RuntimeKind;
  onEvent: (event: ModelPullEvent) => void;
}): Promise<PullOutcomeIpc> {
  const channel = new Channel<ModelPullEvent>();
  channel.onmessage = args.onEvent;
  return invoke<PullOutcomeIpc>("start_model_pull", {
    modelId: args.modelId,
    runtimeKind: args.runtimeKind,
    channel,
  });
}

/** 진행 중인 모델 풀 cancel — idempotent. */
export async function cancelModelPull(modelId: string): Promise<void> {
  return invoke<void>("cancel_model_pull", { modelId });
}

/** 사용자 향 한국어 카피 — backend status string → 해요체 라벨. */
export function statusLabelKo(status: string): string {
  // Ollama 공식 status 6단계 매핑.
  if (status.startsWith("pulling manifest")) return "받을 파일을 확인하고 있어요";
  if (status.startsWith("pulling")) return "받고 있어요";
  if (status.startsWith("verifying")) return "내려받은 파일을 확인하고 있어요";
  if (status.startsWith("writing manifest")) return "마무리하고 있어요";
  if (status.startsWith("removing")) return "예전 파일을 정리하고 있어요";
  if (status === "success") return "받기 완료";
  return status;
}

/** 진행률 (0~100). total=0일 때는 null. */
export function pullPercent(e: ModelPullEvent): number | null {
  if (e.kind !== "progress") return null;
  if (e.total_bytes === 0) return null;
  return Math.min(100, Math.round((e.completed_bytes / e.total_bytes) * 100));
}

/** 사람-친화 사이즈 카피 — "1.2 GB / 4.7 GB". */
export function bytesToSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024)
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

/** 속도 카피 — "28 MB/s". */
export function speedToCopy(bps: number): string {
  return `${bytesToSize(bps)}/s`;
}

/** ETA 카피 — "약 2분 남았어요" / "약 30초 남았어요" / null. */
export function etaToCopy(secs: number | null): string | null {
  if (secs == null || secs <= 0) return null;
  if (secs < 60) return `약 ${secs}초 남았어요`;
  const m = Math.round(secs / 60);
  if (m < 60) return `약 ${m}분 남았어요`;
  const h = Math.floor(m / 60);
  return `약 ${h}시간 ${m % 60}분 남았어요`;
}
