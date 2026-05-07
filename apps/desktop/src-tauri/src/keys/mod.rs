//! API 키 발급/회수/조회 IPC — Phase 3'.b.
//!
//! Phase 8'.0.a (ADR-0035) — `migrate` 서브모듈 추가: SQLCipher passphrase 부트스트랩 + 평문 →
//! 암호화 마이그레이션.

pub mod commands;
pub mod migrate;

pub use commands::{
    create_api_key, list_api_keys, revoke_api_key, update_api_key_pipelines, update_api_key_scope,
};
// Phase R-F+R-G hotfix (ADR-0064 §5) — provision은 provision_v2로 교체.
// 단일 경로 모델 + magic bytes 감지 + atomic 2-phase rename + crash recovery.
pub use migrate::{provision_v2, KeyStoreMode, MigrationOutcome};
