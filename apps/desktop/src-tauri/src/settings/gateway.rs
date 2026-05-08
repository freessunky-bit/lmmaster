//! Phase 8'.c.4 (ADR-0066) — 게이트웨이 사내망 노출 (allow_external) settings IPC.
//!
//! 정책:
//! - `gateway_allow_external = true` → 게이트웨이가 0.0.0.0:port 바인딩 (사내망 노출).
//! - `false` (default) → 127.0.0.1:port 바인딩 (이 PC만).
//! - 변경 후 게이트웨이 재시작 필요 (자동 hot-restart는 v1.x). frontend가 사용자에게 안내.
//! - `LMMASTER_GATEWAY_ALLOW_EXTERNAL` env 변수가 startup 시 주입되며 같은 process 내 갱신도 즉시 반영.
//! - LAN IP는 `local-ip-address` crate로 감지. RFC 1918 private 범위만 노출 (link-local 169.254/16 제외).

use std::net::IpAddr;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum GatewaySettingsError {
    #[error("설정 저장 실패: {message}")]
    Save { message: String },

    #[error("내부 오류: {message}")]
    Internal { message: String },
}

/// 현재 사내망 노출 여부 반환. settings.json + env 변수 fallback.
#[tauri::command]
pub fn get_gateway_allow_external(app: AppHandle) -> Result<bool, GatewaySettingsError> {
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| GatewaySettingsError::Internal {
            message: format!("app_local_data_dir 해결 실패: {e}"),
        })?;
    let s = super::UserSettings::load(&dir);
    if s.gateway_allow_external {
        return Ok(true);
    }
    // Fallback: env 변수 ("1" → true).
    if let Ok(v) = std::env::var("LMMASTER_GATEWAY_ALLOW_EXTERNAL") {
        return Ok(v == "1");
    }
    Ok(false)
}

/// 사내망 노출 토글 — settings.json save + env 즉시 갱신.
///
/// 변경 후 게이트웨이 재시작이 필요해요. frontend가 "재시작 후 적용" 안내 + 사용자가 앱 재시작.
#[tauri::command]
pub fn set_gateway_allow_external(app: AppHandle, allow: bool) -> Result<(), GatewaySettingsError> {
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| GatewaySettingsError::Internal {
            message: format!("app_local_data_dir 해결 실패: {e}"),
        })?;
    let mut s = super::UserSettings::load(&dir);
    s.gateway_allow_external = allow;
    s.save(&dir).map_err(|e| GatewaySettingsError::Save {
        message: e.to_string(),
    })?;
    // 즉시 env 갱신 — 다음 게이트웨이 재시작 시 새 값 적용.
    std::env::set_var(
        "LMMASTER_GATEWAY_ALLOW_EXTERNAL",
        if allow { "1" } else { "0" },
    );
    tracing::info!(allow_external = allow, "사내망 노출 설정 갱신 + env 주입");
    Ok(())
}

/// LAN IP 후보 목록 — 사내망 노출 켜졌을 때 사용자에게 표시할 호출 URL의 호스트 부분.
///
/// 화이트리스트 (RFC 1918 + 일부 사내망 관행):
/// - 10.0.0.0/8
/// - 172.16.0.0/12
/// - 192.168.0.0/16
///
/// 제외: loopback (127/8), link-local (169.254/16), IPv6 (v1은 IPv4만).
/// 다중 NIC 환경에서 모두 반환. 빈 vec → "사내망 IP 감지 실패" 카피.
#[tauri::command]
pub fn list_lan_addresses() -> Result<Vec<String>, GatewaySettingsError> {
    let ifaces = local_ip_address::list_afinet_netifas().map_err(|e| {
        tracing::warn!(error = %e, "list_afinet_netifas 실패");
        GatewaySettingsError::Internal {
            message: format!("LAN IP 감지 실패: {e}"),
        }
    })?;
    let mut out = Vec::new();
    for (_name, ip) in ifaces {
        if let IpAddr::V4(v4) = ip {
            if is_private_lan_ipv4(v4.octets()) {
                out.push(v4.to_string());
            }
        }
    }
    Ok(out)
}

/// RFC 1918 private 범위 + link-local / loopback 제외.
fn is_private_lan_ipv4(o: [u8; 4]) -> bool {
    match o {
        [10, _, _, _] => true,
        [172, b, _, _] if (16..=31).contains(&b) => true,
        [192, 168, _, _] => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_lan_includes_10_block() {
        assert!(is_private_lan_ipv4([10, 0, 0, 1]));
        assert!(is_private_lan_ipv4([10, 255, 255, 254]));
    }

    #[test]
    fn private_lan_includes_192_168_block() {
        assert!(is_private_lan_ipv4([192, 168, 1, 42]));
    }

    #[test]
    fn private_lan_includes_172_16_through_31() {
        assert!(is_private_lan_ipv4([172, 16, 0, 1]));
        assert!(is_private_lan_ipv4([172, 31, 255, 254]));
        // 172.15 / 172.32 은 private 범위 밖.
        assert!(!is_private_lan_ipv4([172, 15, 0, 1]));
        assert!(!is_private_lan_ipv4([172, 32, 0, 1]));
    }

    #[test]
    fn private_lan_excludes_loopback() {
        assert!(!is_private_lan_ipv4([127, 0, 0, 1]));
    }

    #[test]
    fn private_lan_excludes_link_local() {
        // 169.254/16 — link-local. 사용자가 사내망으로 인지하기엔 부적합.
        assert!(!is_private_lan_ipv4([169, 254, 1, 1]));
    }

    #[test]
    fn private_lan_excludes_public() {
        assert!(!is_private_lan_ipv4([8, 8, 8, 8]));
        assert!(!is_private_lan_ipv4([1, 1, 1, 1]));
    }
}
