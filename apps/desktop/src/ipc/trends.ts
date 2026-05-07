// Trends 요약 IPC 래퍼 — Phase 22'.e.4.
//
// backend `apps/desktop/src-tauri/src/trends.rs`의 `summarize_trends` Tauri command
// + `trend-summarizer` crate types를 mirror.

import { invoke } from "@tauri-apps/api/core";

/** Rust `trend_summarizer::SummaryKind` (kebab-case). */
export type SummaryKind =
  | "paper"
  | "blog"
  | "news"
  | "video"
  | "github"
  | "sns";

/** 카테고리 한국어 라벨 — backend `SummaryKind::label_ko` mirror. */
export const SUMMARY_KIND_LABEL_KO: Record<SummaryKind, string> = {
  paper: "논문",
  blog: "블로그",
  news: "뉴스",
  video: "영상",
  github: "오픈소스",
  sns: "SNS",
};

/** Rust `trend_summarizer::SummaryInput` mirror. */
export interface SummaryInput {
  id: string;
  kind: SummaryKind;
  title: string;
  /** 큐레이터 작성 한국어 한 줄 요약 (≤ 200자, fair use 정합). */
  summary_ko: string;
  /** 매체 이름 (예: "OpenAI 블로그", "AI타임스"). */
  source: string;
  source_url: string;
}

/** Rust `trend_summarizer::TrendsSummary` mirror. */
export interface TrendsSummary {
  schema_version: number;
  /**
   * 카테고리 → "1~2문장 해요체 한국어".
   * BTreeMap이라 직렬화 시 키 순서 결정적 (paper / blog / news / video / github / sns).
   */
  sections: Partial<Record<SummaryKind, string>>;
  /** 사용된 모델 식별자 — `mock-summary` / `ollama:gemma3:4b` / `lm-studio:exaone-3.5`. */
  model_kind: string;
  /** sha256 64 hex. */
  cache_key: string;
}

/** Rust `trends::TrendsApiError` (kebab-tag). */
export type TrendsApiError =
  | { kind: "store-open"; message: string }
  | { kind: "store-failed"; message: string }
  | { kind: "summary-failed"; message: string }
  | { kind: "internal"; message: string };

/**
 * 트렌드 항목 요약 — cache hit이면 즉시, miss이면 LLM 호출.
 *
 * @param items - trends-bundle items (또는 부분 집합).
 * @param forceRefresh - true면 cache 무시하고 새로 생성.
 * @param runtimeKind - "ollama" | "lm-studio" | undefined. undefined면 Mock fallback.
 * @param modelId - runtime별 모델 id (예: "gemma3:4b").
 */
export async function summarizeTrends(
  items: SummaryInput[],
  forceRefresh: boolean,
  runtimeKind?: "ollama" | "lm-studio",
  modelId?: string,
): Promise<TrendsSummary> {
  return invoke<TrendsSummary>("summarize_trends", {
    items,
    forceRefresh,
    runtimeKind,
    modelId,
  });
}
