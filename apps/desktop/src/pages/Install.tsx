// Install — 런타임 설치 메인 화면 (마법사와 분리된 진입점). Phase 4.b.
//
// 정책 (phase-4-screens-decision.md §1.1 install + phase-4b-install-screen-decision.md):
// - 카드 그리드 (Ollama + LM Studio 2 카드).
// - 카드 상태별 액션: not-installed → "받을게요", running/installed → "재설치", 공통 "자세히" / "폴더 열기".
// - 카드 클릭 시 우측 drawer로 manifest detail.
// - 설치 진행 중일 때 하단 진행 패널 (InstallProgress compact 모드) — 접힘 가능.
// - 둘 다 설치되어 있으면 빈 상태 + 카탈로그 이동 CTA.
// - 디자인 토큰만 사용. 인라인 스타일 금지.

import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
} from "react";
import { useTranslation } from "react-i18next";

import { StatusPill, type PillStatus } from "@lmmaster/design-system/react";

import { InstallProgress } from "../components/InstallProgress";
import { detectEnvironment, type DetectResult, type EnvironmentReport } from "../ipc/environment";
import { cancelInstall, installApp, type InstallApiError } from "../ipc/install";
import {
  isTerminal,
  type ActionOutcome,
  type InstallEvent,
} from "../ipc/install-events";

import "./install.css";

// ── 카드 메타 정적 정의 (manifest id + 한국어 메타) ───────────────

interface RuntimeCardDef {
  /** Manifest id로 install_app에 그대로 전달. */
  id: "ollama" | "lm-studio";
  nameKey: string;
  reasonKey: string;
  licenseKey: string;
  /** 설치 크기 표시(라이선스 분리). */
  installSize: string;
  /** 공식 사이트 링크. drawer에 노출. */
  homepage: string;
}

const RUNTIME_DEFS: RuntimeCardDef[] = [
  {
    id: "ollama",
    nameKey: "screens.install.cards.ollama.name",
    reasonKey: "screens.install.cards.ollama.reason",
    licenseKey: "screens.install.cards.ollama.license",
    installSize: "약 800MB",
    homepage: "https://ollama.com/",
  },
  {
    id: "lm-studio",
    nameKey: "screens.install.cards.lmStudio.name",
    reasonKey: "screens.install.cards.lmStudio.reason",
    licenseKey: "screens.install.cards.lmStudio.license",
    installSize: "공식 사이트 안내",
    homepage: "https://lmstudio.ai/",
  },
];

// ── State 모델 ────────────────────────────────────────────────────

type RuntimeStatusKind = "running" | "installed" | "not-installed" | "unknown";

interface InstallProgressData {
  latest?: InstallEvent;
  progress?: { downloaded: number; total: number | null; speed_bps: number };
  log: InstallEvent[];
  retryAttempt?: number;
}

interface ActiveInstallState {
  id: "ollama" | "lm-studio";
  data: InstallProgressData;
}

// ── 페이지 ────────────────────────────────────────────────────────

