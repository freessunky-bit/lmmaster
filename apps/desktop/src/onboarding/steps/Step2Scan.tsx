// Step 2 — 환경 점검. Phase 1A.4.b.
//
// 머신 entry 시 자동으로 detect_environment 호출 (fromPromise actor).
// substate별 UI:
//   running → skeleton 카드 + "잠시만 기다려 주세요"
//   done    → 4 카드 (OS / 메모리 / GPU / 런타임) + "계속할게요" 활성
//   failed  → 에러 카드 + RETRY 버튼

import { useTranslation } from "react-i18next";

import {
  useOnboardingEnv,
  useOnboardingScanError,
  useOnboardingScanSub,
  useOnboardingSend,
} from "../context";
import {
  formatGiB,
  osFamilyLabel,
  runtimeKindLabel,
  type DetectResult,
  type EnvironmentReport,
  type GpuInfo,
} from "../../ipc/environment";

const RAM_WARN_BYTES = 8 * 1024 * 1024 * 1024; // 8GB 미만 경고
const DISK_WARN_BYTES = 20 * 1024 * 1024 * 1024; // 가용 20GB 미만 경고

export function Step2Scan() {
  const { t } = useTranslation();
  const sub = useOnboardingScanSub();
  const env = useOnboardingEnv();
  const scanError = useOnboardingScanError();
  const send = useOnboardingSend();

  const isRunning = sub === "running" || sub === "idle";
  const isDone = sub === "done" && env;
  const isFailed = sub === "failed";

  return (
    <section className="onb-step" aria-labelledby="onb-step2-title">
      <header className="onb-step-header">
        <h1 id="onb-step2-title" className="onb-step-title">
          {t("onboarding.scan.title")}
        </h1>
        <p className="onb-step-subtitle">
          {isRunning && t("onboarding.scan.subtitle.running")}
          {isDone && t("onboarding.scan.subtitle.done")}
          {isFailed && t("onboarding.scan.subtitle.failed")}
        </p>
      </header>

      {isRunning && <ScanSkeleton />}
      {isDone && env && <ScanResult env={env} />}
      {isFailed && (
        <ScanFailure
          message={scanError ?? "unknown error"}
          onRetry={() => send({ type: "RETRY" })}
        />
      )}

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
          disabled={!isDone}
        >
          {t("onboarding.actions.next")}
        </button>
      </div>
    </section>
  );
}

// ── Skeleton (running) ──────────────────────────────────────────────────

function ScanSkeleton() {
  const { t } = useTranslation();
  return (
    <div className="onb-scan-cards" aria-busy="true" aria-live="polite">
      {([0, 1, 2, 3] as const).map((i) => (
        <div className="onb-scan-card onb-skeleton" key={i}>
          <div className="onb-skeleton-bar onb-skeleton-bar-sm" />
          <div className="onb-skeleton-bar onb-skeleton-bar-md" />
        </div>
      ))}
      <p className="onb-scan-caption num">
        {t("onboarding.scan.captionRunning")}
      </p>
    </div>
  );
}

// ── Result (done) ───────────────────────────────────────────────────────

function ScanResult({ env }: { env: EnvironmentReport }) {
  const { t } = useTranslation();
  const { hardware, runtimes } = env;
  const ramWarn = hardware.mem.total_bytes < RAM_WARN_BYTES;
  const diskWarn =
    hardware.disks.length > 0 &&
    Math.min(...hardware.disks.map((d) => d.available_bytes)) < DISK_WARN_BYTES;

  const primaryGpu = pickPrimaryGpu(hardware.gpus);

  return (
    <div className="onb-scan-cards" aria-live="polite">
      <ScanCard
        title={t("onboarding.scan.card.os")}
        status="ok"
        statusLabel={t("onboarding.scan.status.ok")}
        body={`${osFamilyLabel(hardware.os.family)} ${hardware.os.version} · ${hardware.os.arch}`}
      />

      <ScanCard
        title={t("onboarding.scan.card.memory")}
        status={ramWarn ? "warn" : "ok"}
        statusLabel={
          ramWarn
            ? t("onboarding.scan.status.warn")
            : t("onboarding.scan.status.ok")
        }
        body={t("onboarding.scan.body.memory", {
          available: formatGiB(hardware.mem.available_bytes),
          total: formatGiB(hardware.mem.total_bytes),
        })}
        hint={ramWarn ? t("onboarding.scan.hint.lowRam") : undefined}
      />

      <ScanCard
        title={t("onboarding.scan.card.gpu")}
        status={primaryGpu ? "ok" : "muted"}
        statusLabel={
          primaryGpu
            ? gpuVendorLabel(primaryGpu.vendor)
            : t("onboarding.scan.status.cpuOnly")
        }
        body={
          primaryGpu
            ? formatGpuBody(primaryGpu)
            : t("onboarding.scan.body.noGpu")
        }
        hint={
          diskWarn ? t("onboarding.scan.hint.lowDisk") : undefined
        }
      />

      <ScanCard
        title={t("onboarding.scan.card.runtimes")}
        status={runtimes.some((r) => r.status === "running") ? "ok" : "muted"}
        statusLabel={
          runtimes.some((r) => r.status === "running")
            ? t("onboarding.scan.status.running")
            : t("onboarding.scan.status.none")
        }
        body={null}
      >
        <ul className="onb-runtime-list">
          {runtimes.map((rt) => (
            <RuntimeRow key={rt.runtime} result={rt} />
          ))}
        </ul>
      </ScanCard>
    </div>
  );
}

