//! crate: shared-types — LMmaster의 모든 crate가 공유하는 도메인 타입.
//!
//! 이 crate는 다른 LMmaster crate에 의존하지 않는다 (의존 방향 규칙).

pub mod intents;
pub use intents::{intent_label_ko, is_registered_intent, IntentId, INTENT_VOCABULARY};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelRef {
    pub id: String,
    pub display_name: String,
    pub category: ModelCategory,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum ModelCategory {
    AgentGeneral,
    Roleplay,
    Coding,
    SoundStt,
    SoundTts,
    Slm,
    Embeddings,
    Rerank,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeKind {
    LlamaCpp,
    KoboldCpp,
    Ollama,
    LmStudio,
    Vllm,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeState {
    NotInstalled,
    Downloading,
    Verifying,
    Extracting,
    Cold,
    WarmingUp,
    Standby,
    Active,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CapabilityMatrix {
    pub vision: bool,
    pub tools: bool,
    pub structured_output: bool,
    pub embeddings: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostFingerprint {
    pub os: String,
    pub arch: String,
    pub cpu: String,
    pub ram_mb: u64,
    pub gpu_vendor: Option<String>,
    pub gpu_model: Option<String>,
    pub vram_mb: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
pub enum LmError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
    #[error("{0}")]
    Other(String),
}

pub type LmResult<T> = Result<T, LmError>;
