// Workbench — Phase 5'.b 5단계 작업대 UI.
//
// 정책 (phase-5p-workbench-decision.md, phase-5pb-4p5b-ipc-reinforcement.md):
// - xstate v5 state machine — Onboarding 패턴 차용. 5단계 + idle/running/completed/failed/cancelled.
// - Channel<WorkbenchEvent>를 onEvent로 받아 머신에 send.
// - Korean 해요체. 디자인 토큰 + 기존 workbench.css 재활용.
// - a11y: 각 step <section role="region" aria-labelledby>, progressbar, focus-visible ring.
// - Cancel 버튼 — cancelWorkbenchRun 호출. terminal 후 새 작업 진입 가능.
//
// 5단계: data → quantize → lora → validate → register.
// 백엔드는 단일 run_workbench 호출 안에서 5단계를 자동 진행한다 (mock).
// 프론트는 매 StageStarted/Progress/Completed event를 받아 UI 갱신.

import {
  useCallback,
  useEffect,
  useReducer,
  useRef,
  useState,
  type Dispatch,
} from "react";
import { useTranslation } from "react-i18next";

import { HelpButton } from "../components/HelpButton";
import {
  cancelWorkbenchRun,
  isTerminalEvent,
  previewJsonl,
  startWorkbenchRun,
  type ChatExample,
  type EvalReport,
  type StartWorkbenchHandle,
  type WorkbenchConfig,
  type WorkbenchEvent,
  type WorkbenchRunSummary,
  type WorkbenchStep,
} from "../ipc/workbench";

import "./workbench.css";

// ── 상수 ─────────────────────────────────────────────────────────────

const STEP_KEYS = ["data", "quantize", "lora", "validate", "register"] as const;
type StepKey = (typeof STEP_KEYS)[number];

const STEP_INDEX: Record<StepKey, number> = {
  data: 0,
  quantize: 1,
  lora: 2,
  validate: 3,
  register: 4,
};

const QUANT_TYPES = ["Q4_K_M", "Q5_K_M", "Q8_0", "FP16"] as const;

type RunStatus = "idle" | "running" | "completed" | "failed" | "cancelled";

// ── State machine (useReducer 기반 — xstate v5 setup 필요한 actor가 없어 단순화) ─

interface WorkbenchState {
  status: RunStatus;
  /** 현재 step key. idle/initial = "data". 실행 중에는 backend 이벤트로 갱신. */
  currentStep: StepKey;
  config: WorkbenchConfig;
  /** 진행 중 run handle — cancel용. */
  handle: StartWorkbenchHandle | null;
  runId: string | null;
  /** 현재 step의 progress percent (0-100). */
  percent: number;
  /** 현재 step의 한국어 메시지. */
  message: string | null;
  /** terminal 종료 시 summary. */
  summary: WorkbenchRunSummary | null;
  /** Validate stage가 publish한 per-case + 카테고리 집계. */
  evalReport: EvalReport | null;
  /** Register stage가 model-registry에 영속한 custom-model id. */
  registeredModelId: string | null;
  /** Phase 5'.e — `ollama create` shell-out 출력 라인. */
  ollamaCreateLines: string[];
  /** Phase 5'.e — `ollama create`가 등록한 모델명 (사용자 향 표시). */
  ollamaOutputName: string | null;
  /** 한국어 에러 메시지 — Failed에서 채움. */
  error: string | null;
}

type WBAction =
  | { type: "SET_CONFIG"; patch: Partial<WorkbenchConfig> }
  | { type: "GO_STEP"; step: StepKey }
  | { type: "START"; handle: StartWorkbenchHandle }
  | { type: "EVENT"; event: WorkbenchEvent }
  | { type: "RESET" }
  | { type: "START_FAILED"; message: string };

const DEFAULT_CONFIG: WorkbenchConfig = {
  base_model_id: "Qwen2.5-3B",
  data_jsonl_path: "",
  quant_type: "Q4_K_M",
  lora_epochs: 3,
  korean_preset: true,
  register_to_ollama: true,
  // Phase 5'.e — 기본은 mock (외부 통신 0). 사용자가 명시 선택 시 실 HTTP.
  responder_runtime: "mock",
  responder_base_url: null,
  responder_model_id: null,
};

const RUNTIME_OPTIONS = ["mock", "ollama", "lm-studio"] as const;
const DEFAULT_BASE_URL: Record<(typeof RUNTIME_OPTIONS)[number], string> = {
  mock: "",
  ollama: "http://localhost:11434",
  "lm-studio": "http://localhost:1234",
};

