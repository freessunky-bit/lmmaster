// BenchChip — 카탈로그 카드 footer에 들어가는 벤치 결과/CTA.
//
// 정책 (phase-2pc-bench-decision.md §6):
// - 측정 완료: "초당 N토큰 · 첫 응답 N초".
// - partial(timeout): "약 N토큰/초 · 30초 부분측정".
// - 미측정: "[측정하기] 버튼".
// - 측정 실패 (RuntimeUnreachable / InsufficientVram 등): 한국어 사실 진술 + 재시도 CTA.
// - 측정 진행 중: spinner + cancel CTA.

import { useTranslation } from "react-i18next";

import type { BenchErrorReport, BenchReport } from "../../ipc/bench";

export type BenchChipState =
  | { kind: "idle" }
  | { kind: "running" }
  | { kind: "report"; report: BenchReport };

export interface BenchChipProps {
  state: BenchChipState;
  onMeasure?: () => void;
  onCancel?: () => void;
  onRetry?: () => void;
}

export function BenchChip({ state, onMeasure, onCancel, onRetry }: BenchChipProps) {
  const { t } = useTranslation();

  if (state.kind === "running") {
    return (
      <div className="bench-chip is-running" role="status" aria-live="polite">
        <span className="bench-chip-spinner" aria-hidden />
        <span className="bench-chip-text">{t("bench.running")}</span>
        {onCancel && (
          <button
            type="button"
            className="bench-chip-action"
            onClick={(e) => {
              e.stopPropagation();
              onCancel();
            }}
            aria-label={t("bench.cancel")}
          >
            {t("bench.cancel")}
          </button>
        )}
      </div>
    );
  }

  if (state.kind === "idle") {
    return (
      <button
        type="button"
        className="bench-chip is-cta"
        onClick={(e) => {
          e.stopPropagation();
          onMeasure?.();
        }}
        aria-label={t("bench.measure")}
      >
        {t("bench.measure")}
      </button>
    );
  }

  // state.kind === "report"
  const r = state.report;
  if (r.error) {
    return (
      <div className="bench-chip is-error" data-testid="bench-error-chip">
        <span className="bench-chip-text">{errorText(t, r.error)}</span>
        {onRetry && (
          <button
            type="button"
            className="bench-chip-action"
            onClick={(e) => {
              e.stopPropagation();
              onRetry();
            }}
          >
            {t("bench.retry")}
          </button>
        )}
      </div>
    );
  }

  if (r.timeout_hit && r.sample_count === 0) {
    return (
      <div className="bench-chip is-warn" data-testid="bench-timeout-chip">
        <span className="bench-chip-text">{t("bench.timeoutZero")}</span>
        {onRetry && (
          <button
            type="button"
            className="bench-chip-action"
            onClick={(e) => {
              e.stopPropagation();
              onRetry();
            }}
          >
            {t("bench.retry")}
          </button>
        )}
      </div>
    );
  }

  const tps = r.tg_tps.toFixed(1);
  const ttft = (r.ttft_ms / 1000).toFixed(1);
  if (r.timeout_hit) {
    return (
      <div className="bench-chip is-partial" data-testid="bench-partial-chip">
        <span className="bench-chip-text">
          {t("bench.partial", { tps, seconds: 30 })}
        </span>
      </div>
    );
  }

  return (
    <div className="bench-chip is-ok" data-testid="bench-ok-chip">
      <span className="bench-chip-text num">
        {t("bench.summary", { tps, ttft })}
      </span>
      {r.metrics_source === "wallclock-est" && (
        <span
          className="bench-chip-badge"
          title={t("bench.wallclockHint")}
          aria-label={t("bench.wallclockHint")}
        >
          {t("bench.wallclockBadge")}
        </span>
      )}
    </div>
  );
}

type TFn = ReturnType<typeof useTranslation>["t"];

function errorText(t: TFn, e: BenchErrorReport): string {
  switch (e.kind) {
    case "runtime-unreachable":
      return t("bench.error.runtimeUnreachable");
    case "model-not-loaded":
      return t("bench.error.modelNotLoaded");
    case "insufficient-vram":
      return t("bench.error.insufficientVram", {
        need: `${(e.need_mb / 1024).toFixed(1)} GB`,
        have: `${(e.have_mb / 1024).toFixed(1)} GB`,
      });
    case "cancelled":
      return t("bench.error.cancelled");
    case "timeout":
      return t("bench.error.timeout");
    case "other":
      return e.message || t("bench.error.other");
  }
}
