use actix::prelude::*;
use actix_web::error::ErrorUnauthorized;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use db::DbPool;
use dto::games::GameDisplayDTO;
use error::error::ApiError;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use security::jwt::{Claims, TokenType};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::env;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::redis_broadcast::{spawn_subscriber_task, RedisBroadcaster};

use chrono::{DateTime, Utc};
use tokio::task::JoinHandle;

/// Player connection status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConnectionStatus {
    Connected,
    Reconnecting,
    Disconnected,
}

/// Player state within a game session
#[derive(Debug)]
pub struct PlayerConnectionState {
    pub player_id: Uuid,
    pub status: ConnectionStatus,
    pub disconnected_at: Option<DateTime<Utc>>,
    pub grace_timer: Option<JoinHandle<()>>,
    pub addr: Option<Recipient<WsMessage>>,
}

/// Game session state tracking all players in a game
#[derive(Debug)]
pub struct GameSessionState {
    pub game_id: String,
    pub players: HashMap<Uuid, PlayerConnectionState>,
    pub is_active: bool,
}

/// Connection state tracker actor that manages all active game sessions
pub struct ConnectionStateTracker {
    game_sessions: HashMap<String, GameSessionState>,
    db_pool: Option<DbPool>,
}

/// Message to mark a player as disconnected (start grace period)
#[derive(Message)]
#[rtype(result = "()")]
pub struct PlayerDisconnected {
    pub game_id: String,
    pub player_id: Uuid,
}

/// Message to mark a player as reconnected
#[derive(Message)]
#[rtype(result = "()")]
pub struct PlayerReconnected {
    pub game_id: String,
    pub player_id: Uuid,
    pub addr: Recipient<WsMessage>,
}

/// Message sent when grace period expires
#[derive(Message)]
#[rtype(result = "()")]
pub struct GracePeriodExpired {
    pub game_id: String,
    pub player_id: Uuid,
}

/// Message to get full game state for syncing on reconnect
#[derive(Message)]
#[rtype(result = "Result<GameDisplayDTO, ApiError>")]
pub struct GetGameState {
    pub game_id: String,
}

/// Introspection message for a single player's tracked connection state.
/// Used by tests to observe `ConnectionStateTracker`'s internal state without
/// reaching into the actor directly (actix actors don't expose that).
#[derive(Message)]
#[rtype(result = "Option<PlayerStatusSnapshot>")]
pub struct GetPlayerStatus {
    pub game_id: String,
    pub player_id: Uuid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerStatusSnapshot {
    pub status: ConnectionStatus,
    pub has_grace_timer: bool,
    pub session_is_active: bool,
}

/// OpponentDisconnected message sent to connected opponent with grace seconds left
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct OpponentDisconnectedPayload {
    pub grace_seconds_left: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "type", content = "payload")]
pub enum ExtendedWsMessage {
    Original(WsMessage),
    OpponentDisconnected(OpponentDisconnectedPayload),
    OpponentReconnected,
}

/// Core WebSocket message types
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[rtype(result = "()")]
#[serde(tag = "type", content = "payload")]
pub enum WsMessage {
    Move {
        from: String,
        to: String,
        san: String,
        fen: String,
    },
    Clock {
        white: u32,
        black: u32,
    },
    End {
        result: String,
        final_fen: String,
    },
    Error {
        code: u16,
        message: String,
    },
    ReconnectToken {
        token: String,
        expires_in: u32,
    },
    /// Engine evaluation update, published to spectators only.
    Eval {
        score_cp: i32,
        depth: u16,
        best_line: Vec<String>,
    },
    /// Spectator chat. Published/consumed entirely via Redis fan-out; never
    /// routed through the core game actor loop.
    Chat {
        user: String,
        message: String,
    },
    /// Current spectator count for a game. Throttled/batched on the
    /// subscriber side so bursts of joins/leaves don't flood clients.
    SpectatorCount {
        count: u32,
    },
    OpponentDisconnected(OpponentDisconnectedPayload),
    OpponentReconnected,
    FullStateSync {
        fen: String,
        move_list: Vec<String>,
        white_time: u32,
        black_time: u32,
    },
}

