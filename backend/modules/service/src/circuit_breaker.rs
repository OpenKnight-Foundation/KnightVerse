use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::Instant;

/// Circuit breaker state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation — requests pass through.
    Closed,
    /// Failing fast — requests are rejected immediately.
    Open,
    /// Testing recovery — a limited number of requests are allowed through.
    HalfOpen,
}

impl fmt::Display for CircuitState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "closed"),
            CircuitState::Open => write!(f, "open"),
            CircuitState::HalfOpen => write!(f, "half-open"),
        }
    }
}

/// Configuration for the circuit breaker.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening the circuit.
    pub failure_threshold: u32,
    /// Number of consecutive successes in HalfOpen state to close the circuit.
    pub success_threshold: u32,
    /// How long the circuit stays Open before transitioning to HalfOpen.
    pub open_timeout: Duration,
    /// Maximum time a request is allowed to take before being considered a timeout failure.
    pub request_timeout: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            open_timeout: Duration::from_secs(30),
            request_timeout: Duration::from_secs(10),
        }
    }
}

/// Errors returned by the circuit breaker.
#[derive(Debug)]
pub enum CircuitBreakerError {
    /// The circuit is open — requests are being rejected to fail fast.
    CircuitOpen,
    /// The underlying operation timed out.
    OperationTimeout,
    /// The underlying operation failed.
    OperationFailed(String),
}

impl fmt::Display for CircuitBreakerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CircuitBreakerError::CircuitOpen => {
                write!(f, "Circuit breaker is open — engine is unavailable")
            }
            CircuitBreakerError::OperationTimeout => {
                write!(f, "Engine operation timed out")
            }
            CircuitBreakerError::OperationFailed(msg) => {
                write!(f, "Engine operation failed: {}", msg)
            }
        }
    }
}

impl std::error::Error for CircuitBreakerError {}

/// Inner state tracked by the circuit breaker.
struct InnerState {
    state: CircuitState,
    consecutive_failures: u32,
    consecutive_successes: u32,
    opened_at: Option<Instant>,
    config: CircuitBreakerConfig,
}

impl InnerState {
    fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: CircuitState::Closed,
            consecutive_failures: 0,
            consecutive_successes: 0,
            opened_at: None,
            config,
        }
    }

    /// Called before executing the wrapped operation.
    /// Returns Err(CircuitOpen) if the circuit breaker rejects the request.
    fn before_call(&mut self) -> Result<(), CircuitBreakerError> {
        match self.state {
            CircuitState::Closed => Ok(()),
            CircuitState::Open => {
                // Check if the open timeout has elapsed
                if let Some(opened_at) = self.opened_at {
                    if opened_at.elapsed() >= self.config.open_timeout {
                        self.state = CircuitState::HalfOpen;
                        self.consecutive_successes = 0;
                        log::info!("Circuit breaker transitioning from open to half-open");
                        return Ok(());
                    }
                }
                Err(CircuitBreakerError::CircuitOpen)
            }
            CircuitState::HalfOpen => Ok(()),
        }
    }

    /// Called on success.
    fn on_success(&mut self) {
        self.consecutive_failures = 0;
        match self.state {
            CircuitState::Closed => {
                // Reset success counter in closed state (don't really need it)
                self.consecutive_successes = 0;
            }
            CircuitState::HalfOpen => {
                self.consecutive_successes += 1;
                if self.consecutive_successes >= self.config.success_threshold {
                    self.state = CircuitState::Closed;
                    self.consecutive_successes = 0;
                    log::info!("Circuit breaker closed after successful half-open tests");
                }
            }
            CircuitState::Open => {
                // Should not happen — before_call prevents this
            }
        }
    }

    /// Called on failure.
    fn on_failure(&mut self) {
        self.consecutive_successes = 0;
        self.consecutive_failures += 1;
        match self.state {
            CircuitState::Closed => {
                if self.consecutive_failures >= self.config.failure_threshold {
                    self.state = CircuitState::Open;
                    self.opened_at = Some(Instant::now());
                    log::warn!(
                        "Circuit breaker opened after {} consecutive failures",
                        self.consecutive_failures
                    );
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open re-opens the circuit
                self.state = CircuitState::Open;
                self.opened_at = Some(Instant::now());
                log::warn!("Circuit breaker re-opened after failure in half-open state");
            }
            CircuitState::Open => {
                // Already open — keep counting but state stays open
            }
        }
    }
}

