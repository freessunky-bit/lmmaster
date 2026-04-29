// ToastUpdate — JetBrains Toolbox 스타일 자동 갱신 토스트.
//
// 정책 (Phase 6'.b 자동 갱신 UI):
// - 우하단 fixed. backdrop fade + slide-up — design-system tokens.css가 처리.
// - 자동 닫힘 없음 — 사용자가 명시 닫기 / "이번 버전은 건너뛰어요" 클릭.
// - "이번 버전은 건너뛰어요"는 localStorage `lmmaster.update.skipped.{version}=true` 저장.
// - 같은 version key가 localStorage에 이미 있으면 마운트 즉시 onDismiss + null 반환.
// - 한국어 해요체. published_at은 한국어 날짜 포맷 (YYYY년 M월 D일).
// - "업데이트 보기"는 release URL을 외부 브라우저로 — Tauri 2 plugin-shell `open` (Phase 8'.b.2).
// - Esc 키 → onDismiss. 첫 마운트 시 close 버튼에 focus.
// - role="status" aria-live="polite" — 새 알림 등장 시 스크린리더 통보.
// - Phase 8'.a.2 — 새 버전 도착 시 이전(낮은) 버전 skipped 키를 LRU 정리.

import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { open as openExternal } from "@tauri-apps/plugin-shell";

import type { ReleaseInfo } from "../ipc/updater";

const SKIP_KEY_PREFIX = "lmmaster.update.skipped.";

/** localStorage 안전 read — Tauri 빌드 / jsdom 모두에서 try/catch. */
function isVersionSkipped(version: string): boolean {
  try {
    return globalThis.localStorage?.getItem(SKIP_KEY_PREFIX + version) === "true";
  } catch {
    return false;
  }
}

function persistSkip(version: string): void {
  try {
    globalThis.localStorage?.setItem(SKIP_KEY_PREFIX + version, "true");
  } catch (e) {
    console.warn("ToastUpdate skip persist failed:", e);
  }
}

/**
 * 간이 semver compare — `major.minor.patch[-prerelease]`.
 *
 * Phase 8'.a.2 LRU cleanup용. v 부착 / pre-release / build metadata는 무시(prefix `v` 한정 strip).
 * 잘못된 형식은 NaN 처리 후 fallback string compare.
 *
 * 반환: a < b → 음수, a == b → 0, a > b → 양수.
 */
export function compareVersion(a: string, b: string): number {
  const stripV = (s: string) => (s.startsWith("v") || s.startsWith("V") ? s.slice(1) : s);
  // pre-release / build 메타 무시 — `1.2.3-rc1+build` → `1.2.3`.
  const core = (s: string) => stripV(s).split(/[-+]/)[0] ?? "";
  const partsOf = (s: string): [number, number, number] => {
    const segs = core(s).split(".");
    const part = (i: number): number => {
      const n = Number(segs[i] ?? "0");
      return Number.isFinite(n) ? n : 0;
    };
    return [part(0), part(1), part(2)];
  };
  const pa = partsOf(a);
  const pb = partsOf(b);
  for (let i = 0; i < 3; i++) {
    const da = pa[i] as number;
    const db = pb[i] as number;
    if (da !== db) return da - db;
  }
  // major.minor.patch가 동률이면 stripV된 string으로 fallback compare
  // (pre-release 등 차이를 안정적으로 정렬). v 접두사는 정렬 영향 없음.
  const sa = stripV(a);
  const sb = stripV(b);
  return sa === sb ? 0 : sa < sb ? -1 : 1;
}

/**
 * 도착한 새 버전(`latest`)보다 낮은 모든 `lmmaster.update.skipped.*` 키를 정리.
 *
 * 정책 (Phase 8'.a.2):
 * - `lmmaster.update.skipped.` prefix가 정확히 일치하는 키만 정리 (다른 모듈의 키와 충돌 방지).
 * - latest와 같거나 큰 키는 유지 — downgrade / 사이드그레이드는 보존.
 * - `localStorage` 미접근 환경(headless Tauri / jsdom 일부)에선 silently no-op.
 */
