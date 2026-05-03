//! 포트 자동 할당 — `TcpListener::bind("127.0.0.1:0")` → OS ephemeral port.
//!
//! 정책 (보강 리서치 §1.1):
//! - 8080 고정 X — 사용자 환경에서 다른 도구(LM Studio 1234, Ollama 11434, 임의 llama-server) 충돌 위험.
//! - listener drop 후 spawn 사이 race window는 단일 사용자 데스크톱에서 무시 OK.

use std::net::TcpListener;

use crate::RunnerError;

/// localhost ephemeral port 할당. listener는 즉시 drop — 호출자가 spawn에 port 전달.
pub fn allocate_localhost_port() -> Result<u16, RunnerError> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| RunnerError::PortAllocFailed {
        message: e.to_string(),
    })?;
    let port = listener
        .local_addr()
        .map_err(|e| RunnerError::PortAllocFailed {
            message: e.to_string(),
        })?
        .port();
    drop(listener);
    Ok(port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_nonzero_port_in_localhost_range() {
        let port = allocate_localhost_port().expect("port alloc ok");
        assert!(port > 0, "0번 포트는 OS가 안 골라줘야 함");
        // ephemeral 범위는 OS별 다름 — Linux 32768~60999, Windows 49152~65535. 대략 1024+.
        assert!(port >= 1024, "ephemeral port는 1024 이상");
    }

    #[test]
    fn allocates_distinct_ports_across_calls() {
        // 동일 호출에서 같은 포트가 나올 수 없음 (서로 다른 listener).
        let mut seen = std::collections::HashSet::new();
        for _ in 0..10 {
            let p = allocate_localhost_port().expect("alloc ok");
            // 같은 포트가 우연히 재할당될 수도 있지만 10회 중 모두 같을 확률 매우 낮음.
            // OS 정책상 round-robin 가까움 — 적어도 2개 이상 distinct 기대.
            seen.insert(p);
        }
        assert!(
            seen.len() >= 2,
            "10회 round-trip에서 최소 2개 distinct port. 실측 {seen:?}",
        );
    }
}
