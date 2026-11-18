//! Route tests for the `/v1/tournaments` scope (BE-144).
//!
//! These mount `tournaments::configure_routes` behind the same idempotency
//! middleware `server.rs` uses, so a duplicate or missing route shows up here
//! instead of silently diverging in production.

use crate::idempotency::IdempotencyMiddleware;
use crate::tournaments::configure_routes;
use actix_web::{http::StatusCode, test, web, App, HttpMessage};
use db::DbPool;
use db_entity::tournament as tournament_entity;
use sea_orm::{DbBackend, MockDatabase};
use security::jwt::{Claims, TokenType};
use std::sync::Arc;
use uuid::Uuid;

fn pool_with(mock: MockDatabase) -> DbPool {
    let conn = Arc::new(mock.into_connection());
    DbPool::from_connections(conn.clone(), conn, false)
}

fn test_claims() -> Claims {
    Claims {
        sub: "101".to_string(),
        user_id: 101,
        player_id: Uuid::new_v4(),
        username: "alice".to_string(),
        exp: 9999999999,
        iat: 1000000000,
        jti: None,
        token_type: TokenType::Access,
    }
}

macro_rules! tournament_app {
    ($pool:expr) => {
        test::init_service(
            App::new().app_data(web::Data::new($pool)).service(
                web::scope("/v1/tournaments")
                    .wrap(IdempotencyMiddleware::in_memory())
                    .configure(configure_routes),
            ),
        )
        .await
    };
}

#[actix_web::test]
async fn get_tournaments_route_lists_tournaments() {
    let pool = pool_with(
        MockDatabase::new(DbBackend::Postgres)
            .append_query_results([Vec::<tournament_entity::Model>::new()]),
    );
    let app = tournament_app!(pool);

    let req = test::TestRequest::get().uri("/v1/tournaments").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["data"], serde_json::json!([]));
}

#[actix_web::test]
async fn register_route_requires_authenticated_player() {
    let app = tournament_app!(pool_with(MockDatabase::new(DbBackend::Postgres)));

    let req = test::TestRequest::post()
        .uri(&format!("/v1/tournaments/{}/register", Uuid::new_v4()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn register_route_reaches_handler_for_unknown_tournament() {
    let pool = pool_with(
        MockDatabase::new(DbBackend::Postgres)
            .append_query_results([Vec::<tournament_entity::Model>::new()]),
    );
    let app = tournament_app!(pool);

    let req = test::TestRequest::post()
        .uri(&format!("/v1/tournaments/{}/register", Uuid::new_v4()))
        .insert_header(("Idempotency-Key", "be-144-register"))
        .to_request();
    req.extensions_mut().insert(test_claims());
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["message"], "Tournament not found");
}
