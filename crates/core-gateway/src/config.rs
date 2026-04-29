use serde::{Deserialize, Serialize};

/// Gateway 설정. localhost 바인딩이 기본이며, `allow_external = true`는
/// 명시적으로 위험성을 인정한 사용자 설정(설정 화면에서 경고 표시 후 토글) 외에는 사용하지 않는다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub host: String,
    pub port: Option<u16>,
    pub allow_external: bool,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: None,
            allow_external: false,
        }
    }
}

impl GatewayConfig {
    /// 바인딩 대상 SocketAddr 후보 문자열을 반환한다.
    /// `allow_external = true`인 경우만 0.0.0.0를 허용. 기본은 항상 127.0.0.1.
    pub fn bind_host(&self) -> &str {
        if self.allow_external {
            "0.0.0.0"
        } else {
            "127.0.0.1"
        }
    }

    /// `port`가 None이면 OS 할당(0)을 의미.
    pub fn bind_port(&self) -> u16 {
        self.port.unwrap_or(0)
    }
}
