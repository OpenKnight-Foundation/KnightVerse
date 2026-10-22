//! KnightVerse Backend - Main Entry Point
//!
// This is the unified entry point for the KnightVerse backend service.
// It initializes the API server with all configured routes and middleware.
//
// Graceful shutdown (BE-26): the Actix-Web server in `api::server` installs the
// SIGTERM/SIGINT handlers and calls `ServerHandle::stop(true)` so in-flight
// HTTP/WebSocket work and the worker thread pool are drained cleanly.
//
// Tracing is initialised in `api::server::main` using `try_init()`, so this
// binary must not register a global subscriber itself (BE-131).

use api::server;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    server::main().await
}
