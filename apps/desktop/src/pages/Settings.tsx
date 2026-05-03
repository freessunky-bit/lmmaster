// Settings — Phase 4.g + Phase 6'.b 자동 갱신 섹션.
//
// 정책 (phase-4-screens-decision.md §1.1 settings, ADR-0026 §2):
// - 좌측 카테고리 nav (sm 240px) — 일반 / 워크스페이스 / 카탈로그 / 고급 4 카테고리.
// - 우측 form 패널 — 선택된 카테고리.
// - 일반: 언어, 테마(dark-only 자물쇠), 자가스캔 주기, 음성 입출력(disabled), 자동 갱신 (Phase 6'.b).
// - 워크스페이스: 경로, "다른 폴더로 옮기기"(disabled v1.1), "워크스페이스 정리".
// - 카탈로그: registry URL(read-only), 마지막 갱신, "지금 갱신"(disabled placeholder).
// - 고급: Gemini opt-in(disabled), SQLCipher env hint, 진단 로그 export(disabled), 빌드 정보.
// - form은 fieldset + legend. 토글은 role="switch" + aria-checked.
// - 자동 갱신: interval 1h~24h 강제 (ADR-0026 §2). 단발 "지금 확인" + Outdated 시 ToastUpdate 인라인.

import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";

import {
  checkWorkspaceRepair,
  getWorkspaceFingerprint,
  type WorkspaceStatus,
} from "../ipc/workspace";

import {
  getEncryptDbHint,
  getNotifyOnPhase5,
  getScanInterval,
  getUpdateChannel,
  setNotifyOnPhase5 as writeNotifyOnPhase5,
  setScanInterval as writeScanInterval,
  setUpdateChannel as writeUpdateChannel,
  type ScanIntervalValue,
  type UpdateChannel,
} from "../ipc/settings";

import {
  checkForUpdate,
  getAutoUpdateStatus,
  startAutoUpdatePoller,
  stopAutoUpdatePoller,
  type PollerStatus,
  type ReleaseInfo,
  type UpdateEvent,
} from "../ipc/updater";
import { ToastUpdate } from "../components/ToastUpdate";
import { PipelinesPanel } from "../components/PipelinesPanel";
import { TelemetryPanel } from "../components/TelemetryPanel";
import { CatalogRefreshPanel } from "../components/CatalogRefreshPanel";
import { WorkbenchArtifactPanel } from "../components/WorkbenchArtifactPanel";
import { HelpButton } from "../components/HelpButton";
import { PortableExportPanel } from "../components/portable/PortableExportPanel";
import { PortableImportPanel } from "../components/portable/PortableImportPanel";

import "./settings.css";

/** 5 카테고리 nav — Phase 11'에서 portable 추가. */
type CategoryKey =
  | "general"
  | "workspace"
  | "portable"
  | "catalog"
  | "advanced";

const CATEGORIES: CategoryKey[] = [
  "general",
  "workspace",
  "portable",
  "catalog",
  "advanced",
];

/** registry URL은 외부 통신 0 정책상 표시만. */
const REGISTRY_URL = "lmmaster://registry/local";

/** 빌드 시점 mock — 실제는 Tauri info()에서 가져옴. v1.1에 wire-up. */
const APP_VERSION = "0.1.0";
const BUILD_COMMIT = "dev";

/** 자동 갱신 — GitHub repo (외부 통신 0 정책 예외, ADR-0026 §1). */
const UPDATE_REPO = "anthropics/lmmaster";
/** Phase 7'.b — 베타 채널 별도 repo. 정식 release와 분리. */
const UPDATE_REPO_BETA = "anthropics/lmmaster-beta";

/** ADR-0026 §2: 1h~24h 허용 범위. UI는 4개 단계로 압축. */
const INTERVAL_OPTIONS: { secs: number; key: "1h" | "6h" | "12h" | "24h" }[] = [
  { secs: 3600, key: "1h" },
  { secs: 6 * 3600, key: "6h" },
  { secs: 12 * 3600, key: "12h" },
  { secs: 24 * 3600, key: "24h" },
];
const DEFAULT_INTERVAL_SECS = 6 * 3600;

