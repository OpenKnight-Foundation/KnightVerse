//! Redis-backed pub/sub fan-out for WebSocket spectators.
//!
//! Player connections stay on the low-latency in-process `LobbyState` actor
//! (see `ws.rs`). Spectators instead subscribe to a per-game Redis channel so
//! that any backend node can broadcast to spectators connected to any other
//! node, without registering (potentially thousands of) spectator recipients
//! with `LobbyState`.
//!
//! # Backpressure policy
//!
//! Every spectator connection owns a bounded, drop-oldest outbound queue (see
//! [`SpectatorQueue`]). Redis fan-out can always produce frames faster than a
//! stalled client can drain its socket, so frames are buffered in that queue
//! and forwarded to the connection actor by a dedicated pump task. The policy
//! is:
//!
//! 1. **Bounded buffering** — a queue never holds more than
//!    [`SPECTATOR_QUEUE_CAPACITY`] frames, so a stalled connection's backlog
//!    (and therefore memory) is bounded no matter how long it stalls.
//! 2. **Drop-oldest** — when a queue is full the oldest buffered frame is
//!    discarded to make room for the newest one. The shared fan-out loop only
//!    enqueues (it never awaits a connection), so one slow spectator can never
//!    block or delay delivery to any other spectator in the same room.
//! 3. **Disconnect on sustained overflow** — a connection still saturated
//!    after [`MAX_CONSECUTIVE_OVERFLOWS`] consecutive fan-out events is
//!    considered unrecoverable and is disconnected instead of being buffered
//!    forever.
//!
//! Per-room queue depth, dropped frames, and backpressure disconnects are
//! exported as Prometheus metrics through `crate::metrics`.

use actix::Recipient;
use futures_util::StreamExt;
use redis::AsyncCommands;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use tracing::error;

use crate::metrics;
use crate::ws::{SpectatorDisconnect, WsMessage};

/// Maximum number of outbound frames buffered for a single spectator
/// connection before the oldest buffered frame is discarded.
pub const SPECTATOR_QUEUE_CAPACITY: usize = 256;

/// Consecutive fan-out events that overflow a spectator's queue before the
/// connection is considered unrecoverable and disconnected.
const MAX_CONSECUTIVE_OVERFLOWS: usize = 64;

// Redis pub/sub channel that carries all spectator-bound messages for a game
fn channel_for(game_id: &str) -> String {
    format!("game:{}:spectators", game_id)
}

// Redis key holding the running spectator count for a game
fn spectator_count_key(game_id: &str) -> String {
    format!("game:{}:spectator_count", game_id)
}

/// Bounded, drop-oldest outbound queue for a single spectator connection.
///
/// See the module-level "Backpressure policy" section for the rationale.
pub struct SpectatorQueue {
    game_id: String,
    capacity: usize,
    // Std (sync) mutex is fine: it's only held briefly and never across an `.await`
    frames: Mutex<VecDeque<WsMessage>>,
    /// Wakes the pump task whenever a frame is buffered or the queue closes.
    notify: Notify,
    /// Consecutive fan-out events that had to drop at least one frame.
    consecutive_overflows: AtomicUsize,
    /// Total frames discarded because the queue was full.
    dropped_frames: AtomicUsize,
    /// Set once the connection must be torn down (sustained backpressure).
    overflowed: AtomicBool,
    /// Set once the subscription has ended for any other reason.
    closed: AtomicBool,
}

impl SpectatorQueue {
    /// Create a queue with the default capacity.
    pub fn new(game_id: impl Into<String>) -> Arc<Self> {
        Self::with_capacity(game_id, SPECTATOR_QUEUE_CAPACITY)
    }

    /// Create a queue with a custom capacity (used by tests).
    /// Shared between the subscriber task (producer) and the pump task (consumer),
    /// hence the `Arc`.
    pub fn with_capacity(game_id: impl Into<String>, capacity: usize) -> Arc<Self> {
        // A zero capacity would make `push` loop forever; enforce at least 1
        let capacity = capacity.max(1);
        Arc::new(Self {
            game_id: game_id.into(),
            capacity,
            frames: Mutex::new(VecDeque::with_capacity(capacity)),
            notify: Notify::new(),
            consecutive_overflows: AtomicUsize::new(0),
            dropped_frames: AtomicUsize::new(0),
            overflowed: AtomicBool::new(false),
            closed: AtomicBool::new(false),
        })
    }

