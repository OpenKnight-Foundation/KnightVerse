//! End-to-end backend integration tests (BE-134).
//!
//! Covers the happy path the issue asks for:
//! **register → login → create game → join → move → complete.**
//!
//! These drive the real handlers behind the real `JwtAuthMiddleware`, so
//! authentication, the service layer and the database are all exercised — the
//! only thing stubbed out is Redis, which the middleware treats as optional.
//!
//! Every test skips (rather than fails) when `TEST_DATABASE_URL` is unset, so
//! `cargo test --workspace` still passes on a developer machine with no
//! database. CI wires the variable up to a `postgres:16` service.

use actix_web::{http::StatusCode, test, web, App};
use dto::games::{CreateGameRequest, GameDisplayDTO, JoinGameRequest, MakeMoveRequest, PlayerColor};
use security::{JwtAuthMiddleware, JwtService};
use serde_json::json;
use uuid::Uuid;

use crate::support::*;

/// 1 hour, long enough that tokens never expire mid-test.
const TOKEN_TTL: usize = 3600;

/// `Authorization: Bearer …` header for a test token.
fn bearer(token: &str) -> (actix_web::http::header::HeaderName, String) {
    (
        actix_web::http::header::AUTHORIZATION,
        format!("Bearer {token}"),
    )
}

#[actix_web::test]
async fn register_then_login_returns_a_usable_token() {
    if test_database_url().is_none() {
        eprintln!("skipping register/login: TEST_DATABASE_URL not set");
        return;
    }

    let pool = migrated_pool().await;
    let player = create_player(&pool, "itest_alice", "correct horse battery").await;

    let token = login(&pool, &player).await;
    assert!(!token.is_empty(), "login must return an access token");

    // The token is a real JWT signed with the harness secret.
    let claims = JwtService::new(TEST_JWT_SECRET.to_string(), TOKEN_TTL)
        .validate_token(&token)
        .expect("login token must validate");
    assert_eq!(claims.username, player.username);
    assert_eq!(claims.player_id, player.id);
}

#[actix_web::test]
async fn login_with_a_wrong_password_is_rejected() {
    if test_database_url().is_none() {
        eprintln!("skipping negative login: TEST_DATABASE_URL not set");
        return;
    }

    let pool = migrated_pool().await;
    let player = create_player(&pool, "itest_bob", "correct horse battery").await;

    let status = login_status(&pool, &player.username, "definitely not the password").await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a wrong password must be rejected"
    );
}

fn create_request() -> CreateGameRequest {
    CreateGameRequest {
        time_control: 600,
        increment: 5,
        player_color: Some(PlayerColor::White),
        opponent_id: None,
    }
}

/// The `/v1/games` routes behind the real JWT middleware, so the tests
/// exercise authentication rather than stubbing out the acting player.
///
/// A macro rather than a function because `test::init_service` returns an
/// opaque service type that cannot be named in a helper's return position.
macro_rules! game_app {
    ($pool:expr) => {
        test::init_service(
            App::new()
                .app_data(web::Data::new($pool.clone()))
                .wrap(JwtAuthMiddleware::new(TEST_JWT_SECRET.to_string(), TOKEN_TTL))
                .service(
                    web::scope("/v1/games")
                        // These handlers carry their own route attributes, so they
                        // are registered as services, not with `.route(..).to(..)`.
                        .service(api::games::create_game)
                        .service(api::games::get_game)
                        .service(api::games::join_game)
                        .service(api::games::make_move)
                        .service(api::games::complete_game),
                ),
        )
        .await
    };
}

/// Unwrap the `{ "message": …, "data": { "game": … } }` envelope.
fn game_from_body(body: &serde_json::Value) -> GameDisplayDTO {
    serde_json::from_value(body["data"]["game"].clone())
        .unwrap_or_else(|e| panic!("could not decode game from response: {e}\n{body}"))
}

