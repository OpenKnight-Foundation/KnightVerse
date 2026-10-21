//! Shared fixtures for the backend integration tests (BE-134).
//!
//! The flows under test (register → login → create game → join → move →
//! complete) are all stateful, and none of them are meaningfully exercised
//! against a mock, so every test here needs a real migrated PostgreSQL
//! instance. Tests skip cleanly when one is not configured, which keeps
//! `cargo test --workspace` green on a machine that has no database.
//!
//! ```sh
//! TEST_DATABASE_URL=postgres://postgres:postgres@localhost:5432/knightverse_test \
//!   cargo test -p integration_tests
//! ```
//!
//! `.github/workflows/backend.yml` starts a `postgres:16` service and points
//! this variable at it, so CI runs the full suite.

use actix_web::{test, web, App};
use db::DbPool;
use sea_orm::{ConnectOptions, Database, Statement};
use security::JwtService;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// Environment variable holding the test database DSN.
pub const TEST_DATABASE_URL: &str = "TEST_DATABASE_URL";

/// JWT secret used by the test harness. Every token in these tests is minted
/// and verified with this value, so no production secret is involved.
pub const TEST_JWT_SECRET: &str = "integration_test_secret";

/// `None` when no test database is configured, in which case the caller should
/// return early instead of failing.
pub fn test_database_url() -> Option<String> {
    std::env::var(TEST_DATABASE_URL)
        .ok()
        .filter(|v| !v.trim().is_empty())
}

/// Connect to the test database and run every migration against it.
///
/// # Panics
/// Panics when `TEST_DATABASE_URL` is set but unreachable — a configured but
/// broken database is a real failure, not something to skip.
pub async fn migrated_pool() -> DbPool {
    let url = test_database_url().expect("TEST_DATABASE_URL must be set for this test");

    let mut options = ConnectOptions::new(url);
    options
        .max_connections(5)
        .connect_timeout(Duration::from_secs(10))
        .sqlx_logging(false);

    let connection = Database::connect(options)
        .await
        .unwrap_or_else(|e| panic!("failed to connect to TEST_DATABASE_URL: {e}"));

    // Migrations are idempotent, so several test binaries may share one
    // database without racing each other into a broken schema.
    <migration::Migrator as migration::MigratorTrait>::up(&connection, None)
        .await
        .unwrap_or_else(|e| panic!("failed to run migrations: {e}"));

    let arc = Arc::new(connection);
    DbPool::from_connections(arc.clone(), arc, false)
}

/// A player row plus the plaintext password needed to log in as them.
#[derive(Debug, Clone)]
pub struct TestPlayer {
    pub id: Uuid,
    pub username: String,
    pub password: String,
}

/// Insert a player with a real argon2 password hash, the same way
/// registration does.
pub async fn create_player(pool: &DbPool, username: &str, password: &str) -> TestPlayer {
    use db_entity::player;
    use sea_orm::ActiveModelTrait;
    use sea_orm::Set;

    let hash = service::helper::password::hash_password(password)
        .unwrap_or_else(|e| panic!("failed to hash password: {e}"));

    let model = player::ActiveModel {
        id: Set(Uuid::new_v4()),
        username: Set(username.to_string()),
        email: Set(format!("{username}@example.test")),
        password_hash: Set(hash.into_bytes()),
        biography: Set(String::new()),
        country: Set(String::new()),
        flair: Set(String::new()),
        real_name: Set(String::new()),
        location: Set(None),
        fide_rating: Set(Some(1500)),
        elo_rating: Set(1500),
        social_links: Set(None),
        is_enabled: Set(true),
        wallet_address: Set(None),
        preferences: Set(serde_json::json!({})),
    }
    .insert(pool.primary())
    .await
    .unwrap_or_else(|e| panic!("failed to insert player {username}: {e}"));

    TestPlayer {
        id: model.id,
        username: username.to_string(),
        password: password.to_string(),
    }
}

/// Log a player in through the real `POST /v1/auth/login` handler and return
/// the access token.
///
/// This goes through actix's `test::init_service` rather than calling the
/// handler directly, so the request/response serde behaviour is covered too.
pub async fn login(pool: &DbPool, player: &TestPlayer) -> String {
    use dto::auth::LoginRequest;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(JwtService::new(
                TEST_JWT_SECRET.to_string(),
                3600,
            )))
            .service(web::scope("/v1/auth").service(api::auth::login)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/auth/login")
        .set_json(LoginRequest {
            username: player.username.clone(),
            password: player.password.clone(),
        })
        .to_request();
    let res = test::call_service(&app, req).await;

    let status = res.status();
    let body: serde_json::Value = test::read_body_json(res).await;
    assert_eq!(
        status,
        actix_web::http::StatusCode::OK,
        "login failed for {}: {body}",
        player.username
    );
    body["access_token"]
        .as_str()
        .unwrap_or_else(|| panic!("login response had no access_token: {body}"))
        .to_string()
}

/// Status code returned by `POST /v1/auth/login` for the given credentials.
///
/// `login` panics on failure, which is what the happy path wants; this variant
/// exists for the negative cases.
pub async fn login_status(pool: &DbPool, username: &str, password: &str) -> actix_web::http::StatusCode {
    use dto::auth::LoginRequest;

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .app_data(web::Data::new(JwtService::new(
                TEST_JWT_SECRET.to_string(),
                3600,
            )))
            .service(web::scope("/v1/auth").service(api::auth::login)),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/v1/auth/login")
        .set_json(LoginRequest {
            username: username.to_string(),
            password: password.to_string(),
        })
        .to_request();
    test::call_service(&app, req).await.status()
}

/// Run a scalar `SELECT` and return the first column of the first row.
pub async fn scalar_i64(pool: &DbPool, sql: &str) -> i64 {
    use sea_orm::ConnectionTrait;

    let row = pool
        .primary()
        .query_one(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            sql.to_string(),
        ))
        .await
        .unwrap_or_else(|e| panic!("query failed ({sql}): {e}"))
        .unwrap_or_else(|| panic!("query returned no rows ({sql})"));

    row.try_get_by_index::<i64>(0)
        .unwrap_or_else(|e| panic!("could not read a scalar from ({sql}): {e}"))
}
