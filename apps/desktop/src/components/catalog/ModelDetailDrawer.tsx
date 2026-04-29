// ModelDetailDrawer — 카드 클릭 시 우측 슬라이드 드로워.
//
// 정책 (phase-2pb-catalog-ui-decision.md §7):
// - quant_options 라디오 그룹 + 권장 quant 표시.
// - warnings + use_case_examples 전체.
// - Esc / 배경 클릭으로 닫기.
// - role="dialog" + aria-labelledby + focus trap (간단 — 첫 focusable로 포커스).

import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  cancelBench,
  getLastBenchReport,
  onBenchFinished,
  startBench,
  type BenchReport,
} from "../../ipc/bench";
import type { ModelEntry, QuantOption, RuntimeKind } from "../../ipc/catalog";
import {
  categoryLabelKo,
  getPresets,
  type Preset,
} from "../../ipc/presets";

import { BenchChip, type BenchChipState } from "./BenchChip";
import { formatSize } from "./format";

export interface ModelDetailDrawerProps {
  model: ModelEntry | null;
  /** 측정용 런타임 — 카탈로그 카드는 첫 호환 런타임 prefer (Ollama 우선). */
  benchRuntime?: RuntimeKind;
  onClose: () => void;
  /** "이 모델 설치할게요" CTA — 클릭 시 InstallPage로 이동하면서 모델 ID 전달. */
  onInstall?: (modelId: string) => void;
}

const DEFAULT_BENCH_RUNTIME: RuntimeKind = "ollama";