const INITIAL_STATE: WorkbenchState = {
  status: "idle",
  currentStep: "data",
  config: DEFAULT_CONFIG,
  handle: null,
  runId: null,
  percent: 0,
  message: null,
  summary: null,
  evalReport: null,
  registeredModelId: null,
  ollamaCreateLines: [],
  ollamaOutputName: null,
  error: null,
};

function reducer(state: WorkbenchState, action: WBAction): WorkbenchState {
  switch (action.type) {
    case "SET_CONFIG":
      return { ...state, config: { ...state.config, ...action.patch } };
    case "GO_STEP":
      // 사용자가 manual 단계 이동 — idle 또는 terminal 상태에서만 허용.
      if (state.status === "running") return state;
      return { ...state, currentStep: action.step };
    case "START":
      return {
        ...state,
        status: "running",
        handle: action.handle,
        runId: action.handle.run_id,
        currentStep: "data",
        percent: 0,
        message: null,
        summary: null,
        evalReport: null,
        registeredModelId: null,
        ollamaCreateLines: [],
        ollamaOutputName: null,
        error: null,
      };
    case "START_FAILED":
      return {
        ...state,
        status: "failed",
        handle: null,
        error: action.message,
      };
    case "EVENT":
      return applyEvent(state, action.event);
    case "RESET":
      return {
        ...INITIAL_STATE,
        config: state.config,
      };
  }
}

function applyEvent(state: WorkbenchState, event: WorkbenchEvent): WorkbenchState {
  switch (event.kind) {
    case "started":
      return { ...state, status: "running", runId: event.run_id, currentStep: "data" };
    case "stage-started":
      return {
        ...state,
        currentStep: event.stage,
        percent: 0,
        message: null,
      };
    case "stage-progress":
      return {
        ...state,
        currentStep: event.progress.stage,
        percent: event.progress.percent,
        message: event.progress.message,
      };
    case "stage-completed":
      return { ...state, percent: 100 };
    case "eval-completed":
      return { ...state, evalReport: event.report };
    case "register-completed":
      return { ...state, registeredModelId: event.model_id };
    case "ollama-create-started":
      return {
        ...state,
        ollamaOutputName: event.output_name,
        ollamaCreateLines: [],
      };
    case "ollama-create-progress":
      return {
        ...state,
        // 마지막 50줄만 보존 (UI 메모리 보호).
        ollamaCreateLines: [...state.ollamaCreateLines, event.line].slice(-50),
      };
    case "ollama-create-completed":
      return state;
    case "ollama-create-failed":
      return state;
    case "completed":
      return {
        ...state,
        status: "completed",
        summary: event.summary,
        // EvalCompleted/RegisterCompleted가 먼저 도착했으면 그 값을 그대로 두고, 아니면 summary에서 보강.
        evalReport: state.evalReport ?? event.summary.eval_report,
        registeredModelId:
          state.registeredModelId ?? event.summary.registered_model_id,
        currentStep: "register",
        percent: 100,
        handle: null,
      };
    case "failed":
      return {
        ...state,
        status: "failed",
        error: event.error,
        handle: null,
      };
    case "cancelled":
      return {
        ...state,
        status: "cancelled",
        handle: null,
      };
  }
}

// ── 컴포넌트 ─────────────────────────────────────────────────────────

