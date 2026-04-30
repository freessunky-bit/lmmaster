// start_workbench_run / cancel_workbench_run / list_workbench_runs Tauri command 래퍼.
//
// Phase 5'.b — Channel<WorkbenchEvent>로 진행 이벤트를 받고, run_id를 즉시 Promise로 반환한다.
// install.ts 패턴과 동일 결.

import { Channel, invoke } from "@tauri-apps/api/core";

// ── Backend 미러 타입 ─────────────────────────────────────────────────

/** Rust workbench_core::flow::WorkbenchStep — kebab-case. */
export type WorkbenchStep = "data" | "quantize" | "lora" | "validate" | "register";

/** Phase 5'.e — Validate stage가 실 호출할 런타임 종류. */
export type ResponderRuntime = "ollama" | "lm-studio" | "mock";

/** Rust workbench_core::flow::WorkbenchConfig 미러. */
export interface WorkbenchConfig {
  base_model_id: string;
  data_jsonl_path: string;
  quant_type: string;
  lora_epochs: number;
  korean_preset: boolean;
  register_to_ollama: boolean;
  /** Phase 5'.e — Validate stage 런타임. null/undefined = mock(기본). */
  responder_runtime?: ResponderRuntime | null;
  /** 런타임 base URL — runtime이 ollama/lm-studio일 때만 의미. */
  responder_base_url?: string | null;
  /** 런타임 모델 식별자 — null이면 base_model_id 사용. */
  responder_model_id?: string | null;
}

/** Rust workbench::StageProgressDetail 미러. */
export interface StageProgressDetail {
  stage: WorkbenchStep;
  percent: number;
  label: string;
  message: string | null;
}

/** Rust workbench_core::EvalResult 미러. */
export interface EvalResult {
  case_id: string;
  passed: boolean;
  failure_reason: string | null;
  model_response: string;
}

/** Rust workbench_core::EvalReport 미러. */
export interface EvalReport {
  model_id: string;
  passed_count: number;
  total: number;
  /** category → [passed, total]. */
  by_category: Record<string, [number, number]>;
  cases: EvalResult[];
}

/** Rust workbench::WorkbenchRunSummary 미러. */
export interface WorkbenchRunSummary {
  run_id: string;
  total_duration_ms: number;
  artifact_paths: string[];
  eval_passed: number;
  eval_total: number;
  modelfile_preview: string | null;
  /** Validate stage의 per-case + 카테고리 집계 보고서. */
  eval_report: EvalReport | null;
  /** Register stage가 model-registry에 영속한 custom-model id. */
  registered_model_id: string | null;
}

/** Rust model_registry::CustomModel 미러 — list_custom_models 반환 항목. */
export interface CustomModel {
  id: string;
  base_model: string;
  quant_type: string;
  lora_adapter: string | null;
  modelfile: string;
  created_at: string;
  eval_passed: number;
  eval_total: number;
  artifact_paths: string[];
}

/** Rust workbench::ActiveRunSnapshot 미러 — list_workbench_runs 반환. */
export interface ActiveRunSnapshot {
  run_id: string;
  started_at: string;
  current_stage: WorkbenchStep;
}

/** Rust workbench::WorkbenchEvent 미러 — kind 기반 discriminated union. */
export type WorkbenchEvent =
  | { kind: "started"; run_id: string; config: WorkbenchConfig }
  | { kind: "stage-started"; run_id: string; stage: WorkbenchStep }
  | { kind: "stage-progress"; run_id: string; progress: StageProgressDetail }
  | { kind: "stage-completed"; run_id: string; stage: WorkbenchStep }
  | { kind: "eval-completed"; run_id: string; report: EvalReport }
  | { kind: "register-completed"; run_id: string; model_id: string }
  | { kind: "ollama-create-started"; run_id: string; output_name: string }
  | { kind: "ollama-create-progress"; run_id: string; line: string }
  | { kind: "ollama-create-completed"; run_id: string }
  | { kind: "ollama-create-failed"; run_id: string; error: string }
  | { kind: "completed"; run_id: string; summary: WorkbenchRunSummary }
  | { kind: "failed"; run_id: string; error: string }
  | { kind: "cancelled"; run_id: string };

/** invoke().reject로 도달하는 backend 에러. */
export type WorkbenchApiError =
  | { kind: "unknown-run"; run_id: string }
  | { kind: "start-failed"; message: string }
  | { kind: "registry-failed"; message: string };

/** Rust workbench_core::ChatMessage 미러. */
export interface ChatMessage {
  role: string;
  content: string;
}

/** Rust workbench_core::ChatExample 미러. */
export interface ChatExample {
  messages: ChatMessage[];
}

// ── helpers ───────────────────────────────────────────────────────────

/** terminal 이벤트 — 더 이상 stage 이벤트가 안 옴. */
export function isTerminalEvent(ev: WorkbenchEvent): boolean {
  return ev.kind === "completed" || ev.kind === "failed" || ev.kind === "cancelled";
}

// ── Tauri command 래퍼 ───────────────────────────────────────────────

export interface StartWorkbenchOptions {
  /** 진행 이벤트 콜백. terminal 이후 정리만 하면 됨 — Tauri가 command 종료 시 Channel close. */
  onEvent: (event: WorkbenchEvent) => void;
}

export interface StartWorkbenchHandle {
  /** uuid run_id. cancel/list lookup 키. */
  run_id: string;
  /** 현재 run을 cancel — idempotent. */
  cancel: () => Promise<void>;
}