    pub fn game_id(&self) -> &str {
        &self.game_id
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of frames currently buffered for this connection.
    pub fn depth(&self) -> usize {
        self.frames.lock().expect("spectator queue poisoned").len()
    }

    /// Total frames discarded by the drop-oldest policy.
    pub fn dropped_frames(&self) -> usize {
        self.dropped_frames.load(Ordering::Relaxed)
    }

    /// Whether the connection has been marked for disconnection.
    pub fn is_overflowed(&self) -> bool {
        self.overflowed.load(Ordering::Relaxed)
    }

    /// Buffer a frame without ever blocking the caller.
    ///
    /// If the queue is at capacity the oldest frame is discarded first
    /// (drop-oldest), keeping the backlog bounded. Returns `true` once the
    /// connection must be disconnected because it stayed saturated for
    /// [`MAX_CONSECUTIVE_OVERFLOWS`] consecutive fan-out events.
    pub fn push(&self, frame: WsMessage) -> bool {
        // Already condemned: just count the drop and keep signalling "disconnect"
        if self.overflowed.load(Ordering::Relaxed) {
            self.record_drop(1);
            return true;
        }

        let mut dropped_now = 0usize;
        // Scope the lock so it is released before touching metrics/notify
        let depth = {
            let mut frames = self.frames.lock().expect("spectator queue poisoned");
            // Evict oldest frames until there is room for the new one
            while frames.len() >= self.capacity {
                frames.pop_front();
                dropped_now += 1;
            }
            frames.push_back(frame);
            frames.len()
        };

        if dropped_now > 0 {
            // This fan-out event overflowed the queue: count it toward the streak
            self.record_drop(dropped_now);
            let overflows = self.consecutive_overflows.fetch_add(1, Ordering::Relaxed) + 1;
            if overflows >= MAX_CONSECUTIVE_OVERFLOWS {
                // Streak too long: mark for teardown and wake the pump so it can exit
                self.overflowed.store(true, Ordering::Relaxed);
                metrics::increment_spectator_backpressure_disconnects(&self.game_id);
                self.notify.notify_one();
                return true;
            }
        } else {
            // The queue had room, so the client is keeping up: reset the streak
            self.consecutive_overflows.store(0, Ordering::Relaxed);
        }

        metrics::set_spectator_queue_depth(&self.game_id, depth);
        // Wake the pump task (a permit is stored if it isn't waiting yet)
        self.notify.notify_one();
        false
    }

    /// Pop the oldest buffered frame, waiting until one is available.
    ///
    /// Returns `None` once the queue is closed or has been marked overflowed,
    /// which tells the pump task to stop.
    pub async fn pop(&self) -> Option<WsMessage> {
        loop {
            // Register interest *before* checking the queue so a push that lands
            // between the check and the await can't be missed
            let notified = self.notify.notified();
            {
                let mut frames = self.frames.lock().expect("spectator queue poisoned");
                if let Some(frame) = frames.pop_front() {
                    let depth = frames.len();
                    // Release the lock before calling into metrics
                    drop(frames);
                    metrics::set_spectator_queue_depth(&self.game_id, depth);
                    return Some(frame);
                }
            }
            // Queue is empty: stop if the subscription is finished
            if self.overflowed.load(Ordering::Relaxed) || self.closed.load(Ordering::Relaxed) {
                return None;
            }
            // Otherwise sleep until `push` or `close` wakes us
            notified.await;
        }
    }

    /// Stop the pump task even if the queue is not overflowing.
    pub fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
        self.notify.notify_one();
    }

    /// Discard everything still buffered.
    pub fn clear(&self) {
        self.frames.lock().expect("spectator queue poisoned").clear();
        metrics::set_spectator_queue_depth(&self.game_id, 0);
    }

    // Bump both the local counter and the Prometheus counter
    fn record_drop(&self, dropped: usize) {
        self.dropped_frames.fetch_add(dropped, Ordering::Relaxed);
        metrics::increment_spectator_frames_dropped(&self.game_id, dropped as u64);
    }
}

