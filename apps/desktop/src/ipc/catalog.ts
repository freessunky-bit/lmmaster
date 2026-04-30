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

/**
 * 모델 사용 목적 — Phase 13'.f.2 (ADR-0048 + DEFERRED §13'.f.2).
 * - general-chat (기본): 일반 채팅 추천에 등장.
 * - fine-tune-base: instruction-tuned 아님 — Workbench LoRA 시드용. chat 추천 자동 제외.
 * - retrieval: 임베딩 모델 (RAG). chat 추천 자동 제외.
 * - reranker: 검색 결과 재정렬. chat 추천 자동 제외.
 */
export type ModelPurpose =
  | "general-chat"
  | "fine-tune-base"
  | "retrieval"
  | "reranker";

/**
 * 콘텐츠 경고 — Phase 13'.f.2.2 (DEFERRED §13'.f.2 §3).
 * - rp-explicit: 성인 RP / NSFW. 사용자 명시 토글 ON 시에만 카탈로그에 노출.
 */
export type ContentWarning = "rp-explicit";

/**
 * 카탈로그 노출 분류 — Phase 13'.e.1.
 *
 * Maturity (모델 자체 안정성)와 별개:
 * - new: 90일 이내 등장 + 트래픽 검증된 신모델. 🔥 NEW 탭.
 * - verified: 큐레이터 검증 완료. 메인 카탈로그.
 * - experimental: chat template 위험 / 사용자 책임 부담 큰 모델.
 * - deprecated: 보안/품질 이슈로 비추천.
 */
export type ModelTier = "new" | "verified" | "experimental" | "deprecated";

export type VerificationTier = "verified" | "community";

export type RuntimeKind =
  | "llama-cpp"
  | "kobold-cpp"
  | "ollama"
  | "lm-studio"
  | "vllm";

/**
 * 의도(intent) 식별자 — Phase 11'.a (ADR-0048).
 *
 * SSOT는 `crates/shared-types/src/intents.rs::INTENT_VOCABULARY`. v1.x 시드 11종.
 * 자유 string이지만 manifest validator + UI 사전이 등록된 것만 통과시킴.
 */
export type IntentId =
  | "vision-image"
  | "vision-multimodal"
  | "translation-ko-en"
  | "translation-multi"
  | "coding-general"
  | "coding-fim"
  | "agent-tool-use"
  | "roleplay-narrative"
  | "ko-conversation"
  | "ko-rag"
  | "voice-stt";

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

/**
 * 큐레이터 작성 커뮤니티 인사이트 — drawer "?" 토글에 4-section으로 노출.
 *
 * 정책 (Phase 13'.e.1):
 * - 외부 LLM 자동 요약 X — 정확도 + "외부 통신 0" 정책.
 * - 큐레이터가 사실 진술 + 코멘트 + 출처 URL 작성.
 */
