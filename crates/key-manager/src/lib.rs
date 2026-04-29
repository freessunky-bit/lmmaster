//! crate: key-manager — API 키 발급/검증/scope/사용 로그.
//!
//! 정책 (ADR-0007, ADR-0022):
//! - v1은 자체 경량 구현. LiteLLM 의존 금지.
//! - argon2id 해시 (mem 64MB / iter 3 / par 1, OWASP 2024).
//! - SQLite + key_prefix 인덱스 lookup → argon2 verify로 narrow.
//! - Origin 정확 매칭 (scheme + host + port). 미매치 시 거부.
//! - 평문 키는 1회 응답에서만 표시 후 폐기.

pub mod hash;
pub mod manager;
pub mod middleware;
pub mod plaintext;
pub mod scope;
pub mod store;

pub use hash::{hash_key, verify_key, HashError};
pub use manager::{AuthOutcome, IssueRequest, IssuedKey, KeyManager, KeyManagerError};
pub use plaintext::{generate as generate_plaintext, prefix_of, GeneratedKey};
pub use scope::{glob_match, RateLimit, Scope};
pub use store::{ApiKeyRow, KeyStore, StoreError};
