// bench IPC — start_bench / cancel_bench / get_last_bench_report.
// Rust crates/bench-harness 의 BenchReport / BenchSample / BenchErrorReport / BenchMetricsSource 미러.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
/** 30초 벤치마크 실행. AlreadyRunning 거부 시 BenchApiError throw. */
export async function startBench(args) {
    return invoke("start_bench", {
        modelId: args.modelId,
        runtimeKind: args.runtimeKind,
        quantLabel: args.quantLabel ?? null,
        digestAtBench: args.digestAtBench ?? null,
    });
}
/** 진행 중인 측정 취소 — idempotent. */
export async function cancelBench(modelId) {
    return invoke("cancel_bench", { modelId });
}
/** 캐시된 최근 측정 결과. 없으면 null. */
export async function getLastBenchReport(args) {
    return invoke("get_last_bench_report", {
        modelId: args.modelId,
        runtimeKind: args.runtimeKind,
        quantLabel: args.quantLabel ?? null,
        digestAtBench: args.digestAtBench ?? null,
    });
}
/** bench:started — UI 진행 spinner 트리거. */
export async function onBenchStarted(cb) {
    return listen("bench:started", (e) => cb(e.payload));
}
/** bench:finished — UI 카드 hint chip 갱신. */
export async function onBenchFinished(cb) {
    return listen("bench:finished", (e) => cb(e.payload));
}