/**
 * Workbench run 시작. Channel<WorkbenchEvent>로 매 단계 진행을 전달받는다.
 *
 * 반환된 handle.cancel()은 Rust cancel_workbench_run을 호출 — 미존재면 no-op.
 *
 * 에러는 invoke().reject로 도달 — kind discriminant 기반 narrow.
 */
export async function startWorkbenchRun(
  config: WorkbenchConfig,
  options: StartWorkbenchOptions,
): Promise<StartWorkbenchHandle> {
  const channel = new Channel<WorkbenchEvent>();
  channel.onmessage = options.onEvent;
  const run_id = await invoke<string>("start_workbench_run", { config, onEvent: channel });
  return {
    run_id,
    cancel: () => cancelWorkbenchRun(run_id),
  };
}

/** 진행 중 run cancel — idempotent. 미존재 id면 no-op. */
export async function cancelWorkbenchRun(run_id: string): Promise<void> {
  await invoke<void>("cancel_workbench_run", { runId: run_id });
}

/** 활성 run snapshot 목록. */
export async function listWorkbenchRuns(): Promise<ActiveRunSnapshot[]> {
  return invoke<ActiveRunSnapshot[]>("list_workbench_runs");
}

/** JSONL 파일 첫 N개 line preview — Step 1에서 사용. */
export async function previewJsonl(
  path: string,
  limit?: number,
): Promise<ChatExample[]> {
  return invoke<ChatExample[]>("workbench_preview_jsonl", { path, limit });
}

/** 정규화된 examples를 JSONL string으로 직렬화. */
export async function serializeExamples(examples: ChatExample[]): Promise<string> {
  return invoke<string>("workbench_serialize_examples", { examples });
}

/** Workbench가 등록한 custom-model 목록 — 카탈로그 페이지 등에서 사용. */
export async function listCustomModels(): Promise<CustomModel[]> {
  return invoke<CustomModel[]>("list_custom_models");
}

// ── Phase 8'.0.c — Workbench artifact retention IPC ───────────────────

/** Rust workbench_core::RetentionPolicy 미러. */
export interface RetentionPolicy {
  max_age_days: number;
  max_total_size_bytes: number;
}

/** Rust workbench_core::ArtifactStats 미러 — get_artifact_stats 반환. */
export interface ArtifactStats {
  run_count: number;
  total_bytes: number;
  oldest_modified_unix: number;
  policy: RetentionPolicy;
}

/** Rust workbench_core::CleanupReport 미러 — cleanup_artifacts_now 반환. */
export interface CleanupReport {
  removed_count: number;
  freed_bytes: number;
  kept_count: number;
  remaining_bytes: number;
}

export async function getArtifactStats(): Promise<ArtifactStats> {
  return invoke<ArtifactStats>("get_artifact_stats");
}

export async function cleanupArtifactsNow(): Promise<CleanupReport> {
  return invoke<CleanupReport>("cleanup_artifacts_now");
}

// ── Phase 9'.b — Workbench 실 모드 (real quantize / real LoRA) ───────────

/**
 * Rust workbench::WorkbenchRealStatus 미러 — workbench_real_status 반환.
 *
 * 정책:
 * - quantize_binary_found: PATH 또는 LMMASTER_LLAMA_QUANTIZE_PATH env에 llama-quantize 있나
 * - trainer_venv_ready: LLaMA-Factory venv가 이미 부트스트랩됐나
 *
 * UI는 둘 다 false면 "실 모드 사용하려면 설치 필요" 안내, true면 토글 활성.
 */
export interface WorkbenchRealStatus {
  quantize_binary_found: boolean;
  quantize_binary_path: string | null;
  trainer_venv_ready: boolean;
  trainer_venv_dir: string;
}

export async function getWorkbenchRealStatus(): Promise<WorkbenchRealStatus> {
  return invoke<WorkbenchRealStatus>("workbench_real_status");
}

/**
 * Rust workbench_core::lora_real::BootstrapEvent 미러.
 * tagged enum (`kind` discriminant) — Probing → PythonReady → CreatingVenv → InstallingDeps... → Done | Failed.
 */
export type BootstrapEvent =
  | { kind: "probing" }
  | { kind: "python-ready"; version: string; path: string }
  | { kind: "creating-venv" }
  | { kind: "installing-deps"; phase: string }
  | { kind: "log"; line: string }
  | { kind: "done" }
  | { kind: "failed"; error: string };

/**
 * LoRA용 Python venv + LLaMA-Factory 부트스트랩 시작.
 *
 * 정책 (ADR-0043 — 실 LoRA):
 * - 5~10GB 다운로드 + Python 의존성 설치. **사용자 명시 동의 후만 호출** (UI에서 dialog).
 * - Channel<BootstrapEvent>로 진행 이벤트 스트림. UI는 단계별 진행 + 라이브 로그 노출.
 * - 즉시 token_id를 반환 — 같은 token_id로 cancel 가능. 백그라운드 task가 실제 부트스트랩.
 */
export async function loraBootstrapVenv(args: {
  onEvent: (event: BootstrapEvent) => void;
}): Promise<string> {
  const channel = new Channel<BootstrapEvent>();
  channel.onmessage = args.onEvent;
  return invoke<string>("lora_bootstrap_venv", { onEvent: channel });
}

/** 진행 중 부트스트랩 cancel — 같은 token_id로. idempotent. */
export async function cancelLoraBootstrap(tokenId: string): Promise<void> {
  return invoke<void>("cancel_lora_bootstrap", { tokenId });
}