export function Workbench() {
  const { t } = useTranslation();
  const [state, dispatch] = useReducer(reducer, INITIAL_STATE);
  const handleRef = useRef<StartWorkbenchHandle | null>(null);

  // handle ref를 state.handle과 동기화 — cleanup에서 사용.
  useEffect(() => {
    handleRef.current = state.handle;
  }, [state.handle]);

  // 언마운트 시 진행 중 run cancel.
  useEffect(() => {
    return () => {
      const h = handleRef.current;
      if (h) {
        void h.cancel().catch(() => {
          /* idempotent — 이미 종료된 run이면 무시 */
        });
      }
    };
  }, []);

  const handleStart = useCallback(async () => {
    if (state.status === "running") return;
    if (!state.config.base_model_id.trim()) {
      dispatch({
        type: "START_FAILED",
        message: t("screens.workbench.errors.missingBaseModel"),
      });
      return;
    }
    try {
      const handle = await startWorkbenchRun(state.config, {
        onEvent: (ev) => {
          dispatch({ type: "EVENT", event: ev });
          if (isTerminalEvent(ev)) {
            handleRef.current = null;
          }
        },
      });
      dispatch({ type: "START", handle });
    } catch (e) {
      const msg = extractErrorMessage(e, t("screens.workbench.errors.startFailed"));
      dispatch({ type: "START_FAILED", message: msg });
    }
  }, [state.config, state.status, t]);

  const handleCancel = useCallback(async () => {
    if (state.handle) {
      try {
        await state.handle.cancel();
      } catch {
        // idempotent — 이미 끝났으면 무시.
      }
    } else if (state.runId) {
      // handle이 null이지만 runId 있으면 backend에 best-effort cancel.
      try {
        await cancelWorkbenchRun(state.runId);
      } catch {
        // 무시.
      }
    }
  }, [state.handle, state.runId]);

  const handleReset = useCallback(() => {
    dispatch({ type: "RESET" });
  }, []);

  const goStep = useCallback((step: StepKey) => {
    dispatch({ type: "GO_STEP", step });
  }, []);

  const stepIndex = STEP_INDEX[state.currentStep];

  return (
    <div className="workbench-root" data-testid="workbench-page">
      <header className="workbench-topbar">
        <h2 className="workbench-page-title">{t("screens.workbench.title")}</h2>
        <HelpButton
          sectionId="workbench"
          hint={t("screens.help.workbench") ?? undefined}
          testId="workbench-help"
        />
        <span className="workbench-coming-badge" aria-label={t("screens.workbench.subtitle")}>
          {t("screens.workbench.subtitle")}
        </span>
        <span className="workbench-flow-status" data-status={state.status} data-testid="workbench-status">
          {t(`screens.workbench.status.${state.status}`)}
        </span>
      </header>

      <ol className="workbench-stepper" aria-label={t("screens.workbench.title")}>
        {STEP_KEYS.map((key, idx) => (
          <li key={key} className="workbench-stepper-item">
            <button
              type="button"
              className="workbench-stepper-trigger"
              aria-current={idx === stepIndex ? "step" : undefined}
              data-active={idx === stepIndex ? "true" : undefined}
              data-testid={`wb-stepper-${key}`}
              disabled={state.status === "running"}
              onClick={() => goStep(key)}
            >
              <span className="workbench-stepper-dot" aria-hidden="true">
                {idx + 1}
              </span>
              <span className="workbench-stepper-title">
                {t(`screens.workbench.stepper.${key}`)}
              </span>
            </button>
            {idx < STEP_KEYS.length - 1 && (
              <span className="workbench-stepper-sep" aria-hidden="true" />
            )}
          </li>
        ))}
      </ol>

      <ProgressPanel state={state} />

        <div className="workbench-step-content" aria-live="polite">
          {state.currentStep === "data" && (
            <DataStep
              config={state.config}
              dispatch={dispatch}
              status={state.status}
              onStart={handleStart}
              onNext={() => goStep("quantize")}
            />
          )}
          {state.currentStep === "quantize" && (
            <QuantizeStep
              config={state.config}
              dispatch={dispatch}
              status={state.status}
              onStart={handleStart}
              onBack={() => goStep("data")}
              onNext={() => goStep("lora")}
            />
          )}
          {state.currentStep === "lora" && (
            <LoraStep
              config={state.config}
              dispatch={dispatch}
              status={state.status}
              onStart={handleStart}
              onBack={() => goStep("quantize")}
              onNext={() => goStep("validate")}
            />
          )}
          {state.currentStep === "validate" && (
            <ValidateStep
              status={state.status}
              summary={state.summary}
              evalReport={state.evalReport}
              onStart={handleStart}
              onBack={() => goStep("lora")}
              onNext={() => goStep("register")}
            />
          )}
          {state.currentStep === "register" && (
            <RegisterStep
              config={state.config}
              dispatch={dispatch}
              status={state.status}
              summary={state.summary}
              registeredModelId={state.registeredModelId}
              ollamaOutputName={state.ollamaOutputName}
              ollamaCreateLines={state.ollamaCreateLines}
              onStart={handleStart}
              onBack={() => goStep("validate")}
              onReset={handleReset}
            />
          )}
        </div>

        <footer className="workbench-actions" role="group" aria-label={t("screens.workbench.actions.cancel")}>
          {state.status === "running" && (
            <button
              type="button"
              className="workbench-button workbench-button-secondary"
              onClick={handleCancel}
              data-testid="workbench-cancel"
            >
              {t("screens.workbench.actions.cancel")}
            </button>
          )}
          {(state.status === "failed" || state.status === "cancelled") && (
            <button
              type="button"
              className="workbench-button workbench-button-primary"
              onClick={handleStart}
              data-testid="workbench-retry"
            >
              {t("screens.workbench.actions.retry")}
            </button>
          )}
          {state.status === "completed" && (
            <button
              type="button"
              className="workbench-button workbench-button-secondary"
              onClick={handleReset}
              data-testid="workbench-new-run"
            >
              {t("screens.workbench.actions.newRun")}
            </button>
          )}
        </footer>

        {state.error && (
          <div className="workbench-error" role="alert" data-testid="workbench-error">
            {state.error}
          </div>
        )}
    </div>
  );
}

