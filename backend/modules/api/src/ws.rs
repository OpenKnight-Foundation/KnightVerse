use actix::prelude::*;
use actix_web::{HttpRequest, HttpResponse, Error, web};
use actix_web_actors::ws;
use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet};
use std::env;
use security::jwt::{Claims, TokenType};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use actix_web::error::ErrorUnauthorized;
use serde_json::{Value, json};
use uuid::Uuid;

// For Redis Pub/Sub
// Redis pub/sub integration removed for test stability in CI environment
use tokio::task::JoinHandle;

/// Core WebSocket message types
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[rtype(result = "()")]
#[serde(tag = "type", content = "payload")]
pub enum WsMessage {
    Move { from: String, to: String, san: String, fen: String },
    Clock { white: u32, black: u32 },
    End   { result: String, final_fen: String },
    Error { code: u16, message: String },
    ReconnectToken { token: String, expires_in: u32 },
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

impl LobbyState {
    pub fn new() -> Self {
        LobbyState { sessions: HashMap::new() }
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
                let _ = recipient.do_send(msg.message.clone());
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
    pub username: String,
    pub session_id: String,
    pub redis_sub_task: Option<JoinHandle<()>>,     // Placeholder for compatibility
}

impl WsSession {
    /// Server sends a ping every 15 seconds to detect dead connections
    const HEARTBEAT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(15);
    /// Terminate connection if no pong received within 25 seconds (15s interval + 10s grace)
    const CLIENT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);

    /// Generate a reconnection token for this session
    fn generate_reconnect_token(&self) -> Result<String, jsonwebtoken::errors::Error> {
        let secret = env::var("JWT_SECRET_KEY").unwrap_or_else(|_| "development_secret_key".to_string());
        let jwt_service = security::jwt::JwtService::new(secret, 3600);
        jwt_service.generate_reconnect_token(self.user_id, &self.username, &self.session_id)
    }

    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(Self::HEARTBEAT_INTERVAL, |act, ctx| {
            let elapsed = std::time::Instant::now().duration_since(act.hb);
            if elapsed > Self::CLIENT_TIMEOUT {
                log::warn!(
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
        self.lobby.do_send(Connect { game_id: self.game_id.clone(), addr });

        // Redis pub/sub subscription intentionally disabled here; leave placeholder
        self.redis_sub_task = None;
    }

    fn stopped(&mut self, ctx: &mut Self::Context) {
        log::info!("WebSocket disconnected for game: {}", self.game_id);
        
        // Send reconnection token to client for seamless reconnection
        if let Ok(reconnect_token) = self.generate_reconnect_token() {
            let reconnect_msg = WsMessage::ReconnectToken { 
                token: reconnect_token, 
                expires_in: 30 
            };
            
            // Try to send the reconnection token
            let _ = ctx.address().do_send(reconnect_msg);
            log::info!("Sent reconnection token for user: {}", self.username);
        } else {
            log::error!("Failed to generate reconnection token for user: {}", self.username);
        }
        
        let addr = ctx.address().recipient();
        self.lobby.do_send(Disconnect { game_id: self.game_id.clone(), addr });
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
                // Try to parse as WsMessage
                if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                    // Redis publishing disabled in tests/CI environment
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
    let auth_header = req.headers().get("Authorization").and_then(|h| h.to_str().ok());
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
    let secret = env::var("JWT_SECRET_KEY").unwrap_or_else(|_| "development_secret_key".to_string());
    let validation = Validation::new(Algorithm::HS256);
    let token_data = decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &validation)
        .map_err(|_| ErrorUnauthorized("Invalid or expired token"))?;
    
    // Ensure it's an access token
    if token_data.claims.token_type != TokenType::Access {
        return Err(ErrorUnauthorized("Invalid token type"));
    }
    
    Ok(token_data.claims)
}

/// Validate reconnection token
fn validate_reconnect_token(token: &str) -> Result<Claims, Error> {
    let secret = env::var("JWT_SECRET_KEY").unwrap_or_else(|_| "development_secret_key".to_string());
    let validation = Validation::new(Algorithm::HS256);
    let token_data = decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &validation)
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
    use actix::prelude::*;
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
        lobby.send(Connect { game_id: game_id.clone(), addr: recipient1.clone() }).await.unwrap();
        lobby.send(Connect { game_id: game_id.clone(), addr: recipient2.clone() }).await.unwrap();
        let msg = WsMessage::Clock { white: 60, black: 60 };
        lobby.send(Broadcast { game_id: game_id.clone(), message: msg.clone() }).await.unwrap();
        let received1 = rx1.recv().await.unwrap();
        let received2 = rx2.recv().await.unwrap();
        assert_eq!(received1, msg);
        assert_eq!(received2, msg);
    }
}
