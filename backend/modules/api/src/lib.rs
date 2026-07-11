pub mod auth;
pub mod ai;
pub mod openapi;
pub mod ws;
mod test;
pub mod config;
pub mod server;
pub mod players;
pub mod games;
pub mod metrics_middleware;

// External modules
extern crate challenge;

// Re-export server module for external use
pub use server::main;
pub use auth::{login, register, refresh, logout};