// ── ProgressPanel ───────────────────────────────────────────────────

function ProgressPanel({ state }: { state: WorkbenchState }) {
  const { t } = useTranslation();
  if (state.status === "idle") return null;
  const stageLabel = t(`screens.workbench.stepper.${state.currentStep}`);
  return (
    <div className="workbench-progress" data-testid="workbench-progress">
      <div className="workbench-progress-meta num">
        <span>{t("screens.workbench.progress.current", { stage: stageLabel })}</span>
        <span>{state.percent}%</span>
      </div>
      <div
        role="progressbar"
        aria-label={t("screens.workbench.progress.label") ?? undefined}
        aria-valuenow={state.percent}
        aria-valuemin={0}
        aria-valuemax={100}
        className="workbench-progress-bar"
      >
        <div className="workbench-progress-fill" style={{ width: `${state.percent}%` }} />
      </div>
      {state.message && (
        <p className="workbench-progress-message" aria-live="polite">
          {state.message}
        </p>
      )}
    </div>
  );
}

// ── Step 1 — Data ────────────────────────────────────────────────────

interface StepCommonProps {
  status: RunStatus;
  onStart: () => void;
  onNext?: () => void;
  onBack?: () => void;
}

interface DataStepProps extends StepCommonProps {
  config: WorkbenchConfig;
  dispatch: Dispatch<WBAction>;
}

