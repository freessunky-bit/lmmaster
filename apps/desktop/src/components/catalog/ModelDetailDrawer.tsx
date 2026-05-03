// ModelDetailDrawer — 카드 클릭 시 우측 슬라이드 드로워.
//
// 정책 (phase-2pb-catalog-ui-decision.md §7 + phase-install-bench-bugfix-decision §2.3):
// - quant_options 라디오 그룹 + 권장 quant 표시.
// - warnings + use_case_examples 전체.
// - Esc / 배경 클릭으로 닫기.
// - role="dialog" + aria-labelledby + focus trap (간단 — 첫 focusable로 포커스).
// - "이 모델 설치할게요" → in-place 풀 진행 패널 (페이지 이동 없음).
// - 풀 완료 시 자동으로 30초 측정 가능 상태로 전환 + CTA 강조.

import { Sparkles } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  cancelBench,
  getLastBenchReport,
  onBenchFinished,
  startBench,
  type BenchReport,
} from "../../ipc/bench";
import {
  runtimeModelId,
  type CommunityInsights,
  type ModelEntry,
  type QuantOption,
  type RuntimeKind,
} from "../../ipc/catalog";
import {
  bytesToSize,
  cancelModelPull,
  etaToCopy,
  speedToCopy,
  startModelPull,
  statusLabelKo,
  type ModelPullEvent,
} from "../../ipc/model-pull";
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
}

const DEFAULT_BENCH_RUNTIME: RuntimeKind = "ollama";

/** 모델 풀 UI 상태. */
type PullState =
  | { kind: "idle" }
  | { kind: "starting" }
  | {
      kind: "running";
      status: string;
      completed: number;
      total: number;
      speedBps: number;
      etaSecs: number | null;
    }
  | { kind: "done" }
  | { kind: "cancelled" }
  | { kind: "failed"; message: string };

