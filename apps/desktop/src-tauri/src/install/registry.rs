//! In-flight install registry — id ↔ CancellationToken 매핑.
//!
//! 정책 (Phase 1A.3.c 보강 리서치):
//! - `try_start(id)` — 동일 id 진행 중이면 `AlreadyInstalling` 거부 (대부분 installer 패턴).
//! - `finish(id)` — RAII guard로 보장 (caller가 `scopeguard::defer!`).
//! - `cancel(id)` — 미존재면 no-op (idempotent).
//! - `cancel_all()` — 앱 종료 시 호출. `CancellationToken::Drop`은 cancel 안 함 → 명시 호출 필수.
//! - `Mutex<HashMap<...>>` (sync) — lock 보유 시간 매우 짧음. tokio::sync::Mutex 불필요.

use std::collections::HashMap;
use std::sync::Mutex;

use thiserror::Error;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Error)]
pub enum InstallRegistryError {
    #[error("이미 설치 중인 앱이에요 (id={0})")]
    AlreadyInstalling(String),
}

#[derive(Default)]
pub struct InstallRegistry {
    inner: Mutex<HashMap<String, CancellationToken>>,
}

impl InstallRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// id에 대한 새 cancel token 등록. 이미 존재하면 거부.
    pub fn try_start(&self, id: &str) -> Result<CancellationToken, InstallRegistryError> {
        let mut g = self.inner.lock().expect("InstallRegistry poisoned");
        if g.contains_key(id) {
            return Err(InstallRegistryError::AlreadyInstalling(id.to_string()));
        }
        let tok = CancellationToken::new();
        g.insert(id.to_string(), tok.clone());
        Ok(tok)
    }

    /// id 등록 해제. 미존재면 no-op.
    pub fn finish(&self, id: &str) {
        let mut g = self.inner.lock().expect("InstallRegistry poisoned");
        g.remove(id);
    }

    /// id에 해당하는 install을 cancel. 미존재면 no-op.
    pub fn cancel(&self, id: &str) {
        let g = self.inner.lock().expect("InstallRegistry poisoned");
        if let Some(tok) = g.get(id) {
            tok.cancel();
        }
    }

    /// 모든 in-flight install cancel — 앱 종료 시 호출.
    pub fn cancel_all(&self) {
        let g = self.inner.lock().expect("InstallRegistry poisoned");
        for tok in g.values() {
            tok.cancel();
        }
    }

    /// 현재 등록된 id 수 — 디버그/메트릭용.
    pub fn in_flight_count(&self) -> usize {
        self.inner.lock().expect("InstallRegistry poisoned").len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_start_new_id_succeeds() {
        let r = InstallRegistry::new();
        let tok = r.try_start("ollama").expect("first start ok");
        assert!(!tok.is_cancelled());
        assert_eq!(r.in_flight_count(), 1);
    }

    #[test]
    fn try_start_duplicate_rejects() {
        let r = InstallRegistry::new();
        let _t1 = r.try_start("ollama").unwrap();
        let r2 = r.try_start("ollama");
        assert!(matches!(
            r2,
            Err(InstallRegistryError::AlreadyInstalling(_))
        ));
    }

    #[test]
    fn finish_removes_entry() {
        let r = InstallRegistry::new();
        let _ = r.try_start("ollama").unwrap();
        r.finish("ollama");
        assert_eq!(r.in_flight_count(), 0);
        // 다시 try_start 가능.
        assert!(r.try_start("ollama").is_ok());
    }

    #[test]
    fn finish_unknown_is_noop() {
        let r = InstallRegistry::new();
        r.finish("nope"); // panic 안 함.
        assert_eq!(r.in_flight_count(), 0);
    }

    #[test]
    fn cancel_marks_token() {
        let r = InstallRegistry::new();
        let tok = r.try_start("ollama").unwrap();
        r.cancel("ollama");
        assert!(tok.is_cancelled());
    }

    #[test]
    fn cancel_unknown_is_noop() {
        let r = InstallRegistry::new();
        r.cancel("nope"); // panic 안 함.
    }

    #[test]
    fn cancel_all_marks_every_token() {
        let r = InstallRegistry::new();
        let t1 = r.try_start("ollama").unwrap();
        let t2 = r.try_start("lm-studio").unwrap();
        r.cancel_all();
        assert!(t1.is_cancelled());
        assert!(t2.is_cancelled());
    }
}
