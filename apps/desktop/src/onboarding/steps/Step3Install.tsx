// Step 3 — 첫 런타임 설치. Phase 1A.4.c.
//
// 분기 (install 서브상태):
//   decide  → always 가드 후 즉시 skip 또는 idle로. UI 없음.
//   skip    → "이미 사용 중이에요. 다음 단계로 갈게요" + 1.2s 후 자동 done.
//   idle    → 카드 그리드 (Ollama / LM Studio) + 사용자 클릭 대기.
//   running → InstallProgress + 취소 + 자세히 보기.
//   failed  → 에러 카드 + RETRY (500ms debounce) + SKIP.
//
// 모델 큐레이션은 Phase 2'로 분리 — 이 단계는 런타임만.

import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  useOnboardingEnv,
  useOnboardingInstallError,
  useOnboardingInstallLatest,
  useOnboardingInstallLog,
  useOnboardingInstallOutcome,
  useOnboardingInstallProgress,
  useOnboardingInstallSub,
  useOnboardingModelId,
  useOnboardingRetryAttempt,
  useOnboardingSend,
} from "../context";
import type { ActionOutcome, InstallEvent } from "../../ipc/install-events";
import { runtimeKindLabel, type RuntimeKind } from "../../ipc/environment";
import { SpotlightCard } from "../../components/SpotlightCard";

const RAM_WARN_BYTES = 8 * 1024 * 1024 * 1024;

// ── Step 3 entry ─────────────────────────────────────────────────────

export function Step3Install() {
  const sub = useOnboardingInstallSub();
  const outcome = useOnboardingInstallOutcome();

  switch (sub) {
    case "skip":
      return <SkipBanner />;
    case "running":
      return <InstallRunningPanel />;
    case "failed":
      return <InstallFailedPanel />;
    case "openedUrl":
      // 1A.4.d.1 Issue A — install actor가 OpenedUrl outcome 반환 시 manual NEXT 대기.
      return outcome ? <OpenedUrlPanel outcome={outcome} /> : <CardGrid />;
    case "decide":
    case "idle":
    default:
      return <CardGrid />;
  }
}

// ── 카드 그리드 (idle) ──────────────────────────────────────────────

function CardGrid() {
  const { t } = useTranslation();
  const env = useOnboardingEnv();
  const send = useOnboardingSend();

  const ramLow =
    env != null && env.hardware.mem.total_bytes < RAM_WARN_BYTES;

  const status = useMemo(() => runtimeStatusMap(env?.runtimes ?? []), [env]);

  return (
    <section className="onb-step" aria-labelledby="onb-step3-title">
      <header className="onb-step-header">
        <h1 id="onb-step3-title" className="onb-step-title">
          {t("onboarding.install.title")}
        </h1>
        <p className="onb-step-subtitle">
          {t("onboarding.install.subtitle")}
        </p>
      </header>

      <div className="onb-runtime-grid">
        <RuntimeCard
          id="ollama"
          pillKind="recommended"
          title="Ollama"
          reason={t("onboarding.install.cards.ollama.reason")}
          metaLicense={t("onboarding.install.cards.ollama.license")}
          metaSize={t("onboarding.install.cards.ollama.size")}
          status={status.ollama}
          ramLow={ramLow}
          onSelect={() => send({ type: "SELECT_MODEL", id: "ollama" })}
        />
        <RuntimeCard
          id="lm-studio"
          pillKind="eula"
          title="LM Studio"
          reason={t("onboarding.install.cards.lmStudio.reason")}
          metaLicense={t("onboarding.install.cards.lmStudio.license")}
          metaSize={t("onboarding.install.cards.lmStudio.size")}
          status={status.lmStudio}
          ramLow={false}
          onSelect={() => send({ type: "SELECT_MODEL", id: "lm-studio" })}
        />
      </div>

      <div className="onb-step-actions">
        <button
          type="button"
          className="onb-button onb-button-secondary"
          onClick={() => send({ type: "BACK" })}
        >
          {t("onboarding.actions.back")}
        </button>
        <button
          type="button"
          className="onb-button onb-button-ghost"
          onClick={() => send({ type: "SKIP" })}
        >
          {t("onboarding.actions.skip")}
        </button>
      </div>
    </section>
  );
}

type RuntimeStatusKind = "running" | "installed" | "not-installed" | "unknown";

function runtimeStatusMap(
  runtimes: Array<{ runtime: RuntimeKind; status: string }>,
): { ollama: RuntimeStatusKind; lmStudio: RuntimeStatusKind } {
  const find = (k: RuntimeKind): RuntimeStatusKind => {
    const r = runtimes.find((x) => x.runtime === k);
    if (!r) return "unknown";
    if (r.status === "running" || r.status === "installed" || r.status === "not-installed") {
      return r.status;
    }
    return "unknown";
  };
  return { ollama: find("ollama"), lmStudio: find("lm-studio") };
}

