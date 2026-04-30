//! API 키 발급/회수/조회 IPC — Phase 3'.b.
//!
//! Phase 8'.0.a (ADR-0035) — `migrate` 서브모듈 추가: SQLCipher passphrase 부트스트랩 + 평문 →
//! 암호화 마이그레이션.

pub mod commands;
pub mod migrate;

pub use commands::{
    create_api_key, list_api_keys, revoke_api_key, update_api_key_pipelines, update_api_key_scope,
};
pub use migrate::{provision, KeyStoreMode, MigrationOutcome};