/// Actor messages (used by the in-process lobby, i.e. the player fast path)
#[derive(Message)]
#[rtype(result = "()")]
pub struct Connect {
    pub game_id: String,
    pub addr: Recipient<WsMessage>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Disconnect {
    pub game_id: String,
    pub addr: Recipient<WsMessage>,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct Broadcast {
    pub game_id: String,
    pub message: WsMessage,
}

/// Lobby state actor.
///
/// IMPORTANT: this now only holds *player* connections (at most two per
/// game, plus any same-node infra that legitimately needs the low-latency
/// path). Spectators are intentionally never registered here — see the
/// module doc in `redis_broadcast.rs` for why. This keeps `Broadcast`'s
/// fan-out cost bounded by player count instead of by (potentially
/// thousands of) spectators, which is what caused the CPU bottleneck this
/// change addresses.
pub struct LobbyState {
    sessions: HashMap<String, HashSet<Recipient<WsMessage>>>,
}

impl Default for LobbyState {
    fn default() -> Self {
        Self::new()
    }
}

impl LobbyState {
    pub fn new() -> Self {
        LobbyState {
            sessions: HashMap::new(),
        }
    }
}

impl Actor for LobbyState {
    type Context = Context<Self>;
}

impl Handler<Connect> for LobbyState {
    type Result = ();

    fn handle(&mut self, msg: Connect, _: &mut Context<Self>) {
        let entry = self.sessions.entry(msg.game_id).or_default();
        entry.insert(msg.addr);
    }
}

impl Handler<Disconnect> for LobbyState {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        if let Some(set) = self.sessions.get_mut(&msg.game_id) {
            set.remove(&msg.addr);
            if set.is_empty() {
                self.sessions.remove(&msg.game_id);
            }
        }
    }
}

impl Handler<Broadcast> for LobbyState {
    type Result = ();

    fn handle(&mut self, msg: Broadcast, _: &mut Context<Self>) {
        if let Some(set) = self.sessions.get(&msg.game_id) {
            for recipient in set.iter() {
                // backpressure: drop if send fails
                recipient.do_send(msg.message.clone());
            }
        }
    }
}

impl Default for ConnectionStateTracker {
    fn default() -> Self {
        Self::new(None)
    }
}

impl ConnectionStateTracker {
    const GRACE_PERIOD_SECONDS: u64 = 60;

    pub fn new(db_pool: Option<DbPool>) -> Self {
        ConnectionStateTracker {
            game_sessions: HashMap::new(),
            db_pool,
        }
    }

    /// Get or create a game session
    fn get_or_create_session(&mut self, game_id: String) -> &mut GameSessionState {
        self.game_sessions
            .entry(game_id.clone())
            .or_insert_with(|| GameSessionState {
                game_id,
                players: HashMap::new(),
                is_active: true,
            })
    }

    /// Broadcast message to all other players in the game
    fn broadcast_to_other_players(
        session: &GameSessionState,
        exclude_player_id: Uuid,
        message: WsMessage,
    ) {
        for (player_id, player_state) in &session.players {
            if *player_id != exclude_player_id {
                if let Some(addr) = &player_state.addr {
                    let _ = addr.do_send(message.clone());
                }
            }
        }
    }
}

impl Actor for ConnectionStateTracker {
    type Context = Context<Self>;
}

/// Handle PlayerDisconnected message - start grace period timer
impl Handler<PlayerDisconnected> for ConnectionStateTracker {
    type Result = ();