export function ModelDetailDrawer({
  model,
  benchRuntime,
  onClose,
}: ModelDetailDrawerProps) {
  const { t } = useTranslation();
  const closeBtnRef = useRef<HTMLButtonElement>(null);
  const [selectedQuant, setSelectedQuant] = useState<string>("");
  const [benchState, setBenchState] = useState<BenchChipState>({ kind: "idle" });
  const [pullState, setPullState] = useState<PullState>({ kind: "idle" });

  const runtime = benchRuntime ?? pickRuntime(model) ?? DEFAULT_BENCH_RUNTIME;

  // 이 모델을 recommended_models[]에 포함하는 preset 목록.
  const [recommendedPresets, setRecommendedPresets] = useState<Preset[]>([]);

  // model이 바뀔 때마다 첫 quant를 default로 + 캐시된 측정 결과 조회.
  useEffect(() => {
    const first = model?.quantization_options[0];
    if (first) {
      setSelectedQuant(first.label);
    }
    // 모델이 바뀌면 풀 상태도 리셋 — 다른 모델 진행을 잘못 노출하지 않도록.
    setPullState({ kind: "idle" });
    if (!model) {
      setBenchState({ kind: "idle" });
      return;
    }
    let cancelled = false;
    // 캐시 lookup도 변환된 id 사용 — backend cache key가 Ollama 측 model_id 기준.
    const lookupId =
      runtimeModelId(model, first?.label ?? null, runtime) ?? model.id;
    getLastBenchReport({
      modelId: lookupId,
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
  // 백엔드 report.model_id는 변환된 Ollama id이므로 model의 모든 가능한 변환을 비교해야
  // 정확히 매칭됨. 첫 quant + 선택 quant 둘 다 시도.
  useEffect(() => {
    if (!model) return;
    let unlisten: (() => void) | null = null;
    onBenchFinished((report) => {
      const candidates = new Set<string>([model.id]);
      for (const q of model.quantization_options) {
        const id = runtimeModelId(model, q.label, runtime);
        if (id) candidates.add(id);
      }
      if (candidates.has(report.model_id)) {
        setBenchState({ kind: "report", report });
      }
    }).then((u) => {
      unlisten = u;
    });
    return () => {
      unlisten?.();
    };
  }, [model, runtime]);

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
    // 측정도 풀과 같은 변환 — runtime이 인식하는 모델 식별자로 보내야 /api/tags + /api/generate 매칭.
    // 변환 실패 (DirectUrl 등 Ollama 미지원 소스) 시 LMmaster id를 fallback으로 보내 backend가
    // ModelNotLoaded 에러를 사용자에게 명확히 노출.
    const benchId =
      runtimeModelId(model, selectedQuant || null, runtime) ?? model.id;
    setBenchState({ kind: "running" });
    try {
      const report: BenchReport = await startBench({
        modelId: benchId,
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

  const handleCancelBench = useCallback(async () => {
    if (!model) return;
    const benchId =
      runtimeModelId(model, selectedQuant || null, runtime) ?? model.id;
    try {
      await cancelBench(benchId);
    } finally {
      setBenchState({ kind: "idle" });
    }
  }, [model, runtime, selectedQuant]);

  /** 모델 풀 시작 — Ollama 호환만 지원. LM Studio는 외부 링크. */
  const handleInstall = useCallback(async () => {
    if (!model) return;

    // LM Studio-only 모델은 외부 안내. EULA 정책 (silent install 금지).
    const isOllamaCompat = model.runner_compatibility.includes("ollama");
    if (!isOllamaCompat) {
      setPullState({
        kind: "failed",
        message:
          "LM Studio는 자체 앱에서 받아주세요. 카탈로그 검색에 모델 이름을 그대로 넣으면 찾을 수 있어요.",
      });
      return;
    }

    // 카탈로그 LMmaster id → Ollama Hub 형식 변환 (hf.co/{repo}:{quant}).
    // 변환 없이 LMmaster id를 그대로 보내면 Ollama가 "찾지 못했어요" 응답.
    const pullId = runtimeModelId(model, selectedQuant || null, "ollama");
    if (!pullId) {
      setPullState({
        kind: "failed",
        message:
          "이 모델은 직접 받기를 지원하지 않아요. 워크벤치의 가져오기를 사용해 보실래요?",
      });
      return;
    }

    setPullState({ kind: "starting" });
    try {
      const outcome = await startModelPull({
        modelId: pullId,
        runtimeKind: "ollama",
        onEvent: (event: ModelPullEvent) => {
          setPullState((prev) => mergePullEvent(prev, event));
        },
      });
      if (outcome.kind === "completed") {
        setPullState({ kind: "done" });
        // 다음 측정에서 fresh 결과 받을 수 있도록 캐시 무효화 — bench가 자동 재요청.
        setBenchState({ kind: "idle" });
      } else if (outcome.kind === "cancelled") {
        setPullState({ kind: "cancelled" });
      } else {
        setPullState({ kind: "failed", message: outcome.message });
      }
    } catch (e) {
      const msg = (e as { message?: string }).message ?? String(e);
      console.warn("startModelPull failed:", e);
      setPullState({ kind: "failed", message: msg });
    }
  }, [model, selectedQuant]);

  const handleCancelPull = useCallback(async () => {
    if (!model) return;
    // cancel은 같은 pullId를 cancel 키로 사용 — 시작 시 사용한 변환된 이름 재계산.
    const pullId =
      runtimeModelId(model, selectedQuant || null, "ollama") ?? model.id;
    try {
      await cancelModelPull(pullId);
    } catch (e) {
      console.warn("cancelModelPull failed:", e);
    }
  }, [model, selectedQuant]);

  /** "채팅으로 시험하기" — 채팅 페이지로 이동 + 현 모델을 preselect. */
  const handleOpenChat = useCallback(() => {
    if (!model) return;
    const chatId = runtimeModelId(model, selectedQuant || null, "ollama");
    if (!chatId) {
      // 직접 채팅 미지원 모델 — 받기 안내.
      setPullState({
        kind: "failed",
        message:
          "이 모델은 채팅으로 시험하기를 지원하지 않아요. 워크벤치에서 가져온 후에 사용해 보실래요?",
      });
      return;
    }
    try {
      window.localStorage.setItem("lmmaster.chat.preselect", chatId);
    } catch {
      /* ignore */
    }
    window.dispatchEvent(
      new CustomEvent("lmmaster:navigate", { detail: "chat" }),
    );
    onClose();
  }, [model, selectedQuant, onClose]);

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

  const installDisabled =
    pullState.kind === "starting" || pullState.kind === "running";
  const installLabel = installLabelFor(pullState, t);

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
            <button
              type="button"
              className="catalog-drawer-install"
              onClick={handleInstall}
              disabled={installDisabled}
              aria-label={t("drawer.install.aria", { name: model.display_name })}
              data-testid="drawer-install-button"
            >
              {installLabel}
            </button>
            <button
              type="button"
              className="catalog-drawer-chat"
              onClick={handleOpenChat}
              aria-label={`${model.display_name}로 채팅 페이지로 이동할게요`}
              data-testid="drawer-chat-button"
            >
              채팅으로 시험하기
            </button>
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
          {pullState.kind !== "idle" && (
            <PullProgressPanel
              state={pullState}
              onCancel={handleCancelPull}
              onRetry={handleInstall}
            />
          )}

          {/* HF metadata pill row (Phase 13'.e.2) — downloads / likes / lastModified.
              hf_meta가 있을 때만 노출. 큐레이션 시점에 비어있어도 백엔드 cron이 자동 채움. */}
          {model.hf_meta && (
            <div className="catalog-drawer-hfmeta" data-testid="drawer-hf-meta">
              <span className="catalog-drawer-hfmeta-pill num">
                ⬇ {formatDownloads(model.hf_meta.downloads)}
              </span>
              <span className="catalog-drawer-hfmeta-pill num">
                ❤ {model.hf_meta.likes.toLocaleString("ko")}
              </span>
              <span className="catalog-drawer-hfmeta-pill">
                업데이트: {formatHfDate(model.hf_meta.last_modified)}
              </span>
              {model.tier === "new" && (
                <span className="catalog-drawer-hfmeta-pill is-new">
                  <Sparkles size={12} aria-hidden="true" />
                  NEW
                </span>
              )}
            </div>
          )}

          {/* Phase 13'.e.4 — 큐레이터 작성 커뮤니티 인사이트 ? 토글 */}
          {model.community_insights && (
            <CommunityInsightsPanel insights={model.community_insights} />
          )}

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
              onCancel={handleCancelBench}
              onRetry={handleMeasure}
              onInstall={handleInstall}
              installInProgress={installDisabled}
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

/** 정체 감지 카피 — 같은 진행률에서 N초 이상 멈췄을 때 사용자 안심용 마이크로카피.
 *
 * 정책 (research note Toss/HelloDigital 패턴):
 * - 0~15s: 보통 진행 라벨 그대로 ("받고 있어요")
 * - 15~60s: "조금 더 걸려요. 큰 모델은 5~10분쯤 걸려요"
 * - 60s+: "네트워크가 느린가 봐요. 다시 시도해 볼래요?"
 *
 * 큰 모델 풀 (7GB+) 동안 사용자가 앱을 닫지 않게 retention 카피.
 */
function useStalledHint(
  state: PullState,
): { hint: string | null; severity: "info" | "warn" } {
  const [{ hint, severity }, setHint] = useState<{
    hint: string | null;
    severity: "info" | "warn";
  }>({ hint: null, severity: "info" });
  const lastChange = useRef<{ pct: number | null; at: number }>({
    pct: null,
    at: Date.now(),
  });

  useEffect(() => {
    if (state.kind !== "running") {
      setHint({ hint: null, severity: "info" });
      lastChange.current = { pct: null, at: Date.now() };
      return;
    }
    const pct =
      state.total > 0
        ? Math.round((state.completed / state.total) * 100)
        : null;
    if (pct !== lastChange.current.pct) {
      lastChange.current = { pct, at: Date.now() };
      setHint({ hint: null, severity: "info" });
      return;
    }
    // 동일 % 유지 중 — 15s/60s 타이머.
    const t1 = window.setTimeout(() => {
      setHint({
        hint: "조금 더 걸려요. 큰 모델은 5~10분쯤 걸려요",
        severity: "info",
      });
    }, 15_000);
    const t2 = window.setTimeout(() => {
      setHint({
        hint: "네트워크가 느린가 봐요. 다시 시도해 볼래요?",
        severity: "warn",
      });
    }, 60_000);
    return () => {
      window.clearTimeout(t1);
      window.clearTimeout(t2);
    };
  }, [state]);

  return { hint, severity };
}

/** 풀 진행 패널 — 진행률 / 상태 / 속도 / ETA / cancel · retry. */
function PullProgressPanel({
  state,
  onCancel,
  onRetry,
}: {
  state: PullState;
  onCancel: () => void;
  onRetry: () => void;
}) {
  const { hint: stalledHint, severity: stalledSeverity } = useStalledHint(state);
  if (state.kind === "idle") return null;
  if (state.kind === "starting") {
    return (
      <section
        className="model-pull-panel is-running"
        role="status"
        aria-live="polite"
        data-testid="model-pull-panel"
      >
        <p className="model-pull-text">받기를 시작하고 있어요</p>
        <button
          type="button"
          className="model-pull-action"
          onClick={onCancel}
        >
          취소할래요
        </button>
      </section>
    );
  }
  if (state.kind === "running") {
    const pct =
      state.total > 0
        ? Math.min(100, Math.round((state.completed / state.total) * 100))
        : null;
    const eta = etaToCopy(state.etaSecs);
    return (
      <section
        className="model-pull-panel is-running"
        role="status"
        aria-live="polite"
        data-testid="model-pull-panel"
      >
        <div className="model-pull-row">
          <span className="model-pull-status">{statusLabelKo(state.status)}</span>
          {pct != null && (
            <span className="model-pull-pct num">{pct}%</span>
          )}
        </div>
        {state.total > 0 && (
          <progress
            className="model-pull-bar"
            value={state.completed}
            max={state.total}
            aria-label="받기 진행률"
          />
        )}
        <div className="model-pull-meta">
          {state.total > 0 && (
            <span className="num">
              {bytesToSize(state.completed)} / {bytesToSize(state.total)}
            </span>
          )}
          {state.speedBps > 0 && (
            <span className="num">{speedToCopy(state.speedBps)}</span>
          )}
          {eta && <span>{eta}</span>}
        </div>
        {stalledHint && (
          <p
            className={`model-pull-stalled is-${stalledSeverity}`}
            role="status"
            data-testid="model-pull-stalled"
          >
            {stalledHint}
          </p>
        )}
        <button
          type="button"
          className="model-pull-action"
          onClick={onCancel}
          data-testid="model-pull-cancel"
        >
          취소할래요
        </button>
      </section>
    );
  }
  if (state.kind === "done") {
    return (
      <section
        className="model-pull-panel is-done"
        role="status"
        aria-live="polite"
        data-testid="model-pull-panel"
      >
        <p className="model-pull-text">받기 완료. 채팅이나 30초 측정으로 검증해 볼래요?</p>
      </section>
    );
  }
  if (state.kind === "cancelled") {
    return (
      <section
        className="model-pull-panel is-warn"
        role="status"
        data-testid="model-pull-panel"
      >
        <p className="model-pull-text">받기를 취소했어요</p>
        <button
          type="button"
          className="model-pull-action"
          onClick={onRetry}
        >
          다시 받을래요
        </button>
      </section>
    );
  }
  // failed
  return (
    <section
      className="model-pull-panel is-error"
      role="alert"
      data-testid="model-pull-panel"
    >
      <p className="model-pull-text">{state.message}</p>
      <button
        type="button"
        className="model-pull-action"
        onClick={onRetry}
      >
        다시 시도할래요
      </button>
    </section>
  );
}

function mergePullEvent(prev: PullState, event: ModelPullEvent): PullState {
  switch (event.kind) {
    case "status": {
      const total = prev.kind === "running" ? prev.total : 0;
      const completed = prev.kind === "running" ? prev.completed : 0;
      const speedBps = prev.kind === "running" ? prev.speedBps : 0;
      const etaSecs = prev.kind === "running" ? prev.etaSecs : null;
      return {
        kind: "running",
        status: event.status,
        total,
        completed,
        speedBps,
        etaSecs,
      };
    }
    case "progress":
      return {
        kind: "running",
        status: prev.kind === "running" ? prev.status : "pulling",
        total: event.total_bytes,
        completed: event.completed_bytes,
        speedBps: event.speed_bps,
        etaSecs: event.eta_secs,
      };
    case "completed":
      return { kind: "done" };
    case "cancelled":
      return { kind: "cancelled" };
    case "failed":
      return { kind: "failed", message: event.message };
  }
}

function installLabelFor(
  state: PullState,
  t: (k: string, opts?: Record<string, unknown>) => string,
): string {
  if (state.kind === "starting" || state.kind === "running") {
    return "받고 있어요";
  }
  if (state.kind === "done") {
    return "다시 받을래요";
  }
  if (state.kind === "failed" || state.kind === "cancelled") {
    return "다시 받을래요";
  }
  return t("drawer.install.cta");
}

// ── Phase 13'.e.4 — 커뮤니티 인사이트 panel (drawer 내 collapsible) ────

/**
 * 큐레이터가 manifest의 `community_insights`에 작성한 4-section 요약을 collapsible로 노출.
 *
 * 정책:
 * - 토글 닫힘 default — drawer가 길어지지 않게.
 * - 4 섹션: 강점 / 약점 / 자주 쓰이는 분야 / 큐레이터 코멘트.
 * - 출처 URL은 footnote로 — 클릭 가능하게 (외부 링크는 v1.x).
 * - last_reviewed_at 60일+ 지나면 hint chip "검토 후 N일 지남".
 */
function CommunityInsightsPanel({ insights }: { insights: CommunityInsights }) {
  const [open, setOpen] = useState(false);
  const reviewAge = reviewAgeDays(insights.last_reviewed_at ?? null);
  return (
    <section
      className="catalog-drawer-insights"
      data-testid="drawer-community-insights"
    >
      <button
        type="button"
        className="catalog-drawer-insights-toggle"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        aria-controls="drawer-insights-body"
      >
        <span>ⓘ 커뮤니티 인사이트</span>
        <span aria-hidden>{open ? "▾" : "▸"}</span>
      </button>
      {open && (
        <div className="catalog-drawer-insights-body" id="drawer-insights-body">
          {insights.strengths_ko.length > 0 && (
            <div className="catalog-drawer-insights-section">
              <h5 className="catalog-drawer-insights-title">강점</h5>
              <ul className="catalog-drawer-insights-list is-pos">
                {insights.strengths_ko.map((s) => (
                  <li key={s}>{s}</li>
                ))}
              </ul>
            </div>
          )}
          {insights.weaknesses_ko.length > 0 && (
            <div className="catalog-drawer-insights-section">
              <h5 className="catalog-drawer-insights-title">약점</h5>
              <ul className="catalog-drawer-insights-list is-neg">
                {insights.weaknesses_ko.map((s) => (
                  <li key={s}>{s}</li>
                ))}
              </ul>
            </div>
          )}
          {insights.use_cases_ko.length > 0 && (
            <div className="catalog-drawer-insights-section">
              <h5 className="catalog-drawer-insights-title">자주 쓰이는 분야</h5>
              <ul className="catalog-drawer-insights-list">
                {insights.use_cases_ko.map((s) => (
                  <li key={s}>{s}</li>
                ))}
              </ul>
            </div>
          )}
          {insights.curator_note_ko && (
            <div className="catalog-drawer-insights-section">
              <h5 className="catalog-drawer-insights-title">큐레이터 코멘트</h5>
              <p className="catalog-drawer-insights-note">
                {insights.curator_note_ko}
              </p>
            </div>
          )}
          {insights.sources.length > 0 && (
            <p className="catalog-drawer-insights-sources">
              출처: {insights.sources.join(" · ")}
            </p>
          )}
          {reviewAge != null && reviewAge > 60 && (
            <p className="catalog-drawer-insights-stale">
              검토 후 {reviewAge}일 지났어요. 최신 커뮤니티 의견과 다를 수 있어요.
            </p>
          )}
        </div>
      )}
    </section>
  );
}

/** HF downloads — 1.2K / 4.5M 형식 한국어. */
function formatDownloads(n: number): string {
  if (n < 1000) return n.toString();
  if (n < 1_000_000) return `${(n / 1000).toFixed(1)}K`;
  return `${(n / 1_000_000).toFixed(1)}M`;
}

/** HF lastModified RFC3339 → "N일 전" 또는 "YYYY-MM-DD". */
function formatHfDate(iso: string): string {
  if (!iso) return "-";
  const t = Date.parse(iso);
  if (isNaN(t)) return "-";
  const days = Math.floor((Date.now() - t) / (24 * 60 * 60 * 1000));
  if (days < 1) return "오늘";
  if (days < 7) return `${days}일 전`;
  if (days < 30) return `${Math.floor(days / 7)}주 전`;
  if (days < 365) return `${Math.floor(days / 30)}개월 전`;
  return `${Math.floor(days / 365)}년 전`;
}

/** community_insights.last_reviewed_at부터 N일 지남. null이면 null 반환. */
function reviewAgeDays(iso: string | null): number | null {
  if (!iso) return null;
  const t = Date.parse(iso);
  if (isNaN(t)) return null;
  return Math.floor((Date.now() - t) / (24 * 60 * 60 * 1000));
}
