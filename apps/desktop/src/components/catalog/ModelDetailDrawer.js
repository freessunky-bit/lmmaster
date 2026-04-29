import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// ModelDetailDrawer — 카드 클릭 시 우측 슬라이드 드로워.
//
// 정책 (phase-2pb-catalog-ui-decision.md §7):
// - quant_options 라디오 그룹 + 권장 quant 표시.
// - warnings + use_case_examples 전체.
// - Esc / 배경 클릭으로 닫기.
// - role="dialog" + aria-labelledby + focus trap (간단 — 첫 focusable로 포커스).
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { cancelBench, getLastBenchReport, onBenchFinished, startBench, } from "../../ipc/bench";
import { categoryLabelKo, getPresets, } from "../../ipc/presets";
import { BenchChip } from "./BenchChip";
import { formatSize } from "./format";
const DEFAULT_BENCH_RUNTIME = "ollama";
export function ModelDetailDrawer({ model, benchRuntime, onClose, onInstall, }) {
    const { t } = useTranslation();
    const closeBtnRef = useRef(null);
    const [selectedQuant, setSelectedQuant] = useState("");
    const [benchState, setBenchState] = useState({ kind: "idle" });
    const runtime = benchRuntime ?? pickRuntime(model) ?? DEFAULT_BENCH_RUNTIME;
    // 이 모델을 recommended_models[]에 포함하는 preset 목록.
    const [recommendedPresets, setRecommendedPresets] = useState([]);
    // model이 바뀔 때마다 첫 quant를 default로 + 캐시된 측정 결과 조회.
    useEffect(() => {
        const first = model?.quantization_options[0];
        if (first) {
            setSelectedQuant(first.label);
        }
        if (!model) {
            setBenchState({ kind: "idle" });
            return;
        }
        let cancelled = false;
        getLastBenchReport({
            modelId: model.id,
            runtimeKind: runtime,
            quantLabel: first?.label ?? null,
        })
            .then((r) => {
            if (cancelled)
                return;
            if (r)
                setBenchState({ kind: "report", report: r });
            else
                setBenchState({ kind: "idle" });
        })
            .catch(() => {
            if (!cancelled)
                setBenchState({ kind: "idle" });
        });
        return () => {
            cancelled = true;
        };
    }, [model, runtime]);
    // bench:finished event 구독 — 측정 완료 시 카드 갱신.
    useEffect(() => {
        if (!model)
            return;
        let unlisten = null;
        onBenchFinished((report) => {
            if (report.model_id === model.id) {
                setBenchState({ kind: "report", report });
            }
        }).then((u) => {
            unlisten = u;
        });
        return () => {
            unlisten?.();
        };
    }, [model]);
    // 추천 프리셋 로드 — 이 모델을 recommended_models[]에 포함한 preset만 필터.
    useEffect(() => {
        if (!model) {
            setRecommendedPresets([]);
            return;
        }
        let cancelled = false;
        getPresets()
            .then((all) => {
            if (cancelled)
                return;
            const matching = all.filter((p) => p.recommended_models.includes(model.id));
            setRecommendedPresets(matching);
        })
            .catch((e) => {
            // preset 로드 실패는 치명적이지 않음 — 빈 목록으로 graceful.
            console.warn("getPresets failed:", e);
            if (!cancelled)
                setRecommendedPresets([]);
        });
        return () => {
            cancelled = true;
        };
    }, [model]);
    const handleMeasure = useCallback(async () => {
        if (!model)
            return;
        setBenchState({ kind: "running" });
        try {
            const report = await startBench({
                modelId: model.id,
                runtimeKind: runtime,
                quantLabel: selectedQuant || null,
            });
            setBenchState({ kind: "report", report });
        }
        catch (e) {
            console.warn("startBench failed:", e);
            // 실패해도 idle로 복귀 — 사용자가 다시 시도 가능.
            setBenchState({ kind: "idle" });
        }
    }, [model, runtime, selectedQuant]);
    const handleCancel = useCallback(async () => {
        if (!model)
            return;
        try {
            await cancelBench(model.id);
        }
        finally {
            setBenchState({ kind: "idle" });
        }
    }, [model]);
    // Esc로 닫기 + 첫 focus.
    useEffect(() => {
        if (!model)
            return;
        const onKey = (e) => {
            if (e.key === "Escape")
                onClose();
        };
        window.addEventListener("keydown", onKey);
        closeBtnRef.current?.focus();
        return () => window.removeEventListener("keydown", onKey);
    }, [model, onClose]);
    if (!model)
        return null;
    return (_jsx("div", { className: "catalog-drawer-backdrop", role: "presentation", onClick: onClose, children: _jsxs("aside", { className: "catalog-drawer", role: "dialog", "aria-modal": "true", "aria-labelledby": "catalog-drawer-title", onClick: (e) => e.stopPropagation(), children: [_jsxs("header", { className: "catalog-drawer-header", children: [_jsx("h3", { id: "catalog-drawer-title", className: "catalog-drawer-title", children: model.display_name }), _jsxs("div", { className: "catalog-drawer-header-actions", children: [onInstall && (_jsx("button", { type: "button", className: "catalog-drawer-install", onClick: () => onInstall(model.id), "aria-label": t("drawer.install.aria", { name: model.display_name }), children: t("drawer.install.cta") })), _jsx("button", { ref: closeBtnRef, type: "button", className: "catalog-drawer-close", onClick: onClose, "aria-label": t("drawer.close"), children: "\u00D7" })] })] }), _jsxs("div", { className: "catalog-drawer-body", children: [model.context_guidance && (_jsxs("section", { children: [_jsx("h4", { className: "catalog-drawer-section-title", children: t("drawer.section.context") }), _jsx("p", { className: "catalog-drawer-text", children: model.context_guidance })] })), model.use_case_examples.length > 0 && (_jsxs("section", { children: [_jsx("h4", { className: "catalog-drawer-section-title", children: t("drawer.section.useCases") }), _jsx("ul", { className: "catalog-drawer-list", children: model.use_case_examples.map((u) => (_jsx("li", { children: u }, u))) })] })), _jsxs("section", { children: [_jsx("h4", { className: "catalog-drawer-section-title", children: t("drawer.section.bench", "30초 측정") }), _jsx(BenchChip, { state: benchState, onMeasure: handleMeasure, onCancel: handleCancel, onRetry: handleMeasure })] }), benchState.kind === "report" && benchState.report.sample_text_excerpt && (_jsx("p", { className: "catalog-drawer-text bench-excerpt", children: benchState.report.sample_text_excerpt })), model.quantization_options.length > 0 && (_jsxs("section", { children: [_jsx("h4", { className: "catalog-drawer-section-title", children: t("drawer.section.quant") }), _jsx("div", { role: "radiogroup", className: "catalog-drawer-quant", children: model.quantization_options.map((q, idx) => (_jsx(QuantRow, { quant: q, isRecommended: idx === 0, isChecked: selectedQuant === q.label, onChange: () => setSelectedQuant(q.label) }, q.label))) })] })), model.warnings.length > 0 && (_jsxs("section", { children: [_jsx("h4", { className: "catalog-drawer-section-title", children: t("drawer.section.warnings") }), _jsx("ul", { className: "catalog-drawer-list catalog-drawer-warnings", children: model.warnings.map((w) => (_jsx("li", { children: w }, w))) })] })), _jsxs("section", { children: [_jsx("h4", { className: "catalog-drawer-section-title", children: t("drawer.section.presets", "이 모델 추천 프리셋") }), recommendedPresets.length === 0 ? (_jsx("p", { className: "catalog-drawer-text", children: t("drawer.section.presetsEmpty", "추천 프리셋이 없어요") })) : (_jsx("ul", { className: "catalog-drawer-list catalog-drawer-presets", children: recommendedPresets.map((p) => (_jsxs("li", { className: "catalog-drawer-preset-item", children: [_jsx("span", { className: "catalog-drawer-preset-name", children: p.display_name_ko }), _jsx("span", { className: "catalog-drawer-preset-subtitle", children: p.subtitle_ko }), _jsx("span", { className: "catalog-drawer-preset-chip", children: categoryLabelKo(p.category) })] }, p.id))) }))] }), _jsxs("section", { children: [_jsx("h4", { className: "catalog-drawer-section-title", children: t("drawer.section.license") }), _jsx("p", { className: "catalog-drawer-text", children: model.license })] })] })] }) }));
}
function pickRuntime(model) {
    if (!model)
        return null;
    // 우선순위: ollama > lm-studio > 기타.
    if (model.runner_compatibility.includes("ollama"))
        return "ollama";
    if (model.runner_compatibility.includes("lm-studio"))
        return "lm-studio";
    return model.runner_compatibility[0] ?? null;
}
function QuantRow({ quant, isRecommended, isChecked, onChange }) {
    const { t } = useTranslation();
    return (_jsxs("label", { className: "catalog-drawer-quant-row", children: [_jsx("input", { type: "radio", name: "quant", checked: isChecked, onChange: onChange }), _jsx("span", { className: "catalog-drawer-quant-label", children: quant.label }), _jsx("span", { className: "catalog-drawer-quant-size num", children: formatSize(quant.size_mb) }), isRecommended && (_jsx("span", { className: "catalog-drawer-quant-rec", children: t("drawer.quantRecommended") }))] }));
}