    fn handle(&mut self, msg: PlayerDisconnected, ctx: &mut Context<Self>) {
        let session = self.get_or_create_session(msg.game_id.clone());

        if !session.is_active {
            return;
        }

        let should_start_grace_period = match session.players.get_mut(&msg.player_id) {
            Some(player_state) if player_state.status != ConnectionStatus::Disconnected => {
                player_state.status = ConnectionStatus::Reconnecting;
                player_state.disconnected_at = Some(Utc::now());
                player_state.addr = None;
                true
            }
            _ => false,
        };

        if !should_start_grace_period {
            return;
        }

        info!(
            "Player {} disconnected from game {}, starting {}s grace period",
            msg.player_id,
            msg.game_id,
            Self::GRACE_PERIOD_SECONDS
        );

        Self::broadcast_to_other_players(
            session,
            msg.player_id,
            WsMessage::OpponentDisconnected(OpponentDisconnectedPayload {
                grace_seconds_left: Self::GRACE_PERIOD_SECONDS as u32,
            }),
        );

        let tracker_addr = ctx.address().clone();
        let game_id_clone = msg.game_id.clone();
        let player_id_clone = msg.player_id;
        let timer_handle = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(Self::GRACE_PERIOD_SECONDS)).await;
            tracker_addr.do_send(GracePeriodExpired {
                game_id: game_id_clone,
                player_id: player_id_clone,
            });
        });

        if let Some(player_state) = session.players.get_mut(&msg.player_id) {
            player_state.grace_timer = Some(timer_handle);
        }
    }
}

/// Handle PlayerReconnected message - cancel timer, sync state
impl Handler<PlayerReconnected> for ConnectionStateTracker {
    type Result = ();

    fn handle(&mut self, msg: PlayerReconnected, _ctx: &mut Context<Self>) {
        let session = self.get_or_create_session(msg.game_id.clone());

        if !session.is_active {
            return;
        }

        if let Some(player_state) = session.players.get_mut(&msg.player_id) {
            if let Some(timer) = player_state.grace_timer.take() {
                timer.abort();
                info!(
                    "Player {} reconnected to game {}, grace period cancelled",
                    msg.player_id, msg.game_id
                );
            }

            player_state.status = ConnectionStatus::Connected;
            player_state.disconnected_at = None;
            player_state.addr = Some(msg.addr.clone());

            Self::broadcast_to_other_players(
                session,
                msg.player_id,
                WsMessage::OpponentReconnected,
            );

            if let Some(db_pool) = &self.db_pool {
                let db_pool_clone = db_pool.clone();
                let addr_clone = msg.addr.clone();
                let game_id_uuid = match Uuid::parse_str(&msg.game_id) {
                    Ok(id) => id,
                    Err(_) => return,
                };

                tokio::spawn(async move {
                    match service::games::GameService::get_game(&db_pool_clone, game_id_uuid).await
                    {
                        Ok(game_state) => {
                            let move_list: Vec<String> = game_state
                                .move_history
                                .into_iter()
                                .map(|m| m.to_string())
                                .collect();

                            let sync_message = WsMessage::FullStateSync {
                                fen: game_state.current_fen,
                                move_list,
                                white_time: game_state.white_time_remaining as u32,
                                black_time: game_state.black_time_remaining as u32,
                            };

                            let _ = addr_clone.do_send(sync_message);
                            info!(
                                "Sent full state sync to reconnected player {} in game {}",
                                msg.player_id, msg.game_id
                            );
                        }
                        Err(e) => {
                            error!("Failed to fetch game state for sync: {}", e);
                        }
                    }
                });
            }
        } else {
            session.players.insert(
                msg.player_id,
                PlayerConnectionState {
                    player_id: msg.player_id,
                    status: ConnectionStatus::Connected,
                    disconnected_at: None,
                    grace_timer: None,
                    addr: Some(msg.addr),
                },
            );
            info!("New player {} added to game {}", msg.player_id, msg.game_id);
        }
    }
}

/// Handle GracePeriodExpired message - trigger abandonment timeout
impl Handler<GracePeriodExpired> for ConnectionStateTracker {
    type Result = ();