export function Settings() {
  const { t, i18n } = useTranslation();
  const [active, setActive] = useState<CategoryKey>("general");
  const [lastChecked] = useState<string>(() => formatDate(new Date()));

  return (
    <div className="settings-root">
      <header className="settings-topbar">
        <div className="settings-topbar-titles">
          <h2 className="settings-page-title">{t("screens.settings.title")}</h2>
          <p className="settings-page-subtitle">
            <span className="settings-version-label">
              {t("screens.settings.versionLabel")}
            </span>
            <span className="settings-version-num num">{APP_VERSION}</span>
            <span className="settings-version-sep" aria-hidden>
              ·
            </span>
            <span>
              {t("screens.settings.lastChecked", { when: lastChecked })}
            </span>
          </p>
        </div>
      </header>

      <div className="settings-shell">
        <aside
          className="settings-sidebar"
          aria-labelledby="settings-sidebar-heading"
        >
          <h3
            id="settings-sidebar-heading"
            className="settings-sidebar-heading"
          >
            {t("screens.settings.title")}
          </h3>
          <div
            className="settings-categories"
            role="radiogroup"
            aria-label={t("screens.settings.title")}
          >
            {CATEGORIES.map((key) => (
              <button
                key={key}
                type="button"
                role="radio"
                aria-checked={active === key}
                className={`settings-category${active === key ? " is-active" : ""}`}
                onClick={() => setActive(key)}
                data-testid={`settings-category-${key}`}
              >
                {t(`screens.settings.categories.${key}`)}
              </button>
            ))}
          </div>
        </aside>

        <main className="settings-main">
          {active === "general" && (
            <GeneralPanel
              currentLang={i18n.resolvedLanguage ?? "ko"}
              onChangeLanguage={(lng) => i18n.changeLanguage(lng)}
            />
          )}
          {active === "workspace" && <WorkspacePanel />}
          {active === "portable" && <PortablePanel />}
          {active === "catalog" && <CatalogPanel />}
          {active === "advanced" && (
            <AdvancedPanel version={APP_VERSION} commit={BUILD_COMMIT} />
          )}
        </main>
      </div>
    </div>
  );
}

// ── 일반 ────────────────────────────────────────────────────────────

interface GeneralPanelProps {
  currentLang: string;
  onChangeLanguage: (lng: string) => void;
}

function GeneralPanel({ currentLang, onChangeLanguage }: GeneralPanelProps) {
  const { t } = useTranslation();
  const [scanMin, setScanMin] = useState<ScanIntervalValue>(60);
  const [notifyOn, setNotifyOn] = useState<boolean>(false);

  // 첫 마운트 시 localStorage 로드.
  useEffect(() => {
    setScanMin(getScanInterval());
    setNotifyOn(getNotifyOnPhase5());
  }, []);

  const handleLangChange = useCallback(
    (lng: "ko" | "en") => {
      onChangeLanguage(lng);
    },
    [onChangeLanguage],
  );

  const handleScanChange = useCallback((next: ScanIntervalValue) => {
    setScanMin(next);
    writeScanInterval(next);
  }, []);

  return (
    <form
      className="settings-form"
      onSubmit={(e) => e.preventDefault()}
      aria-labelledby="settings-form-general"
    >
      <h3 id="settings-form-general" className="visually-hidden">
        {t("screens.settings.categories.general")}
      </h3>

      <fieldset className="settings-fieldset">
        <legend className="settings-legend">
          {t("screens.settings.general.language")}
        </legend>
        <div className="settings-radio-row" role="radiogroup">
          <SettingRadio
            name="language"
            value="ko"
            checked={currentLang === "ko"}
            onChange={() => handleLangChange("ko")}
            label={t("screens.settings.general.language.ko")}
          />
          <SettingRadio
            name="language"
            value="en"
            checked={currentLang === "en"}
            onChange={() => handleLangChange("en")}
            label={t("screens.settings.general.language.en")}
          />
        </div>
      </fieldset>

      <fieldset className="settings-fieldset">
        <legend className="settings-legend">
          {t("screens.settings.general.theme")}
        </legend>
        <div className="settings-radio-row" role="radiogroup">
          <SettingRadio
            name="theme"
            value="dark"
            checked
            label={t("screens.settings.general.theme.dark")}
          />
          <SettingRadio
            name="theme"
            value="light"
            checked={false}
            disabled
            label={t("screens.settings.general.theme.light")}
            iconRight={<LockIcon />}
          />
        </div>
      </fieldset>

      <fieldset className="settings-fieldset">
        <legend className="settings-legend">
          {t("screens.settings.general.scanInterval")}
        </legend>
        <div className="settings-radio-row" role="radiogroup">
          <SettingRadio
            name="scan_interval"
            value="0"
            checked={scanMin === 0}
            onChange={() => handleScanChange(0)}
            label={t("screens.settings.general.scanInterval.off")}
          />
          <SettingRadio
            name="scan_interval"
            value="15"
            checked={scanMin === 15}
            onChange={() => handleScanChange(15)}
            label={t("screens.settings.general.scanInterval.15m")}
          />
          <SettingRadio
            name="scan_interval"
            value="60"
            checked={scanMin === 60}
            onChange={() => handleScanChange(60)}
            label={t("screens.settings.general.scanInterval.60m")}
          />
        </div>
      </fieldset>

      <fieldset className="settings-fieldset">
        <legend className="settings-legend">
          {t("screens.settings.general.voice")}
        </legend>
        <div className="settings-toggle-row">
          <ToggleSwitch
            checked={notifyOn}
            onChange={() => {
              const next = !notifyOn;
              setNotifyOn(next);
              writeNotifyOnPhase5(next);
            }}
            disabled
            ariaLabel={t("screens.settings.general.voice")}
          />
          <span className="settings-coming-soon">
            {t("screens.settings.general.voice.comingSoon")}
          </span>
        </div>
      </fieldset>

      <AutoUpdatePanel />
      <PipelinesPanel />
      <TelemetryPanel />
    </form>
  );
}

