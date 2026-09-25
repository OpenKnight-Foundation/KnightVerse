//! Redis-backed pub/sub fan-out for WebSocket spectators.
//!
//! Player connections stay on the low-latency in-process `LobbyState` actor
//! (see `ws.rs`). Spectators instead subscribe to a per-game Redis channel so
//! that any backend node can broadcast to spectators connected to any other
//! node, without registering (potentially thousands of) spectator recipients
//! with `LobbyState`.

use actix::Recipient;
use futures_util::StreamExt;
use redis::AsyncCommands;
use tokio::task::JoinHandle;
use tracing::error;

use crate::ws::WsMessage;

fn channel_for(game_id: &str) -> String {
    format!("game:{}:spectators", game_id)
}

fn spectator_count_key(game_id: &str) -> String {
    format!("game:{}:spectator_count", game_id)
}

#[derive(Clone)]
pub struct RedisBroadcaster {
    client: redis::Client,
}

impl RedisBroadcaster {
    pub fn new(redis_url: &str) -> Result<Self, redis::RedisError> {
        Ok(Self {
            client: redis::Client::open(redis_url)?,
        })
    }

    /// Publish a message to a game's spectator channel, fire-and-forget. The
    /// publish never blocks the caller — it just spawns a task.
    pub fn publish_fire_and_forget(&self, game_id: &str, message: &WsMessage) {
        let Ok(payload) = serde_json::to_string(message) else {
            return;
        };
        let client = self.client.clone();
        let channel = channel_for(game_id);
        actix::spawn(async move {
            match client.get_multiplexed_async_connection().await {
                Ok(mut conn) => {
                    let _: Result<i64, _> = conn.publish(&channel, payload).await;
                }
                Err(e) => error!("Redis broadcast connection failed: {}", e),
            }
        });
    }

    /// Publish a spectator chat message.
    pub fn publish_chat(&self, game_id: &str, user: String, message: String) {
        self.publish_fire_and_forget(game_id, &WsMessage::Chat { user, message });
    }

    /// Record a spectator joining and publish the updated count.
    pub async fn spectator_joined(&self, game_id: &str) {
        self.bump_spectator_count(game_id, 1).await;
    }

    /// Record a spectator leaving and publish the updated count.
    pub async fn spectator_left(&self, game_id: &str) {
        self.bump_spectator_count(game_id, -1).await;
    }

    async fn bump_spectator_count(&self, game_id: &str, delta: i64) {
        let key = spectator_count_key(game_id);
        let mut conn = match self.client.get_multiplexed_async_connection().await {
            Ok(c) => c,
            Err(e) => {
                error!("Redis spectator count connection failed: {}", e);
                return;
            }
        };
        let count: i64 = match conn.incr(&key, delta).await {
            Ok(c) => c,
            Err(e) => {
                error!("Redis spectator count update failed: {}", e);
                return;
            }
        };
        self.publish_fire_and_forget(
            game_id,
            &WsMessage::SpectatorCount {
                count: count.max(0) as u32,
            },
        );
    }
}

/// Subscribe to a game's Redis channel and forward messages to `recipient`
/// until the connection drops or the returned handle is aborted.
pub fn spawn_subscriber_task(
    redis: RedisBroadcaster,
    game_id: String,
    recipient: Recipient<WsMessage>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let channel = channel_for(&game_id);
        let conn = match redis.client.get_async_connection().await {
            Ok(c) => c,
            Err(e) => {
                error!(
                    "Failed to open Redis pubsub connection for game {}: {}",
                    game_id, e
                );
                return;
            }
        };
        let mut pubsub = conn.into_pubsub();
        if let Err(e) = pubsub.subscribe(&channel).await {
            error!("Failed to subscribe to {}: {}", channel, e);
            return;
        }

        let mut stream = pubsub.on_message();
        while let Some(msg) = stream.next().await {
            let payload = match msg.get_payload::<String>() {
                Ok(p) => p,
                Err(_) => continue,
            };
            let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&payload) else {
                continue;
            };
            recipient.do_send(ws_msg);
        }
    })
}
