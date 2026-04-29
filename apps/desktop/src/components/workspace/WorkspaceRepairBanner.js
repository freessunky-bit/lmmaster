import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// WorkspaceRepairBanner — yellow/red tier 감지 시 사용자에게 안내.
//
// 정책 (ADR-0022 §8):
// - green: 아무 표시 없음 (banner 자체가 unmount).
// - yellow: 토스트 — "벤치 캐시를 새로 측정할게요" + "확인" 버튼.
// - red: 모달 — "다른 OS에서 가져온 워크스페이스" + "런타임 다시 설치" 안내 + "확인" 버튼.
// - 사용자가 확인하면 check_workspace_repair 호출 → silent 동작 + green으로 전환.
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { checkWorkspaceRepair, getWorkspaceFingerprint, } from "../../ipc/workspace";
export function WorkspaceRepairBanner() {
    const { t } = useTranslation();
    const [status, setStatus] = useState(null);
    const [confirming, setConfirming] = useState(false);
    const [report, setReport] = useState(null);
    const [dismissed, setDismissed] = useState(false);
    // 초기 로드 — green이면 banner unmount, yellow/red면 표시.
    useEffect(() => {
        let cancelled = false;
        getWorkspaceFingerprint()
            .then((s) => {
            if (cancelled)
                return;
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
        }
        catch (e) {
            console.warn("checkWorkspaceRepair failed:", e);
        }
        finally {
            setConfirming(false);
        }
    }, []);
    if (dismissed || !status || status.tier === "green") {
        // 적용 결과만 잠깐 토스트로 보여주는 micro UX는 v1.x.
        return report ? (_jsxs("div", { className: "ws-banner ws-banner-success", role: "status", children: [_jsx("span", { className: "ws-banner-text", children: t("workspace.repair.applied", {
                        caches: report.invalidated_caches.join(", ") || "—",
                    }) }), _jsx("button", { type: "button", className: "ws-banner-action", onClick: () => setReport(null), children: t("workspace.repair.close") })] })) : null;
    }
    if (status.tier === "yellow") {
        return (_jsxs("div", { className: "ws-banner ws-banner-yellow", role: "status", "data-testid": "ws-banner-yellow", children: [_jsx("span", { className: "ws-banner-text", children: t("workspace.repair.yellow") }), _jsxs("div", { className: "ws-banner-actions", children: [_jsx("button", { type: "button", className: "ws-banner-action-secondary", onClick: () => setDismissed(true), children: t("workspace.repair.later") }), _jsx("button", { type: "button", className: "ws-banner-action", onClick: handleConfirm, disabled: confirming, children: t("workspace.repair.applyYellow") })] })] }));
    }
    // red — 더 강한 모달 형식.
    return (_jsx("div", { className: "ws-modal-backdrop", role: "presentation", "data-testid": "ws-modal-red", children: _jsxs("div", { className: "ws-modal", role: "dialog", "aria-modal": "true", "aria-labelledby": "ws-modal-title", children: [_jsx("header", { className: "ws-modal-header", children: _jsx("h3", { id: "ws-modal-title", className: "ws-modal-title", children: t("workspace.repair.redTitle") }) }), _jsxs("div", { className: "ws-modal-body", children: [_jsx("p", { className: "ws-modal-text", children: t("workspace.repair.redBody") }), status.previous && (_jsx("p", { className: "ws-modal-detail", children: t("workspace.repair.redDetail", {
                                prevOs: status.previous.os,
                                currentOs: status.fingerprint.os,
                            }) })), _jsx("p", { className: "ws-modal-text", children: t("workspace.repair.redModelsPreserved") })] }), _jsx("footer", { className: "ws-modal-footer", children: _jsx("button", { type: "button", className: "ws-banner-action", onClick: handleConfirm, disabled: confirming, children: t("workspace.repair.applyRed") }) })] }) }));
}