export function ModelDetailDrawer({
  model,
  benchRuntime,
  onClose,
  onInstall,
}: ModelDetailDrawerProps) {
  const { t } = useTranslation();
  const closeBtnRef = useRef<HTMLButtonElement>(null);
  const [selectedQuant, setSelectedQuant] = useState<string>("");
  const [benchState, setBenchState] = useState<BenchChipState>({ kind: "idle" });

  const runtime = benchRuntime ?? pickRuntime(model) ?? DEFAULT_BENCH_RUNTIME;

  // 이 모델을 recommended_models[]에 포함하는 preset 목록.
  const [recommendedPresets, setRecommendedPresets] = useState<Preset[]>([]);

  // model이 바뀔 때마다 첫 quant를 default로 + 캐시된 측정 결과 조회.
  useEffect(() => {
    const first = model?.quantization_options[0];
    if (first) {
      setSelectedQuant(first.label);
    }
    if (!model) {
      setBenchState({ kind: "idle" });
      return;
    }
    let cancelled = false;
    getLastBenchReport({
      modelId: model.id,
      runtimeKind: runtime,
      quantLabel: first?.label ?? null,
    })
      .then((r) => {
        if (cancelled) return;
        if (r) setBenchState({ kind: "report", report: r });
        else setBenchState({ kind: "idle" });
      })
      .catch(() => {
        if (!cancelled) setBenchState({ kind: "idle" });
      });
    return () => {
      cancelled = true;
    };
  }, [model, runtime]);

  // bench:finished event 구독 — 측정 완료 시 카드 갱신.
  useEffect(() => {
    if (!model) return;
    let unlisten: (() => void) | null = null;
    onBenchFinished((report) => {
      if (report.model_id === model.id) {
        setBenchState({ kind: "report", report });
      }
    }).then((u) => {
      unlisten = u;
    });
    return () => {
      unlisten?.();
    };
  }, [model]);

  // 추천 프리셋 로드 — 이 모델을 recommended_models[]에 포함한 preset만 필터.
  useEffect(() => {
    if (!model) {
      setRecommendedPresets([]);
      return;
    }
    let cancelled = false;
    getPresets()
      .then((all) => {
        if (cancelled) return;
        const matching = all.filter((p) =>
          p.recommended_models.includes(model.id),
        );
        setRecommendedPresets(matching);
      })
      .catch((e) => {
        // preset 로드 실패는 치명적이지 않음 — 빈 목록으로 graceful.
        console.warn("getPresets failed:", e);
        if (!cancelled) setRecommendedPresets([]);
      });
    return () => {
      cancelled = true;
    };
  }, [model]);

  const handleMeasure = useCallback(async () => {
    if (!model) return;
    setBenchState({ kind: "running" });
    try {
      const report: BenchReport = await startBench({
        modelId: model.id,
        runtimeKind: runtime,
        quantLabel: selectedQuant || null,
      });
      setBenchState({ kind: "report", report });
    } catch (e) {
      console.warn("startBench failed:", e);
      // 실패해도 idle로 복귀 — 사용자가 다시 시도 가능.
      setBenchState({ kind: "idle" });
    }
  }, [model, runtime, selectedQuant]);

  const handleCancel = useCallback(async () => {
    if (!model) return;
    try {
      await cancelBench(model.id);
    } finally {
      setBenchState({ kind: "idle" });
    }
  }, [model]);

  // Esc로 닫기 + 첫 focus.
  useEffect(() => {
    if (!model) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    closeBtnRef.current?.focus();
    return () => window.removeEventListener("keydown", onKey);
  }, [model, onClose]);

  if (!model) return null;

  return (
    <div
      className="catalog-drawer-backdrop"
      role="presentation"
      onClick={onClose}
    >
      <aside
        className="catalog-drawer"
        role="dialog"
        aria-modal="true"
        aria-labelledby="catalog-drawer-title"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="catalog-drawer-header">
          <h3 id="catalog-drawer-title" className="catalog-drawer-title">
            {model.display_name}
          </h3>
          <div className="catalog-drawer-header-actions">
            {onInstall && (
              <button
                type="button"
                className="catalog-drawer-install"
                onClick={() => onInstall(model.id)}
                aria-label={t("drawer.install.aria", { name: model.display_name })}
              >
                {t("drawer.install.cta")}
              </button>
            )}
            <button
              ref={closeBtnRef}
              type="button"
              className="catalog-drawer-close"
              onClick={onClose}
              aria-label={t("drawer.close")}
            >
              ×
            </button>
          </div>
        </header>

        <div className="catalog-drawer-body">
          {model.context_guidance && (
            <section>
              <h4 className="catalog-drawer-section-title">
                {t("drawer.section.context")}
              </h4>
              <p className="catalog-drawer-text">{model.context_guidance}</p>
            </section>
          )}

          {model.use_case_examples.length > 0 && (
            <section>
              <h4 className="catalog-drawer-section-title">
                {t("drawer.section.useCases")}
              </h4>
              <ul className="catalog-drawer-list">
                {model.use_case_examples.map((u) => (
                  <li key={u}>{u}</li>
                ))}
              </ul>
            </section>
          )}

          <section>
            <h4 className="catalog-drawer-section-title">
              {t("drawer.section.bench", "30초 측정")}
            </h4>
            <BenchChip
              state={benchState}
              onMeasure={handleMeasure}
              onCancel={handleCancel}
              onRetry={handleMeasure}
            />
          </section>
          {benchState.kind === "report" && benchState.report.sample_text_excerpt && (
            <p className="catalog-drawer-text bench-excerpt">
              {benchState.report.sample_text_excerpt}
            </p>
          )}

          {model.quantization_options.length > 0 && (
            <section>
              <h4 className="catalog-drawer-section-title">
                {t("drawer.section.quant")}
              </h4>
              <div role="radiogroup" className="catalog-drawer-quant">
                {model.quantization_options.map((q, idx) => (
                  <QuantRow
                    key={q.label}
                    quant={q}
                    isRecommended={idx === 0}
                    isChecked={selectedQuant === q.label}
                    onChange={() => setSelectedQuant(q.label)}
                  />
                ))}
              </div>
            </section>
          )}

          {model.warnings.length > 0 && (
            <section>
              <h4 className="catalog-drawer-section-title">
                {t("drawer.section.warnings")}
              </h4>
              <ul className="catalog-drawer-list catalog-drawer-warnings">
                {model.warnings.map((w) => (
                  <li key={w}>{w}</li>
                ))}
              </ul>
            </section>
          )}

          <section>
            <h4 className="catalog-drawer-section-title">
              {t("drawer.section.presets", "이 모델 추천 프리셋")}
            </h4>
            {recommendedPresets.length === 0 ? (
              <p className="catalog-drawer-text">
                {t("drawer.section.presetsEmpty", "추천 프리셋이 없어요")}
              </p>
            ) : (
              <ul className="catalog-drawer-list catalog-drawer-presets">
                {recommendedPresets.map((p) => (
                  <li key={p.id} className="catalog-drawer-preset-item">
                    <span className="catalog-drawer-preset-name">
                      {p.display_name_ko}
                    </span>
                    <span className="catalog-drawer-preset-subtitle">
                      {p.subtitle_ko}
                    </span>
                    <span className="catalog-drawer-preset-chip">
                      {categoryLabelKo(p.category)}
                    </span>
                  </li>
                ))}
              </ul>
            )}
          </section>

          <section>
            <h4 className="catalog-drawer-section-title">
              {t("drawer.section.license")}
            </h4>
            <p className="catalog-drawer-text">{model.license}</p>
          </section>
        </div>
      </aside>
    </div>
  );
}

function pickRuntime(model: ModelEntry | null): RuntimeKind | null {
  if (!model) return null;
  // 우선순위: ollama > lm-studio > 기타.
  if (model.runner_compatibility.includes("ollama")) return "ollama";
  if (model.runner_compatibility.includes("lm-studio")) return "lm-studio";
  return model.runner_compatibility[0] ?? null;
}

interface QuantRowProps {
  quant: QuantOption;
  isRecommended: boolean;
  isChecked: boolean;
  onChange: () => void;
}

function QuantRow({ quant, isRecommended, isChecked, onChange }: QuantRowProps) {
  const { t } = useTranslation();
  return (
    <label className="catalog-drawer-quant-row">
      <input
        type="radio"
        name="quant"
        checked={isChecked}
        onChange={onChange}
      />
      <span className="catalog-drawer-quant-label">{quant.label}</span>
      <span className="catalog-drawer-quant-size num">
        {formatSize(quant.size_mb)}
      </span>
      {isRecommended && (
        <span className="catalog-drawer-quant-rec">
          {t("drawer.quantRecommended")}
        </span>
      )}
    </label>
  );
}
