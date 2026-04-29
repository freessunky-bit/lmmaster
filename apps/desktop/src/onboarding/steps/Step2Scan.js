import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// Step 2 — 환경 점검. Phase 1A.4.b.
//
// 머신 entry 시 자동으로 detect_environment 호출 (fromPromise actor).
// substate별 UI:
//   running → skeleton 카드 + "잠시만 기다려 주세요"
//   done    → 4 카드 (OS / 메모리 / GPU / 런타임) + "계속할게요" 활성
//   failed  → 에러 카드 + RETRY 버튼
import { useTranslation } from "react-i18next";
import { useOnboardingEnv, useOnboardingScanError, useOnboardingScanSub, useOnboardingSend, } from "../context";
import { formatGiB, osFamilyLabel, runtimeKindLabel, } from "../../ipc/environment";
const RAM_WARN_BYTES = 8 * 1024 * 1024 * 1024; // 8GB 미만 경고
const DISK_WARN_BYTES = 20 * 1024 * 1024 * 1024; // 가용 20GB 미만 경고
export function Step2Scan() {
    const { t } = useTranslation();
    const sub = useOnboardingScanSub();
    const env = useOnboardingEnv();
    const scanError = useOnboardingScanError();
    const send = useOnboardingSend();
    const isRunning = sub === "running" || sub === "idle";
    const isDone = sub === "done" && env;
    const isFailed = sub === "failed";
    return (_jsxs("section", { className: "onb-step", "aria-labelledby": "onb-step2-title", children: [_jsxs("header", { className: "onb-step-header", children: [_jsx("h1", { id: "onb-step2-title", className: "onb-step-title", children: t("onboarding.scan.title") }), _jsxs("p", { className: "onb-step-subtitle", children: [isRunning && t("onboarding.scan.subtitle.running"), isDone && t("onboarding.scan.subtitle.done"), isFailed && t("onboarding.scan.subtitle.failed")] })] }), isRunning && _jsx(ScanSkeleton, {}), isDone && env && _jsx(ScanResult, { env: env }), isFailed && (_jsx(ScanFailure, { message: scanError ?? "unknown error", onRetry: () => send({ type: "RETRY" }) })), _jsxs("div", { className: "onb-step-actions", children: [_jsx("button", { type: "button", className: "onb-button onb-button-secondary", onClick: () => send({ type: "BACK" }), children: t("onboarding.actions.back") }), _jsx("button", { type: "button", className: "onb-button onb-button-primary", onClick: () => send({ type: "NEXT" }), disabled: !isDone, children: t("onboarding.actions.next") })] })] }));
}
// ── Skeleton (running) ──────────────────────────────────────────────────
function ScanSkeleton() {
    const { t } = useTranslation();
    return (_jsxs("div", { className: "onb-scan-cards", "aria-busy": "true", "aria-live": "polite", children: [[0, 1, 2, 3].map((i) => (_jsxs("div", { className: "onb-scan-card onb-skeleton", children: [_jsx("div", { className: "onb-skeleton-bar onb-skeleton-bar-sm" }), _jsx("div", { className: "onb-skeleton-bar onb-skeleton-bar-md" })] }, i))), _jsx("p", { className: "onb-scan-caption num", children: t("onboarding.scan.captionRunning") })] }));
}
// ── Result (done) ───────────────────────────────────────────────────────
function ScanResult({ env }) {
    const { t } = useTranslation();
    const { hardware, runtimes } = env;
    const ramWarn = hardware.mem.total_bytes < RAM_WARN_BYTES;
    const diskWarn = hardware.disks.length > 0 &&
        Math.min(...hardware.disks.map((d) => d.available_bytes)) < DISK_WARN_BYTES;
    const primaryGpu = pickPrimaryGpu(hardware.gpus);
    return (_jsxs("div", { className: "onb-scan-cards", "aria-live": "polite", children: [_jsx(ScanCard, { title: t("onboarding.scan.card.os"), status: "ok", statusLabel: t("onboarding.scan.status.ok"), body: `${osFamilyLabel(hardware.os.family)} ${hardware.os.version} · ${hardware.os.arch}` }), _jsx(ScanCard, { title: t("onboarding.scan.card.memory"), status: ramWarn ? "warn" : "ok", statusLabel: ramWarn
                    ? t("onboarding.scan.status.warn")
                    : t("onboarding.scan.status.ok"), body: t("onboarding.scan.body.memory", {
                    available: formatGiB(hardware.mem.available_bytes),
                    total: formatGiB(hardware.mem.total_bytes),
                }), hint: ramWarn ? t("onboarding.scan.hint.lowRam") : undefined }), _jsx(ScanCard, { title: t("onboarding.scan.card.gpu"), status: primaryGpu ? "ok" : "muted", statusLabel: primaryGpu
                    ? gpuVendorLabel(primaryGpu.vendor)
                    : t("onboarding.scan.status.cpuOnly"), body: primaryGpu
                    ? formatGpuBody(primaryGpu)
                    : t("onboarding.scan.body.noGpu"), hint: diskWarn ? t("onboarding.scan.hint.lowDisk") : undefined }), _jsx(ScanCard, { title: t("onboarding.scan.card.runtimes"), status: runtimes.some((r) => r.status === "running") ? "ok" : "muted", statusLabel: runtimes.some((r) => r.status === "running")
                    ? t("onboarding.scan.status.running")
                    : t("onboarding.scan.status.none"), body: null, children: _jsx("ul", { className: "onb-runtime-list", children: runtimes.map((rt) => (_jsx(RuntimeRow, { result: rt }, rt.runtime))) }) })] }));
}
// ── Failure ─────────────────────────────────────────────────────────────
function ScanFailure({ message, onRetry, }) {
    const { t } = useTranslation();
    return (_jsxs("div", { className: "onb-error", role: "alert", children: [_jsx("h2", { className: "onb-error-title", children: t("onboarding.scan.failure.title") }), _jsx("p", { className: "onb-error-body", children: t("onboarding.scan.failure.body") }), _jsx("pre", { className: "onb-error-detail", children: message }), _jsx("div", { className: "onb-error-actions", children: _jsx("button", { type: "button", className: "onb-button onb-button-primary", onClick: onRetry, children: t("onboarding.error.retry") }) })] }));
}
function ScanCard({ title, status, statusLabel, body, hint, children, }) {
    return (_jsxs("div", { className: "onb-scan-card", "data-status": status, children: [_jsxs("header", { className: "onb-scan-card-header", children: [_jsx("span", { className: "onb-scan-card-title", children: title }), _jsx("span", { className: "onb-scan-pill", "data-status": status, children: statusLabel })] }), body && _jsx("p", { className: "onb-scan-card-body", children: body }), children, hint && _jsx("p", { className: "onb-scan-card-hint", children: hint })] }));
}
function RuntimeRow({ result }) {
    const { t } = useTranslation();
    const status = result.status;
    return (_jsxs("li", { className: "onb-runtime-row", "data-status": status, children: [_jsx("span", { className: "onb-runtime-name", children: runtimeKindLabel(result.runtime) }), _jsxs("span", { className: "onb-runtime-status", children: [status === "running" && t("onboarding.scan.runtime.running"), status === "installed" && t("onboarding.scan.runtime.installed"), status === "not-installed" && t("onboarding.scan.runtime.notInstalled"), status === "error" && t("onboarding.scan.runtime.error")] }), result.version && (_jsx("span", { className: "onb-runtime-version num", children: result.version }))] }));
}
// ── 포맷 헬퍼 ──────────────────────────────────────────────────────────
function pickPrimaryGpu(gpus) {
    if (gpus.length === 0)
        return undefined;
    // VRAM 가장 큰 것 — discrete GPU 우선.
    return [...gpus].sort((a, b) => (b.vram_bytes ?? 0) - (a.vram_bytes ?? 0))[0];
}
function gpuVendorLabel(vendor) {
    switch (vendor) {
        case "nvidia":
            return "NVIDIA";
        case "amd":
            return "AMD";
        case "intel":
            return "Intel";
        case "apple":
            return "Apple";
        case "qualcomm":
            return "Qualcomm";
        case "microsoft":
            return "Microsoft";
        default:
            return "GPU";
    }
}
function formatGpuBody(gpu) {
    if (gpu.vram_bytes && gpu.vram_bytes > 0) {
        return `${gpu.name} · ${formatGiB(gpu.vram_bytes)} VRAM`;
    }
    return gpu.name;
}
