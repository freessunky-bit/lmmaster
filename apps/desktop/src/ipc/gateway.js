// Typed wrappers around Tauri IPC for gateway state.
// 실제 invoke 키와 event 이름은 src-tauri/src/{commands.rs, gateway.rs}와 일치해야 한다.
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
export async function getGatewayStatus() {
    return invoke("get_gateway_status");
}
export async function onGatewayReady(cb) {
    return listen("gateway://ready", (e) => cb(e.payload));
}
export async function onGatewayFailed(cb) {
    return listen("gateway://failed", (e) => cb(e.payload));
}
