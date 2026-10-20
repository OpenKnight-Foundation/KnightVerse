use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod lc0_orchestrator;
pub mod parser;
pub mod process;
pub mod uci;

pub use lc0_orchestrator::{Lc0Analysis, Lc0Orchestrator};

#[derive(Error, Debug)]
pub enum EngineError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Engine process not running")]
    NotRunning,
    #[error("Engine timeout")]
    Timeout,
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("Unknown error: {0}")]
    Unknown(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GoParams {
    pub depth: Option<u8>,
    pub time_limit_ms: Option<u32>,
    pub search_moves: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineResult {
    pub best_move: String,
    pub evaluation: Option<f32>,
    pub depth: Option<u8>,
    pub principal_variation: Vec<String>,
}

#[async_trait]
pub trait Engine: Send + Sync {
    async fn go(&mut self, params: GoParams) -> Result<EngineResult, EngineError>;
    async fn stop(&mut self) -> Result<(), EngineError>;
    async fn set_position(&mut self, fen: &str) -> Result<(), EngineError>;
    async fn is_ready(&mut self) -> Result<bool, EngineError>;
    async fn quit(&mut self) -> Result<(), EngineError>;
}
