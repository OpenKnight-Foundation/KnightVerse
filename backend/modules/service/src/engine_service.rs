
use crate::circuit_breaker::{
    CircuitBreaker,
    CircuitBreakerConfig,
    CircuitBreakerError,
};
use engine::{
    process::ProcessEngine,
    Engine,
    EngineError,
    EngineResult,
    GoParams,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, warn};
use uuid::Uuid;

/// Service responsible for managing chess engine operations.
///
/// Engine requests are protected by a circuit breaker to prevent
/// repeated failures from overwhelming the engine process.
pub struct EngineService {
    #[allow(dead_code)]
    engines: Arc<Mutex<HashMap<Uuid, Box<dyn Engine>>>>,
    engine_path: String,
    circuit_breaker: CircuitBreaker,
}

impl EngineService {
    /// Create a new engine service using the default circuit breaker configuration.
    pub fn new(engine_path: String) -> Self {
        Self::with_circuit_breaker_config(
            engine_path,
            CircuitBreakerConfig::default(),
        )
    }

    /// Create an engine service with a custom circuit breaker configuration.
    pub fn with_circuit_breaker_config(
        engine_path: String,
        cb_config: CircuitBreakerConfig,
    ) -> Self {
        Self {
            engines: Arc::new(Mutex::new(HashMap::new())),
            engine_path,
            circuit_breaker: CircuitBreaker::new(cb_config),
        }
    }

    /// Request the best move suggestion for a given chess position.
    ///
    /// Supports optional search depth and time limits.
    /// Each request creates an engine process, executes the search,
    /// and attempts to shut down the process afterward.
    pub async fn get_suggestion(
        &self,
        fen: &str,
        depth: Option<u8>,
        time_limit_ms: Option<u32>,
    ) -> Result<EngineResult, EngineError> {
        let fen = fen.to_owned();
        let engine_path = self.engine_path.clone();

        let result = self
            .circuit_breaker
            .call(move || {
                let engine_path = engine_path.clone();
                let fen = fen.clone();

                async move {
                    let mut engine = ProcessEngine::new(&engine_path).await?;

                    // Execute the engine operation separately so that
                    // cleanup is attempted even when an operation fails.
                    let operation_result = async {
                        engine.is_ready().await?;
                        engine.set_position(&fen).await?;

                        let params = GoParams {
                            depth,
                            time_limit_ms,
                            search_moves: None,
                        };

                        engine.go(params).await
                    }
                    .await;

                    // Always attempt to release the engine process.
                    if let Err(cleanup_error) = engine.quit().await {
                        warn!(
                            error = %cleanup_error,
                            "Failed to shut down chess engine process"
                        );
                    }

                    operation_result
                }
            })
            .await;

        match result {
            Ok(engine_result) => Ok(engine_result),

            Err(CircuitBreakerError::CircuitOpen) => {
                warn!("Engine request rejected: circuit breaker is open");

                Err(EngineError::Unknown(
                    "Engine is temporarily unavailable (circuit breaker open)"
                        .to_string(),
                ))
            }

            Err(CircuitBreakerError::OperationTimeout) => {
                error!("Chess engine operation timed out");

                Err(EngineError::Timeout)
            }

            Err(CircuitBreakerError::OperationFailed(message)) => {
                error!(
                    error = %message,
                    "Chess engine operation failed"
                );

                Err(EngineError::Unknown(format!(
                    "Engine failure: {message}"
                )))
            }
        }
    }

    /// Analyze a chess position using a specified search depth.
    ///
    /// This is a convenience wrapper around `get_suggestion`.
    pub async fn analyze_position(
        &self,
        fen: &str,
        depth: u8,
    ) -> Result<EngineResult, EngineError> {
        self.get_suggestion(fen, Some(depth), None).await
    }

    /// Retrieve the current circuit breaker state for monitoring.
    pub async fn circuit_breaker_state(
        &self,
    ) -> crate::circuit_breaker::CircuitState {
        self.circuit_breaker.state().await
    }
}
