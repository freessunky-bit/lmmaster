// 추론 파라미터 조절 Drawer — v0.8.4 (ADR + decision note v0.8.4 §A.B).
//
// 정책:
// - role="dialog" aria-modal="true" + Esc/배경 클릭 닫기 + 첫 input auto-focus.
// - 프리셋 3종 (정확/균형/창의) + "직접 조절" 펼침. raw slider 단독 X.
// - 영속: localStorage `lmmaster.personas-survey.sampling.v1` (Step 4 한정 ephemeral).
// - 5종 노출: max_tokens / temperature / top_p / repeat_penalty / seed. top_k·num_ctx는 v0.8.5+.

import { useCallback, useEffect, useId, useRef, useState } from "react";
import { Sliders, X } from "lucide-react";

import type { SamplingParams } from "../../ipc/personas";

const STORAGE_KEY = "lmmaster.personas-survey.sampling.v1";

export type SamplingPreset = "precise" | "balanced" | "creative" | "custom";

export interface PersistedSampling {
  preset: SamplingPreset;
  custom?: SamplingParams;
}

const PRESET_VALUES: Record<Exclude<SamplingPreset, "custom">, SamplingParams> = {
  precise: { max_tokens: 1024, temperature: 0.2, top_p: 0.9, repeat_penalty: 1.05 },
  balanced: { max_tokens: 1024, temperature: 0.7, top_p: 0.95, repeat_penalty: 1.1 },
  creative: { max_tokens: 1024, temperature: 1.0, top_p: 0.98, repeat_penalty: 1.15 },
};

const DEFAULT_CUSTOM: SamplingParams = {
  max_tokens: 1024,
  temperature: 0.7,
  top_p: 0.95,
  repeat_penalty: 1.1,
  seed: null,
};

export function loadPersistedSampling(): PersistedSampling {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { preset: "balanced" };
    const parsed = JSON.parse(raw) as PersistedSampling;
    if (
      parsed.preset === "precise" ||
      parsed.preset === "balanced" ||
      parsed.preset === "creative" ||
      parsed.preset === "custom"
    ) {
      return parsed;
    }
  } catch {
    /* corrupt — fall through */
  }
  return { preset: "balanced" };
}

export function effectiveSampling(p: PersistedSampling): SamplingParams {
  if (p.preset === "custom") {
    return p.custom ?? DEFAULT_CUSTOM;
  }
  return PRESET_VALUES[p.preset];
}

interface Props {
  open: boolean;
  initial: PersistedSampling;
  onClose: () => void;
  onApply: (next: PersistedSampling) => void;
  /** 자동 포커스 대상 — "잘렸어요" 칩에서 클릭한 경우 max_tokens에 포커스. */
  focusField?: "max_tokens" | null;
}

