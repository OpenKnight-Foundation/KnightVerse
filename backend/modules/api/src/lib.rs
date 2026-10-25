pub mod admin;
pub mod archive;
pub mod ai;
pub mod auth;
pub mod chess960;
#[cfg(test)]
mod auth_tests;
pub mod config;
pub mod games;
// pub mod player_stats;
// ^ #1539's rating-history/online-count module is not compiled yet: it uses
//   sqlx with a PgPool and table names that don't exist in this schema. Port
//   it to db::DbPool + sea-orm (smdb.game) and register it in server.rs.
pub mod idempotency;
pub mod lc0;
pub mod metrics;
pub mod move_order;
pub mod moderation;
pub mod openapi;
pub mod pairings;
pub mod tournaments;
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