#[actix_web::test]
async fn full_lifecycle_create_join_move_complete() {
    if test_database_url().is_none() {
        eprintln!("skipping full lifecycle: TEST_DATABASE_URL not set");
        return;
    }

    let pool = migrated_pool().await;

    let alice = create_player(&pool, "itest_life_alice", "correct horse battery").await;
    let bob = create_player(&pool, "itest_life_bob", "correct horse battery").await;

    // --- login --------------------------------------------------------------
    let alice_token = login(&pool, &alice).await;
    let bob_token = login(&pool, &bob).await;
    assert_ne!(alice_token, bob_token, "each player gets their own token");

    // --- create game --------------------------------------------------------
    let creator_app = game_app!(pool);
    let req = test::TestRequest::post()
        .uri("/v1/games")
        .insert_header(bearer(&alice_token))
        .set_json(create_request())
        .to_request();
    let res = test::call_service(&creator_app, req).await;
    let status = res.status();
    let body: serde_json::Value = test::read_body_json(res).await;
    assert_eq!(status, StatusCode::CREATED, "create_game failed: {body}");

    let game = game_from_body(&body);
    let game_id = game.id;
    assert_eq!(game.white_player_id, alice.id);
    assert!(game.black_player_id.is_none(), "a new game has no black player");

    // --- join ---------------------------------------------------------------
    let joiner_app = game_app!(pool);
    let join = test::TestRequest::post()
        .uri(&format!("/v1/games/{game_id}/join"))
        .insert_header(bearer(&bob_token))
        .set_json(JoinGameRequest {})
        .to_request();
    let res = test::call_service(&joiner_app, join).await;
    let status = res.status();
    let body: serde_json::Value = test::read_body_json(res).await;
    assert!(status.is_success(), "join failed with {status}: {body}");
    assert_eq!(game_from_body(&body).black_player_id, Some(bob.id));

    // --- move ---------------------------------------------------------------
    let mover_app = game_app!(pool);
    let mv = test::TestRequest::put()
        .uri(&format!("/v1/games/{game_id}/move"))
        .insert_header(bearer(&alice_token))
        .set_json(MakeMoveRequest {
            chess_move: "e2e4".to_string(),
        })
        .to_request();
    let res = test::call_service(&mover_app, mv).await;
    let status = res.status();
    let body: serde_json::Value = test::read_body_json(res).await;
    assert!(status.is_success(), "make_move failed with {status}: {body}");
    assert_eq!(
        game_from_body(&body).move_history,
        vec!["e2e4".to_string()],
        "the move must be recorded on the game"
    );

    // --- complete -----------------------------------------------------------
    let completer_app = game_app!(pool);
    let complete = test::TestRequest::put()
        .uri(&format!("/v1/games/{game_id}/complete"))
        .insert_header(bearer(&alice_token))
        .set_json(json!({ "result": "white_wins", "k_factor": 32 }))
        .to_request();
    let res = test::call_service(&completer_app, complete).await;
    let status = res.status();
    let body: serde_json::Value = test::read_body_json(res).await;
    assert!(status.is_success(), "complete failed with {status}: {body}");
    assert_eq!(body["success"], true);
    assert_eq!(body["result"], "white_wins");

    let white_change = body["rating_change_white"].as_i64().expect("white delta");
    let black_change = body["rating_change_black"].as_i64().expect("black delta");
    assert!(white_change > 0, "the winner should gain rating, got {white_change}");
    assert!(black_change < 0, "the loser should lose rating, got {black_change}");

    // --- persisted state ----------------------------------------------------
    assert_eq!(
        scalar_i64(
            &pool,
            &format!("SELECT COUNT(*) FROM smdb.game WHERE id = '{game_id}' AND result IS NOT NULL AND result::text <> 'ongoing'")
        )
        .await,
        1,
        "the settled game and its result must be persisted"
    );
}