// ── 자동 갱신 ────────────────────────────────────────────────────────

interface AutoUpdateOutdatedState {
  release: ReleaseInfo;
  currentVersion: string;
}

function AutoUpdatePanel() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<PollerStatus | null>(null);
  const [intervalSecs, setIntervalSecs] = useState<number>(DEFAULT_INTERVAL_SECS);
  const [channel, setChannel] = useState<UpdateChannel>("stable");
  const [busy, setBusy] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [outdated, setOutdated] = useState<AutoUpdateOutdatedState | null>(null);

  // 첫 마운트 시 상태 로드.
  useEffect(() => {
    let cancelled = false;
    setChannel(getUpdateChannel());
    getAutoUpdateStatus()
      .then((s) => {
        if (cancelled) return;
        setStatus(s);
        if (s.interval_secs && s.interval_secs > 0) {
          setIntervalSecs(s.interval_secs);
        }
      })
      .catch((e) => {
        console.warn("getAutoUpdateStatus failed:", e);
        if (!cancelled) setError("screens.settings.autoUpdate.errorStatus");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  /** 현재 채널에 맞는 release repo. */
  const activeRepo = channel === "beta" ? UPDATE_REPO_BETA : UPDATE_REPO;

  const handleEnable = useCallback(
    async (nextSecs: number) => {
      setBusy(true);
      setError(null);
      setInfo(null);
      try {
        await startAutoUpdatePoller(
          activeRepo,
          APP_VERSION,
          nextSecs,
          (ev: UpdateEvent) => {
            if (ev.kind === "outdated") {
              setOutdated({
                release: ev.latest,
                currentVersion: ev.current_version,
              });
            }
          },
        );
        const s = await getAutoUpdateStatus();
        setStatus(s);
        setIntervalSecs(nextSecs);
      } catch (e) {
        console.warn("startAutoUpdatePoller failed:", e);
        setError("screens.settings.autoUpdate.errorStart");
      } finally {
        setBusy(false);
      }
    },
    [activeRepo],
  );

  const handleDisable = useCallback(async () => {
    setBusy(true);
    setError(null);
    setInfo(null);
    try {
      await stopAutoUpdatePoller();
      const s = await getAutoUpdateStatus();
      setStatus(s);
    } catch (e) {
      console.warn("stopAutoUpdatePoller failed:", e);
      setError("screens.settings.autoUpdate.errorStop");
    } finally {
      setBusy(false);
    }
  }, []);

  const handleToggle = useCallback(() => {
    const isActive = status?.active ?? false;
    if (isActive) {
      void handleDisable();
    } else {
      void handleEnable(intervalSecs);
    }
  }, [status?.active, intervalSecs, handleEnable, handleDisable]);

  const handleIntervalChange = useCallback(
    (next: number) => {
      setIntervalSecs(next);
      // 활성 상태에서 interval 변경 시 재시작.
      if (status?.active) {
        void (async () => {
          setBusy(true);
          try {
            await stopAutoUpdatePoller();
            await startAutoUpdatePoller(
              activeRepo,
              APP_VERSION,
              next,
              (ev: UpdateEvent) => {
                if (ev.kind === "outdated") {
                  setOutdated({
                    release: ev.latest,
                    currentVersion: ev.current_version,
                  });
                }
              },
            );
            const s = await getAutoUpdateStatus();
            setStatus(s);
          } catch (e) {
            console.warn("interval change failed:", e);
            setError("screens.settings.autoUpdate.errorStart");
          } finally {
            setBusy(false);
          }
        })();
      }
    },
    [status?.active, activeRepo],
  );

  /** 채널 토글: stable ↔ beta. 활성 폴러가 있으면 새 repo로 재시작. */
  const handleChannelToggle = useCallback(() => {
    const next: UpdateChannel = channel === "beta" ? "stable" : "beta";
    setChannel(next);
    writeUpdateChannel(next);
    // 활성 상태면 새 repo로 폴러를 재시작.
    if (status?.active) {
      const nextRepo = next === "beta" ? UPDATE_REPO_BETA : UPDATE_REPO;
      void (async () => {
        setBusy(true);
        try {
          await stopAutoUpdatePoller();
          await startAutoUpdatePoller(
            nextRepo,
            APP_VERSION,
            intervalSecs,
            (ev: UpdateEvent) => {
              if (ev.kind === "outdated") {
                setOutdated({
                  release: ev.latest,
                  currentVersion: ev.current_version,
                });
              }
            },
          );
          const s = await getAutoUpdateStatus();
          setStatus(s);
        } catch (e) {
          console.warn("channel switch failed:", e);
          setError("screens.settings.autoUpdate.errorStart");
        } finally {
          setBusy(false);
        }
      })();
    }
  }, [channel, status?.active, intervalSecs]);

  const handleCheckNow = useCallback(async () => {
    setBusy(true);
    setError(null);
    setInfo(null);
    try {
      await checkForUpdate(activeRepo, APP_VERSION, (ev: UpdateEvent) => {
        if (ev.kind === "outdated") {
          setOutdated({
            release: ev.latest,
            currentVersion: ev.current_version,
          });
        } else if (ev.kind === "up-to-date") {
          setInfo("screens.settings.autoUpdate.upToDate");
        } else if (ev.kind === "failed") {
          setError(`screens.settings.autoUpdate.errorCheck::${ev.error}`);
        }
      });
    } catch (e) {
      console.warn("checkForUpdate failed:", e);
      setError("screens.settings.autoUpdate.errorCheck");
    } finally {
      setBusy(false);
    }
  }, [activeRepo]);

  const isActive = status?.active ?? false;
  const lastChecked = status?.last_check_iso ?? null;

  const errorText = useMemo(() => {
    if (!error) return null;
    const idx = error.indexOf("::");
    if (idx > 0) {
      const key = error.slice(0, idx);
      const detail = error.slice(idx + 2);
      return `${t(key)} (${detail})`;
    }
    return t(error);
  }, [error, t]);

  return (
    <fieldset className="settings-fieldset">
      <legend className="settings-legend">
        {t("screens.settings.autoUpdate.title")}
      </legend>

      <div className="settings-toggle-row">
        <ToggleSwitch
          checked={isActive}
          onChange={handleToggle}
          disabled={busy}
          ariaLabel={t("screens.settings.autoUpdate.toggleLabel")}
        />
        <span>
          {isActive
            ? t("screens.settings.autoUpdate.toggleOn")
            : t("screens.settings.autoUpdate.toggleOff")}
        </span>
      </div>

      <fieldset
        className="settings-fieldset"
        style={{ borderStyle: "dashed" }}
        disabled={!isActive}
      >
        <legend className="settings-legend">
          {t("screens.settings.autoUpdate.intervalLabel")}
        </legend>
        <div className="settings-radio-row" role="radiogroup">
          {INTERVAL_OPTIONS.map((opt) => (
            <SettingRadio
              key={opt.key}
              name="auto_update_interval"
              value={String(opt.secs)}
              checked={intervalSecs === opt.secs}
              onChange={isActive ? () => handleIntervalChange(opt.secs) : undefined}
              disabled={!isActive || busy}
              label={t(`screens.settings.autoUpdate.interval.${opt.key}`)}
            />
          ))}
        </div>
      </fieldset>

      <p className="settings-readonly-text">
        <span style={{ color: "var(--text-muted)" }}>
          {t("screens.settings.autoUpdate.repoLabel")}:{" "}
        </span>
        <code className="num">{activeRepo}</code>
      </p>

      {/* Phase 7'.b — 베타 채널 토글. */}
      <div
        className="settings-toggle-row"
        data-testid="settings-autoupdate-beta-row"
      >
        <ToggleSwitch
          checked={channel === "beta"}
          onChange={handleChannelToggle}
          disabled={busy}
          ariaLabel={t("screens.settings.autoUpdate.beta.toggleLabel")}
        />
        <span data-testid="settings-autoupdate-beta-status">
          {channel === "beta"
            ? t("screens.settings.autoUpdate.beta.statusOn")
            : t("screens.settings.autoUpdate.beta.statusOff")}
        </span>
      </div>
      <p className="settings-hint">
        {t("screens.settings.autoUpdate.beta.description")}
      </p>

      <p className="settings-hint">
        {lastChecked
          ? t("screens.settings.autoUpdate.lastChecked", { when: lastChecked })
          : t("screens.settings.autoUpdate.neverChecked")}
      </p>

      <button
        type="button"
        className="settings-btn-primary"
        onClick={handleCheckNow}
        disabled={busy}
        data-testid="settings-autoupdate-check-now"
      >
        {busy
          ? t("screens.settings.autoUpdate.checking")
          : t("screens.settings.autoUpdate.checkNow")}
      </button>

      {info && (
        <p className="settings-success" role="status" aria-live="polite">
          {t(info)}
        </p>
      )}
      {errorText && (
        <p className="settings-error" role="alert">
          {errorText}
        </p>
      )}

      {outdated && (
        <ToastUpdate
          release={outdated.release}
          currentVersion={outdated.currentVersion}
          onSkip={() => setOutdated(null)}
          onDismiss={() => setOutdated(null)}
        />
      )}
    </fieldset>
  );
}

// ── 워크스페이스 ─────────────────────────────────────────────────────

function WorkspacePanel() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<WorkspaceStatus | null>(null);
  const [repairResult, setRepairResult] = useState<string | null>(null);
  const [repairing, setRepairing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getWorkspaceFingerprint()
      .then((s) => {
        if (!cancelled) setStatus(s);
      })
      .catch((e) => {
        console.warn("getWorkspaceFingerprint failed:", e);
        if (!cancelled) setError("screens.settings.workspace.errorLoad");
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleRepair = useCallback(async () => {
    setRepairing(true);
    setError(null);
    setRepairResult(null);
    try {
      const r = await checkWorkspaceRepair();
      const tier = r.tier;
      // i18n key + opts JSON 패턴 — 렌더 시점에 t()로 변환.
      setRepairResult(
        JSON.stringify({
          key: "screens.settings.workspace.repairDone",
          opts: { tier, caches: r.invalidated_caches.length },
        }),
      );
    } catch (e) {
      console.warn("checkWorkspaceRepair failed:", e);
      setError("screens.settings.workspace.errorRepair");
    } finally {
      setRepairing(false);
    }
    // t는 의도적으로 deps 제외 — useTranslation 객체가 매 렌더 새 ref라.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // repairResult를 렌더 시점에 t()로 변환.
  const repairResultText = useMemo(() => {
    if (!repairResult) return null;
    try {
      const parsed = JSON.parse(repairResult) as {
        key: string;
        opts: Record<string, unknown>;
      };
      return t(parsed.key, parsed.opts);
    } catch {
      return repairResult;
    }
  }, [repairResult, t]);

  return (
    <form
      className="settings-form"
      onSubmit={(e) => e.preventDefault()}
      aria-labelledby="settings-form-workspace"
    >
      <h3 id="settings-form-workspace" className="visually-hidden">
        {t("screens.settings.categories.workspace")}
      </h3>

      <fieldset className="settings-fieldset">
        <legend className="settings-legend">
          {t("screens.settings.workspace.path")}
        </legend>
        <p className="settings-readonly-text">
          <code className="num">{status?.workspace_root ?? "…"}</code>
        </p>
        <button
          type="button"
          className="settings-btn-secondary"
          disabled
          aria-label={t("screens.settings.workspace.relocate")}
        >
          <span>{t("screens.settings.workspace.relocate")}</span>
          <span className="settings-coming-soon">
            {t("screens.settings.workspace.relocate.comingSoon")}
          </span>
        </button>
      </fieldset>

      <fieldset className="settings-fieldset">
        <legend className="settings-legend">
          {t("screens.settings.workspace.repair")}
        </legend>
        <button
          type="button"
          className="settings-btn-primary"
          onClick={handleRepair}
          disabled={repairing}
          data-testid="settings-workspace-repair-btn"
        >
          {repairing
            ? t("screens.settings.workspace.repairing")
            : t("screens.settings.workspace.repairButton")}
        </button>
        {repairResultText && (
          <p className="settings-success" role="status" aria-live="polite">
            {repairResultText}
          </p>
        )}
        {error && (
          <p className="settings-error" role="alert">
            {t(error)}
          </p>
        )}
      </fieldset>
    </form>
  );
}

// ── 포터블 이동 (Phase 11') ──────────────────────────────────────────

function PortablePanel() {
  const { t } = useTranslation();
  return (
    <form
      className="settings-form"
      onSubmit={(e) => e.preventDefault()}
      aria-labelledby="settings-form-portable"
    >
      <h3 id="settings-form-portable" className="visually-hidden">
        {t("screens.settings.categories.portable")}
      </h3>
      <div className="settings-portable-help-row">
        <HelpButton
          sectionId="portable"
          hint={t("screens.help.portable") ?? undefined}
          testId="settings-portable-help"
        />
      </div>
      <PortableExportPanel />
      <PortableImportPanel />
    </form>
  );
}

// ── 카탈로그 ─────────────────────────────────────────────────────────

function CatalogPanel() {
  const { t } = useTranslation();
  const lastUpdated = useMemo(
    () => formatDate(new Date()),
    // mock — v1은 카탈로그 최신 매니페스트 시각 placeholder.
    [],
  );
  return (
    <form
      className="settings-form"
      onSubmit={(e) => e.preventDefault()}
      aria-labelledby="settings-form-catalog"
    >
      <h3 id="settings-form-catalog" className="visually-hidden">
        {t("screens.settings.categories.catalog")}
      </h3>

      <fieldset className="settings-fieldset">
        <legend className="settings-legend">
          {t("screens.settings.catalog.registryUrl")}
        </legend>
        <input
          type="text"
          className="settings-input num"
          value={REGISTRY_URL}
          readOnly
          aria-readonly
          data-testid="settings-registry-url"
        />
        <p className="settings-hint">
          {t("screens.settings.catalog.registryHint")}
        </p>
      </fieldset>

      <fieldset className="settings-fieldset">
        <legend className="settings-legend">
          {t("screens.settings.catalog.lastUpdated")}
        </legend>
        <p className="settings-readonly-text num">{lastUpdated}</p>
      </fieldset>

      {/* Phase 1' integration — 자동 갱신 + 수동 트리거. */}
      <CatalogRefreshPanel />
    </form>
  );
}

// ── 고급 ────────────────────────────────────────────────────────────

interface AdvancedPanelProps {
  version: string;
  commit: string;
}

function AdvancedPanel({ version, commit }: AdvancedPanelProps) {
  const { t } = useTranslation();
  const sqlcipher = getEncryptDbHint();
  return (
    <form
      className="settings-form"
      onSubmit={(e) => e.preventDefault()}
      aria-labelledby="settings-form-advanced"
    >
      <h3 id="settings-form-advanced" className="visually-hidden">
        {t("screens.settings.categories.advanced")}
      </h3>

      <fieldset className="settings-fieldset">
        <legend className="settings-legend">
          {t("screens.settings.advanced.gemini")}
        </legend>
        <div className="settings-toggle-row">
          <ToggleSwitch
            checked={false}
            onChange={() => {}}
            disabled
            ariaLabel={t("screens.settings.advanced.gemini")}
          />
          <span className="settings-coming-soon">
            {t("screens.settings.advanced.gemini.comingSoon")}
          </span>
        </div>
      </fieldset>

      <fieldset className="settings-fieldset">
        <legend className="settings-legend">
          {t("screens.settings.advanced.sqlcipher")}
        </legend>
        <p className="settings-readonly-text">
          <code className="num">
            LMMASTER_ENCRYPT_DB={sqlcipher ? "1" : "0"}
          </code>
        </p>
        <p className="settings-hint">
          {t("screens.settings.advanced.sqlcipher.envHint")}
        </p>
      </fieldset>

      <fieldset className="settings-fieldset">
        <legend className="settings-legend">
          {t("screens.settings.advanced.exportLogs")}
        </legend>
        <button
          type="button"
          className="settings-btn-secondary"
          disabled
          aria-label={t("screens.settings.advanced.exportLogs")}
        >
          <span>{t("screens.settings.advanced.exportLogs")}</span>
          <span className="settings-coming-soon">
            {t("screens.settings.advanced.exportLogs.comingSoon")}
          </span>
        </button>
      </fieldset>

      {/* Phase 8'.0.c — Workbench artifact retention */}
      <WorkbenchArtifactPanel />

      <fieldset className="settings-fieldset">
        <legend className="settings-legend">
          {t("screens.settings.advanced.buildInfo")}
        </legend>
        <dl className="settings-build-info">
          <div className="settings-build-info-row">
            <dt>{t("screens.settings.versionLabel")}</dt>
            <dd className="num">{version}</dd>
          </div>
          <div className="settings-build-info-row">
            <dt>commit</dt>
            <dd className="num">{commit}</dd>
          </div>
          <div className="settings-build-info-row">
            <dt>{t("screens.settings.advanced.publisher")}</dt>
            <dd>MOJITO Lab</dd>
          </div>
          <div className="settings-build-info-row">
            <dt>{t("screens.settings.advanced.copyright")}</dt>
            <dd>{t("screens.settings.advanced.copyrightValue")}</dd>
          </div>
        </dl>
      </fieldset>
    </form>
  );
}

// ── 부속 ─────────────────────────────────────────────────────────────

interface SettingRadioProps {
  name: string;
  value: string;
  checked: boolean;
  onChange?: () => void;
  disabled?: boolean;
  label: string;
  iconRight?: ReactNode;
}

function SettingRadio({
  name,
  value,
  checked,
  onChange,
  disabled,
  label,
  iconRight,
}: SettingRadioProps) {
  return (
    <label
      className={`settings-radio${disabled ? " is-disabled" : ""}${checked ? " is-checked" : ""}`}
    >
      <input
        type="radio"
        name={name}
        value={value}
        checked={checked}
        disabled={disabled}
        // onChange 없으면 readOnly — React 경고 회피.
        readOnly={!onChange}
        onChange={onChange ?? (() => {})}
      />
      <span className="settings-radio-label">{label}</span>
      {iconRight && (
        <span className="settings-radio-icon" aria-hidden>
          {iconRight}
        </span>
      )}
    </label>
  );
}

interface ToggleSwitchProps {
  checked: boolean;
  onChange: () => void;
  disabled?: boolean;
  ariaLabel: string;
}

function ToggleSwitch({
  checked,
  onChange,
  disabled,
  ariaLabel,
}: ToggleSwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel}
      aria-disabled={disabled}
      disabled={disabled}
      className={`settings-toggle${checked ? " is-on" : ""}${disabled ? " is-disabled" : ""}`}
      onClick={onChange}
    >
      <span className="settings-toggle-track" aria-hidden>
        <span className="settings-toggle-thumb" />
      </span>
    </button>
  );
}

function LockIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
      <rect
        x="2.5"
        y="6"
        width="9"
        height="6"
        rx="1.2"
        stroke="currentColor"
        strokeWidth="1.2"
      />
      <path
        d="M4.5 6V4.2C4.5 2.85 5.62 1.75 7 1.75C8.38 1.75 9.5 2.85 9.5 4.2V6"
        stroke="currentColor"
        strokeWidth="1.2"
        strokeLinecap="round"
      />
    </svg>
  );
}

function formatDate(d: Date): string {
  // YYYY-MM-DD — UI 단계 단순. v1.x 한국어 상대시각.
  const yyyy = d.getFullYear();
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  const dd = String(d.getDate()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd}`;
}
