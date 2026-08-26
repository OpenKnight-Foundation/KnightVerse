use redis::{AsyncCommands, Client};
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::models::ServerMessage;
use lazy_static::lazy_static;

#[derive(Clone)]
pub struct RedisPubSub {
    client: Client,
    connection: Arc<Mutex<redis::aio::Connection>>,
}

lazy_static! {
    static ref REDIS_URL: String = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
}

impl RedisPubSub {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let client = Client::open(REDIS_URL.as_str())?;
        let connection = client.get_async_connection().await?;
        
        Ok(Self {
            client,
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    /**
     * Publishes a message to a regional Redis channel to sync horizontally scaled nodes.
     * This ensures that players on different server instances stay in sync.
     */
    pub async fn publish_move(&self, room_id: &str, message: &ServerMessage) -> Result<(), Box<dyn std::error::Error>> {
        let payload = serde_json::to_string(message)?;
        let mut conn = self.connection.lock().await;
        let _: () = conn.publish(format!("game:{}", room_id), payload).await?;
        
        log::debug!("[Redis] Published move for room {}", room_id);
        Ok(())
    }

    /**
     * Subscribes to a specific room's updates. 
     * In a production horizontal setup, this would be used by each node to listen for events from others.
     */
    pub async fn subscribe_to_room(&self, room_id: &str) -> Result<redis::aio::PubSub, Box<dyn std::error::Error>> {
        let mut pubsub = self.client.get_async_pubsub().await?;
        pubsub.subscribe(format!("game:{}", room_id)).await?;
        
        log::info!("[Redis] Subscribed to horizontal updates for room {}", room_id);
        Ok(pubsub)
    }
}
