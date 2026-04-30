// Diagnostics — Phase 4.f.
//
// 정책 (phase-4-screens-decision.md §1.1 diagnostics):
// - 4 섹션 grid 2x2 (자가스캔 / 게이트웨이 / 벤치 / 워크스페이스).
// - 종합 health score = 4 섹션 status 합산 (deterministic, server 측 계산 거부).
// - mock 데이터: 게이트웨이 latency / 활성 키 / 최근 요청 / 벤치 막대 / repair history.
//   v1.x에서 실 데이터 IPC 연결 시 // MOCK 마커 영역 교체.
// - a11y: <section aria-labelledby> × 4. 차트는 role="img" + aria-label로 textual 요약.
// - 합산 health는 role="status" aria-live="polite".

import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { StatusPill } from "@lmmaster/design-system/react";
import { listRecentBenchReports, type BenchReport } from "../ipc/bench";
import {
  getCatalogSignatureStatus,
  type CatalogSignatureStatus,
} from "../ipc/catalog-refresh";
import {
  listCrashReports,
  readCrashLog,
  type CrashSummary,
} from "../ipc/crash";
import {
  getGatewayLatencySparkline,
  getGatewayPercentiles,
  getGatewayRecentRequests,
  getGatewayStatus,
  type GatewayState,
  type Percentiles,
  type RequestRecord,
} from "../ipc/gateway";
import { listApiKeys } from "../ipc/keys";
import {
  getLastScan,
  startScan,
  type CheckResult,
  type ScanSummary,
  type Severity,
} from "../ipc/scanner";
import {
  getRepairHistory,
  getWorkspaceFingerprint,
  type RepairHistoryEntry,
  type WorkspaceStatus,
} from "../ipc/workspace";

import "./diagnostics.css";

/**
 * Phase 4.f 통합 시점에 App.tsx가 listen해서 nav 전환에 사용할 수 있는 custom event 이름.
 * window.dispatchEvent(new CustomEvent("lmmaster:navigate", { detail: "catalog" }))
 */
const NAV_EVENT = "lmmaster:navigate";

/** 종합 health 3 tier — 4 섹션 status 합산. */
type HealthTier = "green" | "yellow" | "red";

// MOCK 모두 제거 — Phase 13'.b. 실 IPC로 wire (gateway middleware + bench cache + repair JSONL).

