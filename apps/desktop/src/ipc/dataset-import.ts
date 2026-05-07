// Dataset import IPC 래퍼 — Phase 23'.c.2.d.4.
//
// backend `apps/desktop/src-tauri/src/datasets.rs`의 4 commands를 mirror.
// list_datasets / delete_dataset / dataset_import_start / dataset_import_cancel.
//
// 기존 ipc/datasets.ts는 *bundled 카탈로그 JSON 로드*용 (Phase 22'.c). 본 파일은
// *backend SQLCipher 카탈로그 + import service*용 — 두 모듈은 별도 책임.

import { Channel, invoke } from "@tauri-apps/api/core";

// ── Backend struct mirror ────────────────────────────────────────────

/** Rust `dataset_importer::SampleStrategy` — tagged union. */
export type SampleStrategy =
  | { kind: "full" }
  | { kind: "first"; n: number }
  | { kind: "stratified"; n: number; by: string[] };

/** Rust `datasets::DatasetSummary` — list_datasets 반환. */
export interface InstalledDataset {
  id: string;
  repo: string;
  config: string;
  split: string;
  license: string;
  minorSafety: boolean;
  /** SampleStrategy JSON 문자열 — frontend 측이 다시 parse. */
  sampleStrategy: string;
  embeddingDim: number;
  totalChunks: number;
  createdAt: string;
}

/** Rust `datasets::DatasetImportConfig` — invoke 입력. */
export interface DatasetImportConfig {
  repo: string;
  config: string;
  split: string;
  license: string;
  /** ADR-0062 — false면 즉시 거부. NSFW 데이터셋만 명시 동의. */
  minorSafetyAttestation: boolean;
  sample: SampleStrategy;
  textColumns: string[];
}

/** Rust `datasets::DatasetImportSummary` — Done event payload. */
export interface DatasetImportSummary {
  datasetId: string;
  rowsProcessed: number;
  chunksGenerated: number;
  chunksEmbedded: number;
  chunksInserted: number;
}

/** Rust `datasets::DatasetIngestEvent` — Channel<T> typed stream. */
export type DatasetIngestEvent =
  | { kind: "started"; import_id: string; dataset_id: string; repo: string }
  | { kind: "manifest"; import_id: string; urls: number }
  | {
      kind: "downloading";
      import_id: string;
      urls_fetched: number;
      urls_total: number;
    }
  | {
      kind: "chunking";
      import_id: string;
      rows: number;
      chunks_generated: number;
      chunks_embedded: number;
    }
  | { kind: "embedding"; import_id: string; chunks: number }
  | { kind: "writing"; import_id: string; inserted: number }
  | { kind: "done"; import_id: string; summary: DatasetImportSummary }
  | { kind: "failed"; import_id: string; error: string }
  | { kind: "cancelled"; import_id: string };

/** Rust `datasets::DatasetApiError` — invoke().reject로 도달. */
export type DatasetApiError =
  | { kind: "store-open"; message: string }
  | { kind: "store-failed"; message: string }
  | { kind: "minor-safety-required" }
  | { kind: "invalid-sample-strategy"; message: string }
  | { kind: "internal"; message: string };

// ── Invoke wrappers ──────────────────────────────────────────────────

/** 등록된 import된 데이터셋 목록 (최신순). */
export async function listInstalledDatasets(): Promise<InstalledDataset[]> {
  return invoke<InstalledDataset[]>("list_datasets");
}

/** 데이터셋 삭제 (cascade — chunks 함께 제거). */
export async function deleteInstalledDataset(datasetId: string): Promise<void> {
  return invoke<void>("delete_dataset", { datasetId });
}

/**
 * Dataset import 시작. import_id를 즉시 반환, 진행은 onEvent Channel로.
 *
 * 사용 예:
 * ```ts
 * const channel = new Channel<DatasetIngestEvent>();
 * channel.onmessage = (ev) => { console.log(ev.kind); };
 * const importId = await startDatasetImport({ ... }, channel);
 * ```
 */
export async function startDatasetImport(
  config: DatasetImportConfig,
  onEvent: Channel<DatasetIngestEvent>,
): Promise<string> {
  return invoke<string>("dataset_import_start", { config, onEvent });
}

/** 진행 중 import를 cancel — idempotent. */
export async function cancelDatasetImport(importId: string): Promise<void> {
  return invoke<void>("dataset_import_cancel", { importId });
}

// ── Helpers ──────────────────────────────────────────────────────────

/** SampleStrategy → 사용자 향 한국어 라벨. */
export function sampleStrategyLabel(s: SampleStrategy): string {
  switch (s.kind) {
    case "full":
      return "전체 가져오기 (대용량은 시간 오래 걸려요)";
    case "first":
      return `처음 ${s.n.toLocaleString()}행 미리보기`;
    case "stratified":
      return `${s.n.toLocaleString()}명 균등 분포 (${s.by.join(" × ")})`;
  }
}

/** 권장 default — Personas-Korea 기준 10K stratified. */
export function defaultSampleStrategy(): SampleStrategy {
  return { kind: "stratified", n: 10_000, by: ["province", "occupation"] };
}
