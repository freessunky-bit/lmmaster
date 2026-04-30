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
import {
  getGatewayStatus,
  type GatewayState,
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
  getWorkspaceFingerprint,
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

// ── MOCK 데이터 (v1.x에서 IPC로 교체) ───────────────────────────────

/** MOCK: 게이트웨이 60s latency sparkline. v1.x: gateway latency IPC. */
const MOCK_GATEWAY_LATENCY_MS = [
  18, 22, 19, 25, 24, 21, 19, 23, 28, 24, 22, 20, 19, 21, 24, 23, 25, 22, 20, 21,
  24, 26, 23, 22, 20, 19, 18, 20, 22, 24,
];

// 활성 키 수 — 2026-04-30 audit fix로 listApiKeys()에서 직접 산출. MOCK 제거.

interface RecentRequest {
  ts: string;
  method: string;
  path: string;
  status: number;
  ms: number;
}
/** MOCK: 마지막 5 request log. v1.x: 게이트웨이 access log SQLite IPC. */
const MOCK_RECENT_REQUESTS: RecentRequest[] = [
  { ts: "13:42:08", method: "POST", path: "/v1/chat/completions", status: 200, ms: 412 },
  { ts: "13:42:01", method: "GET", path: "/v1/models", status: 200, ms: 14 },
  { ts: "13:41:54", method: "POST", path: "/v1/chat/completions", status: 200, ms: 1102 },
  { ts: "13:41:38", method: "POST", path: "/v1/chat/completions", status: 200, ms: 587 },
  { ts: "13:41:21", method: "GET", path: "/v1/models", status: 200, ms: 11 },
];

interface BenchEntry {
  modelId: string;
  displayName: string;
  tps: number;
}
/** MOCK: 가장 최근 측정한 모델 5개 token/sec. v1.x: getLastBenchReport batch. */
const MOCK_BENCH_ENTRIES: BenchEntry[] = [
  { modelId: "exaone:1.2b", displayName: "EXAONE 4.0 1.2B", tps: 142.3 },
  { modelId: "qwen2.5:3b", displayName: "Qwen 2.5 3B", tps: 86.5 },
  { modelId: "hyperclova-x-seed:8b", displayName: "HyperCLOVA-X SEED 8B", tps: 38.1 },
  { modelId: "deepseek-coder:6.7b", displayName: "DeepSeek Coder 6.7B", tps: 44.2 },
  { modelId: "qwen2.5-coder:7b", displayName: "Qwen 2.5 Coder 7B", tps: 41.0 },
];

interface RepairHistoryRow {
  date: string;
  tier: HealthTier;
  invalidatedCaches: number;
}
/** MOCK: repair history. v1.x: workspace repair log IPC. */
const MOCK_REPAIR_HISTORY: RepairHistoryRow[] = [
  { date: "2026-04-25", tier: "yellow", invalidatedCaches: 2 },
  { date: "2026-03-12", tier: "yellow", invalidatedCaches: 1 },
];

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
  // Phase 9'.x audit fix — Diagnostics에 가짜 활성 키 카운트 노출하던 것을 실제 listApiKeys()
  // 결과로 교체. revoked 제외 카운트.
  const [activeKeyCount, setActiveKeyCount] = useState<number | null>(null);

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
    })();
    return () => {
      cancelled = true;
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
        <GatewaySection gw={gw} tier={gatewayTier} activeKeyCount={activeKeyCount} />
        <BenchSection entries={MOCK_BENCH_ENTRIES} onStartNewBench={onStartNewBench} />
        <WorkspaceSection ws={ws} tier={wsTier} history={MOCK_REPAIR_HISTORY} />
      </div>
    </div>
  );
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
}

function GatewaySection({
  gw,
  tier,
  activeKeyCount,
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
        <LatencySparkline samples={MOCK_GATEWAY_LATENCY_MS} />
        <h4 className="diag-section-subtitle">
          {t("screens.diagnostics.sections.gateway.recentRequests")}
        </h4>
        {MOCK_RECENT_REQUESTS.length === 0 ? (
          <p className="diag-empty">
            {t("screens.diagnostics.sections.gateway.noRequests")}
          </p>
        ) : (
          <ul className="diag-gateway-requests" role="list">
            {MOCK_RECENT_REQUESTS.map((r, idx) => (
              <li
                key={`${r.ts}-${idx}`}
                role="listitem"
                className="diag-gateway-request num"
              >
                <span className="diag-req-ts">{r.ts}</span>
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
}

function LatencySparkline({ samples }: LatencySparklineProps) {
  const { t } = useTranslation();
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
  const avg = samples.reduce((a, b) => a + b, 0) / Math.max(samples.length, 1);
  const ariaLabel = `${t("screens.diagnostics.sections.gateway.latency")} — 평균 ${avg.toFixed(0)}ms, 최대 ${max}ms`;
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
  entries: BenchEntry[];
  onStartNewBench: () => void;
}

function BenchSection({ entries, onStartNewBench }: BenchSectionProps) {
  const { t } = useTranslation();
  const titleId = "diag-section-bench-title";
  const max = Math.max(...entries.map((e) => e.tps), 1);

  const ariaLabel =
    entries.length === 0
      ? t("screens.diagnostics.sections.bench.empty")
      : entries.map((e) => `${e.displayName} ${e.tps.toFixed(1)} 토큰/초`).join(", ");

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
              const widthPct = Math.max(2, (e.tps / max) * 100);
              return (
                <div key={e.modelId} className="diag-bench-bar-row">
                  <span className="diag-bench-bar-name">{e.displayName}</span>
                  <span className="diag-bench-bar-track">
                    <span
                      className="diag-bench-bar-fill"
                      style={{ width: `${widthPct.toFixed(1)}%` }}
                    />
                  </span>
                  <span className="diag-bench-bar-num num">
                    {e.tps.toFixed(1)} tok/s
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
  history: RepairHistoryRow[];
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
                <tr key={`${h.date}-${i}`}>
                  <td className="num">{h.date}</td>
                  <td>
                    <span className={`diag-tier-chip diag-tier-chip-${h.tier}`}>
                      {h.tier}
                    </span>
                  </td>
                  <td className="num">{h.invalidatedCaches}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </section>
  );
}
