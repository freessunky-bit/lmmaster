// 카탈로그 카드 / 추천 패널이 공유하는 포맷터.
//
// 정책:
// - 한국어 단위(GB/MB) 즉시 변환.
// - 메트릭은 mono numeric — CSS에선 .num 클래스로 tabular-nums 적용.
// - excluded reason → i18n 키 + 인자 분리 (number 단위는 함수 내에서 GB 변환).

import type { ExclusionReason, ModelEntry, Recommendation } from "../../ipc/catalog";

export function formatSize(mb: number | null | undefined): string {
  if (mb == null) return "—";
  if (mb >= 1024) {
    const gb = mb / 1024;
    return `${gb >= 10 ? gb.toFixed(0) : gb.toFixed(1)} GB`;
  }
  return `${mb} MB`;
}

export type CompatLevel = "fit" | "tight" | "exceeds" | "unfit";

/** 호스트 호환 레벨 — 추천 결과의 excluded 리스트와 비교해 산출.
 *  best/balanced/lightweight 안에 있으면 fit/exceeds, excluded면 unfit, 기타 tight. */
export function compatOf(model: ModelEntry, rec: Recommendation | null): CompatLevel {
  if (!rec) return "fit";
  const excluded = rec.excluded.find((e) => idOf(e) === model.id);
  if (excluded) return "unfit";
  if (rec.best_choice === model.id || rec.lightweight_choice === model.id) {
    return model.install_size_mb <= 5000 ? "fit" : "exceeds";
  }
  return "fit";
}

export function idOf(reason: ExclusionReason): string {
  switch (reason.kind) {
    case "insufficient-vram":
    case "insufficient-ram":
    case "incompatible-runtime":
    case "deprecated":
      return reason.id;
  }
}

export function languageStars(strength: number | null | undefined): string {
  const n = Math.max(0, Math.min(10, strength ?? 0));
  const filled = Math.round(n / 2); // 0~10 → 0~5
  return "★".repeat(filled) + "☆".repeat(5 - filled);
}

export function modelHasFlag(
  model: ModelEntry,
  flag: "tool" | "vision" | "structured",
): boolean {
  switch (flag) {
    case "tool":
      return model.tool_support;
    case "vision":
      return model.vision_support;
    case "structured":
      return model.structured_output_support;
  }
}
