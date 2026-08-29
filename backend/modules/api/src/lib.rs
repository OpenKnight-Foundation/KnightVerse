pub mod ai;
pub mod auth;
#[cfg(test)]
mod auth_tests;
pub mod config;
pub mod games;
pub mod idempotency;
pub mod metrics;
pub mod openapi;
pub mod players;
pub mod rate_limiter;
pub mod redis_broadcast;
pub mod request_id;
pub mod server;
mod test;
pub mod ws;

// External modules
extern crate challenge;

// Re-export server module for external use
pub use auth::{login, logout, logout_all, refresh, register};
pub use idempotency::IdempotencyMiddleware;
pub use server::main;
