//! KnightVerse Backend Server Binary
//!
//! Entry point for running the KnightVerse API server.
//! Delegates to the server module for initialization.

use api::server;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    server::main().await
}
