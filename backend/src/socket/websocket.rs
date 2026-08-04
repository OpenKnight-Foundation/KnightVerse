use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};
use tracing::{error, info};

use crate::handlers::handle_client_message;
use crate::models::ServerMessage;

// Handle a WebSocket connection
pub async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    // Accept the WebSocket connection
    let ws_stream = accept_async(stream).await?;
    info!("WebSocket connection established with: {}", addr);

    // Split the WebSocket stream
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    // Keep track of room subscriptions
    let mut room_senders: Vec<(String, broadcast::Sender<ServerMessage>)> = Vec::new();
    let mut room_receivers = Vec::new();

    // Main connection loop
    loop {
        tokio::select! {
            // Handle incoming WebSocket messages
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(msg)) => {
                        match msg {
                            Message::Text(text) => {
                                if let Err(e) = handle_client_message(&text, &mut ws_sender, &mut room_senders).await {
                                    error!("Error handling client message: {}", e);
                                    break;
                                }
                            }
                            Message::Close(_) => {
                                info!("Client {} disconnected", addr);
                                break;
                            }
                            Message::Ping(data) => {
                                if let Err(e) = ws_sender.send(Message::Pong(data)).await {
                                    error!("Error sending pong: {}", e);
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    Some(Err(e)) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                    None => {
                        info!("Client {} disconnected", addr);
                        break;
                    }
                }
            }

            // Handle room broadcasts
            _ = async {
               // Rebuild receivers when room_senders changes
if room_receivers.len() != room_senders.len() {
        room_receivers.clear();
        for (_, sender) in &room_senders {
            room_receivers.push(sender.subscribe());
        }
    }

                // Check for messages from each room
                for (i, receiver) in room_receivers.iter_mut().enumerate() {
                    if let Ok(msg) = receiver.try_recv() {
                        if let Ok(json) = serde_json::to_string(&msg) {
                            if let Err(e) = ws_sender.send(Message::Text(json)).await {
                                error!("Error forwarding room message: {}", e);
                                return;
                            }
                        }
                    }
                }

                // Sleep a bit to avoid busy waiting
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                
                // Never complete this future
                std::future::pending::<()>().await
            } => {}
        }
    }

    Ok(())
}
