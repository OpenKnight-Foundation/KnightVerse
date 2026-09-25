//! In-process mock of the KnightVerse WebSocket move-submission endpoint.
//!
//! Used by `--self-test` so the harness can be exercised end to end (and kept
//! honest in CI) without a running backend, Postgres or Redis. It reproduces
//! the contract the real socket speaks:
//!
//! * the route is `/v1/ws/game/{game_id}` with an optional `?role=spectator`;
//! * `Authorization: Bearer <HS256 access token>` is required and is validated
//!   against the same secret the harness mints with — a bad token is refused
//!   with an HTTP 401, exactly as `ws_route` does;
//! * `Move`/`Clock`/`End` frames from a player are fanned out to every socket
//!   in the same game, the sender included (matching `LobbyState::Broadcast`).
//!
//! It is deliberately not a chess server: frames are relayed, not validated.

use futures_util::{SinkExt, StreamExt};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::Message;

type Registry = Arc<Mutex<HashMap<String, tokio::sync::broadcast::Sender<String>>>>;

/// A running mock endpoint. Dropping it stops the listener.
pub struct MockServer {
    /// Base URL to hand to [`crate::LoadTestConfig::url`].
    pub url: String,
    accept_task: JoinHandle<()>,
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.accept_task.abort();
    }
}

/// Start the mock endpoint on an ephemeral loopback port.
pub async fn spawn(jwt_secret: impl Into<String>) -> std::io::Result<MockServer> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_addr = listener.local_addr()?;
    let secret: Arc<String> = Arc::new(jwt_secret.into());
    let registry: Registry = Arc::new(Mutex::new(HashMap::new()));

    let accept_task = tokio::spawn(async move {
        loop {
            let Ok((stream, _peer)) = listener.accept().await else {
                break;
            };
            let registry = registry.clone();
            let secret = secret.clone();
            tokio::spawn(async move {
                let _ = serve_connection(stream, registry, secret).await;
            });
        }
    });

    Ok(MockServer {
        url: format!("ws://{}", local_addr),
        accept_task,
    })
}

fn is_fanned_out(text: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return false;
    };
    matches!(
        value.get("type").and_then(|t| t.as_str()),
        Some("Move") | Some("Clock") | Some("End")
    )
}

// `Result<Response, ErrorResponse>` is the shape tungstenite's handshake
// callback requires; the error variant is large because it carries the raw
// response, and it cannot be boxed here.
#[allow(clippy::result_large_err)]
async fn serve_connection(
    stream: TcpStream,
    registry: Registry,
    secret: Arc<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let captured: Arc<Mutex<(String, Option<String>)>> =
        Arc::new(Mutex::new((String::new(), None)));
    let capture = captured.clone();
    let secret_for_check = secret.clone();

    let socket = tokio_tungstenite::accept_hdr_async(
        stream,
        move |request: &Request, response: Response| -> Result<Response, ErrorResponse> {
            let path = request.uri().path().to_string();
            let authorization = request
                .headers()
                .get("Authorization")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            *capture.lock().unwrap() = (path, authorization.clone());

            let token = authorization
                .as_deref()
                .and_then(|header| header.strip_prefix("Bearer "));
            let authorized = token
                .map(|token| {
                    decode::<serde_json::Value>(
                        token,
                        &DecodingKey::from_secret(secret_for_check.as_bytes()),
                        &Validation::new(Algorithm::HS256),
                    )
                    .is_ok()
                })
                .unwrap_or(false);

            if authorized {
                Ok(response)
            } else {
                Err(ErrorResponse::new(Some(
                    "invalid or missing access token".to_string(),
                )))
            }
        },
    )
    .await?;

    let (path, _authorization) = captured.lock().unwrap().clone();
    let game_id = path
        .strip_prefix("/v1/ws/game/")
        .map(|rest| rest.split('?').next().unwrap_or(rest).to_string())
        .unwrap_or_default();

    let channel = {
        let mut guard = registry.lock().unwrap();
        guard
            .entry(game_id.clone())
            .or_insert_with(|| tokio::sync::broadcast::channel(1024).0)
            .clone()
    };
    let mut subscriber = channel.subscribe();
    let (mut sink, mut source) = socket.split();

    loop {
        tokio::select! {
            inbound = source.next() => {
                match inbound {
                    None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break,
                    Some(Ok(Message::Text(text))) => {
                        if is_fanned_out(&text) {
                            let _ = channel.send(text);
                        }
                    }
                    Some(Ok(_)) => {}
                }
            }
            outbound = subscriber.recv() => {
                match outbound {
                    Ok(text) => {
                        if sink.send(Message::Text(text)).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(_) => break,
                }
            }
        }
    }

    let mut guard = registry.lock().unwrap();
    if let Some(sender) = guard.get(&game_id) {
        if sender.receiver_count() == 0 {
            guard.remove(&game_id);
        }
    }

    Ok(())
}