function DataStep({ config, dispatch, status, onStart, onNext }: DataStepProps) {
  const { t } = useTranslation();
  const [preview, setPreview] = useState<ChatExample[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);

  // 경로 입력 → 디바운스로 preview 호출.
  useEffect(() => {
    const path = config.data_jsonl_path.trim();
    if (!path) {
      setPreview(null);
      setPreviewError(null);
      setLoading(false);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setPreviewError(null);
    const timer = setTimeout(async () => {
      try {
        const examples = await previewJsonl(path, 5);
        if (cancelled) return;
        setPreview(examples);
      } catch (e) {
        if (cancelled) return;
        setPreview(null);
        setPreviewError(extractErrorMessage(e, t("screens.workbench.data.previewError")));
      } finally {
        if (!cancelled) setLoading(false);
      }
    }, 250);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [config.data_jsonl_path, t]);

  return (
    <section className="workbench-step" role="region" aria-labelledby="wb-step-data-title">
      <header className="workbench-step-header">
        <h3 id="wb-step-data-title" className="workbench-step-title">
          {t("screens.workbench.data.title")}
        </h3>
        <p className="workbench-step-subtitle">{t("screens.workbench.data.subtitle")}</p>
      </header>

      <div className="workbench-form">
        <label className="workbench-field">
          <span className="workbench-field-label">{t("screens.workbench.config.baseModel")}</span>
          <input
            type="text"
            value={config.base_model_id}
            onChange={(e) =>
              dispatch({ type: "SET_CONFIG", patch: { base_model_id: e.target.value } })
            }
            disabled={status === "running"}
            className="workbench-input"
            data-testid="wb-input-base-model"
          />
          <span className="workbench-field-hint">
            {t("screens.workbench.config.baseModelHint")}
          </span>
        </label>

        <label className="workbench-field">
          <span className="workbench-field-label">{t("screens.workbench.config.datasetPath")}</span>
          <input
            type="text"
            value={config.data_jsonl_path}
            onChange={(e) =>
              dispatch({ type: "SET_CONFIG", patch: { data_jsonl_path: e.target.value } })
            }
            disabled={status === "running"}
            className="workbench-input"
            placeholder="/path/to/train.jsonl"
            data-testid="wb-input-dataset-path"
          />
          <span className="workbench-field-hint">
            {t("screens.workbench.config.datasetPathHint")}
          </span>
        </label>
      </div>

      <RuntimeSelector config={config} dispatch={dispatch} status={status} />

      <div className="workbench-preview" data-testid="wb-preview">
        <h4 className="workbench-preview-title">{t("screens.workbench.data.previewTitle")}</h4>
        {loading && <p className="workbench-preview-status">{t("screens.workbench.data.previewLoading")}</p>}
        {!loading && previewError && (
          <p className="workbench-preview-status workbench-preview-error" role="alert">
            {previewError}
          </p>
        )}
        {!loading && !previewError && preview && preview.length === 0 && (
          <p className="workbench-preview-status">{t("screens.workbench.data.previewEmpty")}</p>
        )}
        {!loading && !previewError && preview && preview.length > 0 && (
          <ol className="workbench-preview-list" aria-label={t("screens.workbench.data.previewTitle")}>
            {preview.map((ex, i) => (
              <li key={i} className="workbench-preview-item">
                {ex.messages.map((m, j) => (
                  <div key={j} className="workbench-preview-msg" data-role={m.role}>
                    <span className="workbench-preview-role num">{m.role}</span>
                    <span className="workbench-preview-content">{m.content}</span>
                  </div>
                ))}
              </li>
            ))}
          </ol>
        )}
      </div>

      <StepNav
        onBack={undefined}
        onNext={onNext}
        onStart={onStart}
        status={status}
        showStart
      />
    </section>
  );
}

// ── RuntimeSelector — Phase 5'.e Validate stage 런타임 선택 ─────────

interface RuntimeSelectorProps {
  config: WorkbenchConfig;
  dispatch: Dispatch<WBAction>;
  status: RunStatus;
}

function RuntimeSelector({ config, dispatch, status }: RuntimeSelectorProps) {
  const { t } = useTranslation();
  const runtime = (config.responder_runtime ?? "mock") as (typeof RUNTIME_OPTIONS)[number];
  const showHttpFields = runtime === "ollama" || runtime === "lm-studio";

  return (
    <fieldset
      className="workbench-radiogroup"
      role="radiogroup"
      aria-labelledby="wb-runtime-label"
      data-testid="wb-runtime-selector"
    >
      <legend id="wb-runtime-label" className="workbench-field-label">
        {t("screens.workbench.runtime.label")}
      </legend>
      {RUNTIME_OPTIONS.map((opt) => {
        const checked = runtime === opt;
        return (
          <button
            key={opt}
            type="button"
            role="radio"
            aria-checked={checked}
            className={`workbench-radio${checked ? " is-checked" : ""}`}
            disabled={status === "running"}
            data-testid={`wb-runtime-${opt}`}
            onClick={() =>
              dispatch({
                type: "SET_CONFIG",
                patch: {
                  responder_runtime: opt,
                  responder_base_url:
                    opt === "mock" ? null : DEFAULT_BASE_URL[opt],
                  // mock은 register_to_ollama 강제 OFF (외부 호출 불가능).
                  register_to_ollama: opt === "mock" ? false : config.register_to_ollama,
                },
              })
            }
          >
            <span>{t(`screens.workbench.runtime.option.${opt}`)}</span>
          </button>
        );
      })}
      <span className="workbench-field-hint">
        {t("screens.workbench.runtime.hint")}
      </span>

      {showHttpFields && (
        <div className="workbench-form" data-testid="wb-runtime-http-fields">
          <label className="workbench-field">
            <span className="workbench-field-label">
              {t("screens.workbench.runtime.baseUrlLabel")}
            </span>
            <input
              type="text"
              className="workbench-input"
              value={config.responder_base_url ?? ""}
              placeholder={DEFAULT_BASE_URL[runtime]}
              disabled={status === "running"}
              data-testid="wb-input-runtime-base-url"
              onChange={(e) =>
                dispatch({
                  type: "SET_CONFIG",
                  patch: { responder_base_url: e.target.value },
                })
              }
            />
            <span className="workbench-field-hint">
              {t("screens.workbench.runtime.baseUrlHint")}
            </span>
          </label>
          <label className="workbench-field">
            <span className="workbench-field-label">
              {t("screens.workbench.runtime.modelIdLabel")}
            </span>
            <input
              type="text"
              className="workbench-input"
              value={config.responder_model_id ?? ""}
              placeholder={config.base_model_id}
              disabled={status === "running"}
              data-testid="wb-input-runtime-model-id"
              onChange={(e) =>
                dispatch({
                  type: "SET_CONFIG",
                  patch: { responder_model_id: e.target.value },
                })
              }
            />
            <span className="workbench-field-hint">
              {t("screens.workbench.runtime.modelIdHint")}
            </span>
          </label>
        </div>
      )}

      {runtime === "ollama" && (
        <button
          type="button"
          role="switch"
          aria-checked={config.register_to_ollama}
          className={`workbench-toggle${config.register_to_ollama ? " is-on" : ""}`}
          disabled={status === "running"}
          data-testid="wb-toggle-ollama-create"
          onClick={() =>
            dispatch({
              type: "SET_CONFIG",
              patch: { register_to_ollama: !config.register_to_ollama },
            })
          }
        >
          <span className="workbench-toggle-track" aria-hidden>
            <span className="workbench-toggle-thumb" />
          </span>
          <span className="workbench-toggle-text">
            {t("screens.workbench.runtime.ollamaCreateToggle")}
          </span>
        </button>
      )}
    </fieldset>
  );
}

// ── Step 2 — Quantize ───────────────────────────────────────────────

interface QuantizeStepProps extends StepCommonProps {
  config: WorkbenchConfig;
  dispatch: Dispatch<WBAction>;
}

function QuantizeStep({ config, dispatch, status, onStart, onBack, onNext }: QuantizeStepProps) {
  const { t } = useTranslation();
  return (
    <section className="workbench-step" role="region" aria-labelledby="wb-step-quantize-title">
      <header className="workbench-step-header">
        <h3 id="wb-step-quantize-title" className="workbench-step-title">
          {t("screens.workbench.quantize.title")}
        </h3>
        <p className="workbench-step-subtitle">{t("screens.workbench.quantize.subtitle")}</p>
      </header>

      <fieldset
        className="workbench-radiogroup"
        role="radiogroup"
        aria-labelledby="wb-quant-type-label"
      >
        <legend id="wb-quant-type-label" className="workbench-field-label">
          {t("screens.workbench.quantize.typeLabel")}
        </legend>
        {QUANT_TYPES.map((q) => {
          const checked = config.quant_type === q;
          return (
            <button
              key={q}
              type="button"
              role="radio"
              aria-checked={checked}
              className={`workbench-radio${checked ? " is-checked" : ""}`}
              onClick={() => dispatch({ type: "SET_CONFIG", patch: { quant_type: q } })}
              disabled={status === "running"}
              data-testid={`wb-quant-${q}`}
            >
              <span className="num">{q}</span>
            </button>
          );
        })}
        <span className="workbench-field-hint">{t("screens.workbench.quantize.typeHint")}</span>
      </fieldset>

      <StepNav onBack={onBack} onNext={onNext} onStart={onStart} status={status} showStart />
    </section>
  );
}

// ── Step 3 — LoRA ────────────────────────────────────────────────────

interface LoraStepProps extends StepCommonProps {
  config: WorkbenchConfig;
  dispatch: Dispatch<WBAction>;
}

function LoraStep({ config, dispatch, status, onStart, onBack, onNext }: LoraStepProps) {
  const { t } = useTranslation();
  return (
    <section className="workbench-step" role="region" aria-labelledby="wb-step-lora-title">
      <header className="workbench-step-header">
        <h3 id="wb-step-lora-title" className="workbench-step-title">
          {t("screens.workbench.lora.title")}
        </h3>
        <p className="workbench-step-subtitle">{t("screens.workbench.lora.subtitle")}</p>
      </header>

      <div className="workbench-form">
        <label className="workbench-field">
          <span className="workbench-field-label">{t("screens.workbench.config.epochs")}</span>
          <input
            type="number"
            min={1}
            max={20}
            value={config.lora_epochs}
            onChange={(e) =>
              dispatch({
                type: "SET_CONFIG",
                patch: { lora_epochs: Math.max(1, Number(e.target.value) || 1) },
              })
            }
            disabled={status === "running"}
            className="workbench-input num"
            data-testid="wb-input-epochs"
          />
          <span className="workbench-field-hint">{t("screens.workbench.config.epochsHint")}</span>
        </label>

        <button
          type="button"
          role="switch"
          aria-checked={config.korean_preset}
          className={`workbench-toggle${config.korean_preset ? " is-on" : ""}`}
          onClick={() =>
            dispatch({
              type: "SET_CONFIG",
              patch: { korean_preset: !config.korean_preset },
            })
          }
          disabled={status === "running"}
          data-testid="wb-toggle-korean-preset"
        >
          <span className="workbench-toggle-track" aria-hidden>
            <span className="workbench-toggle-thumb" />
          </span>
          <span className="workbench-toggle-text">
            {config.korean_preset
              ? t("screens.workbench.lora.presetOn")
              : t("screens.workbench.lora.presetOff")}
          </span>
        </button>
      </div>

      <StepNav onBack={onBack} onNext={onNext} onStart={onStart} status={status} showStart />
    </section>
  );
}

// ── Step 4 — Validate ────────────────────────────────────────────────

interface ValidateStepProps extends StepCommonProps {
  summary: WorkbenchRunSummary | null;
  evalReport: EvalReport | null;
}

function ValidateStep({
  status,
  summary,
  evalReport,
  onStart,
  onBack,
  onNext,
}: ValidateStepProps) {
  const { t } = useTranslation();
  // evalReport가 있으면 그쪽이 권위. 없으면 summary 기본값 fallback (idle/외부 진입).
  const passed = evalReport?.passed_count ?? summary?.eval_passed ?? null;
  const total = evalReport?.total ?? summary?.eval_total ?? null;
  const pct =
    passed !== null && total !== null && total > 0
      ? Math.round((passed * 100) / total)
      : null;
  const aggregateLabel =
    passed !== null && total !== null
      ? t("screens.workbench.validate.score", { pct, passed, total })
      : null;

  return (
    <section className="workbench-step" role="region" aria-labelledby="wb-step-validate-title">
      <header className="workbench-step-header">
        <h3 id="wb-step-validate-title" className="workbench-step-title">
          {t("screens.workbench.validate.title")}
        </h3>
        <p className="workbench-step-subtitle">{t("screens.workbench.validate.subtitle")}</p>
      </header>

      {aggregateLabel && (
        <div className="workbench-eval-score num" data-testid="wb-eval-score">
          {aggregateLabel}
        </div>
      )}

      {evalReport && (
        <ul
          className="workbench-eval-by-category num"
          aria-label={t("screens.workbench.validate.byCategoryLabel")}
          data-testid="wb-eval-by-category"
        >
          {Object.entries(evalReport.by_category).map(([category, [pass, tot]]) => (
            <li key={category} className="workbench-eval-category-row">
              <span className="workbench-eval-category-name">
                {t(categoryLabelKey(category))}
              </span>
              <span className="workbench-eval-category-score">
                {pass} / {tot}
              </span>
            </li>
          ))}
        </ul>
      )}

      {evalReport && (
        <ol
          className="workbench-eval-cases"
          aria-label={t("screens.workbench.validate.casesLabel")}
          data-testid="wb-eval-cases"
        >
          {evalReport.cases.map((c) => (
            <li key={c.case_id} className="workbench-eval-case-row" data-passed={c.passed}>
              <span
                className="workbench-eval-case-badge"
                data-passed={c.passed}
                aria-label={
                  c.passed
                    ? t("screens.workbench.validate.passed")
                    : t("screens.workbench.validate.failed")
                }
              >
                {c.passed
                  ? t("screens.workbench.validate.passed")
                  : t("screens.workbench.validate.failed")}
              </span>
              <span className="workbench-eval-case-id num">{c.case_id}</span>
              {!c.passed && c.failure_reason && (
                <span className="workbench-eval-case-reason">{c.failure_reason}</span>
              )}
            </li>
          ))}
        </ol>
      )}

      {!evalReport && (
        <ul className="workbench-eval-categories" aria-label={t("screens.workbench.validate.title")}>
          <li>{t("screens.workbench.validate.categoryFactuality")}</li>
          <li>{t("screens.workbench.validate.categoryInstruction")}</li>
          <li>{t("screens.workbench.validate.categoryToneKorean")}</li>
        </ul>
      )}

      <StepNav onBack={onBack} onNext={onNext} onStart={onStart} status={status} showStart={false} />
    </section>
  );
}

/** category 라벨 i18n key 매핑 — 신규 카테고리는 동일 키 prefix 사용. */
function categoryLabelKey(category: string): string {
  switch (category) {
    case "factuality":
      return "screens.workbench.validate.categoryFactuality";
    case "instruction-following":
      return "screens.workbench.validate.categoryInstruction";
    case "tone-korean":
      return "screens.workbench.validate.categoryToneKorean";
    default:
      return "screens.workbench.validate.categoryUnknown";
  }
}

// ── Step 5 — Register ────────────────────────────────────────────────

interface RegisterStepProps extends StepCommonProps {
  config: WorkbenchConfig;
  dispatch: Dispatch<WBAction>;
  summary: WorkbenchRunSummary | null;
  registeredModelId: string | null;
  ollamaOutputName: string | null;
  ollamaCreateLines: string[];
  onReset: () => void;
}

function RegisterStep({
  config,
  dispatch,
  status,
  summary,
  registeredModelId,
  ollamaOutputName,
  ollamaCreateLines,
  onStart,
  onBack,
  onReset,
}: RegisterStepProps) {
  const { t } = useTranslation();
  // catalog로 이동 — 단순 hash 라우터 패턴 (App.tsx에 동일 구조 다른 페이지가 사용 중).
  const goToCatalog = () => {
    if (typeof window !== "undefined") {
      window.location.hash = "#/catalog";
    }
  };
  return (
    <section className="workbench-step" role="region" aria-labelledby="wb-step-register-title">
      <header className="workbench-step-header">
        <h3 id="wb-step-register-title" className="workbench-step-title">
          {t("screens.workbench.register.title")}
        </h3>
        <p className="workbench-step-subtitle">{t("screens.workbench.register.subtitle")}</p>
      </header>

      <button
        type="button"
        role="switch"
        aria-checked={config.register_to_ollama}
        className={`workbench-toggle${config.register_to_ollama ? " is-on" : ""}`}
        onClick={() =>
          dispatch({
            type: "SET_CONFIG",
            patch: { register_to_ollama: !config.register_to_ollama },
          })
        }
        disabled={status === "running"}
        data-testid="wb-toggle-register"
      >
        <span className="workbench-toggle-track" aria-hidden>
          <span className="workbench-toggle-thumb" />
        </span>
        <span className="workbench-toggle-text">
          {t("screens.workbench.register.togglePullLabel")}
        </span>
      </button>

      {registeredModelId && (
        <div className="workbench-registered" data-testid="wb-registered-id">
          <span className="workbench-registered-label">
            {t("screens.workbench.register.registeredIdLabel")}
          </span>
          <code className="workbench-registered-id num">{registeredModelId}</code>
          <button
            type="button"
            className="workbench-button workbench-button-ghost"
            onClick={goToCatalog}
            data-testid="wb-go-catalog"
          >
            {t("screens.workbench.register.openCatalog")}
          </button>
        </div>
      )}

      {ollamaOutputName && (
        <div className="workbench-registered" data-testid="wb-ollama-output-name">
          <span className="workbench-registered-label">
            {t("screens.workbench.register.ollamaCreate.outputNameLabel")}
          </span>
          <code className="workbench-registered-id num">{ollamaOutputName}</code>
        </div>
      )}

      {ollamaCreateLines.length > 0 && (
        <div className="workbench-modelfile" data-testid="wb-ollama-create-log">
          <h4 className="workbench-preview-title">
            {t("screens.workbench.register.ollamaCreate.logTitle")}
          </h4>
          <pre className="workbench-modelfile-pre num">
            {ollamaCreateLines.join("\n")}
          </pre>
        </div>
      )}

      {summary?.modelfile_preview && (
        <div className="workbench-modelfile" data-testid="wb-modelfile-preview">
          <h4 className="workbench-preview-title">{t("screens.workbench.register.previewTitle")}</h4>
          <pre className="workbench-modelfile-pre num">{summary.modelfile_preview}</pre>
        </div>
      )}

      {summary && (
        <dl className="workbench-summary num" data-testid="wb-summary">
          <div>
            <dt>{t("screens.workbench.validate.title")}</dt>
            <dd>
              {summary.eval_passed} / {summary.eval_total}
            </dd>
          </div>
          <div>
            <dt>{t("screens.workbench.register.summaryDuration", { seconds: Math.round(summary.total_duration_ms / 1000) })}</dt>
            <dd>{Math.round(summary.total_duration_ms / 1000)}s</dd>
          </div>
          <div>
            <dt>{t("screens.workbench.register.summaryArtifacts", { count: summary.artifact_paths.length })}</dt>
            <dd>{summary.artifact_paths.length}</dd>
          </div>
        </dl>
      )}

      <StepNav
        onBack={onBack}
        onNext={status === "completed" ? onReset : undefined}
        onStart={onStart}
        status={status}
        showStart={!summary}
        nextLabelKey="screens.workbench.actions.newRun"
      />
    </section>
  );
}

// ── 공통 nav ────────────────────────────────────────────────────────

interface StepNavProps {
  onBack?: () => void;
  onNext?: () => void;
  onStart: () => void;
  status: RunStatus;
  showStart: boolean;
  nextLabelKey?: string;
}

function StepNav({ onBack, onNext, onStart, status, showStart, nextLabelKey }: StepNavProps) {
  const { t } = useTranslation();
  return (
    <div className="workbench-step-nav">
      {onBack && (
        <button
          type="button"
          className="workbench-button workbench-button-secondary"
          onClick={onBack}
          disabled={status === "running"}
        >
          {t("screens.workbench.actions.back")}
        </button>
      )}
      {showStart && status !== "running" && (
        <button
          type="button"
          className="workbench-button workbench-button-primary"
          onClick={onStart}
          data-testid="workbench-start"
        >
          {t("screens.workbench.actions.start")}
        </button>
      )}
      {onNext && (
        <button
          type="button"
          className="workbench-button workbench-button-ghost"
          onClick={onNext}
          disabled={status === "running"}
        >
          {t(nextLabelKey ?? "screens.workbench.actions.next")}
        </button>
      )}
    </div>
  );
}

// ── helpers ─────────────────────────────────────────────────────────

function extractErrorMessage(err: unknown, fallback: string): string {
  if (err && typeof err === "object") {
    // Tauri 직렬화된 API error — { kind, message } 또는 Error 인스턴스.
    const e = err as Record<string, unknown>;
    if (typeof e.message === "string" && e.message.length > 0) return e.message;
    if (typeof e.kind === "string") return `${e.kind}`;
  }
  if (err instanceof Error) return err.message || fallback;
  if (typeof err === "string") return err;
  return fallback;
}

// 빌드 시 미사용 경고 회피 — 외부에서 import할 수 있도록 유지.
export type { WorkbenchEvent, WorkbenchConfig, WorkbenchRunSummary, WorkbenchStep };
