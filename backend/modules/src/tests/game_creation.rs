// backend/modules/src/tests/game_creation.rs

use actix_web::{test, App};
use serde_json::json;
use sqlx::{PgPool, PgPoolOptions};
use uuid::Uuid;

use crate::routes::game_routes;

async fn setup_test_db() -> PgPool {
    PgPoolOptions::new()
        .connect("postgres://user:pass@localhost/knightverse_test")
        .await
        .expect("Test DB connection failed")
}

#[actix_rt::test]
async fn test_create_game_success() {
    let db_pool = setup_test_db().await;

    let app = test::init_service(
        App::new()
            .app_data(actix_web::web::Data::new(db_pool.clone()))
            .configure(game_routes),
    )
    .await;

    let req_body = json!({
        "players": [Uuid::new_v4(), Uuid::new_v4()],
        "variant": "chess960",
        "time_control": { "initial": 300, "increment": 5, "delay_type": "none" },
        "rated": true
    });

    let req = test::TestRequest::post()
        .uri("/games/new")
        .set_json(&req_body)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.get("game_id").is_some());
    assert!(body.get("session_token").is_some());
    assert!(body.get("initial_state").is_some());
    assert!(body.get("player_assignments").is_some());
    assert!(body.get("join_url").is_some());
}

#[actix_rt::test]
async fn test_create_game_invalid_variant() {
    let db_pool = setup_test_db().await;

    let app = test::init_service(
        App::new()
            .app_data(actix_web::web::Data::new(db_pool.clone()))
            .configure(game_routes),
    )
    .await;

    let req_body = json!({
        "players": [Uuid::new_v4()],
        "variant": "invalid_variant",
        "time_control": { "initial": 300, "increment": 5, "delay_type": "none" },
        "rated": false
    });

    let req = test::TestRequest::post()
        .uri("/games/new")
        .set_json(&req_body)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);

    let body = test::read_body(resp).await;
    assert!(std::str::from_utf8(&body).unwrap().contains("Invalid game variant"));
}

#[actix_rt::test]
async fn test_create_game_missing_players() {
    let db_pool = setup_test_db().await;

    let app = test::init_service(
        App::new()
            .app_data(actix_web::web::Data::new(db_pool.clone()))
            .configure(game_routes),
    )
    .await;

    let req_body = json!({
        "variant": "standard",
        "time_control": { "initial": 300, "increment": 5, "delay_type": "none" },
        "rated": false
    });

    let req = test::TestRequest::post()
        .uri("/games/new")
        .set_json(&req_body)
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 400);

    let body = test::read_body(resp).await;
    assert!(std::str::from_utf8(&body).unwrap().contains("Missing required field: players"));
}
