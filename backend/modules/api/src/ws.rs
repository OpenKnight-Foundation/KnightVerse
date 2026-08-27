use actix::prelude::*;
use actix_web::error::ErrorUnauthorized;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use security::jwt::{Claims, TokenType};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::env;
use tracing::{error, info, warn};
use uuid::Uuid;
use sea_orm::{DatabaseConnection, EntityTrait};
use db_entity::game;
use db::DbPool;
use dto::games::{GameStatus, GameDisplayDTO};
use error::error::ApiError;

// For Redis Pub/Sub
// Redis pub/sub integration removed for test stability in CI environment
use tokio::task::JoinHandle;
use chrono::{DateTime, Utc};

/// Player connection status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConnectionStatus {
    Connected,
    Reconnecting,
    Disconnected,
}

/// Player state within a game session
#[derive(Debug, Clone)]
pub struct PlayerConnectionState {
    pub player_id: Uuid,
    pub status: ConnectionStatus,
    pub disconnected_at: Option<DateTime<Utc>>,
    pub grace_timer: Option<JoinHandle<()>>,
    pub addr: Option<Recipient<WsMessage>>,
}

/// Game session state tracking all players in a game
#[derive(Debug, Clone)]
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

/// OpponentDisconnected message sent to connected opponent with grace seconds left
#[derive(Serialize, Deserialize, Clone, Debug)]
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

/// OpponentDisconnected message payload
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OpponentDisconnectedPayload {
    pub grace_seconds_left: u32,
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
    OpponentDisconnected(OpponentDisconnectedPayload),
    OpponentReconnected,
    FullStateSync {
        fen: String,
        move_list: Vec<String>,
        white_time: u32,
        black_time: u32,
    },
}

/// Actor messages
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

/// Lobby state actor
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
        Self::new()
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
        self.game_sessions.entry(game_id.clone()).or_insert_with(|| {
            GameSessionState {
                game_id,
                players: HashMap::new(),
                is_active: true,
            }
        })
    }

    /// Broadcast message to all other players in the game
    fn broadcast_to_other_players(
        &mut self,
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
        
        // Only process if game is active and player exists
        if !session.is_active {
            return;
        }

        if let Some(player_state) = session.players.get_mut(&msg.player_id) {
            // Only start timer if not already disconnected
            if player_state.status != ConnectionStatus::Disconnected {
                player_state.status = ConnectionStatus::Reconnecting;
                player_state.disconnected_at = Some(Utc::now());
                player_state.addr = None; // Clear old address

                info!(
                    "Player {} disconnected from game {}, starting {}s grace period",
                    msg.player_id, msg.game_id, Self::GRACE_PERIOD_SECONDS
                );

                // Notify opponent that player disconnected with grace period
                self.broadcast_to_other_players(
                    session,
                    msg.player_id,
                    WsMessage::OpponentDisconnected(OpponentDisconnectedPayload {
                        grace_seconds_left: Self::GRACE_PERIOD_SECONDS as u32,
                    }),
                );

                // Spawn grace period timer
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

                player_state.grace_timer = Some(timer_handle);
            }
        }
    }
}

/// Handle PlayerReconnected message - cancel timer, sync state
impl Handler<PlayerReconnected> for ConnectionStateTracker {
    type Result = ();