#[actix_web::test]
async fn playing_twice_in_a_row_is_rejected() {
    if test_database_url().is_none() {
        eprintln!("skipping turn order: TEST_DATABASE_URL not set");
        return;
    }

    let pool = migrated_pool().await;

    let alice = create_player(&pool, "itest_turn_alice", "correct horse battery").await;
    let bob = create_player(&pool, "itest_turn_bob", "correct horse battery").await;
    let alice_token = login(&pool, &alice).await;
    let bob_token = login(&pool, &bob).await;

    let creator_app = game_app!(pool);
    let req = test::TestRequest::post()
        .uri("/v1/games")
        .insert_header(bearer(&alice_token))
        .set_json(create_request())
        .to_request();
    let body: serde_json::Value =
        test::read_body_json(test::call_service(&creator_app, req).await).await;
    let game_id = game_from_body(&body).id;

    let joiner_app = game_app!(pool);
    let join = test::TestRequest::post()
        .uri(&format!("/v1/games/{game_id}/join"))
        .insert_header(bearer(&bob_token))
        .set_json(JoinGameRequest {})
        .to_request();
    test::call_service(&joiner_app, join).await;

    // White plays.
    let mover_app = game_app!(pool);
    let first = test::TestRequest::put()
        .uri(&format!("/v1/games/{game_id}/move"))
        .insert_header(bearer(&alice_token))
        .set_json(MakeMoveRequest {
            chess_move: "e2e4".to_string(),
        })
        .to_request();
    let res = test::call_service(&mover_app, first).await;
    assert!(res.status().is_success(), "white's first move must succeed");

    // White cannot play again before black has moved.
    let twice_app = game_app!(pool);
    let second = test::TestRequest::put()
        .uri(&format!("/v1/games/{game_id}/move"))
        .insert_header(bearer(&alice_token))
        .set_json(MakeMoveRequest {
            chess_move: "g1f3".to_string(),
        })
        .to_request();
    let res = test::call_service(&twice_app, second).await;
    assert_eq!(
        res.status(),
        StatusCode::FORBIDDEN,
        "moving twice in a row must be rejected"
    );

    assert_eq!(
        scalar_i64(
            &pool,
            &format!("SELECT jsonb_array_length(pgn)::bigint FROM smdb.game WHERE id = '{game_id}'")
        )
        .await,
        1,
        "only the accepted move may be persisted"
    );
}

#[actix_web::test]
async fn a_non_participant_cannot_settle_someone_elses_game() {
    if test_database_url().is_none() {
        eprintln!("skipping authorisation: TEST_DATABASE_URL not set");
        return;
    }

    let pool = migrated_pool().await;

    let alice = create_player(&pool, "itest_auth_alice", "correct horse battery").await;
    let bob = create_player(&pool, "itest_auth_bob", "correct horse battery").await;
    let mallory = create_player(&pool, "itest_auth_mallory", "correct horse battery").await;
    let alice_token = login(&pool, &alice).await;
    let bob_token = login(&pool, &bob).await;
    let mallory_token = login(&pool, &mallory).await;

    let creator_app = game_app!(pool);
    let req = test::TestRequest::post()
        .uri("/v1/games")
        .insert_header(bearer(&alice_token))
        .set_json(create_request())
        .to_request();
    let body: serde_json::Value =
        test::read_body_json(test::call_service(&creator_app, req).await).await;
    let game_id = game_from_body(&body).id;

    let joiner_app = game_app!(pool);
    let join = test::TestRequest::post()
        .uri(&format!("/v1/games/{game_id}/join"))
        .insert_header(bearer(&bob_token))
        .set_json(JoinGameRequest {})
        .to_request();
    test::call_service(&joiner_app, join).await;

    let mallory_app = game_app!(pool);
    let complete = test::TestRequest::put()
        .uri(&format!("/v1/games/{game_id}/complete"))
        .insert_header(bearer(&mallory_token))
        .set_json(json!({ "result": "white_wins" }))
        .to_request();
    let res = test::call_service(&mallory_app, complete).await;

    assert!(
        !res.status().is_success(),
        "a non-participant must not be able to settle a game (got {})",
        res.status()
    );
    assert_eq!(
        scalar_i64(
            &pool,
            &format!("SELECT COUNT(*) FROM smdb.game WHERE id = '{game_id}' AND result IS NOT NULL AND result::text <> 'ongoing'")
        )
        .await,
        0,
        "the game must stay unsettled"
    );
}