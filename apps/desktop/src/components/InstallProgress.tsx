// InstallProgress — 재사용 가능한 설치 진행률 패널.
// Phase 1' 추출: Step3Install의 InstallRunningPanel을 props 기반으로 일반화.
//
// 정책:
// - 250ms debounce로 speed/ETA jitter 최소화.
// - reduced-motion은 design-system tokens.css가 처리.
// - 한국어 phase 라벨은 i18n key로 매핑 (caller에서 t() 적용).

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import type {
  DownloadEvent,
  InstallEvent,
} from "../ipc/install-events";

export type InstallPhase =
  | "starting"
  | "download"
  | "extract"
  | "post-check"
  | "finished";

export interface InstallProgressData {
  /** 가장 최근 InstallEvent (phase 추출용). */
  latest?: InstallEvent;
  /** 다운로드 단계 진행률 — Download.Progress가 emit하면 갱신. */
  progress?: {
    downloaded: number;
    total: number | null;
    speed_bps: number;
  };
  /** 마지막 10건 이벤트 로그 — 자세히 보기 details. */
  log?: InstallEvent[];
  /** Download.Retrying의 attempt — 표시 시 "(N차 시도)". */
  retryAttempt?: number;
}

export interface InstallProgressProps {
  /** 설치 대상 표시 이름 (예: "Ollama", "EXAONE 1.2B"). */
  title: string;
  data: InstallProgressData;
  /** 취소 버튼 — 누락 시 버튼 미표시 (모달 안에서 사용 시 등). */
  onCancel?: () => void;
  /**
   * 컴팩트 모드 — Phase 4.b 메인 화면 inline 모드.
   * 기본값(false)은 마법사 전체 화면 모드. true면 단일 라인 progress + 취소 버튼만 노출하고
   * 헤더 / 상세 로그를 간소화한다. 디자인 토큰 기반 — 시각 변경은 CSS의 `.is-compact` 변종이 담당.
   */
  compact?: boolean;
}

export function InstallProgress({
  title,
  data,
  onCancel,
  compact = false,
}: InstallProgressProps) {
  const { t } = useTranslation();
  const phase = phaseOf(data.latest);
  const phaseText = phaseLabel(phase, t);

  if (compact) {
    return (
      <div
        className="onb-install-compact"
        aria-labelledby="install-progress-title"
        aria-live="polite"
      >
        <div className="onb-install-compact-row">
          <span
            id="install-progress-title"
            className="onb-install-compact-title"
          >
            {t("onboarding.install.running.title", { name: title })}
          </span>
          <span
            className="onb-install-compact-phase onb-install-phase"
            data-phase={phase}
          >
            {phaseText}
            {data.retryAttempt != null && (
              <span className="onb-install-retry">
                {" "}
                {t("onboarding.install.retrySuffix", {
                  attempt: data.retryAttempt,
                })}
              </span>
            )}
          </span>
          {onCancel && (
            <button
              type="button"
              className="onb-button onb-button-secondary onb-install-compact-cancel"
              onClick={onCancel}
            >
              {t("onboarding.install.cancel")}
            </button>
          )}
        </div>
        <ProgressBar progress={data.progress} compact />
      </div>
    );
  }

  return (
    <div className="onb-step" aria-labelledby="install-progress-title">
      <header className="onb-step-header">
        <h1 id="install-progress-title" className="onb-step-title">
          {t("onboarding.install.running.title", { name: title })}
        </h1>
        <p
          className="onb-step-subtitle onb-install-phase"
          data-phase={phase}
          aria-live="polite"
        >
          {phaseText}
          {data.retryAttempt != null && (
            <span className="onb-install-retry">
              {" "}
              {t("onboarding.install.retrySuffix", { attempt: data.retryAttempt })}
            </span>
          )}
        </p>
      </header>

      <ProgressBar progress={data.progress} />

      <details className="onb-install-log">
        <summary>{t("onboarding.install.detailsLabel")}</summary>
        <ul>
          {(!data.log || data.log.length === 0) && (
            <li>{t("onboarding.install.noLogYet")}</li>
          )}
          {data.log?.map((e, i) => (
            <li key={i} className="num">
              {describeEvent(e)}
            </li>
          ))}
        </ul>
      </details>

      {onCancel && (
        <div className="onb-step-actions">
          <button
            type="button"
            className="onb-button onb-button-secondary"
            onClick={onCancel}
          >
            {t("onboarding.install.cancel")}
          </button>
        </div>
      )}
    </div>
  );
}

// ── ProgressBar (debounced) ────────────────────────────────────────