/** UNIX epoch ms → "13:42:08" 형식. */
function formatTime(ms: number): string {
  const d = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

/** RFC3339 → "YYYY-MM-DD". */
function formatDate(iso: string): string {
  if (!iso) return "-";
  const t = Date.parse(iso);
  if (isNaN(t)) return iso;
  const d = new Date(t);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

// ── 헬퍼 ────────────────────────────────────────────────────────────

function severityToTier(severity: Severity): HealthTier {
  if (severity === "error") return "red";
  if (severity === "warn") return "yellow";
  return "green";
}

function gatewayToTier(gw: GatewayState): HealthTier {
  if (gw.status === "failed") return "red";
  if (gw.status === "stopping" || gw.status === "booting") return "yellow";
  return "green";
}

function workspaceToTier(ws: WorkspaceStatus | null): HealthTier {
  if (!ws) return "yellow";
  return ws.tier;
}

function combineTiers(tiers: HealthTier[]): HealthTier {
  if (tiers.includes("red")) return "red";
  if (tiers.includes("yellow")) return "yellow";
  return "green";
}

function checksToWorstTier(checks: CheckResult[]): HealthTier {
  if (checks.length === 0) return "yellow";
  const tiers = checks.map((c) => severityToTier(c.severity));
  return combineTiers(tiers);
}

function gatewayPillStatus(gw: GatewayState) {
  // listening / booting / failed / stopping은 PillStatus 그대로.
  return gw.status;
}

// ── 메인 ────────────────────────────────────────────────────────────

export function Diagnostics() {
  const { t } = useTranslation();
  const [scan, setScan] = useState<ScanSummary | null>(null);
  const [gw, setGw] = useState<GatewayState>({ port: null, status: "booting", error: null });
  const [ws, setWs] = useState<WorkspaceStatus | null>(null);
  const [scanLoading, setScanLoading] = useState<boolean>(false);
  // Phase 9'.x audit fix — 활성 키 카운트 listApiKeys() 실데이터.
  const [activeKeyCount, setActiveKeyCount] = useState<number | null>(null);
  // Phase 13'.b — Diagnostics 4 MOCK 제거 → 실 IPC.
  const [latencySparkline, setLatencySparkline] = useState<number[]>([]);
  const [percentiles, setPercentiles] = useState<Percentiles | null>(null);
  const [recentRequests, setRecentRequests] = useState<RequestRecord[]>([]);
  const [recentBench, setRecentBench] = useState<BenchReport[]>([]);
  const [repairHistory, setRepairHistory] = useState<RepairHistoryEntry[]>([]);
  const [crashes, setCrashes] = useState<CrashSummary[]>([]);
  const [crashesNotInitialized, setCrashesNotInitialized] = useState(false);
  // Phase 13'.g.2.c — catalog minisign 서명 검증 상태.
  const [signatureStatus, setSignatureStatus] =
    useState<CatalogSignatureStatus | null>(null);

  // 첫 마운트 — IPC 일괄 fetch.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const cached = await getLastScan();
        if (!cancelled) setScan(cached);
      } catch (e) {
        console.warn("getLastScan failed:", e);
      }
      try {
        const snap = await getGatewayStatus();
        if (!cancelled) setGw(snap);
      } catch (e) {
        console.warn("getGatewayStatus failed:", e);
      }
      try {
        const fp = await getWorkspaceFingerprint();
        if (!cancelled) setWs(fp);
      } catch (e) {
        console.warn("getWorkspaceFingerprint failed:", e);
      }
      try {
        const keys = await listApiKeys();
        if (!cancelled) {
          // revoked 키 제외 — gateway 호출 가능한 active 카운트.
          const active = keys.filter(
            (k) => (k as { revoked_at?: string | null }).revoked_at == null,
          ).length;
          setActiveKeyCount(active);
        }
      } catch (e) {
        console.warn("listApiKeys failed:", e);
      }
      // Phase 13'.b — bench / repair history는 한 번만 (변동 적음).
      try {
        const reports = await listRecentBenchReports(5);
        if (!cancelled) setRecentBench(reports);
      } catch (e) {
        console.warn("listRecentBenchReports failed:", e);
      }
      try {
        const hist = await getRepairHistory(10);
        if (!cancelled) setRepairHistory(hist);
      } catch (e) {
        console.warn("getRepairHistory failed:", e);
      }
      // Phase 13'.c — crash report 목록 (1회 fetch, panic 발생 시에만 변동).
      try {
        const list = await listCrashReports(20);
        if (!cancelled) {
          setCrashes(list);
          setCrashesNotInitialized(false);
        }
      } catch (e) {
        const errKind = (e as { kind?: string })?.kind;
        if (errKind === "not-initialized") {
          if (!cancelled) setCrashesNotInitialized(true);
        } else {
          console.warn("listCrashReports failed:", e);
        }
      }
      // Phase 13'.g.2.c — catalog 서명 검증 상태 (refresh 시점에만 변동).
      try {
        const status = await getCatalogSignatureStatus();
        if (!cancelled) setSignatureStatus(status);
      } catch (e) {
        console.warn("getCatalogSignatureStatus failed:", e);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Phase 13'.b — gateway latency sparkline + 최근 요청 + percentiles는 5초 polling (라이브성).
  useEffect(() => {
    let cancelled = false;
    const tick = async () => {
      if (cancelled) return;
      try {
        const [sparkline, recent, p] = await Promise.all([
          getGatewayLatencySparkline(),
          getGatewayRecentRequests(5),
          getGatewayPercentiles(),
        ]);
        if (cancelled) return;
        setLatencySparkline(sparkline);
        setRecentRequests(recent);
        setPercentiles(p);
      } catch (e) {
        if (!cancelled) console.warn("gateway metrics polling failed:", e);
      }
    };
    void tick();
    const id = window.setInterval(tick, 5000);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, []);

  const onRescan = useCallback(async () => {
    setScanLoading(true);
    try {
      const fresh = await startScan();
      setScan(fresh);
    } catch (e) {
      console.warn("startScan failed:", e);
    } finally {
      setScanLoading(false);
    }
  }, []);

  const onStartNewBench = useCallback(() => {
    // App.tsx가 listen해 nav 전환. 통합 전엔 noop.
    if (typeof window !== "undefined") {
      window.dispatchEvent(
        new CustomEvent(NAV_EVENT, { detail: "catalog" }),
      );
    }
  }, []);

  const scanTier: HealthTier = useMemo(() => {
    if (!scan) return "yellow";
    return checksToWorstTier(scan.checks);
  }, [scan]);

  const gatewayTier = useMemo(() => gatewayToTier(gw), [gw]);
  const wsTier = useMemo(() => workspaceToTier(ws), [ws]);
  // bench는 mock — 항상 green (실 데이터 도착 후 v1.x에서 평가).
  const benchTier: HealthTier = "green";

  const overallTier = useMemo(
    () => combineTiers([scanTier, gatewayTier, wsTier, benchTier]),
    [scanTier, gatewayTier, wsTier, benchTier],
  );

  return (
    <div className="diag-root">
      <header className="diag-topbar">
        <div className="diag-topbar-text">
          <h2 className="diag-page-title">{t("screens.diagnostics.title")}</h2>
          <p className="diag-page-subtitle">{t("screens.diagnostics.subtitle")}</p>
        </div>
        <div
          className={`diag-health diag-health-${overallTier}`}
          role="status"
          aria-live="polite"
          data-testid="diag-overall-health"
        >
          <span className="diag-health-dot" aria-hidden />
          <span className="diag-health-label">
            {t(`screens.diagnostics.health.${overallTier}`)}
          </span>
        </div>
      </header>

      <div className="diag-grid">
        <ScanSection
          scan={scan}
          loading={scanLoading}
          tier={scanTier}
          onRescan={onRescan}
        />
        <GatewaySection
          gw={gw}
          tier={gatewayTier}
          activeKeyCount={activeKeyCount}
          latencySparkline={latencySparkline}
          percentiles={percentiles}
          recentRequests={recentRequests}
        />
        <BenchSection entries={recentBench} onStartNewBench={onStartNewBench} />
        <WorkspaceSection ws={ws} tier={wsTier} history={repairHistory} />
        <CrashSection crashes={crashes} notInitialized={crashesNotInitialized} />
        <SignatureSection status={signatureStatus} />
      </div>
    </div>
  );
}

// ── Phase 13'.g.2.c — Catalog minisign 서명 검증 섹션 (ADR-0047) ──────

interface SignatureSectionProps {
  status: CatalogSignatureStatus | null;
}

function SignatureSection({ status }: SignatureSectionProps) {
  const { t } = useTranslation();

  if (!status) {
    return (
      <section
        className="diag-card"
        role="region"
        aria-labelledby="signature-section-title"
        data-testid="diagnostics-signature-section"
      >
        <h3 id="signature-section-title" className="diag-card-title">
          {t("diagnostics.signature.title", "카탈로그 서명")}
        </h3>
        <p className="diag-card-empty">
          {t(
            "diagnostics.signature.empty",
            "아직 카탈로그를 갱신하지 않아 검증 결과가 없어요.",
          )}
        </p>
      </section>
    );
  }

  const tone = signatureTone(status.kind);
  const message = signatureMessage(status, t);

  return (
    <section
      className={`diag-card diag-card-tone-${tone}`}
      role="region"
      aria-labelledby="signature-section-title"
      data-testid="diagnostics-signature-section"
      data-tone={tone}
    >
      <h3 id="signature-section-title" className="diag-card-title">
        {t("diagnostics.signature.title", "카탈로그 서명")}
      </h3>
      <p
        className="diag-signature-message"
        data-testid="diagnostics-signature-message"
        role={status.kind === "failed" ? "alert" : undefined}
      >
        {message}
      </p>
      <p className="diag-signature-meta num">
        {t("diagnostics.signature.checkedAt", "검증 시각")}:{" "}
        {new Date(status.at_ms).toLocaleString("ko-KR")}
      </p>
    </section>
  );
}

function signatureTone(
  kind: CatalogSignatureStatus["kind"],
): "ok" | "warn" | "error" | "neutral" {
  switch (kind) {
    case "verified":
      return "ok";
    case "failed":
      return "error";
    case "missing-signature":
      return "warn";
    case "bundled-fallback":
    case "disabled":
      return "neutral";
  }
}

type TFn = ReturnType<typeof useTranslation>["t"];

function signatureMessage(
  status: CatalogSignatureStatus,
  t: TFn,
): string {
  switch (status.kind) {
    case "verified":
      return t("diagnostics.signature.verified", {
        source: status.source,
        defaultValue: `검증됨 (${status.source})`,
      });
    case "failed":
      return t("diagnostics.signature.failed", {
        reason: status.reason,
        defaultValue: `❌ 서명 검증 실패: ${status.reason}. 안전을 위해 기본 목록을 사용하고 있어요.`,
      });
    case "missing-signature":
      return t(
        "diagnostics.signature.missing",
        "⚠ 서명 파일을 받지 못했어요. CI 서명 파이프라인을 확인해 주세요.",
      );
    case "bundled-fallback":
      return t(
        "diagnostics.signature.bundled",
        "내장 카탈로그 사용 중 — 서명 검증 부적용 (빌드 시점 신뢰).",
      );
    case "disabled":
      return t(
        "diagnostics.signature.disabled",
        "서명 검증 비활성 (개발 빌드).",
      );
  }
}

// ── 좌상 — 자가스캔 ───────────────────────────────────────────────

interface ScanSectionProps {
  scan: ScanSummary | null;
  loading: boolean;
  tier: HealthTier;
  onRescan: () => void | Promise<void>;
}

function ScanSection({ scan, loading, tier, onRescan }: ScanSectionProps) {
  const { t } = useTranslation();
  const titleId = "diag-section-scan-title";
  return (
    <section
      className="diag-section"
      aria-labelledby={titleId}
      data-testid="diag-section-scan"
      data-tier={tier}
    >
      <header className="diag-section-header">
        <h3 id={titleId} className="diag-section-title">
          {t("screens.diagnostics.sections.scan.title")}
        </h3>
        <button
          type="button"
          className="diag-section-action"
          onClick={() => void onRescan()}
          disabled={loading}
        >
          {t("screens.diagnostics.sections.scan.rescan")}
        </button>
      </header>
      <div className="diag-section-body">
        {scan ? (
          <>
            <p className="diag-scan-summary">{scan.summary_korean}</p>
            <ul className="diag-scan-checks" role="list">
              {scan.checks.slice(0, 6).map((c) => (
                <li key={c.id} className="diag-scan-check" role="listitem">
                  <StatusPill
                    size="sm"
                    status={c.severity === "error" ? "failed" : c.severity === "warn" ? "stopping" : "listening"}
                    label={c.title_ko}
                  />
                  <span className="diag-scan-check-detail">{c.detail_ko}</span>
                </li>
              ))}
            </ul>
          </>
        ) : (
          <p className="diag-empty">{t("screens.diagnostics.sections.scan.empty")}</p>
        )}
      </div>
    </section>
  );
}

// ── 우상 — 게이트웨이 ────────────────────────────────────────────

interface GatewaySectionProps {
  gw: GatewayState;
  tier: HealthTier;
  activeKeyCount: number | null;
  latencySparkline: number[];
  percentiles: Percentiles | null;
  recentRequests: RequestRecord[];
}

function GatewaySection({
  gw,
  tier,
  activeKeyCount,
  latencySparkline,
  percentiles,
  recentRequests,
}: GatewaySectionProps) {
  const { t } = useTranslation();
  const titleId = "diag-section-gateway-title";
  const detail = gw.port != null ? `:${gw.port}` : null;
  return (
    <section
      className="diag-section"
      aria-labelledby={titleId}
      data-testid="diag-section-gateway"
      data-tier={tier}
    >
      <header className="diag-section-header">
        <h3 id={titleId} className="diag-section-title">
          {t("screens.diagnostics.sections.gateway.title")}
        </h3>
        <StatusPill
          size="sm"
          status={gatewayPillStatus(gw)}
          label={t(`gateway.status.${gw.status}`)}
          detail={detail}
        />
      </header>
      <div className="diag-section-body">
        <div className="diag-gateway-meta">
          <span className="diag-gateway-keys num" data-testid="diag-active-key-count">
            {t("screens.diagnostics.sections.gateway.activeKeys", {
              count: activeKeyCount ?? 0,
            })}
          </span>
        </div>
        <LatencySparkline
          samples={latencySparkline}
          percentiles={percentiles}
        />
        <h4 className="diag-section-subtitle">
          {t("screens.diagnostics.sections.gateway.recentRequests")}
        </h4>
        {recentRequests.length === 0 ? (
          <p className="diag-empty">
            {t("screens.diagnostics.sections.gateway.noRequests")}
          </p>
        ) : (
          <ul className="diag-gateway-requests" role="list">
            {recentRequests.map((r) => (
              <li
                key={`${r.ts_ms}-${r.path}`}
                role="listitem"
                className="diag-gateway-request num"
              >
                <span className="diag-req-ts">{formatTime(r.ts_ms)}</span>
                <span className="diag-req-method">{r.method}</span>
                <span className="diag-req-path">{r.path}</span>
                <span className="diag-req-status">{r.status}</span>
                <span className="diag-req-ms">{r.ms}ms</span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

interface LatencySparklineProps {
  samples: number[];
  percentiles?: Percentiles | null;
}

function LatencySparkline({ samples, percentiles }: LatencySparklineProps) {
  const { t } = useTranslation();
  const allZero = samples.length === 0 || samples.every((v) => v === 0);
  const max = Math.max(...samples, 1);
  const min = Math.min(...samples, 0);
  const w = 240;
  const h = 40;
  const stepX = samples.length > 1 ? w / (samples.length - 1) : 0;
  const points = samples
    .map((v, i) => {
      const norm = (v - min) / (max - min || 1);
      const x = i * stepX;
      const y = h - norm * (h - 4) - 2;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
  const nonZero = samples.filter((v) => v > 0);
  const avg =
    nonZero.length > 0
      ? nonZero.reduce((a, b) => a + b, 0) / nonZero.length
      : 0;
  const ariaLabel = allZero
    ? `${t("screens.diagnostics.sections.gateway.latency")} — 최근 60초 동안 요청 없음`
    : `${t("screens.diagnostics.sections.gateway.latency")} — 평균 ${avg.toFixed(0)}ms, 최대 ${max}ms${
        percentiles ? `, p50 ${percentiles.p50_ms}ms, p95 ${percentiles.p95_ms}ms` : ""
      }`;
  return (
    <div className="diag-sparkline-wrap">
      <span className="diag-sparkline-label">
        {t("screens.diagnostics.sections.gateway.latency")}
      </span>
      <svg
        className="diag-sparkline-svg"
        width={w}
        height={h}
        viewBox={`0 0 ${w} ${h}`}
        role="img"
        aria-label={ariaLabel}
        data-testid="diag-gateway-sparkline"
      >
        <polyline
          fill="none"
          stroke="currentColor"
          strokeWidth="1.4"
          strokeLinecap="round"
          strokeLinejoin="round"
          points={points}
        />
      </svg>
    </div>
  );
}

// ── 좌하 — 벤치 ───────────────────────────────────────────────────

interface BenchSectionProps {
  entries: BenchReport[];
  onStartNewBench: () => void;
}

function BenchSection({ entries, onStartNewBench }: BenchSectionProps) {
  const { t } = useTranslation();
  const titleId = "diag-section-bench-title";
  const max = Math.max(...entries.map((e) => e.tg_tps), 1);

  const ariaLabel =
    entries.length === 0
      ? t("screens.diagnostics.sections.bench.empty")
      : entries
          .map((e) => `${e.model_id} ${e.tg_tps.toFixed(1)} 토큰/초`)
          .join(", ");

  return (
    <section
      className="diag-section"
      aria-labelledby={titleId}
      data-testid="diag-section-bench"
    >
      <header className="diag-section-header">
        <h3 id={titleId} className="diag-section-title">
          {t("screens.diagnostics.sections.bench.title")}
        </h3>
        <button
          type="button"
          className="diag-section-action diag-section-action-primary"
          onClick={onStartNewBench}
          data-testid="diag-bench-start-new"
        >
          {t("screens.diagnostics.sections.bench.startNew")}
        </button>
      </header>
      <div className="diag-section-body">
        {entries.length === 0 ? (
          <p className="diag-empty">{t("screens.diagnostics.sections.bench.empty")}</p>
        ) : (
          <div
            className="diag-bench-bars"
            role="img"
            aria-label={ariaLabel}
            data-testid="diag-bench-chart"
          >
            {entries.map((e) => {
              const widthPct = Math.max(2, (e.tg_tps / max) * 100);
              const display =
                e.quant_label != null ? `${e.model_id} (${e.quant_label})` : e.model_id;
              return (
                <div
                  key={`${e.model_id}-${e.quant_label ?? "_"}-${e.host_fingerprint_short}`}
                  className="diag-bench-bar-row"
                >
                  <span className="diag-bench-bar-name">{display}</span>
                  <span className="diag-bench-bar-track">
                    <span
                      className="diag-bench-bar-fill"
                      style={{ width: `${widthPct.toFixed(1)}%` }}
                    />
                  </span>
                  <span className="diag-bench-bar-num num">
                    {e.tg_tps.toFixed(1)} tok/s
                  </span>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </section>
  );
}

// ── 우하 — 워크스페이스 ──────────────────────────────────────────

interface WorkspaceSectionProps {
  ws: WorkspaceStatus | null;
  tier: HealthTier;
  history: RepairHistoryEntry[];
}

function WorkspaceSection({ ws, tier, history }: WorkspaceSectionProps) {
  const { t } = useTranslation();
  const titleId = "diag-section-workspace-title";
  return (
    <section
      className="diag-section"
      aria-labelledby={titleId}
      data-testid="diag-section-workspace"
      data-tier={tier}
    >
      <header className="diag-section-header">
        <h3 id={titleId} className="diag-section-title">
          {t("screens.diagnostics.sections.workspace.title")}
        </h3>
      </header>
      <div className="diag-section-body">
        <h4 className="diag-section-subtitle">
          {t("screens.diagnostics.sections.workspace.fingerprint")}
        </h4>
        {ws ? (
          <dl className="diag-ws-fingerprint">
            <div className="diag-ws-row">
              <dt>OS</dt>
              <dd className="num">{ws.fingerprint.os}</dd>
            </div>
            <div className="diag-ws-row">
              <dt>arch</dt>
              <dd className="num">{ws.fingerprint.arch}</dd>
            </div>
            <div className="diag-ws-row">
              <dt>GPU</dt>
              <dd className="num">{ws.fingerprint.gpu_class}</dd>
            </div>
            <div className="diag-ws-row">
              <dt>VRAM</dt>
              <dd className="num">{ws.fingerprint.vram_bucket_mb}MB</dd>
            </div>
            <div className="diag-ws-row">
              <dt>RAM</dt>
              <dd className="num">{ws.fingerprint.ram_bucket_mb}MB</dd>
            </div>
          </dl>
        ) : (
          <p className="diag-empty">{t("screens.diagnostics.sections.scan.empty")}</p>
        )}

        <h4 className="diag-section-subtitle">
          {t("screens.diagnostics.sections.workspace.history")}
        </h4>
        {history.length === 0 ? (
          <p className="diag-empty">
            {t("screens.diagnostics.sections.workspace.never")}
          </p>
        ) : (
          <table className="diag-ws-history" data-testid="diag-ws-history">
            <thead>
              <tr>
                <th scope="col">날짜</th>
                <th scope="col">tier</th>
                <th scope="col">캐시</th>
              </tr>
            </thead>
            <tbody>
              {history.map((h, i) => (
                <tr key={`${h.at}-${i}`}>
                  <td className="num">{formatDate(h.at)}</td>
                  <td>
                    <span className={`diag-tier-chip diag-tier-chip-${h.tier}`}>
                      {h.tier}
                    </span>
                  </td>
                  <td className="num">{h.invalidated_caches}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </section>
  );
}

// ── 풀폭 (5번째 row) — 크래시 리포트 ──────────────────────────────

interface CrashSectionProps {
  crashes: CrashSummary[];
  notInitialized: boolean;
}

function CrashSection({ crashes, notInitialized }: CrashSectionProps) {
  const { t } = useTranslation();
  const [expandedFilename, setExpandedFilename] = useState<string | null>(null);
  const [expandedBody, setExpandedBody] = useState<string | null>(null);
  const [expandedError, setExpandedError] = useState<string | null>(null);
  const titleId = "diag-section-crash-title";

  const handleToggle = useCallback(
    async (filename: string) => {
      if (expandedFilename === filename) {
        setExpandedFilename(null);
        setExpandedBody(null);
        setExpandedError(null);
        return;
      }
      setExpandedFilename(filename);
      setExpandedBody(null);
      setExpandedError(null);
      try {
        const body = await readCrashLog(filename);
        setExpandedBody(body);
      } catch (e) {
        const kind = (e as { kind?: string })?.kind;
        if (kind === "too-large") {
          setExpandedError(t("screens.diagnostics.sections.crash.tooLarge"));
        } else {
          console.warn("readCrashLog failed:", e);
          setExpandedError(t("screens.diagnostics.sections.crash.errorReading"));
        }
      }
    },
    [expandedFilename, t],
  );

  return (
    <section
      className="diag-section diag-section-fullrow"
      aria-labelledby={titleId}
      data-testid="diag-section-crash"
    >
      <header className="diag-section-header">
        <h3 id={titleId} className="diag-section-title">
          {t("screens.diagnostics.sections.crash.title")}
        </h3>
      </header>
      <div className="diag-section-body">
        <p className="diag-section-subtitle-prose">
          {t("screens.diagnostics.sections.crash.subtitle")}
        </p>
        {notInitialized ? (
          <p className="diag-empty">
            {t("screens.diagnostics.sections.crash.notInitialized")}
          </p>
        ) : crashes.length === 0 ? (
          <p className="diag-empty">
            {t("screens.diagnostics.sections.crash.empty")}
          </p>
        ) : (
          <ul className="diag-crash-list" role="list" data-testid="diag-crash-list">
            {crashes.map((c) => {
              const tsLabel = c.ts_rfc3339 ?? formatTime(c.mtime_ms);
              const isOpen = expandedFilename === c.filename;
              const kb = (c.size_bytes / 1024).toFixed(1);
              return (
                <li key={c.filename} className="diag-crash-item" role="listitem">
                  <div className="diag-crash-row">
                    <span className="diag-crash-ts num">{tsLabel}</span>
                    <span className="diag-crash-name num">{c.filename}</span>
                    <span className="diag-crash-size num">
                      {t("screens.diagnostics.sections.crash.size", { kb })}
                    </span>
                    <button
                      type="button"
                      className="diag-section-action"
                      onClick={() => void handleToggle(c.filename)}
                      data-testid={`diag-crash-toggle-${c.filename}`}
                      aria-expanded={isOpen}
                    >
                      {isOpen
                        ? t("screens.diagnostics.sections.crash.hide")
                        : t("screens.diagnostics.sections.crash.view")}
                    </button>
                  </div>
                  {isOpen && (
                    <div className="diag-crash-body">
                      {expandedError ? (
                        <p className="diag-crash-error" role="alert">
                          {expandedError}
                        </p>
                      ) : expandedBody == null ? (
                        <p className="diag-empty">…</p>
                      ) : (
                        <pre className="diag-crash-pre num">{expandedBody}</pre>
                      )}
                    </div>
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </section>
  );
}