// ── Failure ─────────────────────────────────────────────────────────────

function ScanFailure({
  message,
  onRetry,
}: {
  message: string;
  onRetry: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="onb-error" role="alert">
      <h2 className="onb-error-title">{t("onboarding.scan.failure.title")}</h2>
      <p className="onb-error-body">{t("onboarding.scan.failure.body")}</p>
      <pre className="onb-error-detail">{message}</pre>
      <div className="onb-error-actions">
        <button
          type="button"
          className="onb-button onb-button-primary"
          onClick={onRetry}
        >
          {t("onboarding.error.retry")}
        </button>
      </div>
    </div>
  );
}

// ── 작은 부품들 ─────────────────────────────────────────────────────────

type CardStatus = "ok" | "warn" | "muted";

function ScanCard({
  title,
  status,
  statusLabel,
  body,
  hint,
  children,
}: {
  title: string;
  status: CardStatus;
  statusLabel: string;
  body: string | null;
  hint?: string;
  children?: React.ReactNode;
}) {
  return (
    <div className="onb-scan-card" data-status={status}>
      <header className="onb-scan-card-header">
        <span className="onb-scan-card-title">{title}</span>
        <span className="onb-scan-pill" data-status={status}>
          {statusLabel}
        </span>
      </header>
      {body && <p className="onb-scan-card-body">{body}</p>}
      {children}
      {hint && <p className="onb-scan-card-hint">{hint}</p>}
    </div>
  );
}

function RuntimeRow({ result }: { result: DetectResult }) {
  const { t } = useTranslation();
  const status = result.status;
  return (
    <li className="onb-runtime-row" data-status={status}>
      <span className="onb-runtime-name">{runtimeKindLabel(result.runtime)}</span>
      <span className="onb-runtime-status">
        {status === "running" && t("onboarding.scan.runtime.running")}
        {status === "installed" && t("onboarding.scan.runtime.installed")}
        {status === "not-installed" && t("onboarding.scan.runtime.notInstalled")}
        {status === "error" && t("onboarding.scan.runtime.error")}
      </span>
      {result.version && (
        <span className="onb-runtime-version num">{result.version}</span>
      )}
    </li>
  );
}

// ── 포맷 헬퍼 ──────────────────────────────────────────────────────────

function pickPrimaryGpu(gpus: GpuInfo[]): GpuInfo | undefined {
  if (gpus.length === 0) return undefined;
  // VRAM 가장 큰 것 — discrete GPU 우선.
  return [...gpus].sort(
    (a, b) => (b.vram_bytes ?? 0) - (a.vram_bytes ?? 0),
  )[0];
}

function gpuVendorLabel(vendor: GpuInfo["vendor"]): string {
  switch (vendor) {
    case "nvidia":
      return "NVIDIA";
    case "amd":
      return "AMD";
    case "intel":
      return "Intel";
    case "apple":
      return "Apple";
    case "qualcomm":
      return "Qualcomm";
    case "microsoft":
      return "Microsoft";
    default:
      return "GPU";
  }
}

function formatGpuBody(gpu: GpuInfo): string {
  if (gpu.vram_bytes && gpu.vram_bytes > 0) {
    return `${gpu.name} · ${formatGiB(gpu.vram_bytes)} VRAM`;
  }
  return gpu.name;
}
