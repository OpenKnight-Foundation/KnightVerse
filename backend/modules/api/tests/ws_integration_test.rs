//! BE-20: Integration tests for the game WebSocket endpoint (`/v1/ws/game/{game_id}`),
//! using a real bound `actix_web::test::TestServer` and an `awc` WS client — as
//! close to a real client connection as a test can get, rather than calling
//! the handler function directly in isolation.

use actix::{Actor, Addr};
use actix_web::{web, App};
use futures_util::{SinkExt, StreamExt};
use security::jwt::JwtService;
use uuid::Uuid;

use api::ws::{Broadcast, LobbyState, WsMessage, ws_route};

const JWT_SECRET: &str = "test_secret_for_ws_integration";

fn make_access_token(user_id: i32, username: &str) -> String {
    let jwt_service = JwtService::new(JWT_SECRET.to_string(), 3600);
    jwt_service
        .generate_token(user_id, username, Uuid::new_v4())
        .expect("failed to mint test JWT")
}

/// Starts a real, bound test server with the same route shape as production
/// (`/v1/ws/game/{game_id}`), and returns it along with a handle to the
/// shared `LobbyState` so the test can simulate server-side game events
/// (moves/clock/end) the same way real move-processing logic elsewhere in
/// the app would, via `Broadcast`.
fn start_ws_test_server() -> (actix_test::TestServer, Addr<LobbyState>) {
    std::env::set_var("JWT_SECRET_KEY", JWT_SECRET);

    let lobby = LobbyState::new().start();
    let lobby_for_test = lobby.clone();

    let srv = actix_test::start(move || {
        App::new()
            .app_data(web::Data::new(lobby.clone()))
            .service(web::scope("/v1/ws").route("/game/{game_id}", web::get().to(ws_route)))
    });

    (srv, lobby_for_test)
}

#[actix_web::test]
async fn connects_with_a_valid_token_and_receives_a_simulated_chess_game() {
    let (srv, lobby) = start_ws_test_server();
    let game_id = "integration-game-1".to_string();
    let token = make_access_token(42, "magnus");

    let url = srv.url(&format!("/v1/ws/game/{}", game_id)).replace("http://", "ws://");

    let (_resp, mut connection) = awc::Client::new()
        .ws(url)
        .header("Authorization", format!("Bearer {token}"))
        .connect()
        .await
        .expect("WS handshake with a valid token should succeed");

    // Simulate a short chess game being pushed by server-side game logic:
    // white opens, clock ticks, black responds, then the game ends.
    let sequence = vec![
        WsMessage::Move {
            from: "e2".into(),
            to: "e4".into(),
            san: "e4".into(),
            fen: "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1".into(),
        },
        WsMessage::Clock { white: 598, black: 600 },
        WsMessage::Move {
            from: "e7".into(),
            to: "e5".into(),
            san: "e5".into(),
            fen: "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2".into(),
        },
        WsMessage::Clock { white: 598, black: 597 },
        WsMessage::End {
            result: "1-0".into(),
            final_fen: "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2".into(),
        },
    ];

    for expected in &sequence {
        lobby
            .send(Broadcast { game_id: game_id.clone(), message: expected.clone() })
            .await
            .expect("lobby actor should accept the broadcast");

        let item = connection
            .next()
            .await
            .expect("connection closed before receiving expected message")
            .expect("WS transport error");

        let text = match item {
            awc::ws::Frame::Text(bytes) => String::from_utf8(bytes.to_vec()).unwrap(),
            other => panic!("expected a text frame, got {other:?}"),
        };

        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        // Every server-sent message is stamped with a version field.
        assert_eq!(value["version"], "1.0");

        let received: WsMessage = serde_json::from_value({
            let mut v = value.clone();
            if let serde_json::Value::Object(ref mut m) = v {
                m.remove("version");
            }
            v
        })
        .expect("message should round-trip back into WsMessage");

        assert_eq!(&received, expected);
    }

    let _ = connection.close().await;
}

#[actix_web::test]
async fn rejects_connection_with_no_authorization_header() {
    let (srv, _lobby) = start_ws_test_server();
    let url = srv.url("/v1/ws/game/game-no-auth").replace("http://", "ws://");

    let result = awc::Client::new().ws(url).connect().await;

    assert!(result.is_err(), "connecting with no Authorization header must be rejected");
}