    fn handle(&mut self, msg: GracePeriodExpired, _: &mut Context<Self>) {
        let session = match self.game_sessions.get_mut(&msg.game_id) {
            Some(s) => s,
            None => return,
        };

        if !session.is_active {
            return;
        }

        if let Some(player_state) = session.players.get_mut(&msg.player_id) {
            if player_state.status == ConnectionStatus::Reconnecting {
                info!(
                    "Grace period expired for player {} in game {}, triggering abandonment",
                    msg.player_id, msg.game_id
                );

                player_state.status = ConnectionStatus::Disconnected;
                player_state.grace_timer = None;
                session.is_active = false;

                if let Some(db_pool) = &self.db_pool {
                    let db_pool_clone = db_pool.clone();
                    let game_id_uuid = match Uuid::parse_str(&msg.game_id) {
                        Ok(id) => id,
                        Err(_) => return,
                    };
                    let player_id_clone = msg.player_id;

                    tokio::spawn(async move {
                        match service::games::GameService::abandon_game(
                            &db_pool_clone,
                            game_id_uuid,
                            player_id_clone,
                        )
                        .await
                        {
                            Ok(_) => {
                                info!(
                                    "Successfully marked game {} as abandoned by player {}",
                                    game_id_uuid, player_id_clone
                                );
                            }
                            Err(e) => {
                                error!("Failed to mark game as abandoned: {}", e);
                            }
                        }
                    });
                }
            }
        }
    }
}

/// Handle GetPlayerStatus — introspection for tests.
impl Handler<GetPlayerStatus> for ConnectionStateTracker {
    type Result = Option<PlayerStatusSnapshot>;

    fn handle(&mut self, msg: GetPlayerStatus, _: &mut Context<Self>) -> Self::Result {
        let session = self.game_sessions.get(&msg.game_id)?;
        let player_state = session.players.get(&msg.player_id)?;
        Some(PlayerStatusSnapshot {
            status: player_state.status.clone(),
            has_grace_timer: player_state.grace_timer.is_some(),
            session_is_active: session.is_active,
        })
    }
}

/// WebSocket session actor. Handles both players and spectators; behavior
/// diverges based on `is_spectator`.
pub struct WsSession {
    pub game_id: String,
    pub lobby: Addr<LobbyState>,
    pub connection_tracker: Addr<ConnectionStateTracker>,
    pub redis: RedisBroadcaster,
    pub hb: std::time::Instant,
    pub user_id: i32,
    pub player_id: Uuid,
    pub username: String,
    pub session_id: String,
    pub is_spectator: bool,
    pub redis_sub_task: Option<JoinHandle<()>>,
}

impl WsSession {
    /// Server sends a ping every 15 seconds to detect dead connections
    const HEARTBEAT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(15);
    /// Terminate connection if no pong received within 25 seconds (15s interval + 10s grace)
    const CLIENT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);

    /// Generate a reconnection token for this session
    fn generate_reconnect_token(&self) -> Result<String, jsonwebtoken::errors::Error> {
        let secret =
            env::var("JWT_SECRET_KEY").unwrap_or_else(|_| "development_secret_key".to_string());
        let jwt_service = security::jwt::JwtService::new(secret, 3600);
        jwt_service.generate_reconnect_token(
            self.user_id,
            &self.username,
            self.player_id,
            &self.session_id,
        )
    }

    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(Self::HEARTBEAT_INTERVAL, |act, ctx| {
            let elapsed = std::time::Instant::now().duration_since(act.hb);
            if elapsed > Self::CLIENT_TIMEOUT {
                warn!(
                    "WebSocket timeout for game {}: no pong in {}s, terminating connection",
                    act.game_id,
                    elapsed.as_secs()
                );
                ctx.stop();
                return;
            }
            ctx.ping(b"");
        });
    }
}