export function cleanupOlderSkippedVersions(latest: string): void {
  try {
    const ls = globalThis.localStorage;
    if (!ls) return;
    const keys: string[] = [];
    for (let i = 0; i < ls.length; i++) {
      const k = ls.key(i);
      if (k && k.startsWith(SKIP_KEY_PREFIX)) {
        keys.push(k);
      }
    }
    for (const k of keys) {
      const ver = k.slice(SKIP_KEY_PREFIX.length);
      if (compareVersion(ver, latest) < 0) {
        ls.removeItem(k);
      }
    }
  } catch (e) {
    console.warn("ToastUpdate skipped LRU cleanup 실패:", e);
  }
}

/** YYYY년 M월 D일 형식. ISO 파싱 실패 시 원본 string 노출. */
function formatPublishedKo(iso: string): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const yyyy = d.getFullYear();
  const m = d.getMonth() + 1;
  const dd = d.getDate();
  return `${yyyy}년 ${m}월 ${dd}일`;
}

export interface ToastUpdateProps {
  /** 새 release 메타. */
  release: ReleaseInfo;
  /** 현재 사용 중인 버전 — 부제목 노출용. */
  currentVersion: string;
  /** "이번 버전은 건너뛰어요" 클릭 — localStorage 저장 후 호출. */
  onSkip(version: string): void;
  /** 닫기 (X 버튼) 또는 Esc — 단순 dismiss. localStorage 미저장. */
  onDismiss(): void;
}

export function ToastUpdate({
  release,
  currentVersion,
  onSkip,
  onDismiss,
}: ToastUpdateProps) {
  const { t } = useTranslation();
  const closeBtnRef = useRef<HTMLButtonElement | null>(null);

  // skipped 버전이면 마운트 즉시 dismiss — render 자체를 스킵.
  const skipped = isVersionSkipped(release.version);
  useEffect(() => {
    if (skipped) {
      onDismiss();
    }
    // skipped는 release.version 기준 deterministic.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [skipped, release.version]);

  // Phase 8'.a.2 — 새 버전 도착 시 이전(낮은) 버전 skipped 키 LRU 정리.
  // 사용자가 1.0.0을 skip → 1.1.0 도착 → 1.0.0 키 삭제.
  // skipped이거나 무관한 버전이라도 cleanup은 안전하게 실행 (idempotent).
  useEffect(() => {
    cleanupOlderSkippedVersions(release.version);
  }, [release.version]);

  // 마운트 시 close 버튼 focus — 키보드 네비게이션 진입.
  useEffect(() => {
    if (!skipped) {
      closeBtnRef.current?.focus();
    }
  }, [skipped]);

  // Esc → dismiss.
  useEffect(() => {
    if (skipped) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onDismiss();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [skipped, onDismiss]);

  if (skipped) return null;

  const handleSkip = () => {
    persistSkip(release.version);
    onSkip(release.version);
  };

  const handleViewUpdate = () => {
    if (!release.url) return;
    // Phase 8'.b.2 — Tauri 2 plugin-shell `open`. capability `shell:allow-open` + URL scope 필요.
    // capabilities/main.json이 https://** 스코프를 부여하므로 GitHub release URL은 통과.
    void openExternal(release.url).catch((e) => {
      console.warn("update url open failed:", e);
    });
  };

  const publishedKo = formatPublishedKo(release.published_at_iso);

  return (
    <div
      className="toast-update"
      role="status"
      aria-live="polite"
      data-testid="toast-update"
    >
      <div className="toast-update-body">
        <span className="toast-update-icon" aria-hidden>
          ◆
        </span>
        <div className="toast-update-text">
          <div className="toast-update-title">
            {t("toast.update.title", { version: release.version })}
          </div>
          <div className="toast-update-detail">
            {t("toast.update.detail", {
              currentVersion,
              published: publishedKo,
            })}
          </div>
        </div>
      </div>
      <div className="toast-update-actions">
        <button
          type="button"
          className="onb-button onb-button-primary toast-update-cta"
          onClick={handleViewUpdate}
          data-testid="toast-update-view"
          disabled={!release.url}
        >
          {t("toast.update.view")}
        </button>
        <button
          type="button"
          className="onb-button onb-button-ghost"
          onClick={handleSkip}
          data-testid="toast-update-skip"
        >
          {t("toast.update.skip")}
        </button>
        <button
          type="button"
          className="toast-update-close"
          onClick={onDismiss}
          ref={closeBtnRef}
          aria-label={t("toast.update.close") ?? undefined}
          data-testid="toast-update-close"
        >
          ✕
        </button>
      </div>
    </div>
  );
}