    fn handle(&mut self, msg: PlayerReconnected, ctx: &mut Context<Self>) {
        let session = match self.game_sessions.get_mut(&msg.game_id) {
            Some(s) => s,
            None => return,
        };

        if !session.is_active {
            return;
        }

        if let Some(player_state) = session.players.get_mut(&msg.player_id) {
            // Cancel any existing grace timer
            if let Some(timer) = player_state.grace_timer.take() {
                timer.abort();
                info!(
                    "Player {} reconnected to game {}, grace period cancelled",
                    msg.player_id, msg.game_id
                );
            }

            // Update player state
            player_state.status = ConnectionStatus::Connected;
            player_state.disconnected_at = None;
            player_state.addr = Some(msg.addr.clone());

            // Notify opponent that player reconnected
            self.broadcast_to_other_players(
                session,
                msg.player_id,
                WsMessage::OpponentReconnected,
            );

            // If we have a DB pool, fetch full game state to sync
            if let Some(db_pool) = &self.db_pool {
                let db_pool_clone = db_pool.clone();
                let addr_clone = msg.addr.clone();
                let game_id_uuid = match Uuid::parse_str(&msg.game_id) {
                    Ok(id) => id,
                    Err(_) => return,
                };

                // Spawn task to fetch game state and send full sync
                tokio::spawn(async move {
                    match crate::service::games::GameService::get_game(&db_pool_clone, game_id_uuid).await {
                        Ok(game_state) => {
                            // Convert move history to Vec<String>
                            let move_list: Vec<String> = game_state.move_history
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
                            info!("Sent full state sync to reconnected player {} in game {}", msg.player_id, msg.game_id);
                        }
                        Err(e) => {
                            error!("Failed to fetch game state for sync: {}", e);
                        }
                    }
                });
            }
        } else {
            // New player joining the game
            session.players.insert(msg.player_id, PlayerConnectionState {
                player_id: msg.player_id,
                status: ConnectionStatus::Connected,
                disconnected_at: None,
                grace_timer: None,
                addr: Some(msg.addr),
            });
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

                // Mark player as disconnected permanently
                player_state.status = ConnectionStatus::Disconnected;
                player_state.grace_timer = None;

                // If we have a DB pool, call abandon_game to declare timeout
                if let Some(db_pool) = &self.db_pool {
                    let db_pool_clone = db_pool.clone();
                    let game_id_uuid = match Uuid::parse_str(&msg.game_id) {
                        Ok(id) => id,
                        Err(_) => return,
                    };
                    let player_id_clone = msg.player_id;

                    tokio::spawn(async move {
                        match crate::service::games::GameService::abandon_game(&db_pool_clone, game_id_uuid, player_id_clone).await {
                            Ok(_) => {
                                info!("Successfully marked game {} as abandoned by player {}", game_id_uuid, player_id_clone);
                            }
                            Err(e) => {
                                error!("Failed to mark game as abandoned: {}", e);
                            }
                        }
                    });

                    // Mark game as inactive to prevent further processing
                    session.is_active = false;
                }
            }
        }
    }
}

/// WebSocket session actor
pub struct WsSession {
    pub game_id: String,
    pub lobby: Addr<LobbyState>,
    pub hb: std::time::Instant,
    pub user_id: i32,
    pub player_id: Uuid,
    pub username: String,
    pub session_id: String,
    pub redis_sub_task: Option<JoinHandle<()>>, // Placeholder for compatibility
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
        self.lobby.do_send(Connect {
            game_id: self.game_id.clone(),
            addr,
        });

        // Redis pub/sub subscription intentionally disabled here; leave placeholder
        self.redis_sub_task = None;
    }

    fn stopped(&mut self, ctx: &mut Self::Context) {
        info!("WebSocket disconnected for game: {}", self.game_id);

        // Send reconnection token to client for seamless reconnection
        if let Ok(reconnect_token) = self.generate_reconnect_token() {
            let reconnect_msg = WsMessage::ReconnectToken {
                token: reconnect_token,
                expires_in: 30,
            };

            // Try to send the reconnection token
            ctx.address().do_send(reconnect_msg);
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
        // Cancel Redis subscription task if running
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
                // Parse and broadcast the move to all connected clients in this game.
                if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                    self.lobby.do_send(Broadcast {
                        game_id: self.game_id.clone(),
                        message: ws_msg,
                    });
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

/// WebSocket route handler with auth and reconnection support
pub async fn ws_route(
    req: HttpRequest,
    stream: web::Payload,
    lobby: web::Data<Addr<LobbyState>>,
) -> Result<HttpResponse, Error> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok());
    let mut reconnect_token: Option<String> = None;

    // Parse query string manually
    let query_string = req.query_string();
    if !query_string.is_empty() {
        for param in query_string.split('&') {
            if let Some((key, value)) = param.split_once('=') {
                if key == "reconnect" {
                    reconnect_token = Some(value.to_string());
                    break;
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
            hb: std::time::Instant::now(),
            user_id: claims.user_id,
            player_id: claims.player_id,
            username: claims.username,
            session_id,
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
}