// WorkspaceRepairBanner — yellow/red tier 감지 시 사용자에게 안내.
//
// 정책 (ADR-0022 §8):
// - green: 아무 표시 없음 (banner 자체가 unmount).
// - yellow: 토스트 — "벤치 캐시를 새로 측정할게요" + "확인" 버튼.
// - red: 모달 — "다른 OS에서 가져온 워크스페이스" + "런타임 다시 설치" 안내 + "확인" 버튼.
// - 사용자가 확인하면 check_workspace_repair 호출 → silent 동작 + green으로 전환.

import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  checkWorkspaceRepair,
  getWorkspaceFingerprint,
  type RepairReport,
  type WorkspaceStatus,
} from "../../ipc/workspace";

export function WorkspaceRepairBanner() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<WorkspaceStatus | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [report, setReport] = useState<RepairReport | null>(null);
  const [dismissed, setDismissed] = useState(false);

  // 초기 로드 — green이면 banner unmount, yellow/red면 표시.
  useEffect(() => {
    let cancelled = false;
    getWorkspaceFingerprint()
      .then((s) => {
        if (cancelled) return;
        setStatus(s);
      })
      .catch((e) => {
        console.warn("getWorkspaceFingerprint failed:", e);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const handleConfirm = useCallback(async () => {
    setConfirming(true);
    try {
      const r = await checkWorkspaceRepair();
      setReport(r);
      // green으로 전환 — banner unmount.
      setStatus((prev) => (prev ? { ...prev, tier: "green" } : prev));
    } catch (e) {
      console.warn("checkWorkspaceRepair failed:", e);
    } finally {
      setConfirming(false);
    }
  }, []);

  if (dismissed || !status || status.tier === "green") {
    // 적용 결과만 잠깐 토스트로 보여주는 micro UX는 v1.x.
    return report ? (
      <div className="ws-banner ws-banner-success" role="status">
        <span className="ws-banner-text">
          {t("workspace.repair.applied", {
            caches: report.invalidated_caches.join(", ") || "—",
          })}
        </span>
        <button
          type="button"
          className="ws-banner-action"
          onClick={() => setReport(null)}
        >
          {t("workspace.repair.close")}
        </button>
      </div>
    ) : null;
  }

  if (status.tier === "yellow") {
    return (
      <div className="ws-banner ws-banner-yellow" role="status" data-testid="ws-banner-yellow">
        <span className="ws-banner-text">{t("workspace.repair.yellow")}</span>
        <div className="ws-banner-actions">
          <button
            type="button"
            className="ws-banner-action-secondary"
            onClick={() => setDismissed(true)}
          >
            {t("workspace.repair.later")}
          </button>
          <button
            type="button"
            className="ws-banner-action"
            onClick={handleConfirm}
            disabled={confirming}
          >
            {t("workspace.repair.applyYellow")}
          </button>
        </div>
      </div>
    );
  }

  // red — 더 강한 모달 형식.
  return (
    <div
      className="ws-modal-backdrop"
      role="presentation"
      data-testid="ws-modal-red"
    >
      <div
        className="ws-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="ws-modal-title"
      >
        <header className="ws-modal-header">
          <h3 id="ws-modal-title" className="ws-modal-title">
            {t("workspace.repair.redTitle")}
          </h3>
        </header>
        <div className="ws-modal-body">
          <p className="ws-modal-text">{t("workspace.repair.redBody")}</p>
          {status.previous && (
            <p className="ws-modal-detail">
              {t("workspace.repair.redDetail", {
                prevOs: status.previous.os,
                currentOs: status.fingerprint.os,
              })}
            </p>
          )}
          <p className="ws-modal-text">{t("workspace.repair.redModelsPreserved")}</p>
        </div>
        <footer className="ws-modal-footer">
          <button
            type="button"
            className="ws-banner-action"
            onClick={handleConfirm}
            disabled={confirming}
          >
            {t("workspace.repair.applyRed")}
          </button>
        </footer>
      </div>
    </div>
  );
}
