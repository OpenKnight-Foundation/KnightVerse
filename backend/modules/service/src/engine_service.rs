use crate::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError};
use engine::{process::ProcessEngine, Engine, EngineError, EngineResult, GoParams};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, warn};
use uuid::Uuid;

pub struct EngineService {
    #[allow(dead_code)]
    engines: Arc<Mutex<HashMap<Uuid, Box<dyn Engine>>>>,
    engine_path: String,
    circuit_breaker: CircuitBreaker,
}

impl EngineService {
    pub fn new(engine_path: String) -> Self {
        Self {
            engines: Arc::new(Mutex::new(HashMap::new())),
            engine_path,
            circuit_breaker: CircuitBreaker::default(),
        }
    }

    /// Create a new EngineService with a custom circuit breaker configuration.
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

    pub async fn get_suggestion(
        &self,
        fen: &str,
        depth: Option<u8>,
        time_limit_ms: Option<u32>,
    ) -> Result<EngineResult, EngineError> {
        let fen = fen.to_string();
        let engine_path = self.engine_path.clone();

        let result = self
            .circuit_breaker
            .call(move || {
                let engine_path = engine_path.clone();
                let fen = fen.clone();
                async move {
                    let mut engine: ProcessEngine = ProcessEngine::new(&engine_path).await?;
                    engine.is_ready().await?;
                    engine.set_position(&fen).await?;

                    let params = GoParams {
                        depth,
                        time_limit_ms,
                        search_moves: None,
                    };

                    let result = engine.go(params).await?;
                    let _ = engine.quit().await;

                    Ok::<EngineResult, EngineError>(result)
                }
            })
            .await;

        match result {
            Ok(engine_result) => Ok(engine_result),
            Err(CircuitBreakerError::CircuitOpen) => {
                warn!("Circuit breaker is open — engine request rejected");
                Err(EngineError::Unknown(
                    "Engine is temporarily unavailable (circuit breaker open)".to_string(),
                ))
            }
            Err(CircuitBreakerError::OperationTimeout) => {
                error!("Engine operation timed out");
                Err(EngineError::Timeout)
            }
            Err(CircuitBreakerError::OperationFailed(msg)) => {
                error!("Engine operation failed: {}", msg);
                Err(EngineError::Unknown(format!("Engine failure: {}", msg)))
            }
        }
    }

    pub async fn analyze_position(
        &self,
        fen: &str,
        depth: u8,
    ) -> Result<EngineResult, EngineError> {
        self.get_suggestion(fen, Some(depth), None).await
    }

    /// Return the current state of the circuit breaker for monitoring.
    pub async fn circuit_breaker_state(&self) -> crate::circuit_breaker::CircuitState {
        self.circuit_breaker.state().await
    }
}
