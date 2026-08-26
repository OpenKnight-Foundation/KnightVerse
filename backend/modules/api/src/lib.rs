pub mod ai;
pub mod auth;
pub mod config;
pub mod games;
pub mod metrics;
pub mod openapi;
pub mod players;
pub mod rate_limiter;
pub mod server;
mod test;
pub mod ws;

// External modules
extern crate challenge;

// Re-export server module for external use
pub use auth::{login, logout, refresh, register};
pub use idempotency::IdempotencyMiddleware;
pub use server::main;