impl Actor for WsSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);

        let addr = ctx.address().recipient();
        if self.is_spectator {
            let recipient = ctx.address().recipient();
            let handle = spawn_subscriber_task(self.redis.clone(), self.game_id.clone(), recipient);
            self.redis_sub_task = Some(handle);

            let redis = self.redis.clone();
            let game_id = self.game_id.clone();
            actix::spawn(async move {
                redis.spectator_joined(&game_id).await;
            });
        } else {
            self.lobby.do_send(Connect {
                game_id: self.game_id.clone(),
                addr: addr.clone(),
            });
            self.connection_tracker.do_send(PlayerReconnected {
                game_id: self.game_id.clone(),
                player_id: self.player_id,
                addr,
            });
        }
    }

    fn stopped(&mut self, ctx: &mut Self::Context) {
        info!(
            "WebSocket disconnected for game: {} (spectator={})",
            self.game_id, self.is_spectator
        );

        if self.is_spectator {
            if let Some(handle) = self.redis_sub_task.take() {
                handle.abort();
            }
            let redis = self.redis.clone();
            let game_id = self.game_id.clone();
            actix::spawn(async move {
                redis.spectator_left(&game_id).await;
            });
            // Spectators don't get reconnect tokens today: reconnection
            // re-subscribes fresh and re-syncs from current game state
            // rather than replaying a session.
            return;
        }

        if let Ok(reconnect_token) = self.generate_reconnect_token() {
            let reconnect_msg = WsMessage::ReconnectToken {
                token: reconnect_token,
                expires_in: 60,
            };
            if let Err(e) = ctx.address().try_send(reconnect_msg) {
                warn!(
                    "Could not send reconnection token (connection already closed): {}",
                    e
                );
            }
            info!("Sent reconnection token for user: {}", self.username);
        } else {
            error!(
                "Failed to generate reconnection token for user: {}",
                self.username
            );
        }

        let addr = ctx.address().recipient();
        self.lobby.do_send(Disconnect {
            game_id: self.game_id.clone(),
            addr,
        });

        self.connection_tracker.do_send(PlayerDisconnected {
            game_id: self.game_id.clone(),
            player_id: self.player_id,
        });

        if let Some(handle) = self.redis_sub_task.take() {
            handle.abort();
        }
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = std::time::Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = std::time::Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) else {
                    return;
                };

                if self.is_spectator {
                    self.handle_spectator_message(ws_msg, ctx);
                } else {
                    self.handle_player_message(ws_msg, ctx);
                }
            }
            Ok(ws::Message::Binary(_)) => {}
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => {}
        }
    }
}

impl WsSession {
    /// Player-originated message handling. Moves/clock/end updates go out
    /// on the fast in-process lobby path (unaffected latency for the
    /// opponent) AND are published to Redis, fire-and-forget, so spectators
    /// on any backend node pick them up. The Redis publish never blocks
    /// this handler — `publish_fire_and_forget` just spawns a task.
    fn handle_player_message(&mut self, ws_msg: WsMessage, _ctx: &mut ws::WebsocketContext<Self>) {
        match &ws_msg {
            WsMessage::Move { .. } | WsMessage::Clock { .. } | WsMessage::End { .. } => {
                self.lobby.do_send(Broadcast {
                    game_id: self.game_id.clone(),
                    message: ws_msg.clone(),
                });
                // Single publish per move/clock/end event, non-blocking.
                self.redis.publish_fire_and_forget(&self.game_id, &ws_msg);
            }
            WsMessage::Eval { .. } => {
                // Evaluation updates are spectator-facing only; skip the
                // player lobby entirely and go straight to Redis.
                self.redis.publish_fire_and_forget(&self.game_id, &ws_msg);
            }
            _ => {
                // Chat/SpectatorCount/ReconnectToken/Error aren't expected
                // as inbound player messages; ignore rather than error to
                // stay tolerant of client/version skew.
            }
        }
    }