export function InstallPage({
  onNavigate,
}: {
  /** 빈 상태에서 "카탈로그로 가볼게요" 클릭 시 호출. 메인이 라우팅 처리. */
  onNavigate?: (target: "catalog") => void;
}) {
  const { t } = useTranslation();
  const [env, setEnv] = useState<EnvironmentReport | null>(null);
  const [envError, setEnvError] = useState<string | null>(null);
  const [active, setActive] = useState<ActiveInstallState | null>(null);
  const [selected, setSelected] = useState<RuntimeCardDef | null>(null);

  // 환경 감지 — 마운트 시 1회.
  useEffect(() => {
    let cancelled = false;
    detectEnvironment()
      .then((report) => {
        if (!cancelled) setEnv(report);
      })
      .catch((e) => {
        if (cancelled) return;
        const message = (e as { message?: string }).message ?? String(e);
        setEnvError(message);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // 런타임 → status 매핑.
  const status = useMemo(() => statusMapFrom(env?.runtimes ?? []), [env]);

  const allReady = useMemo(() => {
    return (
      isReadyOrRunning(status.ollama) && isReadyOrRunning(status["lm-studio"])
    );
  }, [status]);

  const handleInstall = useCallback(
    async (id: "ollama" | "lm-studio") => {
      // 이미 설치 진행 중이면 무시 — IPC 측 already-installing도 보호하지만 UI에서 즉시 차단.
      if (active) return;
      const initial: ActiveInstallState = {
        id,
        data: { log: [] },
      };
      setActive(initial);
      try {
        await installApp(id, {
          onEvent: (event) => {
            setActive((prev) => {
              if (!prev || prev.id !== id) return prev;
              return { id, data: applyEvent(prev.data, event) };
            });
          },
        });
        // 설치 종료 후 environment 재감지 — status 갱신.
        try {
          const report = await detectEnvironment();
          setEnv(report);
        } catch (e) {
          console.warn("detectEnvironment after install failed:", e);
        }
      } catch (e) {
        const apiErr = e as InstallApiError;
        console.warn("installApp failed:", apiErr);
        // 사용자에게는 진행 패널의 마지막 이벤트가 failed로 노출됨.
        // already-installing이면 panel 정리.
        if (apiErr.kind === "already-installing") {
          // active 그대로 유지 — 진행 중인 것이 있다는 뜻.
          return;
        }
      } finally {
        // 패널은 사용자가 명시적으로 닫을 수 있도록 잠시 유지 — 다음 install 클릭 시 자동 교체.
        // 종료 이벤트(finished/failed/cancelled) 받으면 active.data.latest로 표시되고,
        // 새 install 호출 시 setActive로 교체된다.
      }
    },
    [active],
  );

  const handleCancel = useCallback(async () => {
    if (!active) return;
    try {
      await cancelInstall(active.id);
    } catch (e) {
      console.warn("cancelInstall failed:", e);
    }
  }, [active]);

  const handleDismissPanel = useCallback(() => {
    setActive(null);
  }, []);

  return (
    <div className="install-root">
      <div className="install-topbar">
        <div className="install-topbar-row">
          <h2 className="install-title">{t("screens.install.title")}</h2>
          <AggregateStatusPill status={status} />
        </div>
        <p className="install-subtitle">{t("screens.install.subtitle")}</p>
      </div>

      {envError && (
        <div className="install-empty" role="alert">
          <h3 className="install-empty-title">{envError}</h3>
        </div>
      )}

      <section
        className="install-card-grid"
        aria-label={t("screens.install.title")}
      >
        {RUNTIME_DEFS.map((def) => (
          <RuntimeCard
            key={def.id}
            def={def}
            status={status[def.id]}
            onInstall={() => handleInstall(def.id)}
            onSelect={() => setSelected(def)}
            isInstalling={active?.id === def.id && !panelDismissable(active)}
          />
        ))}
      </section>

      {active && (
        <ProgressPanel
          active={active}
          onCancel={handleCancel}
          onDismiss={handleDismissPanel}
        />
      )}

      {!active && allReady && (
        <EmptyState onNavigate={onNavigate} />
      )}

      {selected && (
        <ManifestDrawer def={selected} onClose={() => setSelected(null)} />
      )}
    </div>
  );
}

// ── 합산 StatusPill ────────────────────────────────────────────

function AggregateStatusPill({
  status,
}: {
  status: Record<RuntimeCardDef["id"], RuntimeStatusKind>;
}) {
  const { t } = useTranslation();
  const gw = aggregateGatewayLikeStatus(status);
  const label = (() => {
    switch (gw) {
      case "listening":
        return t("gateway.status.listening");
      case "booting":
        return t("gateway.status.booting");
      case "failed":
        return t("gateway.status.failed");
      case "stopping":
        return t("status.standby");
      default:
        return t("status.standby");
    }
  })();
  return <StatusPill status={gw} label={label} size="md" />;
}

function aggregateGatewayLikeStatus(
  status: Record<RuntimeCardDef["id"], RuntimeStatusKind>,
): PillStatus {
  // 런타임 합산 → "running 1+개면 listening, installed 1+개면 stopping(준비됐어요),
  // 모두 unknown이면 booting, 모두 not-installed면 idle".
  const values = Object.values(status);
  if (values.some((s) => s === "running")) return "listening";
  if (values.some((s) => s === "installed")) return "stopping";
  if (values.every((s) => s === "unknown")) return "booting";
  return "idle";
}

// ── RuntimeCard ────────────────────────────────────────────────

function RuntimeCard({
  def,
  status,
  onInstall,
  onSelect,
  isInstalling,
}: {
  def: RuntimeCardDef;
  status: RuntimeStatusKind;
  onInstall: () => void;
  onSelect: () => void;
  isInstalling: boolean;
}) {
  const { t } = useTranslation();
  const isReady = status === "running" || status === "installed";

  const pillState = pillStateFor(status, isInstalling);

  const announcement = isReady
    ? t("screens.install.alreadyReady")
    : t("screens.install.notInstalled");

  // 카드는 region — 액션은 footer 버튼으로만 트리거 (nested interactive 회피).
  // 비-버튼 영역 click도 onSelect 트리거 — 마우스 편의 (키보드는 footer 버튼이 책임).
  const titleId = `install-card-title-${def.id}`;

  const handleAreaClick = (e: ReactMouseEvent<HTMLElement>) => {
    if ((e.target as HTMLElement).closest("button")) return;
    onSelect();
  };

  return (
    <article
      className="install-card"
      data-runtime={def.id}
      data-status={status}
      onClick={handleAreaClick}
      aria-labelledby={titleId}
    >
      <div className="install-card-header">
        <h3 id={titleId} className="install-card-name">
          {t(def.nameKey)}
        </h3>
        <span className="install-card-license">{t(def.licenseKey)}</span>
      </div>

      <div className="install-card-status-row">
        <StatusPill
          status={pillState.status}
          label={pillState.label(t)}
          size="sm"
        />
        <span className="install-card-status-text">{announcement}</span>
      </div>

      <p className="install-card-reason">{t(def.reasonKey)}</p>

      <div className="install-card-footer">
        <button
          type="button"
          className="install-action is-primary"
          onClick={onInstall}
          disabled={isInstalling}
        >
          {isReady
            ? t("screens.install.actions.reinstall")
            : t("screens.install.actions.install")}
        </button>
        <button
          type="button"
          className="install-action"
          onClick={onSelect}
        >
          {t("screens.install.actions.details")}
        </button>
      </div>
    </article>
  );
}

function pillStateFor(
  status: RuntimeStatusKind,
  isInstalling: boolean,
): { status: PillStatus; label: (t: (k: string) => string) => string } {
  if (isInstalling) {
    return {
      status: "booting",
      label: (t) => t("status.installing"),
    };
  }
  switch (status) {
    case "running":
      return { status: "listening", label: (t) => t("gateway.status.listening") };
    case "installed":
      return { status: "stopping", label: (t) => t("status.standby") };
    case "not-installed":
      return { status: "idle", label: (t) => t("status.standby") };
    default:
      return { status: "idle", label: (t) => t("status.standby") };
  }
}

// ── 진행 패널 ──────────────────────────────────────────────────

function ProgressPanel({
  active,
  onCancel,
  onDismiss,
}: {
  active: ActiveInstallState;
  onCancel: () => void;
  onDismiss: () => void;
}) {
  const { t } = useTranslation();
  const isFinished = isTerminalData(active.data);
  return (
    <section
      className="install-progress-panel"
      aria-labelledby="install-progress-panel-title"
      aria-live="polite"
    >
      <div className="install-topbar-row">
        <h3
          id="install-progress-panel-title"
          className="install-progress-panel-title"
        >
          {nameForId(active.id, t)}
        </h3>
        {isFinished && (
          <button
            type="button"
            className="install-action"
            onClick={onDismiss}
          >
            {t("screens.install.drawer.close")}
          </button>
        )}
      </div>
      <InstallProgress
        compact
        title={nameForId(active.id, t)}
        data={active.data}
        onCancel={isFinished ? undefined : onCancel}
      />
    </section>
  );
}

// ── 빈 상태 ────────────────────────────────────────────────────

function EmptyState({
  onNavigate,
}: {
  onNavigate?: (target: "catalog") => void;
}) {
  const { t } = useTranslation();
  return (
    <section className="install-empty" role="status">
      <h3 className="install-empty-title">
        {t("screens.install.empty.title")}
      </h3>
      <p className="install-empty-body">{t("screens.install.empty.body")}</p>
      <button
        type="button"
        className="install-action is-primary install-empty-cta"
        onClick={() => onNavigate?.("catalog")}
      >
        {t("screens.install.empty.cta")}
      </button>
    </section>
  );
}

// ── Drawer (manifest detail) ──────────────────────────────────

function ManifestDrawer({
  def,
  onClose,
}: {
  def: RuntimeCardDef;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const closeBtnRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    closeBtnRef.current?.focus();
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div
      className="install-drawer-backdrop"
      role="presentation"
      onClick={onClose}
    >
      <div
        className="install-drawer"
        role="dialog"
        aria-modal="true"
        aria-labelledby="install-drawer-title"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="install-drawer-header">
          <h3 id="install-drawer-title" className="install-drawer-title">
            {t(def.nameKey)}
          </h3>
          <button
            ref={closeBtnRef}
            type="button"
            className="install-drawer-close"
            onClick={onClose}
            aria-label={t("screens.install.drawer.close")}
          >
            ×
          </button>
        </div>
        <div className="install-drawer-body">
          <section>
            <h4 className="install-drawer-section-title">
              {t("screens.install.drawer.licenseFull")}
            </h4>
            <p className="install-drawer-text">{t(def.licenseKey)}</p>
          </section>
          <section>
            <h4 className="install-drawer-section-title">
              {t("screens.install.drawer.installSize")}
            </h4>
            <p className="install-drawer-text num">{def.installSize}</p>
          </section>
          <section>
            <h4 className="install-drawer-section-title">
              {t("screens.install.drawer.homepage")}
            </h4>
            <p className="install-drawer-text">
              <a
                className="install-drawer-link"
                href={def.homepage}
                target="_blank"
                rel="noreferrer"
              >
                {def.homepage}
              </a>
            </p>
          </section>
          <section>
            <p className="install-drawer-text">{t(def.reasonKey)}</p>
          </section>
        </div>
      </div>
    </div>
  );
}

// ── 헬퍼 ────────────────────────────────────────────────────────

function statusMapFrom(
  runtimes: DetectResult[],
): Record<RuntimeCardDef["id"], RuntimeStatusKind> {
  const find = (k: RuntimeCardDef["id"]): RuntimeStatusKind => {
    const r = runtimes.find((x) => x.runtime === k);
    if (!r) return "unknown";
    if (
      r.status === "running" ||
      r.status === "installed" ||
      r.status === "not-installed"
    ) {
      return r.status;
    }
    return "unknown";
  };
  return {
    ollama: find("ollama"),
    "lm-studio": find("lm-studio"),
  };
}

function isReadyOrRunning(s: RuntimeStatusKind): boolean {
  return s === "running" || s === "installed";
}

function applyEvent(
  prev: InstallProgressData,
  event: InstallEvent,
): InstallProgressData {
  const log = [...prev.log, event].slice(-10);
  let progress = prev.progress;
  let retryAttempt = prev.retryAttempt;
  if (event.kind === "download") {
    if (event.download.kind === "progress") {
      progress = {
        downloaded: event.download.downloaded,
        total: event.download.total,
        speed_bps: event.download.speed_bps,
      };
    } else if (event.download.kind === "retrying") {
      retryAttempt = event.download.attempt;
    }
  }
  return { latest: event, progress, log, retryAttempt };
}

function isTerminalData(data: InstallProgressData): boolean {
  return data.latest != null && isTerminal(data.latest);
}

function panelDismissable(active: ActiveInstallState | null): boolean {
  if (!active) return false;
  return isTerminalData(active.data);
}

function nameForId(
  id: RuntimeCardDef["id"],
  t: (key: string) => string,
): string {
  if (id === "ollama") return t("screens.install.cards.ollama.name");
  return t("screens.install.cards.lmStudio.name");
}

// active.data를 외부에서 ActionOutcome 검사할 때 쓰는 type guard re-export 자리.
// 미사용이지만 향후 toast 등에서 outcome.kind 분기에 활용 가능.
export type { ActionOutcome };
