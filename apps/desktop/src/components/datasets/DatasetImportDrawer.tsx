// DatasetImportDrawer — Phase 23'.c.2.d.4.
//
// 정책:
// - catalog-drawer-* 디자인 토큰 재사용 (모달 backdrop + 우측 슬라이드).
// - role="dialog" + aria-modal + aria-labelledby + Esc/배경 닫기.
// - Sample 3가지 preset radio (preview 1K / 권장 10K stratified / 전체).
// - NSFW (rp-explicit) 데이터셋만 minor_safety_attestation 체크박스 강제.
// - Channel<DatasetIngestEvent>로 5-stage 진행 + cancel.
// - 한국어 해요체 카피 (CLAUDE.md §4.1).

import { Channel } from "@tauri-apps/api/core";
import {
  Check,
  Database,
  Download,
  HardDrive,
  ListChecks,
  Loader2,
  Scissors,
  Sparkles,
  X,
} from "lucide-react";
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactElement,
} from "react";

import type { DatasetEntry } from "../../ipc/datasets";
import {
  cancelDatasetImport,
  defaultSampleStrategy,
  sampleStrategyLabel,
  startDatasetImport,
  type DatasetImportConfig,
  type DatasetImportSummary,
  type DatasetIngestEvent,
  type SampleStrategy,
} from "../../ipc/dataset-import";

import "./DatasetImportDrawer.css";

export interface DatasetImportDrawerProps {
  dataset: DatasetEntry | null;
  onClose: () => void;
  onCompleted?: (summary: DatasetImportSummary) => void;
}

type Step =
  | { kind: "config" }
  | { kind: "running"; importId: string; stage: string; message: string }
  | { kind: "done"; summary: DatasetImportSummary }
  | { kind: "failed"; error: string }
  | { kind: "cancelled" };

const SAMPLE_PRESETS: { id: string; value: SampleStrategy }[] = [
  { id: "preview", value: { kind: "first", n: 1_000 } },
  {
    id: "recommended",
    value: { kind: "stratified", n: 10_000, by: ["province", "occupation"] },
  },
  { id: "full", value: { kind: "full" } },
];

const STAGE_LABEL_KO = {
  manifest: "데이터셋 정보 받고 있어요",
  downloading: "Parquet 받고 있어요",
  chunking: "텍스트를 청크로 나누고 있어요",
  embedding: "임베딩 처리 중이에요",
  writing: "데이터베이스에 저장하고 있어요",
} as const;

/** Stepper 짧은 라벨 (해요체 단어). 한 단어로 끝내 stepper row가 좁게 유지됨. */
const STAGE_SHORT_LABEL_KO = {
  manifest: "정보 확인",
  downloading: "Parquet 받기",
  chunking: "청크 분할",
  embedding: "임베딩",
  writing: "저장",
} as const;

/** stepper 표시 순서 — backend stage emit 순서와 일치. */
const STEPPER_ORDER = [
  "manifest",
  "downloading",
  "chunking",
  "embedding",
  "writing",
] as const;

type StageKey = (typeof STEPPER_ORDER)[number];

const STAGE_ICON: Record<StageKey, ReactElement> = {
  manifest: <ListChecks size={14} aria-hidden="true" />,
  downloading: <Download size={14} aria-hidden="true" />,
  chunking: <Scissors size={14} aria-hidden="true" />,
  embedding: <Sparkles size={14} aria-hidden="true" />,
  writing: <HardDrive size={14} aria-hidden="true" />,
};

function isStageKey(s: string): s is StageKey {
  return (STEPPER_ORDER as readonly string[]).includes(s);
}

