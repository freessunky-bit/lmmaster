//! In-flight bench registry — model_id ↔ CancellationToken.
//!
//! 정책 (Phase 2'.c.2 — install/registry.rs 패턴 차용):
//! - try_start(id) → AlreadyRunning 거부 (대부분 user 패턴은 단일 측정).
//! - finish(id) → 완료/실패 시 명시 호출.
//! - cancel(id) → idempotent, 미존재 no-op.
//! - cancel_all() → 앱 종료 시.

use std::collections::HashMap;
use std::sync::Mutex;

use thiserror::Error;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Error)]
pub enum BenchRegistryError {
    #[error("이 모델은 이미 측정 중이에요 (id={0})")]
    AlreadyRunning(String),
}

#[derive(Default)]
pub struct BenchRegistry {
    inner: Mutex<HashMap<String, CancellationToken>>,
}

impl BenchRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn try_start(&self, id: &str) -> Result<CancellationToken, BenchRegistryError> {
        let mut g = self.inner.lock().expect("BenchRegistry poisoned");
        if g.contains_key(id) {
            return Err(BenchRegistryError::AlreadyRunning(id.to_string()));
        }
        let tok = CancellationToken::new();
        g.insert(id.to_string(), tok.clone());
        Ok(tok)
    }

    pub fn finish(&self, id: &str) {
        let mut g = self.inner.lock().expect("BenchRegistry poisoned");
        g.remove(id);
    }

    pub fn cancel(&self, id: &str) {
        let g = self.inner.lock().expect("BenchRegistry poisoned");
        if let Some(tok) = g.get(id) {
            tok.cancel();
        }
    }

    pub fn cancel_all(&self) {
        let g = self.inner.lock().expect("BenchRegistry poisoned");
        for tok in g.values() {
            tok.cancel();
        }
    }

    pub fn in_flight_count(&self) -> usize {
        self.inner.lock().expect("BenchRegistry poisoned").len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_start_new_id_succeeds() {
        let r = BenchRegistry::new();
        let tok = r.try_start("exaone").expect("first start ok");
        assert!(!tok.is_cancelled());
        assert_eq!(r.in_flight_count(), 1);
    }

    #[test]
    fn try_start_duplicate_rejects() {
        let r = BenchRegistry::new();
        let _t1 = r.try_start("exaone").unwrap();
        let r2 = r.try_start("exaone");
        assert!(matches!(r2, Err(BenchRegistryError::AlreadyRunning(_))));
    }

    #[test]
    fn finish_removes_entry() {
        let r = BenchRegistry::new();
        let _ = r.try_start("x").unwrap();
        r.finish("x");
        assert_eq!(r.in_flight_count(), 0);
        assert!(r.try_start("x").is_ok());
    }

    #[test]
    fn finish_unknown_is_noop() {
        let r = BenchRegistry::new();
        r.finish("nope");
    }

    #[test]
    fn cancel_marks_token() {
        let r = BenchRegistry::new();
        let tok = r.try_start("x").unwrap();
        r.cancel("x");
        assert!(tok.is_cancelled());
    }

    #[test]
    fn cancel_unknown_is_noop() {
        let r = BenchRegistry::new();
        r.cancel("nope");
    }

    #[test]
    fn cancel_all_marks_every_token() {
        let r = BenchRegistry::new();
        let t1 = r.try_start("a").unwrap();
        let t2 = r.try_start("b").unwrap();
        r.cancel_all();
        assert!(t1.is_cancelled());
        assert!(t2.is_cancelled());
    }
}
