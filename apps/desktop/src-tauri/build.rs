fn main() {
    // tauri-build는 capabilities/와 permissions/만 자동 추적하고, src/lib.rs의 invoke_handler!
    // 변경은 감지 못 해 새 #[tauri::command]가 캐시된 permission file과 mismatch → build 실패.
    // src/ 전체를 트래킹해서 자동 재생성 강제.
    println!("cargo:rerun-if-changed=src");
    tauri_build::build()
}
