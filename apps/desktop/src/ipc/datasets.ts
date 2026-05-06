// datasets.ts — Phase 22'.c IPC stub (v0.2.0).
//
// 정책:
// - 현재는 *Vite static import*로 bundled `datasets-bundle.json` 직접 read.
// - v0.3.0 (Phase 22'.c.2)에서 registry-fetcher 통합 — 4-tier fallback (jsdelivr → GitHub → Bundled).
// - 신규 모델 카탈로그(`getCatalog`)와 같은 패턴으로 마이그레이션 예정.
// - 외부 통신 0 — 본 build 시점에 정적 import.

import bundle from "../../../../manifests/apps/datasets-bundle.json";

/** Phase 23'.a 데이터셋 카테고리 (ADR-0061). */
export type DatasetCategory =
  | "sft-seed"
  | "lora-seed"
  | "rag-corpus"
  | "persona-seed"
  | "eval-benchmark";

export type DatasetFormat = "parquet" | "jsonl" | "csv" | "arrow" | "tsv";

export type ContentWarning = "rp-explicit";

export interface DatasetSource {
  type: "hugging-face" | "direct-url" | "bundled";
  repo?: string;
  url?: string;
  file?: string;
  path?: string;
}

export interface MinorSafetyAttestation {
  verified_at: string;
  verified_by: string;
  keyword_scan_clean: boolean;
  hf_nfaa_flag: boolean;
  license_whitelist: boolean;
  curator_note_ko: string;
}

export interface DatasetUseCase {
  kind:
    | "sft-seed"
    | "lora-seed"
    | "rag-corpus"
    | "persona-seed"
    | "eval-benchmark";
  format?: string;
  language?: string[];
  count?: number;
  narrative_field?: string;
  base_model_hint?: string | null;
  target_layers?: string[] | null;
  chunk_strategy?: string;
  default_chunk_size?: number;
  metric_keys?: string[];
}

export interface DatasetEntry {
  id: string;
  display_name: string;
  category: DatasetCategory;
  source: DatasetSource;
  size_mb: number;
  row_count?: number;
  languages: string[];
  license: string;
  commercial: boolean;
  content_warning?: ContentWarning;
  minor_safety_attestation?: MinorSafetyAttestation;
  use_case: DatasetUseCase;
  format: DatasetFormat;
  checksums?: Record<string, string>;
  curator_note_ko?: string;
  sources?: string[];
}

export interface DatasetBundle {
  schema_version: number;
  generated_at: string;
  entries: DatasetEntry[];
}

const TYPED_BUNDLE = bundle as unknown as DatasetBundle;

/** datasets-bundle 읽기 — 현재 정적 import. */
export async function listDatasets(): Promise<DatasetEntry[]> {
  return Promise.resolve(TYPED_BUNDLE.entries);
}

/** datasets-bundle 메타 (generated_at 등). */
export async function getDatasetBundleMeta(): Promise<{
  schema_version: number;
  generated_at: string;
  count: number;
}> {
  return Promise.resolve({
    schema_version: TYPED_BUNDLE.schema_version,
    generated_at: TYPED_BUNDLE.generated_at,
    count: TYPED_BUNDLE.entries.length,
  });
}
