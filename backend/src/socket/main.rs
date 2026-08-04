mod game;
mod handlers;
mod models;
mod websocket;

use std::env;
use tokio::net::TcpListener;
use tracing::{info, error};
use tracing_subscriber::EnvFilter;
use websocket::handle_connection;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured logger
    {
        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info"));
        #[cfg(debug_assertions)]
        let subscriber = tracing_subscriber::fmt().with_env_filter(env_filter).pretty();
        #[cfg(not(debug_assertions))]
        let subscriber = tracing_subscriber::fmt().with_env_filter(env_filter).json();
        subscriber.init();
    }
    
    // Get the address from environment or use default
    let addr = env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    
    info!("Starting WebSocket server on {}", addr);
    
    // Initialize the game state
    game::init_game_state();
    
    // Create the TCP listener
    let listener = TcpListener::bind(&addr).await?;
    info!("WebSocket server listening on: {}", addr);
    
       // Accept connections
loop {
       match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New connection from: {}", addr);
                    
                    // Spawn a new task for each connection
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, addr).await {
                            error!("Error handling connection: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                    // Continue accepting connections despite errors
                }
            }
        }
    
    Ok(())
}