// ── 단일 카드 ────────────────────────────────────────────────────────

function RuntimeCard({
  pillKind,
  title,
  reason,
  metaLicense,
  metaSize,
  status,
  ramLow,
  onSelect,
}: {
  id: string;
  pillKind: "recommended" | "eula";
  title: string;
  reason: string;
  metaLicense: string;
  metaSize: string;
  status: RuntimeStatusKind;
  ramLow: boolean;
  onSelect: () => void;
}) {
  const { t } = useTranslation();
  const isRunning = status === "running";
  const cardData =
    isRunning ? "running" : status === "installed" ? "installed" : "default";

  return (
    <SpotlightCard className="onb-runtime-card" data-status={cardData}>
      <header className="onb-runtime-card-header">
        <span className="onb-runtime-card-title">{title}</span>
        <span className="onb-runtime-card-pill" data-kind={pillKind}>
          {pillKind === "recommended"
            ? t("onboarding.install.pill.recommended")
            : t("onboarding.install.pill.eula")}
        </span>
      </header>
      <p className="onb-runtime-card-reason">{reason}</p>
      <dl className="onb-runtime-card-meta num">
        <div>
          <dt>{t("onboarding.install.meta.license")}</dt>
          <dd>{metaLicense}</dd>
        </div>
        <div>
          <dt>{t("onboarding.install.meta.size")}</dt>
          <dd>{metaSize}</dd>
        </div>
      </dl>
      {isRunning && (
        <p className="onb-runtime-card-hint">
          {t("onboarding.install.alreadyRunning")}
        </p>
      )}
      {!isRunning && ramLow && (
        <p className="onb-runtime-card-hint">
          {t("onboarding.install.ramLowHint")}
        </p>
      )}
      <button
        type="button"
        className="onb-button onb-button-primary onb-runtime-card-cta"
        onClick={onSelect}
        disabled={isRunning}
      >
        {isRunning
          ? t("onboarding.install.cta.alreadyOk")
          : t("onboarding.install.cta.install")}
      </button>
    </SpotlightCard>
  );
}

// ── Skip banner ────────────────────────────────────────────────────

function SkipBanner() {
  const { t } = useTranslation();
  return (
    <section className="onb-step" aria-labelledby="onb-step3-title">
      <header className="onb-step-header">
        <h1 id="onb-step3-title" className="onb-step-title">
          {t("onboarding.install.skip.title")}
        </h1>
        <p className="onb-step-subtitle">{t("onboarding.install.skip.body")}</p>
      </header>
      <div
        className="onb-placeholder onb-skip-banner"
        role="status"
        aria-live="polite"
      >
        <span className="onb-placeholder-icon" aria-hidden>
          ✓
        </span>
        <span>{t("onboarding.install.skip.body")}</span>
      </div>
    </section>
  );
}

// ── Running 패널 ───────────────────────────────────────────────────

function InstallRunningPanel() {
  const { t } = useTranslation();
  const send = useOnboardingSend();
  const modelId = useOnboardingModelId();
  const latest = useOnboardingInstallLatest();
  const progress = useOnboardingInstallProgress();
  const log = useOnboardingInstallLog();
  const retryAttempt = useOnboardingRetryAttempt();
  const phase = phaseOf(latest);

  const title = useMemo(() => {
    const id = (modelId ?? "") as RuntimeKind;
    if (id === "ollama" || id === "lm-studio") return runtimeKindLabel(id);
    return modelId ?? "";
  }, [modelId]);

  const phaseText = (() => {
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
  })();

  return (
    <section className="onb-step" aria-labelledby="onb-step3-title">
      <header className="onb-step-header">
        <h1 id="onb-step3-title" className="onb-step-title">
          {t("onboarding.install.running.title", { name: title })}
        </h1>
        <p
          className="onb-step-subtitle onb-install-phase"
          data-phase={phase}
          aria-live="polite"
        >
          {phaseText}
          {retryAttempt != null && (
            <span className="onb-install-retry">
              {" "}
              {t("onboarding.install.retrySuffix", { attempt: retryAttempt })}
            </span>
          )}
        </p>
      </header>

      <ProgressBar progress={progress} />

      <details className="onb-install-log">
        <summary>{t("onboarding.install.detailsLabel")}</summary>
        <ul>
          {log.length === 0 && <li>{t("onboarding.install.noLogYet")}</li>}
          {log.map((e, i) => (
            <li key={i} className="num">
              {describeEvent(e)}
            </li>
          ))}
        </ul>
      </details>

      <div className="onb-step-actions">
        <button
          type="button"
          className="onb-button onb-button-secondary"
          onClick={() => send({ type: "BACK" })}
        >
          {t("onboarding.install.cancel")}
        </button>
      </div>
    </section>
  );
}