/// Forward buffered frames from `queue` to the spectator's connection actor.
///
/// Because [`SpectatorQueue::push`] never blocks, a stalled actor can only
/// ever delay frames in its own connection's queue — never the fan-out loop or
/// any other spectator.
pub fn spawn_outbox_pump(
    queue: Arc<SpectatorQueue>,
    recipient: Recipient<WsMessage>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        // Loop until the queue signals shutdown (None)
        while let Some(frame) = queue.pop().await {
            // `send(..).await` waits for the actor to accept the message; an error
            // means the actor is gone (connection closed), so stop pumping
            if recipient.send(frame).await.is_err() {
                break;
            }
        }
    })
}

/// Handles belonging to one spectator's Redis subscription. Both must be
/// aborted together to fully tear the connection's fan-out down.
pub struct SpectatorSubscription {
    // Task reading from Redis and pushing into the queue
    subscriber: JoinHandle<()>,
    // Task draining the queue into the connection actor
    pump: JoinHandle<()>,
}

impl SpectatorSubscription {
    pub fn abort(&self) {
        self.subscriber.abort();
        self.pump.abort();
    }
}

/// Thin wrapper around a Redis client for publishing spectator messages and
/// maintaining spectator counts. Cheap to clone.
#[derive(Clone)]
pub struct RedisBroadcaster {
    client: redis::Client,
}

impl RedisBroadcaster {
    /// Parse the URL and build a client (no connection is opened yet).
    pub fn new(redis_url: &str) -> Result<Self, redis::RedisError> {
        Ok(Self {
            client: redis::Client::open(redis_url)?,
        })
    }

