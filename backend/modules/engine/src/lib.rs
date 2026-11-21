
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod lc0_orchestrator;
pub mod parser;
pub mod process;
pub mod uci;

pub use lc0_orchestrator::{Lc0Analysis, Lc0Orchestrator};

/// Errors that can occur during chess engine operations.
#[derive(Debug, Error)]
pub enum EngineError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Engine process is not running")]
    NotRunning,

    #[error("Engine operation timed out")]
    Timeout,

    #[error("Failed to parse engine response: {0}")]
    ParseError(String),

    #[error("Engine returned an invalid response: {0}")]
    InvalidResponse(String),

    #[error("Engine operation failed: {0}")]
    Unknown(String),
}

/// Parameters controlling an engine search.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GoParams {
    /// Maximum search depth in plies.
    pub depth: Option<u16>,

    /// Maximum search duration in milliseconds.
    pub time_limit_ms: Option<u64>,

    /// Restrict search to the specified UCI moves.
    pub search_moves: Option<Vec<String>>,
}

/// Result returned after an engine search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineResult {
    /// Best move found. None when no legal move exists.
    pub best_move: Option<String>,

    /// Position evaluation in pawns.
    /// Positive values favor the side to move.
    pub evaluation: Option<f32>,

    /// Search depth reached, in plies.
    pub depth: Option<u16>,

    /// Principal variation (best continuation).
    #[serde(default)]
    pub principal_variation: Vec<String>,
}

/// Common interface for chess engine implementations.
#[async_trait]
pub trait Engine: Send + Sync {
    /// Starts a search using the supplied parameters.
    async fn go(
        &mut self,
        params: GoParams,
    ) -> Result<EngineResult, EngineError>;

    /// Stops the current search.
    async fn stop(&mut self) -> Result<(), EngineError>;

    /// Sets the current board position using FEN notation.
    async fn set_position(
        &mut self,
        fen: &str,
    ) -> Result<(), EngineError>;

    /// Checks whether the engine is ready to receive commands.
    async fn is_ready(&mut self) -> Result<bool, EngineError>;

    /// Terminates the engine process gracefully.
    async fn quit(&mut self) -> Result<(), EngineError>;
}