function ProgressBar({
  progress,
}: {
  progress?: { downloaded: number; total: number | null; speed_bps: number };
}) {
  const { t } = useTranslation();
  // 250ms debounce — speed/ETA jitter 회피.
  const [smoothed, setSmoothed] = useState(progress);
  const lastUpdateRef = useRef(0);
  useEffect(() => {
    const now = Date.now();
    if (now - lastUpdateRef.current < 250 && progress) {
      const t = setTimeout(() => {
        setSmoothed(progress);
        lastUpdateRef.current = Date.now();
      }, 250 - (now - lastUpdateRef.current));
      return () => clearTimeout(t);
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
    <div className="onb-install-progress">
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

// ── Failed 패널 ────────────────────────────────────────────────────

function InstallFailedPanel() {
  const { t } = useTranslation();
  const send = useOnboardingSend();
  const error = useOnboardingInstallError();
  const [retryDisabled, setRetryDisabled] = useState(false);

  // 1A.4.d.1 Issue A 후속 — OpenedUrl outcome은 별도 substate에서 처리. failed에 도달했다면 진짜 실패.
  const code = error?.code ?? "default";
  const i18nKey = `onboarding.install.error.${code}`;
  const fallbackKey = "onboarding.install.error.default";

  const handleRetry = () => {
    if (retryDisabled) return;
    setRetryDisabled(true);
    send({ type: "RETRY" });
    // 500ms 후 활성 — Rust 측 InstallRegistry.finish 완료 시간 확보.
    setTimeout(() => setRetryDisabled(false), 500);
  };

  return (
    <section className="onb-step" aria-labelledby="onb-step3-title">
      <header className="onb-step-header">
        <h1 id="onb-step3-title" className="onb-step-title">
          {t("onboarding.install.failed.title")}
        </h1>
      </header>
      <div className="onb-error" role="alert">
        <p className="onb-error-body">
          {t(i18nKey, { defaultValue: t(fallbackKey) })}
        </p>
        {error?.message && (
          <pre className="onb-error-detail">{error.message}</pre>
        )}
        <div className="onb-error-actions">
          <button
            type="button"
            className="onb-button onb-button-secondary"
            onClick={() => send({ type: "BACK" })}
          >
            {t("onboarding.actions.back")}
          </button>
          <button
            type="button"
            className="onb-button onb-button-ghost"
            onClick={() => send({ type: "SKIP" })}
          >
            {t("onboarding.actions.skip")}
          </button>
          <button
            type="button"
            className="onb-button onb-button-primary"
            onClick={handleRetry}
            disabled={retryDisabled}
          >
            {t("onboarding.error.retry")}
          </button>
        </div>
      </div>
    </section>
  );
}

// ── OpenedUrl outcome 안내 (LM Studio 등) ──────────────────────────

function OpenedUrlPanel({ outcome }: { outcome: ActionOutcome }) {
  const { t } = useTranslation();
  const send = useOnboardingSend();
  return (
    <section className="onb-step" aria-labelledby="onb-step3-title">
      <header className="onb-step-header">
        <h1 id="onb-step3-title" className="onb-step-title">
          {t("onboarding.install.openedUrl.title")}
        </h1>
        <p className="onb-step-subtitle">
          {t("onboarding.install.openedUrl.body")}
        </p>
      </header>
      <p className="onb-error-detail num">
        {outcome.kind === "opened-url" ? outcome.url : ""}
      </p>
      <div className="onb-step-actions">
        <button
          type="button"
          className="onb-button onb-button-secondary"
          onClick={() => send({ type: "BACK" })}
        >
          {t("onboarding.actions.back")}
        </button>
        <button
          type="button"
          className="onb-button onb-button-primary"
          onClick={() => send({ type: "NEXT" })}
        >
          {t("onboarding.actions.next")}
        </button>
      </div>
    </section>
  );
}

// ── helpers ────────────────────────────────────────────────────────

type Phase = "starting" | "download" | "extract" | "post-check" | "finished";

function phaseOf(ev?: InstallEvent): Phase {
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
      return "finished";
    case "failed":
    case "cancelled":
      return "finished";
  }
}

function formatSpeed(bps: number): string {
  if (bps >= 1024 * 1024) return `${(bps / (1024 * 1024)).toFixed(1)} MB/s`;
  if (bps >= 1024) return `${(bps / 1024).toFixed(0)} KB/s`;
  return `${bps} B/s`;
}

function describeEvent(ev: InstallEvent): string {
  switch (ev.kind) {
    case "started":
      return `[start] ${ev.id} · ${ev.method}`;
    case "download": {
      const inner = ev.download;
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