#[actix_web::test]
async fn rejects_connection_with_an_invalid_token() {
    let (srv, _lobby) = start_ws_test_server();
    let url = srv.url("/v1/ws/game/game-bad-auth").replace("http://", "ws://");

    let result = awc::Client::new()
        .ws(url)
        .header("Authorization", "Bearer not-a-real-token")
        .connect()
        .await;

    assert!(result.is_err(), "connecting with an invalid token must be rejected");
}

#[actix_web::test]
async fn two_clients_in_the_same_game_both_receive_broadcasts() {
    let (srv, lobby) = start_ws_test_server();
    let game_id = "integration-game-2".to_string();
    let url_a = srv.url(&format!("/v1/ws/game/{game_id}")).replace("http://", "ws://");
    let url_b = url_a.clone();

    let (_r1, mut client_a) = awc::Client::new()
        .ws(url_a)
        .header("Authorization", format!("Bearer {}", make_access_token(1, "alice")))
        .connect()
        .await
        .unwrap();

    let (_r2, mut client_b) = awc::Client::new()
        .ws(url_b)
        .header("Authorization", format!("Bearer {}", make_access_token(2, "bob")))
        .connect()
        .await
        .unwrap();

    let msg = WsMessage::Clock { white: 300, black: 300 };
    lobby
        .send(Broadcast { game_id: game_id.clone(), message: msg.clone() })
        .await
        .unwrap();

    for connection in [&mut client_a, &mut client_b] {
        let item = connection.next().await.unwrap().unwrap();
        let text = match item {
            awc::ws::Frame::Text(bytes) => String::from_utf8(bytes.to_vec()).unwrap(),
            other => panic!("expected a text frame, got {other:?}"),
        };
        assert!(text.contains("\"Clock\""));
    }
}

#[actix_web::test]
async fn client_ping_is_answered_with_pong() {
    let (srv, _lobby) = start_ws_test_server();
    let game_id = "integration-game-ping".to_string();
    let url = srv.url(&format!("/v1/ws/game/{}", game_id)).replace("http://", "ws://");
    let token = make_access_token(7, "heartbeat_test");

    let (_resp, mut connection) = awc::Client::new()
        .ws(url)
        .header("Authorization", format!("Bearer {token}"))
        .connect()
        .await
        .unwrap();

    connection.send(awc::ws::Message::Ping(actix_web::web::Bytes::from_static(b"hi"))).await.unwrap();

    let item = connection.next().await.unwrap().unwrap();
    match item {
        awc::ws::Frame::Pong(bytes) => assert_eq!(&bytes[..], b"hi"),
        other => panic!("expected a Pong frame in response to our Ping, got {other:?}"),
    }
}

/// Documents a real, current gap found while writing these tests (not
/// asserted as a bug fix — flagged here so it isn't silently relied upon):
/// `WsSession`'s `StreamHandler` for `ws::Message::Text` parses an incoming
/// client message into a `WsMessage` but its match arm body is empty — a
/// client sending a `Move` over the socket produces no broadcast, no
/// validation, and no response today. All moves in this test suite are
/// therefore simulated via the `Broadcast` actor message directly (as real
/// move-processing logic elsewhere in the app presumably does), not by
/// sending a `Move` WsMessage from the client. See PR description for the
/// recommended follow-up.
#[actix_web::test]
async fn client_sent_move_message_currently_produces_no_response() {
    let (srv, _lobby) = start_ws_test_server();
    let game_id = "integration-game-noop".to_string();
    let url = srv.url(&format!("/v1/ws/game/{}", game_id)).replace("http://", "ws://");
    let token = make_access_token(9, "client_move_sender");

    let (_resp, mut connection) = awc::Client::new()
        .ws(url)
        .header("Authorization", format!("Bearer {token}"))
        .connect()
        .await
        .unwrap();

    let client_move = serde_json::json!({
        "type": "Move",
        "payload": { "from": "e2", "to": "e4", "san": "e4", "fen": "..." }
    });
    connection.send(awc::ws::Message::Text(client_move.to_string().into())).await.unwrap();

    // Race the (lack of a) response against a short timeout — if this ever
    // starts failing because a response *does* arrive, that's good news:
    // it means the no-op gap above has been fixed, and this test (along
    // with its doc comment) should be updated to assert the new behavior
    // instead of removed outright.
    let outcome = tokio::time::timeout(std::time::Duration::from_millis(300), connection.next()).await;
    assert!(outcome.is_err(), "expected no response to a client-sent Move (current behavior is a no-op)");
}