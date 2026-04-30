//! In-flight chat registry — chat_id ↔ CancellationToken 매핑.
//!
//! 정책: 동시 다중 채팅 허용 (사용자가 여러 메시지 빠르게 보내면 각각 별도). cancel_all 호출 시
//! 모든 진행 채팅 abort — 앱 종료 시 호출.

use std::collections::HashMap;
use std::sync::Mutex;

use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub struct ChatRegistry {
    inner: Mutex<HashMap<String, CancellationToken>>,
}

impl ChatRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&self, chat_id: &str) -> CancellationToken {
        let mut g = self.inner.lock().expect("ChatRegistry poisoned");
        let tok = CancellationToken::new();
        g.insert(chat_id.to_string(), tok.clone());
        tok
    }

    pub fn finish(&self, chat_id: &str) {
        let mut g = self.inner.lock().expect("ChatRegistry poisoned");
        g.remove(chat_id);
    }

    pub fn cancel_all(&self) {
        let g = self.inner.lock().expect("ChatRegistry poisoned");
        for tok in g.values() {
            tok.cancel();
        }
    }

    pub fn in_flight_count(&self) -> usize {
        self.inner.lock().expect("ChatRegistry poisoned").len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_registers_token() {
        let r = ChatRegistry::new();
        let t = r.start("c1");
        assert!(!t.is_cancelled());
        assert_eq!(r.in_flight_count(), 1);
    }

    #[test]
    fn finish_removes() {
        let r = ChatRegistry::new();
        let _ = r.start("c1");
        r.finish("c1");
        assert_eq!(r.in_flight_count(), 0);
    }

    #[test]
    fn cancel_all_marks_all() {
        let r = ChatRegistry::new();
        let t1 = r.start("c1");
        let t2 = r.start("c2");
        r.cancel_all();
        assert!(t1.is_cancelled());
        assert!(t2.is_cancelled());
    }
}