function DatasetStepper({ currentStage }: { currentStage: string }) {
  const curIdx = isStageKey(currentStage)
    ? STEPPER_ORDER.indexOf(currentStage)
    : -1;
  return (
    <ol className="dataset-stepper" aria-label="가져오기 진행 단계">
      {STEPPER_ORDER.map((stage, idx) => {
        const status =
          idx < curIdx ? "done" : idx === curIdx ? "current" : "pending";
        return (
          <li
            key={stage}
            className={`dataset-stepper-step dataset-stepper-step-${status}`}
            aria-current={status === "current" ? "step" : undefined}
          >
            <span className="dataset-stepper-icon">
              {status === "done" ? (
                <Check size={14} aria-hidden="true" />
              ) : status === "current" ? (
                <Loader2 size={14} aria-hidden="true" className="spin" />
              ) : (
                STAGE_ICON[stage]
              )}
            </span>
            <span className="dataset-stepper-label">
              {STAGE_SHORT_LABEL_KO[stage]}
            </span>
          </li>
        );
      })}
    </ol>
  );
}

export function DatasetImportDrawer({
  dataset,
  onClose,
  onCompleted,
}: DatasetImportDrawerProps) {
  const closeBtnRef = useRef<HTMLButtonElement>(null);
  const [step, setStep] = useState<Step>({ kind: "config" });
  const [sample, setSample] = useState<SampleStrategy>(
    defaultSampleStrategy(),
  );
  const [eulaAccepted, setEulaAccepted] = useState(false);
  const [noncommercialAccepted, setNoncommercialAccepted] = useState(false);

  const isNsfw = dataset?.content_warning === "rp-explicit";
  const isNoncommercial = dataset != null && !dataset.commercial;

  // Esc 닫기 (running 상태에서는 cancel 권장 — 닫지 않음).
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape" && step.kind !== "running") {
        onClose();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [step.kind, onClose]);

  // 첫 focus.
  useEffect(() => {
    closeBtnRef.current?.focus();
  }, [dataset]);

  // 카드 닫혔다 다시 열렸을 때 state 초기화.
  useEffect(() => {
    if (dataset) {
      setStep({ kind: "config" });
      setSample(defaultSampleStrategy());
      setEulaAccepted(false);
      setNoncommercialAccepted(false);
    }
  }, [dataset]);

  const handleStart = useCallback(async () => {
    if (!dataset) return;
    if (isNsfw && !eulaAccepted) {
      return;
    }
    if (isNoncommercial && !noncommercialAccepted) {
      return;
    }

    const channel = new Channel<DatasetIngestEvent>();
    channel.onmessage = (ev) => {
      switch (ev.kind) {
        case "started":
          setStep({
            kind: "running",
            importId: ev.import_id,
            stage: "manifest",
            message: STAGE_LABEL_KO.manifest,
          });
          break;
        case "manifest":
          setStep((prev) =>
            prev.kind === "running"
              ? {
                  ...prev,
                  stage: "manifest",
                  message: `${STAGE_LABEL_KO.manifest} (${ev.urls}개 shard)`,
                }
              : prev,
          );
          break;
        case "downloading":
          setStep((prev) =>
            prev.kind === "running"
              ? {
                  ...prev,
                  stage: "downloading",
                  message: `${STAGE_LABEL_KO.downloading} (${ev.urls_fetched}/${ev.urls_total})`,
                }
              : prev,
          );
          break;
        case "chunking":
          setStep((prev) =>
            prev.kind === "running"
              ? {
                  ...prev,
                  stage: "chunking",
                  message: `${STAGE_LABEL_KO.chunking} — ${ev.rows.toLocaleString()}행 / ${ev.chunks_generated.toLocaleString()}청크 생성`,
                }
              : prev,
          );
          break;
        case "embedding":
          setStep((prev) =>
            prev.kind === "running"
              ? {
                  ...prev,
                  stage: "embedding",
                  message: `${STAGE_LABEL_KO.embedding} — ${ev.chunks.toLocaleString()}청크 처리`,
                }
              : prev,
          );
          break;
        case "writing":
          setStep((prev) =>
            prev.kind === "running"
              ? {
                  ...prev,
                  stage: "writing",
                  message: `${STAGE_LABEL_KO.writing} — ${ev.inserted.toLocaleString()}청크 저장`,
                }
              : prev,
          );
          break;
        case "done":
          setStep({ kind: "done", summary: ev.summary });
          onCompleted?.(ev.summary);
          break;
        case "failed":
          setStep({ kind: "failed", error: ev.error });
          break;
        case "cancelled":
          setStep({ kind: "cancelled" });
          break;
      }
    };

    const repo = dataset.source.repo ?? "";
    const narrative = dataset.use_case.narrative_field ?? "persona";
    const config: DatasetImportConfig = {
      repo,
      config: "default",
      split: "train",
      license: dataset.license,
      minorSafetyAttestation: isNsfw ? eulaAccepted : true,
      sample,
      textColumns: [narrative],
    };

    try {
      const importId = await startDatasetImport(config, channel);
      setStep({
        kind: "running",
        importId,
        stage: "manifest",
        message: STAGE_LABEL_KO.manifest,
      });
    } catch (e) {
      setStep({ kind: "failed", error: String(e) });
    }
  }, [
    dataset,
    sample,
    isNsfw,
    eulaAccepted,
    isNoncommercial,
    noncommercialAccepted,
    onCompleted,
  ]);

  const handleCancel = useCallback(async () => {
    if (step.kind === "running") {
      await cancelDatasetImport(step.importId).catch(() => {
        /* idempotent — backend가 알림 처리 */
      });
    }
  }, [step]);

  if (!dataset) return null;

  const startDisabled =
    (isNsfw && !eulaAccepted) || (isNoncommercial && !noncommercialAccepted);

  return (
    <div
      className="catalog-drawer-backdrop"
      role="presentation"
      onClick={step.kind === "running" ? undefined : onClose}
    >
      <aside
        className="catalog-drawer"
        role="dialog"
        aria-modal="true"
        aria-labelledby="dataset-import-drawer-title"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="catalog-drawer-header">
          <h3
            id="dataset-import-drawer-title"
            className="catalog-drawer-title"
          >
            <Database
              size={16}
              aria-hidden="true"
              style={{ marginRight: "var(--space-2)" }}
            />
            {dataset.display_name}
          </h3>
          <button
            ref={closeBtnRef}
            type="button"
            className="catalog-drawer-close"
            aria-label="닫기"
            onClick={onClose}
            disabled={step.kind === "running"}
          >
            <X size={16} aria-hidden="true" />
          </button>
        </header>

        <div className="catalog-drawer-body">
          {step.kind === "config" && (
            <>
              <p className="catalog-drawer-hint">
                {dataset.curator_note_ko ?? ""}
              </p>

              <section aria-labelledby="dataset-import-license-heading">
                <h4 id="dataset-import-license-heading">라이선스</h4>
                <p>
                  {dataset.license}
                  {isNoncommercial && " (비상업 전용)"}
                </p>
              </section>

              {isNoncommercial && (
                <section aria-labelledby="dataset-import-noncommercial-heading">
                  <h4 id="dataset-import-noncommercial-heading">
                    비상업 라이선스 동의
                  </h4>
                  <p className="catalog-drawer-hint">
                    이 데이터셋은 비상업 라이선스(CC-BY-NC 등)에요. 개인 학습 /
                    연구 / 비영리 목적으로만 사용할 수 있고, 매출이 발생하는
                    상업 환경에서는 별도 라이선스 협의가 필요해요.
                  </p>
                  <label
                    style={{
                      display: "flex",
                      alignItems: "flex-start",
                      gap: "var(--space-2)",
                      padding: "var(--space-2) 0",
                    }}
                  >
                    <input
                      type="checkbox"
                      checked={noncommercialAccepted}
                      onChange={(e) =>
                        setNoncommercialAccepted(e.target.checked)
                      }
                    />
                    <span>
                      이 데이터셋과 그 파생물을 비상업 용도로만 사용할게요.
                    </span>
                  </label>
                </section>
              )}

              <section
                aria-labelledby="dataset-import-sample-heading"
                role="radiogroup"
                aria-describedby="dataset-import-sample-desc"
              >
                <h4 id="dataset-import-sample-heading">샘플 크기</h4>
                <p
                  id="dataset-import-sample-desc"
                  className="catalog-drawer-hint"
                >
                  처음에는 작게 시도해 보세요. 만족하면 더 크게 다시 가져올 수
                  있어요.
                </p>
                {SAMPLE_PRESETS.map((preset) => {
                  const checked =
                    JSON.stringify(preset.value) === JSON.stringify(sample);
                  return (
                    <label
                      key={preset.id}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: "var(--space-2)",
                        padding: "var(--space-2) 0",
                      }}
                    >
                      <input
                        type="radio"
                        name="dataset-import-sample"
                        value={preset.id}
                        checked={checked}
                        onChange={() => setSample(preset.value)}
                      />
                      <span>{sampleStrategyLabel(preset.value)}</span>
                    </label>
                  );
                })}
              </section>

              {isNsfw && (
                <section aria-labelledby="dataset-import-eula-heading">
                  <h4 id="dataset-import-eula-heading">미성년 보호 동의</h4>
                  <p className="catalog-drawer-hint">
                    이 데이터셋은 성인 콘텐츠를 포함해요. 미성년 보호 키워드
                    스캔과 라이선스 화이트리스트는 통과했지만, 사용자 본인이
                    18세 이상이고 책임 있는 용도임을 확인해 주세요.
                  </p>
                  <label
                    style={{
                      display: "flex",
                      alignItems: "flex-start",
                      gap: "var(--space-2)",
                      padding: "var(--space-2) 0",
                    }}
                  >
                    <input
                      type="checkbox"
                      checked={eulaAccepted}
                      onChange={(e) => setEulaAccepted(e.target.checked)}
                    />
                    <span>
                      18세 이상이며 콘텐츠 정책을 확인했어요. 미성년 관련
                      자료가 아님을 인지하고 진행할게요.
                    </span>
                  </label>
                </section>
              )}

              <button
                type="button"
                className="catalog-drawer-install"
                onClick={handleStart}
                disabled={startDisabled}
              >
                이 데이터셋 가져올게요
              </button>
            </>
          )}

          {step.kind === "running" && (
            <>
              <DatasetStepper currentStage={step.stage} />
              <p className="catalog-drawer-hint">{step.message}</p>
              <button
                type="button"
                className="catalog-drawer-install"
                onClick={handleCancel}
              >
                취소할래요
              </button>
            </>
          )}

          {step.kind === "done" && (
            <>
              <p>가져오기 완료했어요.</p>
              <ul>
                <li>처리한 행: {step.summary.rowsProcessed.toLocaleString()}</li>
                <li>
                  생성한 청크: {step.summary.chunksGenerated.toLocaleString()}
                </li>
                <li>
                  임베딩 완료: {step.summary.chunksEmbedded.toLocaleString()}
                </li>
                <li>
                  데이터베이스 저장:{" "}
                  {step.summary.chunksInserted.toLocaleString()}
                </li>
              </ul>
              <button
                type="button"
                className="catalog-drawer-install"
                onClick={onClose}
              >
                닫기
              </button>
            </>
          )}

          {step.kind === "failed" && (
            <>
              <p>가져오기 실패했어요.</p>
              <p className="catalog-drawer-hint">{step.error}</p>
              <button
                type="button"
                className="catalog-drawer-install"
                onClick={onClose}
              >
                닫기
              </button>
            </>
          )}

          {step.kind === "cancelled" && (
            <>
              <p>사용자가 취소했어요. 다음에 다시 시도해 주세요.</p>
              <button
                type="button"
                className="catalog-drawer-install"
                onClick={onClose}
              >
                닫기
              </button>
            </>
          )}
        </div>
      </aside>
    </div>
  );
}