/// A circuit breaker that wraps fallible async operations.
///
/// Designed to protect the Rust backend thread pool from hanging
/// when the external AI/Python engine is unresponsive.
pub struct CircuitBreaker {
    inner: Arc<Mutex<InnerState>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration.
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(InnerState::new(config))),
        }
    }

    /// Create a new circuit breaker with default configuration.
    pub fn default() -> Self {
        Self::new(CircuitBreakerConfig::default())
    }

    /// Execute an async operation through the circuit breaker.
    ///
    /// If the circuit is open, returns `CircuitBreakerError::CircuitOpen` immediately.
    /// If the operation times out, returns `CircuitBreakerError::OperationTimeout`
    /// and records a failure.
    /// Otherwise, records success or failure based on the result.
    pub async fn call<F, Fut, T, E>(
        &self,
        operation: F,
    ) -> Result<T, CircuitBreakerError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        // Check circuit state
        {
            let mut inner = self.inner.lock().await;
            inner.before_call()?;
        }

        // Execute with timeout
        let config = {
            let inner = self.inner.lock().await;
            inner.config.clone()
        };

        let result = tokio::time::timeout(config.request_timeout, operation()).await;

        match result {
            Ok(Ok(value)) => {
                // Success
                let mut inner = self.inner.lock().await;
                inner.on_success();
                Ok(value)
            }
            Ok(Err(e)) => {
                // Operation failed
                let mut inner = self.inner.lock().await;
                inner.on_failure();
                Err(CircuitBreakerError::OperationFailed(e.to_string()))
            }
            Err(_elapsed) => {
                // Timeout
                let mut inner = self.inner.lock().await;
                inner.on_failure();
                Err(CircuitBreakerError::OperationTimeout)
            }
        }
    }

    /// Return the current state of the circuit breaker.
    pub async fn state(&self) -> CircuitState {
        let inner = self.inner.lock().await;
        inner.state
    }

    /// Reset the circuit breaker to its initial closed state.
    pub async fn reset(&self) {
        let mut inner = self.inner.lock().await;
        inner.state = CircuitState::Closed;
        inner.consecutive_failures = 0;
        inner.consecutive_successes = 0;
        inner.opened_at = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_closed_state_passes_through() {
        let cb = CircuitBreaker::default();
        let result = cb
            .call(|| async { Ok::<&str, &str>("success") })
            .await;
        assert!(result.is_ok());
        assert_eq!(cb.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_opens_after_failures() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            open_timeout: Duration::from_secs(60),
            request_timeout: Duration::from_secs(5),
        });

        // Two failures should open the circuit
        let _ = cb.call(|| async { Err::<(), &str>("fail") }).await;
        let _ = cb.call(|| async { Err::<(), &str>("fail") }).await;

        assert_eq!(cb.state().await, CircuitState::Open);

        // Third call should be rejected immediately
        let result = cb.call(|| async { Ok::<&str, &str>("should not run") }).await;
        assert!(matches!(result, Err(CircuitBreakerError::CircuitOpen)));
    }

    #[tokio::test]
    async fn test_half_open_recovery() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 2,
            open_timeout: Duration::from_millis(10), // Very short timeout for test
            request_timeout: Duration::from_secs(5),
        });

        // Open the circuit
        let _ = cb.call(|| async { Err::<(), &str>("fail") }).await;
        assert_eq!(cb.state().await, CircuitState::Open);

        // Wait for timeout to elapse
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Now it should be half-open and accept requests
        let result = cb.call(|| async { Ok::<&str, &str>("success") }).await;
        assert!(result.is_ok());

        // Need one more success to close
        let result = cb.call(|| async { Ok::<&str, &str>("success") }).await;
        assert!(result.is_ok());
        assert_eq!(cb.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_half_open_failure_reopens() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 2,
            open_timeout: Duration::from_millis(10),
            request_timeout: Duration::from_secs(5),
        });

        // Open the circuit
        let _ = cb.call(|| async { Err::<(), &str>("fail") }).await;
        assert_eq!(cb.state().await, CircuitState::Open);

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Half-open: first success
        let _ = cb.call(|| async { Ok::<&str, &str>("ok") }).await;

        // Half-open: failure should re-open
        let _ = cb.call(|| async { Err::<(), &str>("fail again") }).await;
        assert_eq!(cb.state().await, CircuitState::Open);
    }
}
