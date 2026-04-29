import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// PortableExportPanel — Phase 11' (ADR-0039).
//
// 정책:
// - "이 워크스페이스 내보낼게요" 버튼 → 옵션 dialog (모델 포함 / 키 포함 + 패스프레이즈) → 진행률.
// - 옵션 default: include_models=false, include_keys=false (사용자 명시 opt-in).
// - 진행률은 ExportEvent stream → progress bar + 현재 파일명 + cancel 버튼.
// - 완료 시 toast + sha256 + 사이즈 + target 경로.
// - a11y: dialog role + aria-modal + Esc / 배경 클릭 닫기 (옵션 단계만, 진행 중에는 cancel만).
// - 한국어 카피 해요체.
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { cancelWorkspaceExport, isTerminalExportEvent, startWorkspaceExport, } from "../../ipc/portable";
import "./portable.css";
/** byte → 한국어 친화적 사이즈 표기. */
function formatBytes(bytes) {
    if (!Number.isFinite(bytes) || bytes <= 0)
        return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    let v = bytes;
    let idx = 0;
    while (v >= 1024 && idx < units.length - 1) {
        v /= 1024;
        idx += 1;
    }
    return `${idx === 0 ? Math.round(v) : v.toFixed(1)} ${units[idx]}`;
}
export function PortableExportPanel() {
    const { t } = useTranslation();
    const [phase, setPhase] = useState("idle");
    const [includeModels, setIncludeModels] = useState(false);
    const [includeKeys, setIncludeKeys] = useState(false);
    const [passphrase, setPassphrase] = useState("");
    const [targetPath, setTargetPath] = useState("");
    const [progress, setProgress] = useState({ processed: 0, total: 0 });
    const [currentPath, setCurrentPath] = useState("");
    const [error, setError] = useState(null);
    const [done, setDone] = useState(null);
    const exportIdRef = useRef(null);
    const dialogCloseRef = useRef(null);
    const targetInputRef = useRef(null);
    const isOpen = phase === "options" || phase === "running";
    // 옵션 dialog 진입 시 첫 input에 포커스.
    useEffect(() => {
        if (phase === "options") {
            targetInputRef.current?.focus();
        }
        else if (phase === "running") {
            dialogCloseRef.current?.focus();
        }
    }, [phase]);
    // Esc 키 닫기 (옵션 단계만).
    useEffect(() => {
        const onKey = (e) => {
            if (e.key === "Escape" && phase === "options") {
                setPhase("idle");
            }
        };
        window.addEventListener("keydown", onKey);
        return () => window.removeEventListener("keydown", onKey);
    }, [phase]);
    const reset = useCallback(() => {
        setPhase("idle");
        setIncludeModels(false);
        setIncludeKeys(false);
        setPassphrase("");
        setTargetPath("");
        setProgress({ processed: 0, total: 0 });
        setCurrentPath("");
        setError(null);
        setDone(null);
        exportIdRef.current = null;
    }, []);
    const handleStart = useCallback(async () => {
        setError(null);
        if (targetPath.trim().length === 0) {
            setError("screens.settings.portable.export.errors.emptyTarget");
            return;
        }
        if (includeKeys && passphrase.trim().length === 0) {
            setError("screens.settings.portable.export.errors.emptyPassphrase");
            return;
        }
        const opts = {
            include_models: includeModels,
            include_keys: includeKeys,
            key_passphrase: includeKeys ? passphrase : null,
            target_path: targetPath.trim(),
        };
        setPhase("running");
        setProgress({ processed: 0, total: 0 });
        try {
            const res = await startWorkspaceExport(opts, (ev) => {
                if (ev.kind === "counting") {
                    setProgress({ processed: 0, total: Number(ev.total_files) });
                }
                else if (ev.kind === "compressing") {
                    setProgress({
                        processed: Number(ev.processed),
                        total: Number(ev.total),
                    });
                    setCurrentPath(ev.current_path);
                }
                else if (ev.kind === "done") {
                    setDone({
                        sha256: ev.sha256,
                        archiveSizeBytes: Number(ev.archive_size_bytes),
                        targetPath: ev.target_path,
                    });
                }
                else if (ev.kind === "failed") {
                    setError(`screens.settings.portable.export.errors.runner::${ev.error}`);
                }
                if (isTerminalExportEvent(ev)) {
                    // 별도 처리 — invoke의 await에서 추가 응답이 옴.
                }
            });
            exportIdRef.current = res.export_id;
            setDone({
                sha256: res.summary.sha256,
                archiveSizeBytes: Number(res.summary.archive_size_bytes),
                targetPath: opts.target_path,
            });
            setPhase("done");
        }
        catch (e) {
            console.warn("startWorkspaceExport failed:", e);
            const msg = e && typeof e === "object" && "message" in e
                ? `screens.settings.portable.export.errors.runner::${e.message}`
                : "screens.settings.portable.export.errors.start";
            setError(msg);
            setPhase("failed");
        }
    }, [includeKeys, includeModels, passphrase, targetPath]);
    const handleCancel = useCallback(async () => {
        if (!exportIdRef.current) {
            setPhase("idle");
            return;
        }
        try {
            await cancelWorkspaceExport(exportIdRef.current);
        }
        catch (e) {
            console.warn("cancelWorkspaceExport failed:", e);
        }
    }, []);
    const errorText = (() => {
        if (!error)
            return null;
        const idx = error.indexOf("::");
        if (idx > 0) {
            const key = error.slice(0, idx);
            const detail = error.slice(idx + 2);
            return `${t(key)} (${detail})`;
        }
        return t(error);
    })();
    const percent = progress.total > 0
        ? Math.min(100, Math.round((progress.processed / progress.total) * 100))
        : 0;
    return (_jsxs("fieldset", { className: "settings-fieldset", "data-testid": "portable-export-panel", children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.portable.export.title") }), _jsx("p", { className: "settings-hint", children: t("screens.settings.portable.export.subtitle") }), _jsx("button", { type: "button", className: "settings-btn-primary", onClick: () => setPhase("options"), disabled: isOpen, "data-testid": "portable-export-start-btn", children: t("screens.settings.portable.export.start") }), phase === "done" && done && (_jsxs("div", { className: "portable-result-card", role: "status", "aria-live": "polite", "data-testid": "portable-export-done", children: [_jsx("p", { className: "portable-result-headline", children: t("screens.settings.portable.export.done") }), _jsxs("dl", { className: "portable-result-meta", children: [_jsxs("div", { className: "portable-result-row", children: [_jsx("dt", { children: t("screens.settings.portable.export.targetPath") }), _jsx("dd", { className: "num", children: done.targetPath })] }), _jsxs("div", { className: "portable-result-row", children: [_jsx("dt", { children: t("screens.settings.portable.export.archiveSize") }), _jsx("dd", { className: "num", children: formatBytes(done.archiveSizeBytes) })] }), _jsxs("div", { className: "portable-result-row", children: [_jsx("dt", { children: "sha256" }), _jsx("dd", { className: "num portable-sha", children: done.sha256 })] })] }), _jsx("button", { type: "button", className: "settings-btn-secondary", onClick: reset, "data-testid": "portable-export-reset-btn", children: t("screens.settings.portable.export.again") })] })), phase === "failed" && errorText && (_jsx("p", { className: "settings-error", role: "alert", children: errorText })), isOpen && (_jsx("div", { className: "portable-modal-backdrop", role: "presentation", onClick: () => phase === "options" && setPhase("idle"), children: _jsxs("div", { className: "portable-modal", role: "dialog", "aria-modal": "true", "aria-labelledby": "portable-export-modal-title", onClick: (e) => e.stopPropagation(), "data-testid": "portable-export-modal", children: [_jsx("header", { className: "portable-modal-header", children: _jsx("h3", { id: "portable-export-modal-title", className: "portable-modal-title", children: phase === "options"
                                    ? t("screens.settings.portable.export.dialogTitle")
                                    : t("screens.settings.portable.export.runningTitle") }) }), phase === "options" && (_jsxs("div", { className: "portable-modal-body", children: [_jsxs("label", { className: "portable-input-field", children: [_jsx("span", { children: t("screens.settings.portable.export.targetPath") }), _jsx("input", { ref: targetInputRef, type: "text", className: "settings-input num", value: targetPath, onChange: (e) => setTargetPath(e.target.value), placeholder: "C:\\\\Users\\\\me\\\\Desktop\\\\workspace.zip", "data-testid": "portable-export-target-input" }), _jsx("small", { className: "settings-hint", children: t("screens.settings.portable.export.targetHint") })] }), _jsxs("label", { className: "portable-checkbox-row", children: [_jsx("input", { type: "checkbox", checked: includeModels, onChange: (e) => setIncludeModels(e.target.checked), "data-testid": "portable-export-include-models" }), _jsxs("span", { children: [_jsx("strong", { children: t("screens.settings.portable.export.includeModels") }), _jsx("small", { className: "settings-hint", children: t("screens.settings.portable.export.includeModelsHint") })] })] }), _jsxs("label", { className: "portable-checkbox-row", children: [_jsx("input", { type: "checkbox", checked: includeKeys, onChange: (e) => {
                                                setIncludeKeys(e.target.checked);
                                                if (!e.target.checked)
                                                    setPassphrase("");
                                            }, "data-testid": "portable-export-include-keys" }), _jsxs("span", { children: [_jsx("strong", { children: t("screens.settings.portable.export.includeKeys") }), _jsx("small", { className: "settings-hint", children: t("screens.settings.portable.export.includeKeysHint") })] })] }), includeKeys && (_jsxs("label", { className: "portable-input-field", children: [_jsx("span", { children: t("screens.settings.portable.export.passphraseLabel") }), _jsx("input", { type: "password", className: "settings-input", value: passphrase, onChange: (e) => setPassphrase(e.target.value), autoComplete: "new-password", "data-testid": "portable-export-passphrase" }), _jsx("small", { className: "settings-hint", children: t("screens.settings.portable.export.passphraseHint") })] })), errorText && (_jsx("p", { className: "settings-error", role: "alert", children: errorText })), _jsxs("div", { className: "portable-modal-footer", children: [_jsx("button", { type: "button", className: "settings-btn-secondary", onClick: () => setPhase("idle"), "data-testid": "portable-export-dialog-close", children: t("screens.settings.portable.export.cancel") }), _jsx("button", { type: "button", className: "settings-btn-primary", onClick: handleStart, "data-testid": "portable-export-confirm-btn", children: t("screens.settings.portable.export.confirm") })] })] })), phase === "running" && (_jsxs("div", { className: "portable-modal-body", children: [_jsx("p", { className: "portable-running-message", children: t("screens.settings.portable.export.running") }), _jsx("div", { className: "portable-progress", role: "progressbar", "aria-valuenow": percent, "aria-valuemin": 0, "aria-valuemax": 100, "data-testid": "portable-export-progress", children: _jsx("div", { className: "portable-progress-fill", style: { width: `${percent}%` } }) }), _jsxs("p", { className: "portable-progress-meta num", children: [progress.processed, " / ", progress.total, " \u00B7", " ", percent, "%"] }), currentPath && (_jsx("p", { className: "portable-progress-current num", "data-testid": "portable-export-current", children: currentPath })), _jsx("div", { className: "portable-modal-footer", children: _jsx("button", { ref: dialogCloseRef, type: "button", className: "settings-btn-secondary", onClick: handleCancel, "data-testid": "portable-export-cancel-btn", children: t("screens.settings.portable.export.cancel") }) })] }))] }) }))] }));
}
