// bench IPC — start_bench / cancel_bench / get_last_bench_report.
// Rust crates/bench-harness 의 BenchReport / BenchSample / BenchErrorReport / BenchMetricsSource 미러.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { RuntimeKind } from "./catalog";

export type BenchMetricsSource = "native" | "wallclock-est";

export type BenchErrorReport =
  | { kind: "runtime-unreachable"; message: string }
  | { kind: "model-not-loaded"; model_id: string }
  | { kind: "insufficient-vram"; need_mb: number; have_mb: number }
  | { kind: "cancelled" }
  | { kind: "timeout" }
  | { kind: "other"; message: string };

export interface BenchReport {
  runtime_kind: RuntimeKind;
  model_id: string;
  quant_label: string | null;
  host_fingerprint_short: string;
  /** SystemTime — 직접 표시하지 않음 (UI는 "방금" / "N분 전" 가공). */
  bench_at: unknown;
  digest_at_bench: string | null;

  /** generation tokens/s — 카드 primary metric. */
  tg_tps: number;
  /** 첫 응답 ms — 카드 secondary metric. */
  ttft_ms: number;

  pp_tps: number | null;
  e2e_ms: number;
  cold_load_ms: number | null;

  peak_vram_mb: number | null;
  peak_ram_delta_mb: number | null;

  metrics_source: BenchMetricsSource;
  sample_count: number;
  prompts_used: string[];
  timeout_hit: boolean;
  sample_text_excerpt: string | null;
  took_ms: number;

  error: BenchErrorReport | null;
}

export type BenchApiError =
  | { kind: "already-running" }
  | { kind: "host-not-probed" }
  | { kind: "unsupported-runtime"; runtime: string }
  | { kind: "internal"; message: string };

/** 30초 벤치마크 실행. AlreadyRunning 거부 시 BenchApiError throw. */
export async function startBench(args: {
  modelId: string;
  runtimeKind: RuntimeKind;
  quantLabel?: string | null;
  digestAtBench?: string | null;
}): Promise<BenchReport> {
  return invoke<BenchReport>("start_bench", {
    modelId: args.modelId,
    runtimeKind: args.runtimeKind,
    quantLabel: args.quantLabel ?? null,
    digestAtBench: args.digestAtBench ?? null,
  });
}

/** 진행 중인 측정 취소 — idempotent. */
export async function cancelBench(modelId: string): Promise<void> {
  return invoke<void>("cancel_bench", { modelId });
}

/** 캐시된 최근 측정 결과. 없으면 null. */
export async function getLastBenchReport(args: {
  modelId: string;
  runtimeKind: RuntimeKind;
  quantLabel?: string | null;
  digestAtBench?: string | null;
}): Promise<BenchReport | null> {
  return invoke<BenchReport | null>("get_last_bench_report", {
    modelId: args.modelId,
    runtimeKind: args.runtimeKind,
    quantLabel: args.quantLabel ?? null,
    digestAtBench: args.digestAtBench ?? null,
  });
}

/** bench:started — UI 진행 spinner 트리거. */
export async function onBenchStarted(
  cb: (event: { model_id: string }) => void,
): Promise<UnlistenFn> {
  return listen<{ model_id: string }>("bench:started", (e) => cb(e.payload));
}

/** bench:finished — UI 카드 hint chip 갱신. */
export async function onBenchFinished(
  cb: (report: BenchReport) => void,
): Promise<UnlistenFn> {
  return listen<BenchReport>("bench:finished", (e) => cb(e.payload));
}