    /// Spectator-originated message handling. Only chat is accepted, and it
    /// is published directly to the Redis fan-out channel — it never
    /// touches `LobbyState` or the core game actor loop, per the issue's
    /// "what not to do" constraint.
    fn handle_spectator_message(
        &mut self,
        ws_msg: WsMessage,
        _ctx: &mut ws::WebsocketContext<Self>,
    ) {
        match ws_msg {
            WsMessage::Chat { message, .. } => {
                self.redis
                    .publish_chat(&self.game_id, self.username.clone(), message);
            }
            _ => {
                // Spectators can't submit moves, clocks, etc. Silently drop;
                // a malicious/broken client shouldn't be able to influence
                // game state or spam the lobby.
            }
        }
    }
}

impl Handler<WsMessage> for WsSession {
    type Result = ();

    fn handle(&mut self, msg: WsMessage, ctx: &mut ws::WebsocketContext<Self>) {
        // Serialize message and inject version field
        let mut val = serde_json::to_value(&msg).unwrap();
        if let Value::Object(ref mut m) = val {
            m.insert("version".into(), json!("1.0"));
        }
        let text = serde_json::to_string(&val).unwrap();
        ctx.text(text);
    }
}

/// WebSocket route handler with auth and reconnection support.
///
/// Spectator vs. player is selected via `?role=spectator` (default: player).
/// Spectators still authenticate (so we know who's chatting / for
/// abuse-mitigation and stats) but are never registered with `LobbyState`.
pub async fn ws_route(
    req: HttpRequest,
    stream: web::Payload,
    lobby: web::Data<Addr<LobbyState>>,
    redis: web::Data<RedisBroadcaster>,
    connection_tracker: web::Data<Addr<ConnectionStateTracker>>,
) -> Result<HttpResponse, Error> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok());
    let mut reconnect_token: Option<String> = None;
    let mut is_spectator = false;

    // Parse query string manually
    let query_string = req.query_string();
    if !query_string.is_empty() {
        for param in query_string.split('&') {
            if let Some((key, value)) = param.split_once('=') {
                match key {
                    "reconnect" => reconnect_token = Some(value.to_string()),
                    "role" if value == "spectator" => is_spectator = true,
                    _ => {}
                }
            }
        }
    }

    let claims = if let Some(ref reconnect_token_str) = reconnect_token {
        // Validate reconnection token
        validate_reconnect_token(reconnect_token_str)?
    } else {
        // Validate regular JWT token from header
        if let Some(header) = auth_header {
            if !header.starts_with("Bearer ") {
                return Err(ErrorUnauthorized("Invalid authorization token format"));
            }
            let token = &header[7..];
            validate_access_token(token)?
        } else {
            return Err(ErrorUnauthorized("Missing authorization token"));
        }
    };

    let game_id = req.match_info().get("game_id").unwrap_or("").to_string();
    let session_id = Uuid::new_v4().to_string();

    ws::start(
        WsSession {
            game_id,
            lobby: lobby.get_ref().clone(),
            connection_tracker: connection_tracker.get_ref().clone(),
            redis: redis.get_ref().clone(),
            hb: std::time::Instant::now(),
            user_id: claims.user_id,
            player_id: claims.player_id,
            username: claims.username,
            session_id,
            is_spectator,
            redis_sub_task: None,
        },
        &req,
        stream,
    )
}

/// Validate access token
fn validate_access_token(token: &str) -> Result<Claims, Error> {
    let secret =
        env::var("JWT_SECRET_KEY").unwrap_or_else(|_| "development_secret_key".to_string());
    let validation = Validation::new(Algorithm::HS256);
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|_| ErrorUnauthorized("Invalid or expired token"))?;

    // Ensure it's an access token
    if token_data.claims.token_type != TokenType::Access {
        return Err(ErrorUnauthorized("Invalid token type"));
    }

    Ok(token_data.claims)
}

