//! KnightVerse Backend - Main Entry Point
//!
//! This is the unified entry point for the KnightVerse backend service.
//! It initializes the API server with all configured routes and middleware.

use api::server;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    server::main().await
}