    /// Publish a message to a game's spectator channel, fire-and-forget. The
    /// publish never blocks the caller — it just spawns a task.
    pub fn publish_fire_and_forget(&self, game_id: &str, message: &WsMessage) {
        // Serialization failures are silently ignored
        let Ok(payload) = serde_json::to_string(message) else {
            return;
        };
        let client = self.client.clone();
        let channel = channel_for(game_id);
        actix::spawn(async move {
            match client.get_multiplexed_async_connection().await {
                Ok(mut conn) => {
                    // Result (number of receivers) is intentionally ignored
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

    // Atomically adjust the shared counter in Redis (so it's correct across
    // nodes), then broadcast the new value to all spectators of the game
    async fn bump_spectator_count(&self, game_id: &str, delta: i64) {
        let key = spectator_count_key(game_id);
        let mut conn = match self.client.get_multiplexed_async_connection().await {
            Ok(c) => c,
            Err(e) => {
                error!("Redis spectator count connection failed: {}", e);
                return;
            }
        };
        // INCRBY returns the new value
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
                // Clamp so a drifted counter can never go negative on the wire
                count: count.max(0) as u32,
            },
        );
    }
}

/// Subscribe to a game's Redis channel and forward messages to `recipient`
/// through a bounded, drop-oldest outbound queue (see the module-level
/// "Backpressure policy"). Runs until the Redis connection drops, the queue
/// overflows (at which point `disconnect` is notified), or the returned
/// subscription is aborted.
pub fn spawn_subscriber_task(
    redis: RedisBroadcaster,
    game_id: String,
    recipient: Recipient<WsMessage>,
    disconnect: Recipient<SpectatorDisconnect>,
) -> SpectatorSubscription {
    // One queue per spectator connection, plus a pump to drain it
    let queue = SpectatorQueue::new(game_id.clone());
    let pump = spawn_outbox_pump(queue.clone(), recipient);

    let subscriber_queue = queue.clone();
    let subscriber_game_id = game_id.clone();
    let subscriber = tokio::spawn(async move {
        let channel = channel_for(&subscriber_game_id);
        // Pub/sub needs a dedicated (non-multiplexed) connection
        let conn = match redis.client.get_async_connection().await {
            Ok(c) => c,
            Err(e) => {
                error!(
                    "Failed to open Redis pubsub connection for game {}: {}",
                    subscriber_game_id, e
                );
                // Close the queue so the pump task exits too
                subscriber_queue.close();
                return;
            }
        };
        let mut pubsub = conn.into_pubsub();
        if let Err(e) = pubsub.subscribe(&channel).await {
            error!("Failed to subscribe to {}: {}", channel, e);
            subscriber_queue.close();
            return;
        }

        let mut stream = pubsub.on_message();
        while let Some(msg) = stream.next().await {
            // Skip anything that isn't a UTF-8 string payload
            let payload = match msg.get_payload::<String>() {
                Ok(p) => p,
                Err(_) => continue,
            };
            // Skip payloads that don't deserialize into a known WsMessage
            let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&payload) else {
                continue;
            };
            // `push` never awaits, so this loop can't be stalled by a slow client.
            // `true` means sustained overflow: ask the connection actor to disconnect
            if subscriber_queue.push(ws_msg) {
                error!(
                    "Spectator outbound queue for game {} stayed saturated; disconnecting",
                    subscriber_game_id
                );
                disconnect.do_send(SpectatorDisconnect {
                    game_id: subscriber_game_id.clone(),
                });
                break;
            }
        }

        // Stream ended (Redis dropped, or we broke out above): stop the pump
        subscriber_queue.close();
    });

    SpectatorSubscription { subscriber, pump }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix::Actor;
    use std::time::Duration;

    /// Captures every frame an actor receives so tests can assert on it.
    struct ChannelSink {
        tx: tokio::sync::mpsc::UnboundedSender<WsMessage>,
    }

    impl Actor for ChannelSink {
        type Context = actix::Context<Self>;
    }

    // Forward each received message into the channel for the test to read
    impl actix::Handler<WsMessage> for ChannelSink {
        type Result = ();

        fn handle(&mut self, msg: WsMessage, _: &mut Self::Context) {
            let _ = self.tx.send(msg);
        }
    }

    // A slow consumer must stay within its capacity and must not affect others
    #[actix_web::test]
    async fn stalled_consumer_backlog_is_bounded_and_does_not_stall_others() {
        // `stalled` never pops (simulating a stuck client); `peer` is roomy enough to hold all frames
        let stalled = SpectatorQueue::with_capacity("room-1", 8);
        let peer = SpectatorQueue::with_capacity("room-1", 256);

        for i in 0..200u32 {
            let frame = WsMessage::Clock { white: i, black: i };
            stalled.push(frame.clone());
            peer.push(frame);
        }

        // Bounded backlog: the stalled connection never buffers more than its
        // configured capacity, no matter how many frames are fanned out.
        assert!(stalled.depth() <= 8, "stalled depth = {}", stalled.depth());
        // 200 pushed - 8 retained = 192 dropped
        assert_eq!(stalled.dropped_frames(), 192);

        // Independent: the healthy peer loses nothing and holds every frame.
        assert_eq!(peer.dropped_frames(), 0);
        assert_eq!(peer.depth(), 200);

        // ...and the peer still receives every frame, in order.
        for i in 0..200u32 {
            match peer.pop().await {
                Some(WsMessage::Clock { white, black }) => assert_eq!((white, black), (i, i)),
                other => panic!("unexpected frame: {:?}", other),
            }
        }
    }

    // Pushing past capacity for long enough must eventually signal a disconnect
    #[actix_web::test]
    async fn sustained_overflow_flags_connection_for_disconnect() {
        // Tiny capacity so nearly every push after the first two overflows
        let queue = SpectatorQueue::with_capacity("room-2", 2);

        let mut disconnect = false;
        // Allow a few extra pushes beyond the threshold as headroom
        for i in 0..(MAX_CONSECUTIVE_OVERFLOWS + 16) as u32 {
            if queue.push(WsMessage::Clock { white: i, black: i }) {
                disconnect = true;
                break;
            }
        }

        assert!(disconnect, "sustained backpressure must flag a disconnect");
        assert!(queue.is_overflowed());
        assert!(queue.depth() <= 2, "depth = {}", queue.depth());
    }

    // The pump should deliver buffered frames in order, then exit once closed
    #[actix_web::test]
    async fn outbox_pump_forwards_frames_and_stops_on_close() {
        let queue = SpectatorQueue::with_capacity("room-3", 4);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let sink = ChannelSink { tx }.start();
        let pump = spawn_outbox_pump(queue.clone(), sink.recipient());

        queue.push(WsMessage::Clock { white: 1, black: 2 });
        queue.push(WsMessage::Clock { white: 3, black: 4 });
        // Closing after pushing: pop() still drains remaining frames before returning None
        queue.close();

        assert_eq!(
            rx.recv().await,
            Some(WsMessage::Clock { white: 1, black: 2 })
        );
        assert_eq!(
            rx.recv().await,
            Some(WsMessage::Clock { white: 3, black: 4 })
        );

        // The pump task must terminate within the timeout and not have panicked
        tokio::time::timeout(Duration::from_secs(2), pump)
            .await
            .expect("pump should stop after the queue closes")
            .expect("pump task should not panic");
    }
}