import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// PortableImportPanel — Phase 11' (ADR-0039).
//
// 정책:
// - "워크스페이스 가져올게요" 버튼 → 파일 경로 입력 → 검증(verify_archive) → preview ("어떤 PC, 언제, NN GB").
// - 패스프레이즈 입력 (preview.has_keys=true 시).
// - conflict_policy 라디오 (skip / overwrite / rename).
// - "가져올게요" 버튼 → 진행률 stream.
// - 완료 후 repair_tier 안내 (green / yellow / red).
// - a11y: dialog role + aria-modal + Esc / 배경 클릭 닫기 (옵션 단계만).
// - 한국어 카피 해요체.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { cancelWorkspaceImport, isTerminalImportEvent, startWorkspaceImport, verifyWorkspaceArchive, } from "../../ipc/portable";
import "./portable.css";
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
export function PortableImportPanel() {
    const { t } = useTranslation();
    const [phase, setPhase] = useState("idle");
    const [sourcePath, setSourcePath] = useState("");
    const [preview, setPreview] = useState(null);
    const [verifying, setVerifying] = useState(false);
    const [passphrase, setPassphrase] = useState("");
    const [conflictPolicy, setConflictPolicy] = useState("rename");
    const [progress, setProgress] = useState({ processed: 0, total: 0 });
    const [error, setError] = useState(null);
    const [done, setDone] = useState(null);
    const importIdRef = useRef(null);
    const sourceInputRef = useRef(null);
    const cancelBtnRef = useRef(null);
    const isOpen = phase === "preview" ||
        phase === "options" ||
        phase === "running";
    useEffect(() => {
        if (phase === "preview") {
            sourceInputRef.current?.focus();
        }
        else if (phase === "running") {
            cancelBtnRef.current?.focus();
        }
    }, [phase]);
    useEffect(() => {
        const onKey = (e) => {
            if (e.key === "Escape" &&
                (phase === "preview" || phase === "options")) {
                setPhase("idle");
            }
        };
        window.addEventListener("keydown", onKey);
        return () => window.removeEventListener("keydown", onKey);
    }, [phase]);
    const reset = useCallback(() => {
        setPhase("idle");
        setSourcePath("");
        setPreview(null);
        setVerifying(false);
        setPassphrase("");
        setConflictPolicy("rename");
        setProgress({ processed: 0, total: 0 });
        setError(null);
        setDone(null);
        importIdRef.current = null;
    }, []);
    const handleVerify = useCallback(async () => {
        setError(null);
        if (sourcePath.trim().length === 0) {
            setError("screens.settings.portable.import.errors.emptySource");
            return;
        }
        setVerifying(true);
        try {
            const p = await verifyWorkspaceArchive(sourcePath.trim());
            setPreview(p);
            setPhase("options");
        }
        catch (e) {
            console.warn("verifyWorkspaceArchive failed:", e);
            const msg = e && typeof e === "object" && "message" in e
                ? `screens.settings.portable.import.errors.verify::${e.message}`
                : "screens.settings.portable.import.errors.verify";
            setError(msg);
        }
        finally {
            setVerifying(false);
        }
    }, [sourcePath]);
    const handleStart = useCallback(async () => {
        if (!preview)
            return;
        setError(null);
        if (preview.has_keys && passphrase.trim().length === 0) {
            setError("screens.settings.portable.import.errors.emptyPassphrase");
            return;
        }
        const opts = {
            source_path: sourcePath.trim(),
            target_workspace_root: null,
            key_passphrase: preview.has_keys ? passphrase : null,
            conflict_policy: conflictPolicy,
            expected_sha256: null,
        };
        setPhase("running");
        setProgress({ processed: 0, total: preview.entries_count });
        try {
            const res = await startWorkspaceImport(opts, (ev) => {
                if (ev.kind === "extracting") {
                    setProgress({
                        processed: Number(ev.processed),
                        total: Number(ev.total),
                    });
                }
                else if (ev.kind === "done") {
                    setDone({
                        manifestSummary: ev.manifest_summary,
                        repairTier: ev.repair_tier,
                    });
                }
                else if (ev.kind === "failed") {
                    setError(`screens.settings.portable.import.errors.runner::${ev.error}`);
                }
                if (isTerminalImportEvent(ev)) {
                    // 별도 처리 — invoke의 await에서 추가 응답이 옴.
                }
            });
            importIdRef.current = res.import_id;
            setDone({
                manifestSummary: res.summary.manifest_summary,
                repairTier: res.summary.repair_tier,
            });
            setPhase("done");
        }
        catch (e) {
            console.warn("startWorkspaceImport failed:", e);
            const msg = e && typeof e === "object" && "message" in e
                ? `screens.settings.portable.import.errors.runner::${e.message}`
                : "screens.settings.portable.import.errors.start";
            setError(msg);
            setPhase("failed");
        }
    }, [preview, sourcePath, passphrase, conflictPolicy]);
    const handleCancel = useCallback(async () => {
        if (!importIdRef.current) {
            setPhase("idle");
            return;
        }
        try {
            await cancelWorkspaceImport(importIdRef.current);
        }
        catch (e) {
            console.warn("cancelWorkspaceImport failed:", e);
        }
    }, []);
    const errorText = useMemo(() => {
        if (!error)
            return null;
        const idx = error.indexOf("::");
        if (idx > 0) {
            const key = error.slice(0, idx);
            const detail = error.slice(idx + 2);
            return `${t(key)} (${detail})`;
        }
        return t(error);
    }, [error, t]);
    const percent = progress.total > 0
        ? Math.min(100, Math.round((progress.processed / progress.total) * 100))
        : 0;
    return (_jsxs("fieldset", { className: "settings-fieldset", "data-testid": "portable-import-panel", children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.portable.import.title") }), _jsx("p", { className: "settings-hint", children: t("screens.settings.portable.import.subtitle") }), _jsx("button", { type: "button", className: "settings-btn-primary", onClick: () => setPhase("preview"), disabled: isOpen, "data-testid": "portable-import-start-btn", children: t("screens.settings.portable.import.start") }), phase === "done" && done && (_jsxs("div", { className: "portable-result-card", role: "status", "aria-live": "polite", "data-testid": "portable-import-done", children: [_jsx("p", { className: "portable-result-headline", children: t("screens.settings.portable.import.done") }), _jsx("p", { className: `portable-tier portable-tier-${done.repairTier}`, "data-testid": "portable-import-tier", children: t(`screens.settings.portable.import.tier.${done.repairTier}`) }), _jsx("p", { className: "portable-result-summary num", children: done.manifestSummary }), _jsx("button", { type: "button", className: "settings-btn-secondary", onClick: reset, "data-testid": "portable-import-reset-btn", children: t("screens.settings.portable.import.again") })] })), phase === "failed" && errorText && (_jsx("p", { className: "settings-error", role: "alert", children: errorText })), isOpen && (_jsx("div", { className: "portable-modal-backdrop", role: "presentation", onClick: () => (phase === "preview" || phase === "options") && setPhase("idle"), children: _jsxs("div", { className: "portable-modal", role: "dialog", "aria-modal": "true", "aria-labelledby": "portable-import-modal-title", onClick: (e) => e.stopPropagation(), "data-testid": "portable-import-modal", children: [_jsx("header", { className: "portable-modal-header", children: _jsx("h3", { id: "portable-import-modal-title", className: "portable-modal-title", children: phase === "preview"
                                    ? t("screens.settings.portable.import.dialogTitle")
                                    : phase === "options"
                                        ? t("screens.settings.portable.import.optionsTitle")
                                        : t("screens.settings.portable.import.runningTitle") }) }), phase === "preview" && (_jsxs("div", { className: "portable-modal-body", children: [_jsxs("label", { className: "portable-input-field", children: [_jsx("span", { children: t("screens.settings.portable.import.selectFile") }), _jsx("input", { ref: sourceInputRef, type: "text", className: "settings-input num", value: sourcePath, onChange: (e) => setSourcePath(e.target.value), placeholder: "C:\\\\Users\\\\me\\\\Desktop\\\\workspace.zip", "data-testid": "portable-import-source-input" }), _jsx("small", { className: "settings-hint", children: t("screens.settings.portable.import.selectFileHint") })] }), errorText && (_jsx("p", { className: "settings-error", role: "alert", children: errorText })), _jsxs("div", { className: "portable-modal-footer", children: [_jsx("button", { type: "button", className: "settings-btn-secondary", onClick: () => setPhase("idle"), "data-testid": "portable-import-dialog-close", children: t("screens.settings.portable.import.cancel") }), _jsx("button", { type: "button", className: "settings-btn-primary", onClick: handleVerify, disabled: verifying, "data-testid": "portable-import-verify-btn", children: verifying
                                                ? t("screens.settings.portable.import.verifying")
                                                : t("screens.settings.portable.import.verify") })] })] })), phase === "options" && preview && (_jsxs("div", { className: "portable-modal-body", children: [_jsxs("section", { className: "portable-preview-card", "data-testid": "portable-import-preview", children: [_jsx("h4", { className: "portable-preview-title", children: t("screens.settings.portable.import.preview.title") }), _jsx("p", { className: "portable-preview-summary num", children: preview.manifest_summary }), _jsxs("dl", { className: "portable-result-meta", children: [_jsxs("div", { className: "portable-result-row", children: [_jsx("dt", { children: t("screens.settings.portable.import.preview.size") }), _jsx("dd", { className: "num", children: formatBytes(preview.size_bytes) })] }), _jsxs("div", { className: "portable-result-row", children: [_jsx("dt", { children: t("screens.settings.portable.import.preview.entries") }), _jsx("dd", { className: "num", children: preview.entries_count })] }), _jsxs("div", { className: "portable-result-row", children: [_jsx("dt", { children: t("screens.settings.portable.import.preview.contents") }), _jsxs("dd", { children: [preview.has_models
                                                                    ? t("screens.settings.portable.import.preview.hasModels")
                                                                    : t("screens.settings.portable.import.preview.metaOnly"), preview.has_keys
                                                                    ? ` · ${t("screens.settings.portable.import.preview.hasKeys")}`
                                                                    : ""] })] })] })] }), preview.has_keys && (_jsxs("label", { className: "portable-input-field", children: [_jsx("span", { children: t("screens.settings.portable.import.passphrase") }), _jsx("input", { type: "password", className: "settings-input", value: passphrase, onChange: (e) => setPassphrase(e.target.value), autoComplete: "current-password", "data-testid": "portable-import-passphrase" })] })), _jsxs("fieldset", { className: "settings-fieldset", style: { borderStyle: "dashed" }, children: [_jsx("legend", { className: "settings-legend", children: t("screens.settings.portable.import.conflictPolicy.label") }), _jsx("div", { className: "settings-radio-row", role: "radiogroup", children: ["skip", "overwrite", "rename"].map((p) => (_jsxs("label", { className: `settings-radio${conflictPolicy === p ? " is-checked" : ""}`, children: [_jsx("input", { type: "radio", name: "conflict_policy", value: p, checked: conflictPolicy === p, onChange: () => setConflictPolicy(p), "data-testid": `portable-import-policy-${p}` }), _jsx("span", { className: "settings-radio-label", children: t(`screens.settings.portable.import.conflictPolicy.${p}`) })] }, p))) })] }), errorText && (_jsx("p", { className: "settings-error", role: "alert", children: errorText })), _jsxs("div", { className: "portable-modal-footer", children: [_jsx("button", { type: "button", className: "settings-btn-secondary", onClick: () => setPhase("idle"), "data-testid": "portable-import-options-close", children: t("screens.settings.portable.import.cancel") }), _jsx("button", { type: "button", className: "settings-btn-primary", onClick: handleStart, "data-testid": "portable-import-confirm-btn", children: t("screens.settings.portable.import.confirm") })] })] })), phase === "running" && (_jsxs("div", { className: "portable-modal-body", children: [_jsx("p", { className: "portable-running-message", children: t("screens.settings.portable.import.running") }), _jsx("div", { className: "portable-progress", role: "progressbar", "aria-valuenow": percent, "aria-valuemin": 0, "aria-valuemax": 100, "data-testid": "portable-import-progress", children: _jsx("div", { className: "portable-progress-fill", style: { width: `${percent}%` } }) }), _jsxs("p", { className: "portable-progress-meta num", children: [progress.processed, " / ", progress.total, " \u00B7 ", percent, "%"] }), _jsx("div", { className: "portable-modal-footer", children: _jsx("button", { ref: cancelBtnRef, type: "button", className: "settings-btn-secondary", onClick: handleCancel, "data-testid": "portable-import-cancel-btn", children: t("screens.settings.portable.import.cancel") }) })] }))] }) }))] }));
}
