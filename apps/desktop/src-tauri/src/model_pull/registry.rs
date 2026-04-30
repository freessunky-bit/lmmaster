//! In-flight 모델 풀 registry — model_id ↔ CancellationToken 매핑.
//!
//! 정책 (phase-install-bench-bugfix-decision §2.2):
//! - InstallRegistry 미러 — 동일 패턴으로 멘탈 모델 일치.
//! - try_start: 같은 model_id 중복 거부.
//! - finish: RAII guard에서 호출.
//! - cancel: idempotent.
//! - cancel_all: 앱 종료 시 호출.

use std::collections::HashMap;
use std::sync::Mutex;

use thiserror::Error;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Error)]
pub enum ModelPullRegistryError {
    #[error("이미 받고 있는 모델이에요 (id={0})")]
    AlreadyPulling(String),
}

#[derive(Default)]
pub struct ModelPullRegistry {
    inner: Mutex<HashMap<String, CancellationToken>>,
}

impl ModelPullRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn try_start(&self, model_id: &str) -> Result<CancellationToken, ModelPullRegistryError> {
        let mut g = self.inner.lock().expect("ModelPullRegistry poisoned");
        if g.contains_key(model_id) {
            return Err(ModelPullRegistryError::AlreadyPulling(model_id.to_string()));
        }
        let tok = CancellationToken::new();
        g.insert(model_id.to_string(), tok.clone());
        Ok(tok)
    }

    pub fn finish(&self, model_id: &str) {
        let mut g = self.inner.lock().expect("ModelPullRegistry poisoned");
        g.remove(model_id);
    }

    pub fn cancel(&self, model_id: &str) {
        let g = self.inner.lock().expect("ModelPullRegistry poisoned");
        if let Some(tok) = g.get(model_id) {
            tok.cancel();
        }
    }

    pub fn cancel_all(&self) {
        let g = self.inner.lock().expect("ModelPullRegistry poisoned");
        for tok in g.values() {
            tok.cancel();
        }
    }

    pub fn in_flight_count(&self) -> usize {
        self.inner.lock().expect("ModelPullRegistry poisoned").len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_start_new_id_succeeds() {
        let r = ModelPullRegistry::new();
        let tok = r.try_start("polyglot-ko").expect("first start ok");
        assert!(!tok.is_cancelled());
        assert_eq!(r.in_flight_count(), 1);
    }

    #[test]
    fn try_start_duplicate_rejects() {
        let r = ModelPullRegistry::new();
        let _t1 = r.try_start("polyglot-ko").unwrap();
        let r2 = r.try_start("polyglot-ko");
        assert!(matches!(r2, Err(ModelPullRegistryError::AlreadyPulling(_))));
    }

    #[test]
    fn finish_removes_entry() {
        let r = ModelPullRegistry::new();
        let _ = r.try_start("polyglot-ko").unwrap();
        r.finish("polyglot-ko");
        assert_eq!(r.in_flight_count(), 0);
        assert!(r.try_start("polyglot-ko").is_ok());
    }

    #[test]
    fn finish_unknown_is_noop() {
        let r = ModelPullRegistry::new();
        r.finish("nope");
        assert_eq!(r.in_flight_count(), 0);
    }

    #[test]
    fn cancel_marks_token() {
        let r = ModelPullRegistry::new();
        let tok = r.try_start("polyglot-ko").unwrap();
        r.cancel("polyglot-ko");
        assert!(tok.is_cancelled());
    }

    #[test]
    fn cancel_unknown_is_noop() {
        let r = ModelPullRegistry::new();
        r.cancel("nope");
    }

    #[test]
    fn cancel_all_marks_every_token() {
        let r = ModelPullRegistry::new();
        let t1 = r.try_start("polyglot-ko").unwrap();
        let t2 = r.try_start("exaone").unwrap();
        r.cancel_all();
        assert!(t1.is_cancelled());
        assert!(t2.is_cancelled());
    }
}
