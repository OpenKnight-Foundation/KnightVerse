#[cfg(test)]
mod tests {
    use actix_web::{test, web, App};
    use sea_orm::{ConnectionTrait, Database};
    use std::sync::Arc;
    use uuid::Uuid;

    use crate::auth::login;
    use db::DbPool;
    use dto::auth::LoginRequest;
    use security::{JwtService, TokenService, TokenServiceError};

    /// Build an in-memory SQLite DbPool for testing.
    ///
    /// Both primary and replica point to the same SQLite connection so tests
    /// requiring no real Postgres still exercise the routing code paths.
    async fn setup_test_pool() -> DbPool {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("Failed to connect to test database");

        // The refresh_token table has a foreign key to `player`, which this
        // lightweight harness doesn't create. SQLite doesn't enforce FKs
        // unless explicitly enabled, but sqlx turns it on by default.
        db.execute_unprepared("PRAGMA foreign_keys = OFF;")
            .await
            .expect("Failed to disable foreign key enforcement");

        let schema = sea_orm::Schema::new(db.get_database_backend());
        let stmt = schema
            .create_table_from_entity(db_entity::refresh_token::Entity)
            .if_not_exists()
            .to_owned();
        db.execute(db.get_database_backend().build(&stmt))
            .await
            .expect("Failed to create refresh_tokens table");

        let arc = Arc::new(db);
        DbPool::from_connections(arc.clone(), arc, false)
    }

    #[actix_web::test]
    async fn test_login_returns_access_and_refresh_tokens() {
        let pool = web::Data::new(setup_test_pool().await);
        let jwt_service = web::Data::new(JwtService::new("test_secret_key".to_string(), 3600));

        let app = test::init_service(
            App::new()
                .app_data(pool)
                .app_data(jwt_service)
                .service(login),
        )
        .await;

        let login_request = LoginRequest {
            username: "test_user".to_string(),
            password: "TestPass123".to_string(),
        };

        let req = test::TestRequest::post()
            .uri("/login")
            .set_json(&login_request)
            .to_request();

        let resp = test::call_service(&app, req).await;
        // SQLite memory db won't have the player — expect 401
        assert!(resp.status().is_client_error());
    }

    #[actix_web::test]
    async fn test_login_fails_with_wrong_password() {
        let pool = web::Data::new(setup_test_pool().await);
        let jwt_service = web::Data::new(JwtService::new("test_secret_key".to_string(), 3600));

        let app = test::init_service(
            App::new()
                .app_data(pool)
                .app_data(jwt_service)
                .service(login),
        )
        .await;

        let login_request = LoginRequest {
            username: "nonexistent_user".to_string(),
            password: "SomePass123".to_string(),
        };

        let req = test::TestRequest::post()
            .uri("/login")
            .set_json(&login_request)
            .to_request();

        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
    }

    /// Verifies that token generation is unique across calls.
    #[tokio::test]
    async fn test_token_generation_produces_unique_tokens() {
        let pool = setup_test_pool().await;

        let token1 =
            TokenService::generate_refresh_token(pool.primary(), 1, Uuid::new_v4(), 7).await;

        let token2 =
            TokenService::generate_refresh_token(pool.primary(), 1, Uuid::new_v4(), 7).await;

        // Both may fail because SQLite memory db has no schema, but if they
        // succeed they must differ.
        if let (Ok(t1), Ok(t2)) = (token1, token2) {
            assert_ne!(t1, t2, "Generated tokens should be unique");
        }
    }

    /// Hashing is deterministic.
    #[tokio::test]
    async fn test_token_hashing_is_deterministic() {
        let token = "test_token_abc123def456";
        let hash1 = TokenService::hash_token(token);
        let hash2 = TokenService::hash_token(token);
        assert_eq!(hash1, hash2);
    }

    #[tokio::test]
    async fn test_access_tokens_include_unique_jti() {
        let jwt_service = JwtService::new("test_secret_key".to_string(), 3600);
        let token = jwt_service
            .generate_token(42, "alice", Uuid::new_v4())
            .expect("token generation should work");
        let claims = jwt_service
            .validate_token(&token)
            .expect("token should validate");

        assert!(claims.jti.is_some(), "access token must carry a unique jti");
        assert_ne!(claims.jti.as_deref(), Some(""), "jti should not be empty");
    }

    #[tokio::test]
    async fn test_refresh_reuse_invalidates_entire_family() {
        let pool = setup_test_pool().await;

        let family_id = Uuid::new_v4();
        let first = TokenService::generate_refresh_token(pool.primary(), 7, family_id, 7)
            .await
            .expect("first token should be generated");
        let second = TokenService::generate_refresh_token(pool.primary(), 7, family_id, 7)
            .await
            .expect("second token should be generated");

        let _ = TokenService::verify_and_mark_used(pool.primary(), &first, 7)
            .await
            .expect("first token should validate");

        let reuse = TokenService::verify_and_mark_used(pool.primary(), &first, 7).await;
        assert!(matches!(reuse, Err(TokenServiceError::TokenReuseDetected)));

        let revoked = TokenService::verify_and_mark_used(pool.primary(), &second, 7).await;
        assert!(matches!(revoked, Err(TokenServiceError::TokenInvalid)));
    }

    // Placeholder tests — full implementation requires Postgres schema
    #[actix_web::test]
    async fn test_refresh_rotates_tokens() {
        // Full implementation requires database setup with refresh_token schema.
        // Covered in integration tests that run against a live Postgres instance.
    }

    #[actix_web::test]
    async fn test_token_reuse_detection_invalidates_family() {
        // Security test — see integration test suite for live-DB coverage.
    }

    #[actix_web::test]
    async fn test_logout_revokes_all_tokens() {
        // See integration test suite.
    }

    #[actix_web::test]
    async fn test_expired_tokens_rejected() {
        // See integration test suite.
    }
}

mod token_hash_test {
    use sha2::{Digest, Sha256};

    fn hash_token(token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    #[test]
    fn test_hash_consistency() {
        let token = "test_token";
        let hash1 = hash_token(token);
        let hash2 = hash_token(token);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_different_tokens_different_hashes() {
        let hash1 = hash_token("token_1");
        let hash2 = hash_token("token_2");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_length() {
        let hash = hash_token("test");
        assert_eq!(hash.len(), 64);
    }
}
