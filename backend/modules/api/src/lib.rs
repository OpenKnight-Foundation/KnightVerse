pub mod admin;
pub mod archive;
pub mod ai;
pub mod auth;
pub mod chess960;
#[cfg(test)]
mod auth_tests;
pub mod config;
pub mod games;
pub mod idempotency;
pub mod lc0;
pub mod metrics;
pub mod move_order;
pub mod moderation;
pub mod openapi;
pub mod pairings;
pub mod players;
pub mod rate_limiter;
pub mod redis_broadcast;
pub mod request_id;
pub mod server;
mod test;
pub mod wallet_link;
pub mod ws;

// External modules
extern crate challenge;

// Re-export server module for external use
pub use auth::{login, logout, logout_all, refresh, register};
pub use idempotency::IdempotencyMiddleware;
pub use players::export_player_data;
pub use server::main;