export function SamplingDrawer({ open, initial, onClose, onApply, focusField }: Props) {
  const titleId = useId();
  const dialogRef = useRef<HTMLDivElement | null>(null);
  const closeBtnRef = useRef<HTMLButtonElement | null>(null);
  const maxTokensRef = useRef<HTMLInputElement | null>(null);
  const [preset, setPreset] = useState<SamplingPreset>(initial.preset);
  const [custom, setCustom] = useState<SamplingParams>(
    initial.preset === "custom" ? initial.custom ?? DEFAULT_CUSTOM : DEFAULT_CUSTOM,
  );

  // open 진입 시 initial reset.
  useEffect(() => {
    if (open) {
      setPreset(initial.preset);
      setCustom(initial.preset === "custom" ? initial.custom ?? DEFAULT_CUSTOM : DEFAULT_CUSTOM);
    }
  }, [open, initial]);

  // a11y — open 시 첫 요소 focus.
  useEffect(() => {
    if (!open) return;
    const t = setTimeout(() => {
      if (focusField === "max_tokens") {
        maxTokensRef.current?.focus();
        maxTokensRef.current?.select();
      } else {
        closeBtnRef.current?.focus();
      }
    }, 30);
    return () => clearTimeout(t);
  }, [open, focusField]);

  // Esc 닫기.
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [open, onClose]);

  const handleApply = useCallback(() => {
    const persisted: PersistedSampling =
      preset === "custom" ? { preset: "custom", custom } : { preset };
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(persisted));
    } catch {
      /* private mode 등 — 무시 */
    }
    onApply(persisted);
    onClose();
  }, [preset, custom, onApply, onClose]);

  const handleReset = useCallback(() => {
    setPreset("balanced");
    setCustom(DEFAULT_CUSTOM);
  }, []);

  if (!open) return null;

  return (
    <>
      <div className="personas-drawer-backdrop" onClick={onClose} aria-hidden="true" />
      <div
        ref={dialogRef}
        className="personas-drawer"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
      >
        <div className="personas-drawer-header">
          <Sliders size={18} aria-hidden="true" />
          <h3 id={titleId} className="personas-drawer-title">
            추론 파라미터
          </h3>
          <button
            ref={closeBtnRef}
            type="button"
            className="personas-drawer-close"
            onClick={onClose}
            aria-label="닫기"
          >
            <X size={16} aria-hidden="true" />
          </button>
        </div>

        <div className="personas-drawer-body">
          <fieldset className="personas-drawer-fieldset">
            <legend>프리셋</legend>
            <div role="radiogroup" aria-label="프리셋 선택" className="personas-preset-group">
              {[
                { id: "precise", label: "정확하게", desc: "객관식·통계 일관성 ↑" },
                { id: "balanced", label: "균형 (기본)", desc: "일반 추천" },
                { id: "creative", label: "창의적", desc: "주관식 다양성 ↑" },
                { id: "custom", label: "직접 조절", desc: "고급 사용자" },
              ].map((opt) => (
                <button
                  type="button"
                  key={opt.id}
                  role="radio"
                  aria-checked={preset === opt.id}
                  className={`personas-preset-btn${
                    preset === opt.id ? " is-selected" : ""
                  }`}
                  onClick={() => setPreset(opt.id as SamplingPreset)}
                >
                  <strong>{opt.label}</strong>
                  <span>{opt.desc}</span>
                </button>
              ))}
            </div>
          </fieldset>

          {preset === "custom" && (
            <fieldset className="personas-drawer-fieldset">
              <legend>직접 조절</legend>

              <ParamRow
                label="최대 답변 길이"
                helper="값이 클수록 답변이 길어요. 너무 작으면 잘릴 수 있어요."
                control={
                  <input
                    ref={maxTokensRef}
                    type="number"
                    min={64}
                    max={8192}
                    step={64}
                    value={custom.max_tokens ?? 1024}
                    onChange={(e) =>
                      setCustom((c) => ({
                        ...c,
                        max_tokens: parseInt(e.target.value || "1024", 10),
                      }))
                    }
                  />
                }
                unit="토큰"
              />

              <ParamRow
                label="창의성 (temperature)"
                helper="0에 가까우면 일관, 1 이상이면 다양성 ↑."
                control={
                  <SliderWithValue
                    min={0}
                    max={1.5}
                    step={0.05}
                    value={custom.temperature ?? 0.7}
                    onChange={(v) => setCustom((c) => ({ ...c, temperature: v }))}
                  />
                }
              />

              <ParamRow
                label="다양성 (top_p)"
                helper="0.95가 일반적, 0.9 이하는 매우 보수적."
                control={
                  <SliderWithValue
                    min={0.1}
                    max={1.0}
                    step={0.05}
                    value={custom.top_p ?? 0.95}
                    onChange={(v) => setCustom((c) => ({ ...c, top_p: v }))}
                  />
                }
              />

              <ParamRow
                label="반복 억제 (repeat_penalty)"
                helper="같은 단어 반복이 많으면 1.2~1.3으로 올려보세요."
                control={
                  <SliderWithValue
                    min={1.0}
                    max={1.5}
                    step={0.01}
                    value={custom.repeat_penalty ?? 1.1}
                    onChange={(v) => setCustom((c) => ({ ...c, repeat_penalty: v }))}
                  />
                }
              />

              <ParamRow
                label="재현 시드 (seed)"
                helper="같은 시드 + 같은 입력 = 항상 같은 답. 실험 재현용."
                control={
                  <div className="personas-form-inline">
                    <label className="personas-seed-toggle">
                      <input
                        type="checkbox"
                        checked={custom.seed !== null && custom.seed !== undefined}
                        onChange={(e) =>
                          setCustom((c) => ({
                            ...c,
                            seed: e.target.checked ? 42 : null,
                          }))
                        }
                      />
                      <span>고정</span>
                    </label>
                    <input
                      type="number"
                      min={0}
                      value={custom.seed ?? 42}
                      onChange={(e) =>
                        setCustom((c) => ({
                          ...c,
                          seed: parseInt(e.target.value || "42", 10),
                        }))
                      }
                      disabled={custom.seed === null || custom.seed === undefined}
                    />
                  </div>
                }
              />
            </fieldset>
          )}
        </div>

        <div className="personas-drawer-footer">
          <button type="button" className="personas-btn-secondary" onClick={handleReset}>
            기본값으로 되돌릴게요
          </button>
          <div className="personas-form-actions">
            <button type="button" className="personas-btn-secondary" onClick={onClose}>
              취소할래요
            </button>
            <button type="button" className="personas-btn-primary" onClick={handleApply}>
              적용할게요
            </button>
          </div>
        </div>
      </div>
    </>
  );
}

function ParamRow({
  label,
  helper,
  control,
  unit,
}: {
  label: string;
  helper: string;
  control: React.ReactNode;
  unit?: string;
}) {
  return (
    <div className="personas-param-row">
      <div className="personas-param-label">
        <strong>{label}</strong>
        <span className="personas-param-helper">{helper}</span>
      </div>
      <div className="personas-param-control">
        {control}
        {unit && <span className="personas-param-unit num">{unit}</span>}
      </div>
    </div>
  );
}

function SliderWithValue({
  min,
  max,
  step,
  value,
  onChange,
}: {
  min: number;
  max: number;
  step: number;
  value: number;
  onChange: (v: number) => void;
}) {
  return (
    <div className="personas-slider-row">
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(parseFloat(e.target.value))}
        className="personas-slider"
      />
      <span className="personas-slider-value num">{value.toFixed(2)}</span>
    </div>
  );
}