function ProgressBar({
  progress,
  compact = false,
}: {
  progress?: { downloaded: number; total: number | null; speed_bps: number };
  compact?: boolean;
}) {
  const { t } = useTranslation();
  const [smoothed, setSmoothed] = useState(progress);
  const lastUpdateRef = useRef(0);

  useEffect(() => {
    const now = Date.now();
    if (now - lastUpdateRef.current < 250 && progress) {
      const handle = setTimeout(() => {
        setSmoothed(progress);
        lastUpdateRef.current = Date.now();
      }, 250 - (now - lastUpdateRef.current));
      return () => clearTimeout(handle);
    }
    setSmoothed(progress);
    lastUpdateRef.current = now;
    return undefined;
  }, [progress]);

  const downloaded = smoothed?.downloaded ?? 0;
  const total = smoothed?.total ?? null;
  const speed = smoothed?.speed_bps ?? 0;
  const ratio =
    total != null && total > 0 ? Math.min(1, downloaded / total) : null;
  const etaSec =
    total != null && total > 0 && speed > 0
      ? Math.max(0, Math.round((total - downloaded) / speed))
      : null;

  const etaText = (() => {
    if (etaSec == null) return t("onboarding.install.etaPending");
    if (etaSec >= 60) {
      return t("onboarding.install.etaMinutes", {
        minutes: Math.floor(etaSec / 60),
        seconds: etaSec % 60,
      });
    }
    return t("onboarding.install.etaSeconds", { seconds: etaSec });
  })();

  return (
    <div
      className={`onb-install-progress${compact ? " is-compact" : ""}`}
    >
      <progress
        className="onb-install-bar"
        value={ratio == null ? undefined : ratio}
        max={ratio == null ? undefined : 1}
        aria-label={t("onboarding.install.progressAria") ?? undefined}
      />
      <div className="onb-install-meta num">
        <span>{ratio != null ? `${Math.round(ratio * 100)}%` : "—"}</span>
        <span>
          {speed > 0 ? formatSpeed(speed) : t("onboarding.install.speedPending")}
        </span>
        <span>{etaText}</span>
      </div>
    </div>
  );
}

// ── 헬퍼 ───────────────────────────────────────────────────────────

export function phaseOf(ev?: InstallEvent): InstallPhase {
  if (!ev) return "starting";
  switch (ev.kind) {
    case "started":
      return "starting";
    case "download":
      return "download";
    case "extract":
      return "extract";
    case "post-check":
      return "post-check";
    case "finished":
    case "failed":
    case "cancelled":
      return "finished";
  }
}

export function phaseLabel(
  phase: InstallPhase,
  t: (key: string) => string,
): string {
  switch (phase) {
    case "starting":
      return t("onboarding.install.phase.starting");
    case "download":
      return t("onboarding.install.phase.download");
    case "extract":
      return t("onboarding.install.phase.extract");
    case "post-check":
      return t("onboarding.install.phase.postCheck");
    case "finished":
      return t("onboarding.install.phase.finished");
  }
}

export function formatSpeed(bps: number): string {
  if (bps >= 1024 * 1024) return `${(bps / (1024 * 1024)).toFixed(1)} MB/s`;
  if (bps >= 1024) return `${(bps / 1024).toFixed(0)} KB/s`;
  return `${bps} B/s`;
}

export function describeEvent(ev: InstallEvent): string {
  switch (ev.kind) {
    case "started":
      return `[start] ${ev.id} · ${ev.method}`;
    case "download": {
      const inner: DownloadEvent = ev.download;
      switch (inner.kind) {
        case "started":
          return `[download.start] ${inner.url} (resume_from=${inner.resume_from})`;
        case "progress":
          return `[download.progress] ${inner.downloaded}/${inner.total ?? "?"} · ${inner.speed_bps}B/s`;
        case "verified":
          return `[download.verified] sha256=${inner.sha256_hex.slice(0, 12)}…`;
        case "finished":
          return `[download.finished] ${inner.bytes}B`;
        case "retrying":
          return `[download.retrying] attempt=${inner.attempt} delay=${inner.delay_ms}ms reason=${inner.reason}`;
      }
      break;
    }
    case "extract":
      return `[extract.${ev.phase}] entries=${ev.entries} bytes=${ev.total_bytes}`;
    case "post-check":
      return `[post-check] ${ev.status}`;
    case "finished":
      return `[finished] ${ev.outcome.kind}`;
    case "failed":
      return `[failed] ${ev.code}: ${ev.message}`;
    case "cancelled":
      return `[cancelled]`;
  }
  return "";
}