/// Validate reconnection token
fn validate_reconnect_token(token: &str) -> Result<Claims, Error> {
    let secret =
        env::var("JWT_SECRET_KEY").unwrap_or_else(|_| "development_secret_key".to_string());
    let validation = Validation::new(Algorithm::HS256);
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map_err(|_| ErrorUnauthorized("Invalid or expired reconnection token"))?;

    // Ensure it's a reconnection token
    if token_data.claims.token_type != TokenType::Reconnect {
        return Err(ErrorUnauthorized("Invalid token type"));
    }

    // Check if reconnection token has JTI (session identifier)
    if token_data.claims.jti.is_none() {
        return Err(ErrorUnauthorized("Invalid reconnection token format"));
    }

    Ok(token_data.claims)
}

// Unit tests for LobbyState and session
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc::unbounded_channel;

    struct TestRecipient {
        tx: tokio::sync::mpsc::UnboundedSender<WsMessage>,
    }

    impl Actor for TestRecipient {
        type Context = Context<Self>;
    }

    impl Handler<WsMessage> for TestRecipient {
        type Result = ();

        fn handle(&mut self, msg: WsMessage, _: &mut Context<Self>) {
            let _ = self.tx.send(msg);
        }
    }

    #[actix_web::test]
    async fn test_broadcast_to_two_clients() {
        let lobby = LobbyState::new().start();
        let (tx1, mut rx1) = unbounded_channel();
        let (tx2, mut rx2) = unbounded_channel();
        let recipient1 = TestRecipient { tx: tx1 }.start().recipient();
        let recipient2 = TestRecipient { tx: tx2 }.start().recipient();
        let game_id = "game123".to_string();

        lobby
            .send(Connect {
                game_id: game_id.clone(),
                addr: recipient1.clone(),
            })
            .await
            .unwrap();
        lobby
            .send(Connect {
                game_id: game_id.clone(),
                addr: recipient2.clone(),
            })
            .await
            .unwrap();

        let msg = WsMessage::Clock {
            white: 60,
            black: 60,
        };
        lobby
            .send(Broadcast {
                game_id: game_id.clone(),
                message: msg.clone(),
            })
            .await
            .unwrap();

        let received1 = rx1.recv().await.unwrap();
        let received2 = rx2.recv().await.unwrap();
        assert_eq!(received1, msg);
        assert_eq!(received2, msg);
    }

    #[actix_web::test]
    async fn test_websocket_drop_and_reconnect() {
        let connection_tracker = ConnectionStateTracker::new(None).start();

        let player1_id = Uuid::new_v4();
        let player2_id = Uuid::new_v4();
        let game_id = Uuid::new_v4().to_string();

        let (tx2, mut rx2) = unbounded_channel();
        let test_recipient = TestRecipient { tx: tx2 }.start();
        let player2_addr = test_recipient.recipient();

        connection_tracker.do_send(PlayerReconnected {
            game_id: game_id.clone(),
            player_id: player2_id,
            addr: player2_addr,
        });

        let (tx1, _rx1) = unbounded_channel();
        let test_recipient1 = TestRecipient { tx: tx1 }.start();
        let player1_addr = test_recipient1.recipient();

        connection_tracker.do_send(PlayerReconnected {
            game_id: game_id.clone(),
            player_id: player1_id,
            addr: player1_addr,
        });

        let status1 = connection_tracker
            .send(GetPlayerStatus {
                game_id: game_id.clone(),
                player_id: player1_id,
            })
            .await
            .unwrap()
            .unwrap();
        let status2 = connection_tracker
            .send(GetPlayerStatus {
                game_id: game_id.clone(),
                player_id: player2_id,
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status1.status, ConnectionStatus::Connected);
        assert_eq!(status2.status, ConnectionStatus::Connected);

        connection_tracker.do_send(PlayerDisconnected {
            game_id: game_id.clone(),
            player_id: player1_id,
        });

        let msg = tokio::time::timeout(std::time::Duration::from_millis(100), rx2.recv()).await;
        assert!(msg.is_ok());
        if let Ok(Some(WsMessage::OpponentDisconnected(payload))) = msg {
            assert_eq!(payload.grace_seconds_left, 60);
        } else {
            panic!("Expected OpponentDisconnected message");
        }

        let status1 = connection_tracker
            .send(GetPlayerStatus {
                game_id: game_id.clone(),
                player_id: player1_id,
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status1.status, ConnectionStatus::Reconnecting);
        assert!(status1.has_grace_timer);

        tokio::time::sleep(std::time::Duration::from_secs(10)).await;

        let (tx1_new, _rx1_new) = unbounded_channel();
        let test_recipient1_new = TestRecipient { tx: tx1_new }.start();
        let player1_new_addr = test_recipient1_new.recipient();

        connection_tracker.do_send(PlayerReconnected {
            game_id: game_id.clone(),
            player_id: player1_id,
            addr: player1_new_addr,
        });

        let msg = tokio::time::timeout(std::time::Duration::from_millis(100), rx2.recv()).await;
        assert!(msg.is_ok());
        if let Ok(Some(WsMessage::OpponentReconnected)) = msg {
        } else {
            panic!("Expected OpponentReconnected message");
        }

        let status1 = connection_tracker
            .send(GetPlayerStatus {
                game_id: game_id.clone(),
                player_id: player1_id,
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status1.status, ConnectionStatus::Connected);
        assert!(!status1.has_grace_timer);
    }

    #[actix_web::test]
    async fn test_grace_period_expiry() {
        let connection_tracker = ConnectionStateTracker::new(None).start();

        let player1_id = Uuid::new_v4();
        let player2_id = Uuid::new_v4();
        let game_id = Uuid::new_v4().to_string();

        let (tx2, _rx2) = unbounded_channel();
        let test_recipient = TestRecipient { tx: tx2 }.start();
        connection_tracker.do_send(PlayerReconnected {
            game_id: game_id.clone(),
            player_id: player2_id,
            addr: test_recipient.recipient(),
        });

        let (tx1, _rx1) = unbounded_channel();
        let test_recipient1 = TestRecipient { tx: tx1 }.start();
        connection_tracker.do_send(PlayerReconnected {
            game_id: game_id.clone(),
            player_id: player1_id,
            addr: test_recipient1.recipient(),
        });

        connection_tracker.do_send(PlayerDisconnected {
            game_id: game_id.clone(),
            player_id: player1_id,
        });

        let status1 = connection_tracker
            .send(GetPlayerStatus {
                game_id: game_id.clone(),
                player_id: player1_id,
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status1.status, ConnectionStatus::Reconnecting);

        connection_tracker.do_send(GracePeriodExpired {
            game_id: game_id.clone(),
            player_id: player1_id,
        });

        let status1 = connection_tracker
            .send(GetPlayerStatus {
                game_id: game_id.clone(),
                player_id: player1_id,
            })
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status1.status, ConnectionStatus::Disconnected);
        assert!(!status1.session_is_active);
    }

    #[actix_web::test]
    async fn test_lobby_only_ever_holds_explicitly_connected_recipients() {
        let lobby = LobbyState::new().start();
        let (tx1, _rx1) = unbounded_channel();
        let recipient1 = TestRecipient { tx: tx1 }.start().recipient();
        let game_id = "game456".to_string();

        lobby
            .send(Connect {
                game_id: game_id.clone(),
                addr: recipient1.clone(),
            })
            .await
            .unwrap();

        lobby
            .send(Broadcast {
                game_id: "unknown-game".to_string(),
                message: WsMessage::Clock { white: 1, black: 1 },
            })
            .await
            .unwrap();

        lobby
            .send(Disconnect {
                game_id,
                addr: recipient1,
            })
            .await
            .unwrap();
    }
}
