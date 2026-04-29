// InstallEvent — Rust crates/installer/src/install_event.rs의 Serialize 미러.
// kind 기반 discriminated union. 변형은 #[serde(tag = "kind", rename_all = "kebab-case")]에 1:1 대응.
//
// Phase 1A.3.c: 수동 미러. Phase 후순위에서 tauri-specta로 자동 생성 예정 (ADR-0015).
// ── Type guards (간단한 narrowing 헬퍼) ────────────────────────────────────
export function isTerminal(ev) {
    return (ev.kind === "finished" || ev.kind === "failed" || ev.kind === "cancelled");
}
export function totalProgress(ev) {
    if (ev.kind === "download" && ev.download.kind === "progress") {
        return { downloaded: ev.download.downloaded, total: ev.download.total };
    }
    return null;
}
