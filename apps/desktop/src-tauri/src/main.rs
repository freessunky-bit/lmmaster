// Windows release build에서 콘솔 창을 숨긴다.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    lmmaster_desktop_lib::run()
}