export interface CommunityInsights {
  strengths_ko: string[];
  weaknesses_ko: string[];
  use_cases_ko: string[];
  curator_note_ko: string;
  sources: string[];
  /** RFC3339. */
  last_reviewed_at?: string | null;
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
  /**
   * 검증된 Ollama Hub 모델명. 큐레이터가 명시한 wrapper 경로 (예: "sam860/exaone-4.0:1.2b").
   *
   * 정책 (2026-04-30 — Ollama HF 통합 정밀 리서치):
   * - EXAONE/HCX 같은 한국어 특수 architecture는 `hf.co/{repo}` 직접 풀 시 chat template
   *   누락으로 출력 깨짐 위험. 큐레이터가 검증된 Modelfile wrapper로 매핑.
   * - 있으면 `runtimeModelId`가 그대로 사용. 없으면 source.repo로 자동 derivation.
   */
  hub_id?: string | null;
  /** 카탈로그 노출 분류 — Phase 13'.e.1. 누락 시 verified 폴백. */
  tier?: ModelTier;
  /** 큐레이터 커뮤니티 인사이트 — drawer "?" 토글에 노출. 누락 가능. */
  community_insights?: CommunityInsights | null;
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
  /**
   * 의도 태그 — Phase 11'.a (ADR-0048). N:N 매핑.
   * 빈 배열 OK (점진 백필 정책).
   */
  intents?: IntentId[];
  /**
   * 도메인 벤치마크 점수 — `IntentId → 0..=100`. Phase 11'.a (ADR-0048).
   * 누락된 intent는 점수 미보유. 큐레이터가 점진 백필.
   */
  domain_scores?: Partial<Record<IntentId, number>>;
  /** Phase 13'.f.2 — 모델 사용 목적. 누락 시 general-chat 폴백. */
  purpose?: ModelPurpose;
  /** Phase 13'.f.2.2 — 상업 사용 가능 여부. 누락 시 true 폴백. */
  commercial?: boolean;
  /** Phase 13'.f.2.2 — 콘텐츠 경고 (성인 등). 누락 시 None. */
  content_warning?: ContentWarning | null;
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

/**
 * 카테고리 + (선택) 의도 기반 결정적 추천. host fingerprint 미보장 시 host-not-probed reject.
 *
 * Phase 11'.b (ADR-0048): `intent`가 있으면 `domain_scores[intent]`가 ranking에 가중,
 * 없으면 기존 카테고리 기반 추천 (backward compat).
 */
export async function getRecommendation(
  category: ModelCategory,
  intent?: IntentId,
): Promise<Recommendation> {
  return invoke<Recommendation>("get_recommendation", { category, intent });
}

/**
 * 카탈로그의 LMmaster 내부 id를 런타임 어댑터가 사용할 수 있는 모델 식별자로 변환.
 *
 * 정책 (2026-04-30 — 사용자 첫 풀 root cause + Ollama HF 통합 정밀 리서치):
 * - **우선**: `model.hub_id` 가 명시되어 있으면 *그대로 사용* (예: "sam860/exaone-4.0:1.2b").
 *   이건 큐레이터가 검증한 Ollama Hub wrapper로 chat template + stop sequence 보장.
 * - **폴백**: HuggingFace 소스면 `hf.co/{repo}:{tag}` 자동 derivation
 *   ([Ollama HF 공식 통합](https://huggingface.co/docs/hub/ollama)).
 *   주의: 한국어 특수 architecture (EXAONE 4.0, HCX-Seed 등)는 chat template이 GGUF에 없어
 *   출력이 깨질 수 있음 — 가능하면 `hub_id` 명시 권장.
 * - **거부**: DirectUrl 소스는 Ollama 미지원 → null (호출 측이 친화 안내).
 *
 * @param model 카탈로그의 ModelEntry.
 * @param quantLabel 사용자가 고른 양자화 라벨 (예: "Q4_K_M"). null이면 첫 옵션 기본.
 * @param runtime 풀/측정에 사용할 런타임 — "ollama" 외엔 null 반환.
 * @returns Ollama API의 model 필드에 그대로 넣을 수 있는 이름.
 */
export function runtimeModelId(
  model: ModelEntry,
  quantLabel: string | null,
  runtime: RuntimeKind,
): string | null {
  if (runtime !== "ollama") return null;
  // 1) 큐레이터 명시 hub_id 우선 — chat template 검증된 wrapper.
  if (model.hub_id && model.hub_id.length > 0) {
    return model.hub_id;
  }
  // 2) HuggingFace 자동 derivation 폴백.
  switch (model.source.type) {
    case "hugging-face": {
      const quant = quantLabel ?? model.quantization_options[0]?.label ?? null;
      if (!quant) return `hf.co/${model.source.repo}`;
      return `hf.co/${model.source.repo}:${quant}`;
    }
    case "direct-url":
      // Ollama는 임의 URL pull을 지원하지 않아요. 호출 측은 안내 메시지로 fallback.
      return null;
  }
}
