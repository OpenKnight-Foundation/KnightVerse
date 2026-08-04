//! KnightVerse Backend - Main Entry Point
//!
//! This is the unified entry point for the KnightVerse backend service.
//! It initializes the API server with all configured routes and middleware.
//!
//! Graceful shutdown (BE-26): the Actix-Web server in `api::server` installs
//! SIGTERM/SIGINT handlers and calls `ServerHandle::stop(true)` so in-flight
//! HTTP/WebSocket work and the worker thread pool are drained cleanly.

use api::server;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize basic tracing for telemetry
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    server::main().await
}
