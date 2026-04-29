// catalog IPC — get_catalog / get_recommendation.
// Rust crates/model-registry의 ModelEntry/Recommendation serde 미러.

import { invoke } from "@tauri-apps/api/core";

export type ModelCategory =
  | "agent-general"
  | "roleplay"
  | "coding"
  | "sound-stt"
  | "sound-tts"
  | "slm"
  | "embeddings"
  | "rerank";

export type Maturity = "experimental" | "beta" | "stable" | "deprecated";

export type VerificationTier = "verified" | "community";

export type RuntimeKind =
  | "llama-cpp"
  | "kobold-cpp"
  | "ollama"
  | "lm-studio"
  | "vllm";

export interface VerificationInfo {
  tier: VerificationTier;
  verified_at?: string;
  verified_by?: string;
}

export interface HfMeta {
  downloads: number;
  likes: number;
  last_modified: string;
}

export interface QuantOption {
  label: string;
  size_mb: number;
  sha256: string;
  file_path?: string;
}

export type ModelSource =
  | { type: "hugging-face"; repo: string; file?: string }
  | { type: "direct-url"; url: string };

export interface ModelEntry {
  id: string;
  display_name: string;
  category: ModelCategory;
  model_family: string;
  source: ModelSource;
  runner_compatibility: RuntimeKind[];
  quantization_options: QuantOption[];
  min_vram_mb: number | null;
  rec_vram_mb: number | null;
  min_ram_mb: number;
  rec_ram_mb: number;
  install_size_mb: number;
  context_guidance?: string;
  language_strength?: number;
  roleplay_strength?: number;
  coding_strength?: number;
  tool_support: boolean;
  vision_support: boolean;
  structured_output_support: boolean;
  license: string;
  maturity: Maturity;
  portable_suitability: number;
  on_device_suitability: number;
  fine_tune_suitability: number;
  verification: VerificationInfo;
  hf_meta?: HfMeta | null;
  use_case_examples: string[];
  notes?: string | null;
  warnings: string[];
}

export type ExclusionReason =
  | { kind: "insufficient-vram"; id: string; need_mb: number; have_mb: number }
  | { kind: "insufficient-ram"; id: string; need_mb: number; have_mb: number }
  | { kind: "incompatible-runtime"; id: string }
  | { kind: "deprecated"; id: string };

export interface Recommendation {
  best_choice: string | null;
  balanced_choice: string | null;
  lightweight_choice: string | null;
  fallback_choice: string | null;
  excluded: ExclusionReason[];
  expected_tradeoffs: string[];
}

export interface CatalogView {
  entries: ModelEntry[];
  recommendation: Recommendation | null;
}

export type CatalogApiError =
  | { kind: "not-loaded" }
  | { kind: "host-not-probed" }
  | { kind: "internal"; message: string };

/** 카탈로그 entries — category가 없으면 전체. */
export async function getCatalog(
  category?: ModelCategory,
): Promise<CatalogView> {
  return invoke<CatalogView>("get_catalog", { category });
}

/** 카테고리별 결정적 추천. host fingerprint 미보장 시 host-not-probed reject. */
export async function getRecommendation(
  category: ModelCategory,
): Promise<Recommendation> {
  return invoke<Recommendation>("get_recommendation", { category });
}
