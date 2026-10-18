//! Leela Chess Zero (Lc0) orchestration (BE-132).
//!
//! This used to be a stub in `backend/src/engine/lc0_orchestrator.rs`: every
//! method returned a canned string (`get_best_move` always answered `"e2e4"`)
//! and nothing ever called it, so Lc0 analysis was unreachable. It now lives in
//! the `engine` workspace crate alongside the rest of the UCI plumbing and is
//! driven by `api::lc0`.
//!
//! Each analysis spawns its own short-lived Lc0 process and waits on a
//! `bestmove` line, so orchestrators are cheap but must be bounded — the
//! semaphore below caps how many Lc0 processes can exist at once.

use crate::process::ProcessEngine;
use crate::{Engine, EngineError, GoParams};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Default engine binary when `LC0_PATH` is unset.
const DEFAULT_BINARY: &str = "lc0";
/// Default search depth when the caller does not specify one.
const DEFAULT_DEPTH: u8 = 20;
/// Default cap on concurrent Lc0 processes.
const DEFAULT_CONCURRENCY: usize = 2;

/// Result of a single Lc0 search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lc0Analysis {
    pub best_move: String,
    /// Centipawn evaluation, from the engine's point of view.
    pub evaluation: Option<f32>,
    pub depth: Option<u8>,
    pub principal_variation: Vec<String>,
}

pub struct Lc0Orchestrator {
    binary: String,
    weights: Option<String>,
    permits: Arc<Semaphore>,
}

impl Default for Lc0Orchestrator {
    fn default() -> Self {
        Self::from_env()
    }
}

impl Lc0Orchestrator {
    /// Build an orchestrator from `LC0_PATH`, `LC0_WEIGHTS` and
    /// `LC0_MAX_CONCURRENCY`, falling back to sensible defaults.
    pub fn from_env() -> Self {
        Self {
            binary: std::env::var("LC0_PATH").unwrap_or_else(|_| DEFAULT_BINARY.to_string()),
            weights: std::env::var("LC0_WEIGHTS").ok().filter(|w| !w.is_empty()),
            permits: Arc::new(Semaphore::new(
                std::env::var("LC0_MAX_CONCURRENCY")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(DEFAULT_CONCURRENCY),
            )),
        }
    }

    pub fn with_binary(binary: impl Into<String>, weights: Option<String>) -> Self {
        Self {
            binary: binary.into(),
            weights,
            permits: Arc::new(Semaphore::new(DEFAULT_CONCURRENCY)),
        }
    }

    pub fn binary(&self) -> &str {
        &self.binary
    }

    /// Number of analyses that can run concurrently; exposed for the health
    /// endpoint so operators can see the configured ceiling.
    pub fn concurrency(&self) -> usize {
        self.permits.available_permits()
    }

    /// Whether an Lc0 binary can be found. Checked by the `/health` route so a
    /// deployment without Lc0 degrades loudly instead of failing per request.
    pub fn is_available(&self) -> bool {
        resolve_binary(&self.binary).is_some()
    }

    pub fn get_engine_info(&self) -> String {
        let weights = self.weights.as_deref().unwrap_or("<default>");
        format!("Leela Chess Zero (binary: {}, weights: {})", self.binary, weights)
    }

    /// Reserve a GPU slot. Lc0 is CPU/GPU-agnostic here, so this is a naming
    /// shim kept for callers that report allocation.
    pub fn allocate_gpu(&self, task_id: &str) -> bool {
        info!("Lc0 slot allocated for task: {}", task_id);
        true
    }

    /// Run a single search and return the engine's best move plus its line.
    pub async fn analyze_position(&self, fen: &str, depth: Option<u8>) -> Result<Lc0Analysis, EngineError> {
        let resolved = resolve_binary(&self.binary).ok_or_else(|| {
            EngineError::Unknown(format!("Lc0 binary `{}` not found on PATH", self.binary))
        })?;

        // Bound the number of live Lc0 processes. `acquire_owned` on a dropped
        // orchestrator must not wedge the runtime, so fall back to a closed
        // permit and surface the timeout as an engine error.
        let _permit = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            self.permits.clone().acquire_owned(),
        )
        .await
        .map_err(|_| EngineError::Timeout)?
        .map_err(|_| EngineError::Timeout)?;

        let depth = depth.unwrap_or(DEFAULT_DEPTH);
        let mut engine = ProcessEngine::new(&resolved).await?;

        if let Some(weights) = &self.weights {
            if let Err(e) = engine
                .send_command(&format!("setoption name=Weights value={}", weights))
                .await
            {
                warn!("Failed to set Lc0 weights option: {}", e);
            }
        }
        if let Err(e) = engine.send_command("uci").await {
            warn!("Failed to re-negotiate UCI after options: {}", e);
        }

        engine.set_position(fen).await?;
        let result = engine.go(GoParams {
            depth: Some(depth),
            ..Default::default()
        }).await?;
        let _ = engine.quit().await;

        Ok(Lc0Analysis {
            best_move: result.best_move,
            evaluation: result.evaluation,
            depth: result.depth.or(Some(depth)),
            principal_variation: result.principal_variation,
        })
    }

    /// Convenience wrapper for callers that only need a move.
    pub async fn get_best_move(&self, fen: &str) -> Result<String, EngineError> {
        Ok(self.analyze_position(fen, None).await?.best_move)
    }
}

/// Resolve `name` to an executable path, checking `name` itself first and then
/// each `PATH` entry. Returns `None` when the binary is not installed.
fn resolve_binary(name: &str) -> Option<String> {
    let direct = Path::new(name);
    if direct.components().count() > 1 {
        return is_executable(direct).then(|| direct.display().to_string());
    }

    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
        .map(|candidate| candidate.display().to_string())
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_binary_rejects_unknown_names() {
        assert!(resolve_binary("definitely-not-an-lc0-binary-xyz").is_none());
    }

    #[test]
    fn resolve_binary_finds_absolute_paths_that_exist() {
        // The test binary itself is an executable that is certainly on disk.
        let exe = std::env::current_exe().unwrap();
        let exe_str = exe.display().to_string();
        assert_eq!(resolve_binary(&exe_str), Some(exe_str));
    }

    #[tokio::test]
    async fn analyze_position_errors_when_binary_is_missing() {
        let orchestrator = Lc0Orchestrator::with_binary("definitely-not-an-lc0-binary-xyz", None);
        assert!(!orchestrator.is_available());
        let err = orchestrator
            .analyze_position("startpos", Some(4))
            .await
            .expect_err("missing binary must not succeed");
        assert!(matches!(err, EngineError::Unknown(_)));
    }

    #[tokio::test]
    async fn concurrency_is_bounded_by_configuration() {
        let orchestrator = Lc0Orchestrator::with_binary("lc0", None);
        assert_eq!(orchestrator.concurrency(), DEFAULT_CONCURRENCY);
        assert!(orchestrator.allocate_gpu("task-1"));
    }
}